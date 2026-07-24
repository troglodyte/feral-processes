# Travelling Base — design

**Status:** approved design, not yet implemented
**Date:** 2026-07-24

## Problem

Staying on one zone level long enough to build a base isn't worth it. The
cause is a curve mismatch, not a tuning miss:

| Axis | Scaling |
|---|---|
| XP per kill (`= wild max_hp`) | ×2 per zone, unbounded |
| Gear bonus (`GEAR_LEVEL_GROWTH`, baked in at equip time) | ×2 per zone, unbounded |
| Distance-from-spawn danger | ×1.25 per 15 tiles, capped at ×3 |
| Mining Node output | 1 core fragment / 10 ticks × 50% — flat |
| Research Node output | 1 research_data / 14 ticks — flat |
| Structure build costs | flat |

On top of that, `Game::enter_next_zone` (lib.rs:3377) despawns every
`Structure`, so the base is a 100% write-off on breach. A Mining Node costs
12 core fragments and yields 0.05/tick, needing 240 ticks to break even —
while occupying a pet that would otherwise be in the party earning half the
player's XP. The same 240 ticks one zone deeper is worth double.

The base is the only progression system denominated in a currency that
doesn't scale with depth, and the only one deleted when you advance.

## Goals

Base-building becomes a core pillar: every zone involves settling and
building, and rushing through without a base is actively bad play.

## Non-goals

- Changing how taming, battle, fusion, or research *content* works.
- Reworking raids (retuned separately on 2026-07-23).
- Any per-zone rebuild-from-scratch loop. The base travels intact; the
  per-zone build phase comes from having to *upgrade* it, not replace it.

---

## 1. The travelling platform

### `Biome::Platform`

A new `Biome` variant. Walkable, and listed as a habitat by no shipped
species. `try_spawn_habitat_creature` already returns `false` when both the
ordinary and boss candidate pools come back empty (lib.rs:3885–3887), so
the safe-haven behaviour needs **no new spawn-suppression code** — it falls
out of the existing habitat lookup.

`WorldMap::classify` never produces `Platform`; it only ever arrives via
`set_override`. That keeps generated terrain and player-caused terrain
cleanly separated.

Both renderers match `Biome` exhaustively with no wildcard arm
(`tui/src/ui.rs:305`, `gui/src/render.rs:322`), so adding the variant is a
compile error in exactly two places until a deliberate glyph/colour is
chosen for each. That is the desired fail-fast behaviour, not an obstacle.

A modder *may* list `Platform` in a species' `habitats` if they want
something that lives on your base. Left open deliberately; documented in
`assets/species/README.md`.

### Footprint

`MAX_BUILD_DISTANCE_FROM_HOME` (lib.rs:236, currently 15) is reused as the
platform radius rather than introducing a second constant. The platform is
*exactly* the buildable area — Chebyshev radius 15, a 31×31 disc of 961
tiles. Keeping these one constant means the two can never drift apart.

### Placing a Home

`Game::place_structure` (lib.rs:2000) gains a Home-specific step:

1. Stamp `Tile { biome: Platform, walkable: true }` overrides across the
   31×31 disc centered on the Home.
2. Despawn every `Hostile` and every `Nest` inside the disc.
3. Set the `Platform` resource's center (see below).

`set_override` (world.rs:116) currently has **zero callers outside
`world.rs`** — the override mechanism is built, persisted and tested, but
nothing in the game uses it yet. The platform slab is its first and only
user, so blanket stamp/clear of a disc cannot collide with any other tile
change. If that ever stops being true, this assumption needs revisiting.

The existing one-Home rule (lib.rs:2016–2018) and the existing walkable-tile
check (lib.rs:2034–2037) both stay as-is. The latter guarantees the disc's
founding tile was walkable, though every tile becomes walkable anyway once
stamped.

### Demolishing a Home

