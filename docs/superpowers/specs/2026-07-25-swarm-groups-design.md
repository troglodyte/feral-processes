# Swarm groups: enemy groups scale to 100

**Date:** 2026-07-25
**Branch:** `feat/swarm-groups`
**Status:** Design, approved

## Problem

Wild packs are small and stop growing early. `MAX_PACK_SIZE` is 12 across the
whole intrusion, so the deepest zone throws at most four groups of three at a
five-slot party. A zone's depth changes how hard each creature hits, but not
what a fight *looks* like — a zone 1 encounter and a zone 6 encounter are the
same shape, three or four bars on a screen.

We want distance and depth to change the shape of the encounter, not just its
numbers: solo programs at the start, and swarms of a hundred deep in a zone,
far from base. Four groups of a hundred is the ceiling.

## Existing architecture this builds on

Verified by reading, not assumed:

- `Game::max_pack_size` (`crates/engine/src/game/spawning.rs:178`) caps a pack
  at `(zone * PACK_SIZE_PER_ZONE).clamp(1, MAX_PACK_SIZE)` — 3 per zone level,
  hard ceiling 12 — and grows into that cap linearly with distance, one extra
  member per `PACK_SIZE_STEP_TILES` (30) from the danger origin.
- `Game::try_spawn_habitat_creature` (`spawning.rs:275`) picks **one** species
  per spawn roll and places `1..=max_pack_size` of it, clustered within
  `PACK_GATHER_RADIUS` (3) of the rolled tile.
- `Game::gather_pack` (`crates/engine/src/game/combat.rs:15`) collects every
  `Hostile` within `PACK_GATHER_RADIUS` of the bumped anchor and truncates the
  flat list to `max_pack_size`.
- `Game::group_pack` (`combat.rs:43`) partitions that list by species in
  first-appearance order, keeping the largest `MAX_ENEMY_GROUPS` (4) groups.
  Surplus groups are *not* despawned — they stay on the map as ordinary
  hostiles and are met on the next bump.
- `Game::roll_initiative` (`combat.rs:166`) enumerates **every** member of
  **every** group as an actor, so every living member of a reachable group
  attacks every round. `ENGAGED_GROUPS` (2) is the only brake: groups past
  index 1 can only act with a move flagged `ranged`.
- Only a group's *front* member is targetable by single-target actions;
  `AbilityTarget::WholeEnemyGroup` (`crates/engine/src/game/combat_round.rs:480`)
  already resolves to every living member, and `AllEnemies` to the whole field.
- `WILD_CREATURE_CAP` (100) bounds `Hostile` count map-wide.
  `Game::maybe_spawn_wild_creature` (`spawning.rs:226`) despawns exactly one
  hostile — the farthest from the player — when at the cap, then spawns.
- `crates/engine/src/balance.rs` projects fights headlessly:
  `full_pack_at_zone` builds the zone's full pack, `split_into_groups` divides
  it, and `simulate_roster_fight` runs a deterministic round loop.
- The renderer draws a group as one row, `"{count} {species}s"` with the front
  member's HP bar (`crates/gui/src/render/battle.rs:140`). Group size needs no
  renderer change.

## Design

### 1. Group size curve

`max_pack_size` is replaced by `max_group_size(x, y)` — the size of **one**
species group, not the whole pack. Two dials, both geometric:

```
zone cap:   zone 1 → 1
            zone n → min(MAX_GROUP_SIZE, ZONE_GROUP_GROWTH^(n-1))
            z2=3   z3=9   z4=27   z5=81   z6+=100

distance:   doubles every GROUP_SIZE_STEP_TILES (15) from the danger origin
            0–14→1  15–29→2  30–44→4  45–59→8  60–74→16
            75–89→32  90–104→64  105+→128

group size = min(zone cap, distance factor)
```

`try_spawn_habitat_creature` then rolls uniformly in `1..=group size`, exactly
as it rolls `1..=max_pack` today.

The distance dial tops out at 105 tiles, close to where
`distance_stat_multiplier` already saturates (120 tiles), so both danger curves
finish at roughly the same range. Zone 1 is solo per group; four overlapping
species still means up to four singles in one fight, which is what zone 1 does
today.

The shift `1 << (dist / GROUP_SIZE_STEP_TILES)` must clamp its exponent — the
map is unbounded and a shift of 32 or more is a panic in debug and garbage in
release. Clamping the exponent at 7 is exact, since `1 << 7` is 128 and the cap
is 100.