The slab is *defined as* "centered on the current Home". Demolishing the
Home clears its overrides, so there is no way to litter a map with orphan
sanctuaries. This composes with the existing cascade behaviour
(lib.rs:2094–2098): removing a Home already demolishes every other
structure, since nothing can exist outside its radius.

### Breaching

`enter_next_zone` (lib.rs:3377) changes as follows:

- `Structure` comes **out** of the despawn query. Only `Hostile` and `Nest`
  are still despawned.
- The Home is repositioned to the new `ZoneSpawnPoint`; every other
  structure keeps its offset relative to the Home.
- The slab is re-stamped around the Home's new position on the new map.
- **The dangling-`Task` cleanup (lib.rs:3387–3396) is deleted.** It exists
  only because structures used to be despawned out from under their
  assigned workers. With structures surviving, cronjob assignments stay
  valid through the breach — less code and a real quality-of-life win.

Because the structure entities are never despawned, their `Durability`,
`ResourceNode` stock, and `Temporary` tick counters travel with them for
free. No component needs explicit copying.

The departed zone's slab needs no cleanup: `enter_next_zone` replaces the
whole `WorldMap` resource, and a fresh `WorldMap::new(seed)` starts with an
empty `overrides` map. The old slab is discarded with the old map. Only one
slab therefore ever exists at a time, and `tile_overrides` in a save never
accumulates across zones.

Structures cannot collide on arrival: every one shifts by the same delta as
the Home, so distinct positions stay distinct. World coordinates are `i32`
over a lazily-chunked map, so there is no boundary to land outside of.

### Why this can't be exploited

- A Portal requires a Home to build (lib.rs:2013), and demolishing a Home
  cascade-demolishes the Portal with it. **You can never breach without a
  Home**, so the platform always exists at breach time. No edge case.
- Placing a second Home requires demolishing the first at a 30% refund
  (`STRUCTURE_REMOVAL_REFUND_PERCENT`, lib.rs:242) — losing 70% of the
  entire base. Home-as-panic-button (its obliteration clears hostiles) is
  therefore only free on the very first placement. `place_structure`
  already refuses during an active battle (lib.rs:2001), so it can't be
  used to escape a fight in progress. No additional cost or cooldown is
  needed.

---

## 2. One-use Portal

The Portal is consumed when stepped on — despawned *before* the
carry-forward snapshot, so it never travels.

This is load-bearing, not flavour. Structures now survive the breach; if the
Portal survived too, the player would breach free forever and the entire
`10 × zone_level` portal-fragment cost would stop applying after the first
one.

---

## 3. Danger scaling measured from the platform edge

Today `distance_stat_multiplier` (lib.rs:3543) measures Chebyshev distance
from `ZoneSpawnPoint`, stepping up ×0.25 every `DISTANCE_STAT_STEP_TILES`
(15). With the platform occupying exactly radius 15, the first danger step
would land precisely on the platform edge — the base would sit at the
boundary of escalating territory rather than inside safe territory.

**The whole platform becomes distance-zero.** Both scaling functions
subtract the platform radius from the measured distance (clamped at 0)
before dividing by their step:

- `distance_stat_multiplier`: first ×1.25 step now lands 30 tiles from
  Home (15 platform + 15 step) instead of 15.
- `max_pack_size` (lib.rs:3579): same offset, for coherence. Its
  `PACK_SIZE_STEP_TILES` is *derived* from `DISTANCE_STAT_STEP_TILES`
  (lib.rs:75), and letting pack size escalate inside territory that is
  still stat-×1.0 would be incoherent. First step moves from 30 tiles from
  Home to 45.

`MAX_DISTANCE_STAT_MULTIPLIER` (3.0) is unchanged; it's now reached 135
tiles from Home rather than 120.

### The `Platform` resource

Both scaling functions take `&self`, while `home_position` (lib.rs:3325)
takes `&mut self` because it runs an ECS query. The platform center
therefore cannot be looked up per spawn — it lives in a resource instead:

```rust
#[derive(Resource, Default, Clone, Copy)]
pub struct Platform {
    /// Center of the current base platform, or `None` before the run's
    /// first Home is placed.
    pub center: Option<(i32, i32)>,
}
```

Written in exactly three places: Home placement, Home demolition, and
breach. Read by the two scaling functions and by the slab stamp/clear.

`center: None` (no Home yet, i.e. the opening minutes of a run) means no
offset is applied and distance measures from `ZoneSpawnPoint` exactly as it
does today. Early game before the first Home is behaviourally unchanged.

Not persisted — it is reconstructed on load from the Home's position, which
`SaveData.structures` already carries.

### Tests that must change

`distance_stat_multiplier_grows_with_distance_from_the_zone_spawn_point_and_caps`
(lib.rs:11498) and `max_pack_size_grows_with_zone_and_distance_and_caps_per_zone`
(lib.rs:11532) assert the current origin semantics. Both must be **revised
to assert the new offset behaviour**, including a case with no Home
(offset absent) and a case with a Home (offset applied) — not deleted.

---

## 4. Yields scale with zone depth

A worked node's payout multiplies by `ZoneLevel::stat_multiplier()` —
`2^(zone-1)`, the same doubling base as wild stats and `GEAR_LEVEL_GROWTH`.

This is the codebase's established principle, not a new one. balance.rs:255–265
documents deliberately bringing `GEAR_LEVEL_GROWTH` *down* to match
`stat_multiplier`'s doubling base, specifically so gear neither overtakes
deep zones nor collapses behind them. Node yields share that base for the
same reason: anything sub-exponential still loses to the exponential
eventually, which is the original bug.

Implementation lands in `systems::task_progress_system` (systems.rs:160–166),
which currently hardcodes both the stock decrement and the payout to 1. The
system gains a `Res<ZoneLevel>` parameter.

The multiplier is read at **cycle-completion time, from the current
`ZoneLevel`** — not baked in when the structure was deployed. A base that
travels from zone 1 to zone 4 immediately produces at the zone-4 rate. This
is the opposite of how `Game::equip` handles gear (which deliberately bakes
in the zone level at equip time), and the difference is intentional: gear is
a snapshot of a decision, a node is an ongoing process.

**Node stock still decrements by 1 per cycle regardless of payout.**
`capacity` refills instantly on hitting 0 (documented in
`structures::WorkDef::capacity`), so stock is a pacing detail rather than a
real budget; coupling it to an exponential payout would add nothing.

### Banked currencies are excluded

`research_data` carries `bank_limit: Some(200)`; `core_fragment` is
unbounded cargo. At zone 5 a scaled Research Node would produce 16 per
cycle and fill the bank in ~13 cycles, converting the research economy into
constant "no room to store it" spam.

Research is a pacing gate over a finite tree — it does not need to scale.
The build economy does. So yield scaling applies only to items **without**
a `bank_limit`. This needs no new mechanic: the check is already available
via `ItemDb`.

---

## 5. Upgrade tiers

One new `#[serde(default)]` field on `StructureDef`, per the moddability
rules:

```ron
upgrade: Some((max_tier: 5, cost: [("core_fragment", 10)])),
```

The cost charged to reach tier N is each amount × N — so Mk1→Mk2 costs 20,
Mk2→Mk3 costs 30. A structure with no `upgrade` field cannot be upgraded,
and every existing `.ron` file (including third-party mods) keeps parsing
untouched.

A `StructureTier` component (default 1) is added to upgradeable structures
on deploy. The tier value does two things with one number:

- multiplies the node's payout, on top of the zone multiplier
- feeds `ResourceNode.level`, which already drives
  `mining_success_chance(level) = (0.4 + level × 0.1).min(1.0)`
  (systems.rs:101)