Constant changes in `crates/engine/src/lib.rs`:

| Old | New |
| --- | --- |
| `PACK_SIZE_PER_ZONE: u32 = 3` | `ZONE_GROUP_GROWTH: u32 = 3` (geometric base, not an addend) |
| `MAX_PACK_SIZE: u32 = 12` | `MAX_GROUP_SIZE: u32 = 100` (per group, not per pack) |
| `PACK_SIZE_STEP_TILES: i32 = 30` | `GROUP_SIZE_STEP_TILES: i32 = 15` (a doubling, not a +1) |
| `WILD_CREATURE_CAP: usize = 100` | `WILD_CREATURE_CAP: usize = 2000` |

`MAX_ENEMY_GROUPS` (4) and `ENGAGED_GROUPS` (2) are unchanged. The whole-pack
ceiling is `MAX_GROUP_SIZE * MAX_ENEMY_GROUPS` = 400, expressed as that product
rather than as a fifth constant.

### 2. Attackers per round

`roll_initiative` caps how many of a group's members act: the front
`ceil(sqrt(n))` living members, where `n` is the group's current member count.

```
n:          1   3   9   27   81   100
attackers:  1   2   3    6    9    10
```

Dead members are already removed from `EnemyGroup::members`
(`combat_round.rs:383`), so "the front `ceil(sqrt(n))` slots" needs no
liveness filtering beyond what `roll_initiative` already does.

Reach still applies on top: a group past `ENGAGED_GROUPS` contributes
attackers only if its move is `ranged`.

This deliberately softens fights that ship today — a 12-member group drops from
12 attackers to 4 — and the swarm sizes are the counterweight. At zone 6 the
deep-field maximum goes from 6 attackers (two engaged groups of three) to 20
(two engaged groups of 100), about 3.3×.

The rule lives in one function so the balance sim and the engine cannot drift:

```rust
/// How many of a group's `n` members can bring weapons to bear in one round.
pub(crate) fn attackers_in_group(n: usize) -> usize
```

### 3. Per-group cap enforcement

A spawn roll places one species, so a single roll's cluster is one group. Left
alone, a 400-strong cluster would be one group of 400 rather than four of 100.
So:

- `group_pack` truncates each group's `members` to `max_group_size(anchor)`.
- `gather_pack`'s total cap becomes `max_group_size(anchor) * MAX_ENEMY_GROUPS`.

Truncated members are not despawned. They stay on the map as ordinary hostiles
and are met on the next bump — the behaviour surplus *groups* already have,
extended to surplus members of a kept group.

Both caps derive from the anchor tile's local `max_group_size`, so the danger
curve — not a global constant — decides how big a fight near the base can get,
even in a deep zone.

### 4. Map budget and clustering

`WILD_CREATURE_CAP` rises to 2000. A single pack can now be 400, so a 100-wide
map budget cannot hold one encounter.

`maybe_spawn_wild_creature`'s cull becomes "free enough room for the incoming
group", not "despawn one". Culling one per roll while spawning up to 100 would
let the population grow ~99 per roll and never converge. It culls farthest-first
(Chebyshev from the player, the existing rule) until the group fits under the
cap.

Spawn scatter and gather radius scale together so a spawned cluster always
pulls into exactly one fight:

```rust
/// Radius a group of `n` scatters across, and the radius `gather_pack`
/// searches — the same number, so a spawned cluster is always collectable.
/// `PACK_GATHER_RADIUS` stays the floor, so nothing shrinks below today.
fn swarm_radius(n: u32) -> i32 { PACK_GATHER_RADIUS.max(ceil_sqrt(n) as i32) }
```

For 100 that is 10, a 21×21 area — roughly a quarter of the tiles occupied,
which reads as a swarm on the map. At the fixed radius of 3, a hundred
creatures pile into 49 tiles at about two per tile and look identical to a
twelve-pack. `gather_pack` derives its radius from `max_group_size(anchor)`,
the same function the spawner used, so the two stay in lockstep without the
battle needing to know which spawn roll produced the cluster.

The anchor the player bumps is a cluster *member*, not necessarily the tile
the spawn roll landed on, so its `max_group_size` can differ by one distance
step from the spawner's — a cluster straddling a step boundary can gather
slightly wider or narrower than it scattered. That is the same looseness
spawning already has (`spawn_wild_creature` does not recheck walkability for
packmates), and the failure mode is a member or two left standing, which is
already an expected outcome.