The reliability half is therefore pre-built — `WorkDef.level` and
`ResourceNode.level` already exist and already work this way. No new
balance math is introduced for it. Note that reliability saturates at
level 6 (100%); beyond that, tiers contribute payout only.

`assets/structures/README.md` is updated in the same change, per the
schema-change rules.

---

## 6. What this does to the numbers

| Scenario | Core fragments/tick |
|---|---|
| Zone 1, Mk1 (50% success) | 0.05 |
| Zone 4, Mk3 (70% success) | 1.68 |

The loop closes through an existing structure. The Market
(`assets/structures/black_market.ron`, id `market`) buys `portal_fragment`
at 8 core fragments each. A zone-4 Portal costs 40 portal fragments = 320
core fragments ≈ **190 ticks of a Mk3 base.**

Settling therefore becomes the fastest route to breaching, which is the
behaviour change this whole design exists to produce. Kills still drop
portal fragments directly (`PORTAL_FRAGMENT_DROP_CHANCE` 0.35, bosses
3..=6), so fighting remains a viable parallel route rather than being
replaced.

---

## 7. Save format

No new `SaveData` fields. `tile_overrides` already carries the slab,
`structures` already carries the buildings, and `Platform.center` is
reconstructed from the Home on load.

`StructureTier` does need persisting, which means `StructureSave` gains a
field. Combined with `Biome` gaining a variant, that is a shape change, so
`SAVE_FORMAT_VERSION` goes 8 → 9 (save.rs:168). Existing saves stop
loading — the documented, intentional tradeoff for this project.

Note the separately-queued `PlayerBuff` persistence fix also claims a v8 → v9
bump. Whichever lands second takes v10 — or, if they land together, a single
bump covers both. Not a conflict, but the two must not both be written as
"v9" independently.

The slab adds ~961 `tile_overrides` entries to every save. Acceptable, but
it is the first time that vector is non-empty in practice.

---

## 8. Testing

Per the repo's testing rules — business logic gets unit tests, no wall-clock
or unseeded-RNG dependence, full `cargo test --workspace` as the final gate.

**Platform:**
- Placing a Home stamps a walkable `Platform` disc of the expected extent;
  the tile just outside the radius is untouched natural terrain.
- Placing a Home despawns hostiles and nests inside the radius and leaves
  those outside it alone.
- Demolishing a Home clears the slab back to natural terrain.
- No wild creature ever spawns on a `Platform` tile.

**Travel:**
- Breaching preserves every structure, its relative offset from Home, its
  `Durability`, and its `ResourceNode` stock.
- A worker's cronjob `Task` survives the breach and still points at a live
  structure.
- The Portal is gone after breaching, and a second breach requires building
  a new one.

**Scaling:**
- `distance_stat_multiplier` is 1.0 across the whole platform and first
  steps up at 30 tiles from Home; with no Home placed, it behaves exactly
  as it does today.
- Node payout doubles per zone level; a `bank_limit` item's payout does not
  scale.
- Upgrading raises both payout and `mining_success_chance`, and refuses
  past `max_tier` or without the materials.

**Balance projection** (`crates/engine/src/balance.rs`, the established home
for offline arithmetic proofs): a projection that settling beats rushing —
ticks to afford a zone-N Portal with a tiered base versus by combat drops
alone. This is the regression check that the curve mismatch stays fixed,
in the same spirit as the existing zone-scaling sweeps.

---

## Open risks

- **Arrival tension.** You now materialise inside your own sanctuary every
  breach. The "step into a dangerous unknown" moment is gone by
  construction. Judged an acceptable consequence of a travelling base, but
  worth feeling in play before deciding it is fine.
- **Balance is arithmetic-only.** Like the 2026-07-23 raid retune, the
  numbers here are proven on paper and unplayed. The Mk3-at-zone-4 figure
  above assumes a fully upgraded node and one worker.
- **961 overrides per save** is a first for this codebase; worth a glance
  at save size and load time once it is real.