`ceil_sqrt` is a small integer helper, not a float `sqrt().ceil()` — it is
used for both `attackers_in_group` and `swarm_radius`, and both must be exact
at perfect squares (81 → 9, not 10).

### 5. Balance sim

Two changes, both to `crates/engine/src/balance.rs`:

**Honest focus fire.** `simulate_roster_fight` currently pools the roster's
damage and spills overkill into the next member, then into the next group
(`balance.rs:289`). The real game does not: one action kills at most one
member, and excess damage is discarded. At 12 members that gap is a rounding
error; at 400 it is the entire verdict. The loop changes to apply each living
fighter's damage to the current front member individually, discarding the
excess. This raises every projected level requirement, including for content
that already ships.

**Swarm-shaped packs.** `full_pack_at_zone` builds `MAX_ENEMY_GROUPS` groups
of the zone's group cap rather than splitting a flat pack size, and the enemy
half of the round loop uses `attackers_in_group` instead of `*remaining`.
`split_into_groups` has no remaining caller and is deleted.

### 6. What the survivability test asserts now

`a_full_party_survives_a_full_pack_at_each_zone` cannot survive this change as
written. With honest focus fire, clearing 400 zone-scaled members exceeds the
300-turn cap regardless of level, and the sim models no abilities — so it
cannot score a fight whose intended answer is AoE.

It is re-pointed rather than deleted: **a full party must be able to clear one
full-size group** at each swept zone, keeping the same level sweep and the same
grind-only / geared split. That remains a real gate on the party-to-enemy
ratio, and it is a claim the sim can actually evaluate.

A second test pins the curve itself, which the old test never covered:

- zone 1 yields group size 1 at any distance;
- the zone cap sequence is 1, 3, 9, 27, 81, 100, 100;
- the distance factor doubles per 15 tiles and clamps to the zone cap;
- `attackers_in_group` matches `ceil(sqrt(n))` and never exceeds `n`.

### 7. Consequences accepted, not fixed

- **Rewards scale linearly.** Loot rolls and XP are per defeated member, so a
  cleared 400-swarm pays about 33× what a 12-pack pays. Unchanged by this work.
- **A deep swarm without AoE is a wall.** Only `cascade_overflow`
  (`WholeEnemyGroup`) and `broadcast_storm` (`AllEnemies`) deal group-wide
  damage. Bringing one is the counterplay; the alternative is fleeing.
- **Long fights.** A 400-member intrusion is hundreds of single-target actions.
  No spillover, rout, or per-swarm stat discount is introduced.
- **Save size.** Up to 2000 hostiles serialize instead of 100. No format
  change — `Hostile` entities already round-trip — but files grow.

## Testing

Engine unit tests, per `CLAUDE.md`:

- `max_group_size` at the zone/distance grid above, including the clamped
  shift at absurd distances.
- `attackers_in_group` at the boundaries, and that `roll_initiative` emits
  exactly that many `Actor::Enemy` per group.
- `gather_pack` truncates a >100 single-species cluster to the local cap and
  leaves the remainder on the map (entity count before/after).
- `group_pack` keeps four groups of 100 from a 500-strong mixed cluster.
- `maybe_spawn_wild_creature` at the cap culls enough room for the incoming
  group rather than one entity, and the `Hostile` count never exceeds
  `WILD_CREATURE_CAP`.
- Balance sim: the re-pointed survivability sweep, plus the curve test.

Seeded RNG only, no wall-clock, no `sleep` — background habitat spawning will
otherwise perturb any naive count assertion. `cargo test --workspace` is the
final gate.

## Documentation

- Root `README.md` — lines ~198–210 describe the pack cap and its linear
  distance growth; lines ~346–358 use "a twelve-program pack" as the worked
  example. Both are falsified by this change.
- `assets/species/README.md` mentions packs in passing (the `ranged` move flag
  at line 53, habitat spawning at line 148). Neither states a size; check, and
  update only if the wording implies the old cap.
- No `.ron` schema field is added or removed, so no `#[serde(default)]`
  obligation arises.

## Out of scope

- New AoE abilities. The two that ship are the counterplay; authoring more is
  a data change anyone can make.
- Reward scaling, rout/morale, overkill spillover, per-swarm stat discounts.
- Any change to `MAX_ENEMY_GROUPS`, `ENGAGED_GROUPS`, or the reach rule.
- Renderer changes. A group already draws as one counted row.
