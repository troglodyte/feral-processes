# Swarm Groups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wild enemy groups scale from solo in zone 1 to 100 deep in a zone and far from base, capped at four groups of 100, with only `ceil(sqrt(n))` of a group swinging per round.

**Architecture:** Two geometric dials replace the linear pack curve — a per-zone group cap (`zone_group_cap`) and a distance doubling (`max_group_size`) — feeding both the spawner and the battle's per-group ceiling. A new `attackers_in_group` rule caps how many of a group act each round, shared verbatim by the engine and the offline balance sim so the two cannot drift.

**Tech Stack:** Rust 2024 edition (rustc 1.95), `bevy_ecs` standalone, `.ron` assets, `cargo test --workspace`.

## Global Constants

Copied verbatim from the spec. Every task's requirements implicitly include these.

- `MAX_GROUP_SIZE = 100` — per group, not per pack.
- `ZONE_GROUP_GROWTH = 3` — geometric base: zone 1 solo, then ×3 per zone level.
- `GROUP_SIZE_STEP_TILES = DISTANCE_STAT_STEP_TILES` (15) — group size doubles per step.
- `WILD_CREATURE_CAP = 2000` — map-wide `Hostile` budget.
- `MAX_ENEMY_GROUPS = 4` and `ENGAGED_GROUPS = 2` are **unchanged**.
- Whole-pack ceiling is the product `MAX_GROUP_SIZE * MAX_ENEMY_GROUPS` (400), never a fifth constant.
- Attackers per group per round: `ceil(sqrt(n))`, exact at perfect squares (81 → 9, not 10).

## Repo Facts You Need

- Engine tests live in `crates/engine/src/tests/*.rs` (registered in `tests/mod.rs`), not `tests/`. They reach into components and resources directly.
- `cargo test --workspace` is the final gate (484 tests, ~3s today). Run `cargo fmt` and `cargo clippy --workspace` after every change and fix warnings rather than silencing them.
- If many tests fail at once with `NotFound` on an assets path, that's stale build artifacts from an old directory rename — fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core`, **not** a full `cargo clean` (`target/` is ~4 GB).
- No `.ron` schema field is added or removed anywhere in this plan, so no `#[serde(default)]` obligation arises.
- Existing values you will reference: `DISTANCE_STAT_STEP_TILES = 15`, `MAX_BUILD_DISTANCE_FROM_HOME = 7`, `PACK_GATHER_RADIUS = 3`, `NEST_GUARDIAN_MAX = 5`, `xp_for_level(level) = level * 20`.
- Commit at each task boundary. You are on branch `feat/swarm-groups`; do not push.

## File Structure

| File | Responsibility | Tasks |
| --- | --- | --- |
| `crates/engine/src/battle.rs` | `ceil_sqrt` + `attackers_in_group` — the two pure helpers shared by engine and sim | 2 |
| `crates/engine/src/game/combat.rs` | `roll_initiative` attacker cap; `gather_pack`/`group_pack` per-group ceiling | 2, 5 |
| `crates/engine/src/game/combat_round.rs` | Doc fix on `all_wild_retaliate` | 2 |
| `crates/engine/src/game/spawning.rs` | `zone_group_cap`, `max_group_size`, `swarm_radius`, spawn roll, cull-to-fit | 4, 5, 6 |
| `crates/engine/src/lib.rs` | Constants and their doc comments | 4, 6 |
| `crates/engine/src/balance.rs` | Honest focus fire, swarm-shaped packs, re-pointed sweeps | 3, 4 |
| `crates/engine/src/tests/combat_packs.rs` | Attacker-cap and per-group-ceiling tests | 2, 5 |
| `crates/engine/src/tests/combat_rewards.rs` | Per-member XP regression | 1 |
| `crates/engine/src/tests/zone.rs` | Curve tests (replacing the `max_pack_size` pair) | 4 |
| `crates/engine/src/tests/spawning.rs` | Cull-to-fit test | 6 |
| `README.md` | Player-facing pack/group documentation | 7 |

No new files. No renderer changes — a group already draws as one counted row.

---

### Task 1: Pin per-member XP before anything else moves

Every vanquished member already pays its own XP (`finish_member`, `combat_round.rs:397`, awards `max_hp` then calls `award_loot`). Nothing in this plan is allowed to change that, and at 400 members it becomes load-bearing, so it gets pinned first.

**Files:**
- Test: `crates/engine/src/tests/combat_rewards.rs` (append at end of file)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: nothing later tasks call. This is a guard, not a dependency.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/combat_rewards.rs`. The fixture sits deep and far on purpose: Task 5 makes `group_pack` truncate a group to the local `max_group_size`, and a five-member fixture near a zone-1 spawn point would silently shrink to one member once that lands.

```rust
/// Each member of a group pays its own XP when it dies — five kills pay
/// five times, not once. A swarm's whole reward curve rests on this, and
/// `finish_member` is reached from every death path (attack, ability,
/// status tick), so it is worth pinning independently of any of them.
#[test]
fn every_member_of_a_group_pays_its_own_xp_when_it_dies() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 6;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Deep and far, so the per-group ceiling this fixture will live under
    // once Task 5 lands is 100 rather than 1.
    let (x, y) = (spawn.x + DISTANCE_STAT_STEP_TILES * 8, spawn.y);
    let player = game.player_entity();

    let members: Vec<Entity> = (0..5)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i).unwrap())
        .collect();
    // Uniform, tiny HP: XP awarded per kill is the victim's max_hp, and
    // 5 x 3 stays under xp_for_level(1) = 20 so no level-up spends the
    // total being measured.
    for &m in &members {
        let mut stats = game.world.get_mut::<Stats>(m).unwrap();
        stats.max_hp = 3;
        stats.hp = 3;
    }
    game.start_battle(members.clone());
    let before = game.world.get::<Experience>(player).unwrap().xp;

    for _ in 0..members.len() {
        game.finish_member(0, 0, player);
    }

    let exp = game.world.get::<Experience>(player).unwrap();
    assert_eq!(
        exp.level, 1,
        "the fixture must not level up, or the XP total below measures nothing"
    );
    assert_eq!(
        exp.xp - before,
        3 * members.len() as u32,
        "every vanquished member should pay its own max_hp in XP"
    );
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p feral-processes-engine every_member_of_a_group_pays_its_own_xp -- --nocapture`

Expected: **PASS.** This is a characterization test of behaviour that already ships. If it fails, stop and report — the failure is the finding, and the rest of the plan assumes per-member XP works.

- [ ] **Step 3: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/tests/combat_rewards.rs
git commit -m "test: pin that every group member pays its own XP"
```

---

### Task 2: Cap how many of a group act each round

**Files:**
- Modify: `crates/engine/src/battle.rs` (add helpers after `EnemyGroup`'s impl block, around line 20)
- Modify: `crates/engine/src/game/combat.rs:181-183` (`roll_initiative`)
- Modify: `crates/engine/src/game/combat_round.rs:352-355` (`all_wild_retaliate` doc comment)
- Test: `crates/engine/src/tests/combat_packs.rs` (append)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub(crate) fn ceil_sqrt(n: u32) -> u32` in `crate::battle`
  - `pub(crate) fn attackers_in_group(n: usize) -> usize` in `crate::battle`

  Task 3 calls `attackers_in_group`; Task 5 calls `ceil_sqrt`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_packs.rs`:

```rust
#[test]
fn ceil_sqrt_is_exact_at_perfect_squares() {
    for (n, expected) in [(0, 0), (1, 1), (2, 2), (3, 2), (4, 2), (9, 3), (10, 4), (81, 9), (100, 10)] {
        assert_eq!(
            crate::battle::ceil_sqrt(n),
            expected,
            "ceil_sqrt({n}) should be {expected} — a float sqrt().ceil() rounds \
             the wrong way at perfect squares"
        );
    }
}

/// A swarm is an attrition wall, not a linear damage multiplier: only the
/// front `ceil(sqrt(n))` members of a group get an initiative slot.
#[test]
fn only_the_front_ceil_sqrt_of_a_group_acts_each_round() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 6;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + DISTANCE_STAT_STEP_TILES * 8, spawn.y);

    let members: Vec<Entity> = (0..9)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i).unwrap())
        .collect();
    game.start_battle(members);

    let acting = game
        .roll_initiative()
        .into_iter()
        .filter(|a| matches!(a, crate::battle::Actor::Enemy { .. }))
        .count();

    assert_eq!(
        acting, 3,
        "a group of nine should swing three at a time, not nine"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine -- ceil_sqrt only_the_front_ceil_sqrt`

Expected: FAIL — `ceil_sqrt` does not exist (compile error), and once it does, the initiative test reports 9 instead of 3.

- [ ] **Step 3: Add the helpers**

In `crates/engine/src/battle.rs`, after the `impl EnemyGroup` block:

```rust
/// Smallest `k` where `k * k >= n`. Integer throughout: `(n as f64).sqrt().ceil()`
/// can round the wrong way at perfect squares, and both callers need 81 to
/// give exactly 9.
pub(crate) fn ceil_sqrt(n: u32) -> u32 {
    let root = n.isqrt();
    if root * root == n { root } else { root + 1 }
}

/// How many of a group's `n` living members can bring weapons to bear in one
/// round. A hundred-strong swarm cannot all reach the party at once, so it
/// swings ten at a time — which is what makes a swarm an attrition problem
/// rather than an instant wipe. Shared with `crate::balance` so the offline
/// projections and the real round loop cannot drift.
pub(crate) fn attackers_in_group(n: usize) -> usize {
    ceil_sqrt(n as u32) as usize
}
```

- [ ] **Step 4: Apply the cap in `roll_initiative`**

In `crates/engine/src/game/combat.rs`, replace lines 181-183:

```rust
        for (group, size) in group_sizes.into_iter().enumerate() {
            actors.extend((0..size).map(|slot| battle::Actor::Enemy { group, slot }));
        }
```

with:

```rust
        for (group, size) in group_sizes.into_iter().enumerate() {
            actors.extend(
                (0..battle::attackers_in_group(size))
                    .map(|slot| battle::Actor::Enemy { group, slot }),
            );
        }
```

`all_wild_retaliate` (`combat_round.rs:356`) walks the same `roll_initiative` output, so it inherits the cap for free. Its doc comment at lines 352-355 currently claims "Every currently-alive member of the active pack retaliates this round" — replace that first sentence with:

```rust
    /// The front `battle::attackers_in_group` members of each reachable
    /// group retaliate this round — enough of a pack to make it more
    /// dangerous than a solo encounter, without a hundred-strong swarm
    /// simply deleting the party. Each one independently rolls its own move
    /// and target (see `wild_retaliate`).
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine -- ceil_sqrt only_the_front_ceil_sqrt`

Expected: PASS.

- [ ] **Step 6: Run the full suite**

Run: `cargo test --workspace`

Expected: PASS. No shipped test asserts that every packmate attacks (checked — `combat_status.rs:7` and `combat_targeting.rs:11` assert *a* retaliation happens, and both use groups small enough that `ceil_sqrt` leaves at least one attacker). If a combat test fails on damage totals, it is because the pack got gentler; report the test name rather than re-tuning it silently.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/battle.rs crates/engine/src/game/combat.rs \
        crates/engine/src/game/combat_round.rs crates/engine/src/tests/combat_packs.rs
git commit -m "feat: only ceil(sqrt(n)) of an enemy group swings per round"
```

---

### Task 3: Make the balance sim's focus fire honest

`simulate_roster_fight` pools the roster's damage and spills overkill into the next member and then the next group. The real battle does not: only a group's front member is targetable, so one action kills at most one member and the excess is discarded. At 12 members that gap is a rounding error; at 400 it is the whole verdict.

**Files:**
- Modify: `crates/engine/src/balance.rs:283-338` (the round loop in `simulate_roster_fight`)
- Modify: `crates/engine/src/balance.rs:222-249` (the doc comment's second bullet)

**Interfaces:**
- Consumes: `crate::battle::attackers_in_group` (Task 2).
- Produces: no new signatures. `simulate_roster_fight`'s signature is unchanged.

- [ ] **Step 1: Replace the party half of the round loop**

In `crates/engine/src/balance.rs`, replace the block that begins `let mut incoming: i32 = roster` and ends at the closing brace of the `while incoming > 0 && !groups.is_empty()` loop (lines 284-305) with:

```rust
        // Focus fire on the front group, one fighter at a time, discarding
        // overkill. Only a group's front member is targetable in the real
        // battle, so a single action kills at most one member — pooling the
        // roster's damage would let a big group evaporate at a rate nothing
        // in the game can reproduce.
        for idx in 0..roster.len() {
            if roster[idx].hp <= 0.0 || groups.is_empty() {
                continue;
            }
            let dealt = compute_damage(roster[idx].atk, groups[0].0.stats.def, roster[idx].move_power);
            let (group, front_hp, remaining) = &mut groups[0];
            *front_hp -= dealt;
            if *front_hp <= 0 {
                *remaining -= 1;
                if *remaining == 0 {
                    groups.remove(0);
                } else {
                    *front_hp = group.stats.hp;
                }
            }
        }
```

Leave the `if groups.is_empty() { return BattleOutcome { player_won: true, .. } }` block that follows exactly as it is.

- [ ] **Step 2: Cap the enemy half by `attackers_in_group`**

In the same loop, replace `for _ in 0..*remaining {` (line 332) with:

```rust
            for _ in 0..crate::battle::attackers_in_group(*remaining as usize) {
```

- [ ] **Step 3: Fix the doc comment that now lies**

Replace the second bullet of `simulate_roster_fight`'s doc comment (lines 231-233, "**The party focuses the front group,** ... then into the group behind it as one empties.") with:

```rust
/// - **The party focuses the front group,** which is what a player does and
///   what the reach rule rewards. Each fighter's hit lands on that group's
///   front member and any overkill is discarded — the real battle can only
///   ever address the front of a group, so one action removes at most one
///   member however hard it lands.
/// - **Only `battle::attackers_in_group` of a group swing back,** the same
///   rule the real round loop applies in `Game::roll_initiative`.
```

- [ ] **Step 4: Run the balance suite and read the numbers**

Run: `cargo test -p feral-processes-engine balance -- --nocapture`

Expected: PASS, with every printed `needs level N` at or above what it printed before — discarding overkill can only slow the party down. The `[no gear]` sweep doc comment at `balance.rs:565` cites "confirmed empirically: 7, 15, 29, 57, 111 for zones 1-5"; update those five numbers to what this run prints.

**Hard stop:** if any sweep now panics with "isn't clearable by level 200", stop and report the zone and species. That means today's shipped pack sizes are already unwinnable under honest focus fire, which is a finding about the current game, not something to tune around inside this task.

- [ ] **Step 5: Run the full suite**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/balance.rs
git commit -m "fix: balance sim discards overkill the way the real battle does"
```

---

### Task 4: The group-size curve

**Files:**
- Modify: `crates/engine/src/lib.rs:98-125` (constants and docs)
- Modify: `crates/engine/src/game/spawning.rs:171-184` (`max_pack_size` → `zone_group_cap` + `max_group_size`), `:337-343` (the spawn roll), `:271` (doc reference)
- Modify: `crates/engine/src/game/combat.rs:12,34` (`gather_pack`'s cap call site — rename only; the real per-group work is Task 5)
- Modify: `crates/engine/src/balance.rs:197-220` (`split_into_groups` deleted, `full_pack_at_zone` rebuilt, `full_group_at_zone` added), `:396` and `:626` (call sites)
- Modify: `crates/engine/src/resources.rs:171` (doc reference to `max_pack_size`)
- Test: `crates/engine/src/tests/zone.rs:137-216` (both `max_pack_size` tests replaced)
- Test: `crates/engine/src/tests/combat_packs.rs:8-55` (the `PACK_SIZE_PER_ZONE` fixture)

**Interfaces:**
- Consumes: `crate::battle::attackers_in_group` (Task 2, already wired into the sim by Task 3).
- Produces:
  - `pub(crate) fn zone_group_cap(zone: u32) -> u32` — free function in `crate::game::spawning`
  - `pub(crate) fn Game::max_group_size(&self, x: i32, y: i32) -> u32`
  - `crate::MAX_GROUP_SIZE`, `crate::ZONE_GROUP_GROWTH`, `crate::GROUP_SIZE_STEP_TILES`

  `crate::game` is a private module at the crate root, which makes its `pub(crate)` items reachable from every module in the crate — `crate::balance` calls `crate::game::spawning::zone_group_cap` directly.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/zone.rs`, replace both `max_pack_size_*` tests (lines 137-216) with:

```rust
#[test]
fn zone_group_cap_is_geometric_and_never_passes_max_group_size() {
    use crate::game::spawning::zone_group_cap;
    assert_eq!(zone_group_cap(1), 1, "zone 1 is solo, whatever else is true");
    assert_eq!(zone_group_cap(2), 3);
    assert_eq!(zone_group_cap(3), 9);
    assert_eq!(zone_group_cap(4), 27);
    assert_eq!(zone_group_cap(5), 81);
    assert_eq!(zone_group_cap(6), MAX_GROUP_SIZE, "3^5 is 243, so zone 6 clamps");
    assert_eq!(
        zone_group_cap(99),
        MAX_GROUP_SIZE,
        "a deep zone must clamp rather than overflow the pow"
    );
}

#[test]
fn max_group_size_also_counts_from_the_platform_edge() {
    let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    place_home(&mut game, 0, 0);

    assert_eq!(
        game.max_group_size(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y),
        1,
        "groups shouldn't grow inside territory that's still stat-x1.0"
    );
    // The discriminating case: without the platform offset this is a full
    // GROUP_SIZE_STEP_TILES from spawn and would already have doubled.
    assert_eq!(
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES, spawn.y),
        1,
        "a full step from spawn is only half a step from the platform edge"
    );
    assert_eq!(
        game.max_group_size(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + GROUP_SIZE_STEP_TILES,
            spawn.y
        ),
        2,
        "the first doubling lands one full step past the platform edge"
    );
}

#[test]
fn max_group_size_doubles_with_distance_and_caps_per_zone() {
    // No Home is placed, so distances count straight from the spawn point —
    // see the platform-edge test above for the case where one exists.
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let at = |game: &Game, steps: i32| {
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES * steps, spawn.y)
    };

    assert_eq!(at(&game, 0), 1, "right at spawn, groups are always solo");
    assert_eq!(
        at(&game, 10),
        1,
        "zone 1 is solo however far you walk — that is the whole point of zone 1"
    );

    game.world.resource_mut::<ZoneLevel>().0 = 2;
    assert_eq!(at(&game, 0), 1, "every zone starts solo at its entry point");
    assert_eq!(at(&game, 1), 2, "one step out doubles");
    assert_eq!(at(&game, 2), 3, "two steps would be 4, but zone 2 caps at 3");
    assert_eq!(at(&game, 10), 3, "and it stays capped however far out");

    game.world.resource_mut::<ZoneLevel>().0 = 5;
    assert_eq!(at(&game, 6), 64, "six steps is 2^6, still under zone 5's cap of 81");
    assert_eq!(at(&game, 7), 81, "seven steps would be 128, so the zone cap binds");

    game.world.resource_mut::<ZoneLevel>().0 = 6;
    assert_eq!(at(&game, 7), MAX_GROUP_SIZE, "zone 6 is where the hard ceiling is reachable");

    // The exponent is clamped, so an absurd distance must not shift past
    // the width of the type.
    game.world.resource_mut::<ZoneLevel>().0 = 99;
    assert_eq!(
        game.max_group_size(spawn.x + 10_000, spawn.y),
        MAX_GROUP_SIZE,
        "no zone or distance may push a group past MAX_GROUP_SIZE"
    );
}
```

In `crates/engine/src/tests/combat_packs.rs`, the existing `gather_pack_pulls_in_nearby_hostiles_and_caps_at_max_pack_size` fixture uses `PACK_SIZE_STEP_TILES` and asserts a zone-1 cap of `PACK_SIZE_PER_ZONE`. Zone 1 is now solo, so retarget it at zone 3 and rename it:

```rust
#[test]
fn gather_pack_pulls_in_nearby_hostiles_and_caps_at_the_local_group_size() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Zone 3 caps a group at 9; one step out doubles to 2, which is what
    // this fixture pins — a cap that binds with three others in range.
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (ax, ay) = (spawn.x + GROUP_SIZE_STEP_TILES, spawn.y);
    let spawn_hostile = |game: &mut Game, x: i32, y: i32| {
        game.world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x, y },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 0,
                },
            ))
            .id()
    };
    let anchor = spawn_hostile(&mut game, ax, ay);
    for i in 1..=3 {
        spawn_hostile(&mut game, ax + i, ay);
    }

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack[0], anchor,
        "the creature actually bumped into should always be the pack's front"
    );
    assert_eq!(
        pack.len(),
        4,
        "one step out in zone 3 allows groups of 2, so all four gathered \
         hostiles fit under the 2 x MAX_ENEMY_GROUPS pack ceiling"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine -- zone_group_cap max_group_size gather_pack_pulls`

Expected: FAIL to compile — `zone_group_cap`, `max_group_size`, `GROUP_SIZE_STEP_TILES`, and `MAX_GROUP_SIZE` do not exist yet.

- [ ] **Step 3: Replace the constants**

In `crates/engine/src/lib.rs`, replace lines 98-119 (the `PACK_SIZE_STEP_TILES`, `PACK_SIZE_PER_ZONE`, and `MAX_PACK_SIZE` blocks, keeping `PACK_GATHER_RADIUS` where it is) with:

```rust
/// Tile distance per doubling of a wild group's size, counted from the same
/// origin as `DISTANCE_STAT_STEP_TILES` (the platform's edge once a Home
/// exists) — see `Game::max_group_size`. Deliberately equal to
/// `DISTANCE_STAT_STEP_TILES`: how many programs meet you and how hard each
/// one hits escalate on the same footing as you push out.
const GROUP_SIZE_STEP_TILES: i32 = DISTANCE_STAT_STEP_TILES;
```

and, after `PACK_GATHER_RADIUS`:

```rust
/// Geometric base for the group size each zone level allows: zone 1 is solo,
/// and every level after multiplies the cap by this against `MAX_GROUP_SIZE`
/// (1, 3, 9, 27, 81, 100). Only `battle::attackers_in_group` of a group
/// swing per round, so a deep swarm is an attrition wall rather than a
/// linear multiplier on incoming damage.
const ZONE_GROUP_GROWTH: u32 = 3;

/// Hard ceiling on a single species group. With `MAX_ENEMY_GROUPS` groups on
/// the field, one intrusion tops out at four hundred programs.
const MAX_GROUP_SIZE: u32 = 100;
```

- [ ] **Step 4: Replace `max_pack_size`**

In `crates/engine/src/game/spawning.rs`, replace `max_pack_size` (lines 171-184) with:

```rust
/// The zone's ceiling on one species group: zone 1 is solo, every level
/// after multiplies by `ZONE_GROUP_GROWTH`, and `MAX_GROUP_SIZE` is the
/// hard stop. `checked_pow` because zones are unbounded and `3^21`
/// overflows `u32` long before the clamp would catch it.
pub(crate) fn zone_group_cap(zone: u32) -> u32 {
    ZONE_GROUP_GROWTH
        .checked_pow(zone.saturating_sub(1))
        .unwrap_or(MAX_GROUP_SIZE)
        .clamp(1, MAX_GROUP_SIZE)
}
```

as a free function above `impl Game`, and inside the impl:

```rust
    /// Maximum size of one wild species group at `(x, y)`: capped by the
    /// zone (`zone_group_cap`), and reached by doubling every
    /// `GROUP_SIZE_STEP_TILES` from the danger origin — solo at your base,
    /// a swarm deep in the field. Used to pick how many creatures a group
    /// spawn roll places together (`try_spawn_habitat_creature`), as the
    /// per-group ceiling on one fight (`gather_pack`/`group_pack`), and to
    /// size the room a spawn roll needs (`maybe_spawn_wild_creature`).
    pub(crate) fn max_group_size(&self, x: i32, y: i32) -> u32 {
        let cap = zone_group_cap(self.world.resource::<ZoneLevel>().0);
        let dist = self.distance_from_danger_origin(x, y);
        // The map is unbounded and a shift of 32 or more is a panic in
        // debug; `1 << 7` already exceeds MAX_GROUP_SIZE, so clamping the
        // exponent there is exact rather than a fudge.
        let steps = (dist / GROUP_SIZE_STEP_TILES).clamp(0, 7) as u32;
        (1u32 << steps).min(cap)
    }
```

- [ ] **Step 5: Update the call sites**

Three renames, no logic change:

- `spawning.rs:340` — `let max_pack = self.max_pack_size(x, y);` becomes `let max_group = self.max_group_size(x, y);`, and the line below it becomes `rng.0.random_range(1..=max_group)`.
- `spawning.rs:271` and `resources.rs:171` — doc comments naming `max_pack_size` now name `max_group_size`.
- `combat.rs:34` — `self.max_pack_size(...)` becomes `self.max_group_size(...)`; the doc comment at `combat.rs:12` gets the same rename. Task 5 changes what this cap *means*; here it is only renamed.

- [ ] **Step 6: Rebuild the sim's packs**

In `crates/engine/src/balance.rs`, delete `split_into_groups` entirely (lines 197-213) and replace `full_pack_at_zone` (lines 215-220) with:

```rust
/// The swarm one intrusion throws at the player deep in `zone`: a full
/// `MAX_ENEMY_GROUPS` groups, each at the zone's group cap — what
/// `Game::max_group_size` allows once distance growth is fully unlocked.
fn full_pack_at_zone(species: &SpeciesDef, zone: u32) -> Vec<GroupSim> {
    let group = full_group_at_zone(species, zone);
    std::iter::repeat_n(group[0], crate::MAX_ENEMY_GROUPS).collect()
}

/// One species group at `zone`'s cap — the unit `min_level_to_clear_zone`
/// projects against. The four-group swarm is the reach rule's test case
/// (`the_reach_rule_measurably_softens_a_full_pack`), not the progression
/// baseline: the sim models no abilities, and AoE is what a four-group
/// swarm is answered with.
fn full_group_at_zone(species: &SpeciesDef, zone: u32) -> Vec<GroupSim> {
    vec![GroupSim {
        stats: wild_stats_at_zone(species, zone),
        count: crate::game::spawning::zone_group_cap(zone),
        move_power: average_move_power(species),
        ranged_move_power: average_ranged_move_power(species),
    }]
}
```

`GroupSim` is `Copy` (it is copied at `balance.rs:278` today), so `repeat_n` is fine.

At `balance.rs:396`, `min_level_to_clear_zone` switches to the single group:

```rust
    let groups = full_group_at_zone(wild_species, zone);
```

Its doc comment (lines 354-372) says the pack "rather than a lone creature, is the meaningful unit now". Replace that paragraph with:

```rust
/// The unit is one full-size *group* — every member of one species, at the
/// zone's cap. A lone creature is no contest at any level and would report
/// level 1 everywhere; the full four-group swarm is not something the sim
/// can score, because its intended answer is AoE and no ability is
/// modelled here.
```

At `balance.rs:626`, the survivability test's `eprintln!` prints the pack size from the old constants — change that argument to `crate::game::spawning::zone_group_cap(zone)` and the surrounding wording from "pack of {} {}s" to "group of {} {}s". Rename the test itself to `a_full_party_survives_a_full_group_at_each_zone`, and update its doc comment (lines 585-594) so "The pack-size increase is the counterweight" reads "The group-size increase is the counterweight".

- [ ] **Step 7: Run the new tests**

Run: `cargo test -p feral-processes-engine -- zone_group_cap max_group_size gather_pack_pulls`

Expected: PASS.

- [ ] **Step 8: Run the balance suite**

Run: `cargo test -p feral-processes-engine balance -- --nocapture`

Expected: PASS. Required levels *drop* versus Task 3's run, because the sweeps now face one group rather than four. Update the "confirmed empirically" numbers at `balance.rs:565` again to match this run.

**Bounded decision:** `the_reach_rule_measurably_softens_a_full_pack` fights the four-group swarm at zone 4 (now 4 × 27 = 108 members) and calls `.expect("the reach case must be clearable")`. If that expect fires, lower that test's `let zone = 4;` to `3` (4 × 9 = 36 members), which still exercises the reach valve across four groups, and say so in the commit message. Do not delete the test and do not raise `MAX_LEVEL_SEARCHED`.

- [ ] **Step 9: Run the full suite**

Run: `cargo test --workspace`

Expected: PASS. Any remaining failure naming `PACK_SIZE_PER_ZONE`, `MAX_PACK_SIZE`, or `PACK_SIZE_STEP_TILES` is a missed call site — those three constants no longer exist.

- [ ] **Step 10: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A crates/engine/src
git commit -m "feat: group size doubles with distance and grows geometrically per zone"
```

---

### Task 5: Per-group ceiling and swarm scatter

A spawn roll places one species, so a single roll's cluster is one group — left alone, 400 of one species is one group of 400 rather than four of 100.

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` (`swarm_radius`, and the scatter in `try_spawn_habitat_creature:344-356`)
- Modify: `crates/engine/src/game/combat.rs:15-37` (`gather_pack`), `:43-69` (`group_pack`)
- Test: `crates/engine/src/tests/combat_packs.rs` (append)

**Interfaces:**
- Consumes: `Game::max_group_size` (Task 4), `crate::battle::ceil_sqrt` (Task 2).
- Produces: `pub(crate) fn swarm_radius(n: u32) -> i32` in `crate::game::spawning`. Task 6 does not use it; nothing else does.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_packs.rs`:

```rust
/// A single-species cluster is one group, so without a per-group ceiling a
/// 30-strong cluster would fight as one 30-deep column regardless of what
/// the local danger curve allows.
#[test]
fn a_group_is_capped_at_the_local_group_size_and_the_rest_stay_on_the_map() {
    let mut game = Game::new(311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Far enough out that zone 3's cap of 9 is fully unlocked.
    let (x, y) = (spawn.x + GROUP_SIZE_STEP_TILES * 7, spawn.y);

    let members: Vec<Entity> = (0..30)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i % 3).unwrap())
        .collect();

    let groups = game.group_pack(members.clone());

    assert_eq!(groups.len(), 1, "one species is one group");
    assert_eq!(
        groups[0].members.len(),
        9,
        "zone 3 caps a group at 9 however many gathered"
    );
    let still_alive = members
        .iter()
        .filter(|&&e| game.world.get_entity(e).is_ok())
        .count();
    assert_eq!(
        still_alive, 30,
        "members over the ceiling stay standing on the map, they are not despawned"
    );
}

/// The headline shape: four groups of a hundred, and nothing bigger, out of
/// a cluster that could supply five hundred.
#[test]
fn a_mixed_swarm_fights_as_four_groups_of_a_hundred() {
    let mut game = Game::new(313, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 6;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + GROUP_SIZE_STEP_TILES * 7, spawn.y);

    let mut cluster = Vec::new();
    for species in ["glitch", "scrapper", "drone", "sprite", "ghost"] {
        for i in 0..105 {
            cluster.push(game.spawn_wild_creature(species, x, y + i % 5).unwrap());
        }
    }

    let groups = game.group_pack(cluster);

    assert_eq!(
        groups.len(),
        MAX_ENEMY_GROUPS,
        "five species can't all engage — the largest four do"
    );
    for group in &groups {
        assert_eq!(
            group.members.len(),
            MAX_GROUP_SIZE as usize,
            "no group may pass MAX_GROUP_SIZE, however deep the cluster is"
        );
    }
}

/// The gather radius has to widen with the swarm, or a group scattered
/// across a 21-tile span pulls into the fight in fragments.
#[test]
fn gather_radius_widens_with_the_local_group_size() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs().into_iter().next().unwrap().id.clone()
    };
    let mut game = Game::new(312, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 5;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Zone 5, fully unlocked: groups of 81, so a radius of ceil_sqrt(81) = 9.
    let (ax, ay) = (spawn.x + GROUP_SIZE_STEP_TILES * 7, spawn.y);
    let mut hostile = |game: &mut Game, x: i32, y: i32| {
        game.world
            .spawn((
                Creature { species: species_id.clone() },
                Hostile,
                Position { x, y },
                Stats { hp: 10, max_hp: 10, atk: 1, def: 0 },
            ))
            .id()
    };
    let anchor = hostile(&mut game, ax, ay);
    hostile(&mut game, ax + 8, ay);

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack.len(),
        2,
        "eight tiles out is inside a zone-5 swarm's radius, though it is well \
         outside the PACK_GATHER_RADIUS a small pack uses"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine -- a_group_is_capped_at_the_local gather_radius_widens`

Expected: FAIL — the first reports 30 members in the group, the second reports a pack of 1.

- [ ] **Step 3: Add `swarm_radius` and use it when scattering**

In `crates/engine/src/game/spawning.rs`, next to `zone_group_cap`:

```rust
/// How far a group of `n` scatters when it spawns, and how far `gather_pack`
/// searches from the member the player bumped — the same radius from the
/// same input, so a spawned cluster always pulls into exactly one fight.
/// `PACK_GATHER_RADIUS` stays the floor: nothing gets tighter than it was.
pub(crate) fn swarm_radius(n: u32) -> i32 {
    PACK_GATHER_RADIUS.max(crate::battle::ceil_sqrt(n) as i32)
}
```

In `try_spawn_habitat_creature`, hoist the radius above the placement loop (it takes no RNG, so the seeded sequence is untouched) and use it for the offsets:

```rust
        let radius = swarm_radius(group_size);
        for i in 0..group_size {
            // The first member anchors the roll's own tile; the rest
            // cluster loosely around it (walkability isn't rechecked for
            // these — same looseness the rest of spawning already has).
            let (gx, gy) = if i == 0 {
                (x, y)
            } else {
                let mut rng = self.world.resource_mut::<GameRng>();
                (
                    x + rng.0.random_range(-radius..=radius),
                    y + rng.0.random_range(-radius..=radius),
                )
            };
            self.spawn_wild_creature(&pick, gx, gy);
        }
```

- [ ] **Step 4: Widen `gather_pack` and cap `group_pack`**

In `crates/engine/src/game/combat.rs`, `gather_pack` gains a location-derived radius and a whole-pack ceiling:

```rust
        let group_cap = self.max_group_size(anchor_pos.x, anchor_pos.y).max(1);
        let radius = crate::game::spawning::swarm_radius(group_cap);
        let mut pack = vec![anchor];
```

with the distance test becoming `if dist <= radius {`, and the truncation:

```rust
        pack.truncate(group_cap as usize * MAX_ENEMY_GROUPS);
```

In `group_pack`, derive the same ceiling from the pack's anchor and stop adding members past it:

```rust
        // The anchor's own tile decides how big a group may be here, the
        // same input the spawner used. It is a cluster *member* rather than
        // the tile the spawn roll landed on, so a cluster straddling a
        // distance step can gather a little wider or narrower than it
        // scattered — the same looseness spawning already has, and the
        // failure mode is a member or two left standing.
        let cap = pack
            .first()
            .and_then(|&e| self.world.get::<Position>(e).copied())
            .map(|p| self.max_group_size(p.x, p.y) as usize)
            .unwrap_or(1)
            .max(1);
```

then, in the partition loop, replace `Some(group) => group.members.push(entity),` with:

```rust
                Some(group) => {
                    // Over the ceiling: left standing on the map, met on the
                    // next bump — what surplus groups already do.
                    if group.members.len() < cap {
                        group.members.push(entity);
                    }
                }
```

Update `group_pack`'s doc comment (lines 39-42) to say each group is also capped at the anchor's `max_group_size`, with the surplus staying on the map.

- [ ] **Step 5: Run the new tests**

Run: `cargo test -p feral-processes-engine -- a_group_is_capped_at_the_local gather_radius_widens`

Expected: PASS.

- [ ] **Step 6: Run the full suite**

Run: `cargo test --workspace`

Expected: PASS. Watch for battle tests that build multi-member fixtures near a zone-1 spawn point — those now truncate to one member. If one fails, move its fixture deep-and-far the way Task 1's XP test does rather than weakening the assertion.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A crates/engine/src
git commit -m "feat: cap each species group at the local size and scatter swarms wider"
```

---

### Task 6: Map budget

One pack can now be 400 programs, so a 100-wide map budget cannot hold a single encounter — and culling one hostile per spawn roll while a roll places up to 100 would let the population climb ~99 per roll and never come back under the cap.

**Files:**
- Modify: `crates/engine/src/lib.rs:134-143` (`WILD_CREATURE_CAP` and its doc)
- Modify: `crates/engine/src/game/spawning.rs:226-267` (`maybe_spawn_wild_creature`)
- Test: `crates/engine/src/tests/spawning.rs` (append)

**Interfaces:**
- Consumes: `Game::max_group_size` (Task 4).
- Produces: nothing new. `maybe_spawn_wild_creature`'s signature is unchanged.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/spawning.rs`:

```rust
/// One roll can place a whole group, so the cull has to free room for the
/// group rather than for one creature — otherwise the population ratchets
/// up past the cap and never comes back down.
#[test]
fn a_spawn_roll_culls_enough_room_for_the_whole_group_it_places() {
    let mut game = Game::new(425, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let species_id = game.species_defs().into_iter().next().unwrap().id;

    // Put the player deep in the field, where a roll places a real group
    // rather than a single creature.
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let player = game.player_entity();
    let far = Position {
        x: spawn.x + GROUP_SIZE_STEP_TILES * 7,
        y: spawn.y,
    };
    *game.world.get_mut::<Position>(player).unwrap() = far;
    assert!(
        game.max_group_size(far.x, far.y) > 1,
        "the fixture is pointless unless a roll here places more than one"
    );

    // Fill the cap with a population far from the player.
    let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
    let already = hostile_query.iter(&game.world).count();
    for _ in 0..WILD_CREATURE_CAP - already {
        game.world.spawn((
            Creature { species: species_id.clone() },
            Position { x: far.x + 500, y: far.y + 500 },
            Stats { hp: 10, max_hp: 10, atk: 1, def: 1 },
            Hostile,
        ));
    }

    for _ in 0..60 {
        game.maybe_spawn_wild_creature();
        // Bind the query before iterating: `query_filtered` takes `&mut
        // World`, so it can't be chained straight into an `iter(&world)`.
        let live = hostile_query.iter(&game.world).count();
        assert!(
            live <= WILD_CREATURE_CAP + NEST_GUARDIAN_MAX as usize,
            "the hostile population ran past the cap ({live} of {WILD_CREATURE_CAP}) — \
             the cull is freeing room for one creature, not for the group being placed"
        );
    }
}
```

The `NEST_GUARDIAN_MAX` slack is deliberate and worth its comment: a nest roll spawns its guardians through `spawn_nest`, which is not sized by `max_group_size`, so it is the one path that can overspend the budget by a bounded amount.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine a_spawn_roll_culls_enough_room -- --nocapture`

Expected: FAIL — the population climbs past `WILD_CREATURE_CAP` within the first few rolls.

- [ ] **Step 3: Raise the cap**

In `crates/engine/src/lib.rs`, `WILD_CREATURE_CAP` becomes `2000`, and the sentence in its doc comment describing the cull ("reaching it culls the `Hostile` farthest from the player to free a slot") becomes:

```rust
/// reaching it culls the `Hostile`s farthest from the player until the group
/// about to spawn fits — see `Game::maybe_spawn_wild_creature`. One roll can
/// place up to `MAX_GROUP_SIZE` creatures, so freeing a single slot would let
/// the population ratchet upward with every roll.
```

- [ ] **Step 4: Cull to fit**

In `crates/engine/src/game/spawning.rs`, restructure `maybe_spawn_wild_creature` so the offset is drawn *before* the cull. The RNG call order is unchanged — the 5% roll, then the offset draw, then whatever `try_spawn_habitat_creature` rolls — so no seeded test shifts:

```rust
        let (dx, dy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (rng.0.random_range(-12..=12), rng.0.random_range(-12..=12))
        };
        let (tx, ty) = (player_pos.x + dx, player_pos.y + dy);
        // Make room for the whole group this roll may place, by despawning
        // the `Hostile`s farthest (Chebyshev, matching 8-directional
        // movement) from where the player is now — the ones least likely to
        // ever be encountered again. `NestGuardian`s are eligible like any
        // other hostile; a cull is a plain despawn, so it deliberately
        // doesn't feed the nest's `pending_respawns` the way an actual
        // defeat does. Guardian counts are best-effort once a nest is far
        // behind the player.
        let needed = self.max_group_size(tx, ty) as usize;
        let mut hostiles: Vec<(Entity, i32)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&self.world)
                .map(|(e, p)| {
                    (
                        e,
                        (p.x - player_pos.x).abs().max((p.y - player_pos.y).abs()),
                    )
                })
                .collect()
        };
        let over = (hostiles.len() + needed).saturating_sub(WILD_CREATURE_CAP);
        if over > 0 {
            hostiles.sort_by_key(|&(_, dist)| std::cmp::Reverse(dist));
            for &(entity, _) in hostiles.iter().take(over) {
                self.world.despawn(entity);
            }
        }
        self.try_spawn_habitat_creature(tx, ty);
```

This replaces the existing hostiles query, the `if hostiles.len() >= WILD_CREATURE_CAP && let Some(...)` block, and the trailing offset draw and spawn call.

- [ ] **Step 5: Run the spawning tests**

Run: `cargo test -p feral-processes-engine spawning -- --nocapture`

Expected: PASS, including the two existing cap tests (`a_full_wild_population_far_away_is_culled_...` and `nest_guardians_are_eligible_to_be_culled_...`), which now build 2000-entity fixtures instead of 100.

If the suite's wall-clock time has grown noticeably, cut those two tests' `for _ in 0..500` loops to `for _ in 0..50` — they only need one successful spawn to prove their point, and 500 attempts against a 2000-entity population is 20× the work it was. Note the change in the commit message.

- [ ] **Step 6: Run the full suite and time it**

Run: `cargo test --workspace`

Expected: PASS. Report the total wall-clock time against the ~3s baseline in `CLAUDE.md`.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A crates/engine/src
git commit -m "feat: size the map's hostile budget and its cull for whole groups"
```

---

### Task 7: Documentation

**Files:**
- Modify: `README.md:195-206` (the **Packs** paragraph) and `README.md:346-358` (the **Enemies fight as groups** / reach paragraphs)
- Check: `assets/species/README.md:53` and `:148`

**Interfaces:**
- Consumes: the shipped behaviour from Tasks 2, 4, 5, 6.
- Produces: nothing.

- [ ] **Step 1: Rewrite the Packs paragraph**

`README.md`'s **Packs** paragraph currently claims the cap is "your current zone level + 1 (zone 1 → at most 2, zone 2 → at most 3, and so on)" — already wrong before this change, and doubly so now. Replace the sentence beginning "How large a pack can get is capped" through the end of that paragraph with:

```markdown
How large a group can get depends on both how deep the zone is and how far
the encounter is from your base: zone 1 is solo programs wherever you go,
and every zone after that triples the ceiling (zone 2 → 3, zone 3 → 9,
zone 4 → 27, zone 5 → 81, zone 6 and deeper → 100). You only meet that
ceiling out in the field — a group doubles in size every 15 tiles from the
edge of your platform, so encounters near home stay small however deep
you've breached. See [Zones and portals](#zones-and-portals) for the
matching distance scaling on individual stats.
```

- [ ] **Step 2: Fix the group paragraphs**

In the **Enemies fight as groups** paragraph, "three Glitches are one addressable unit — `A  3 Glitches`" still holds; leave it. In the reach paragraph, "This is what keeps a twelve-program pack survivable" is now wrong twice over (the ceiling is 400, and reach is no longer the only valve). Replace that sentence with:

```markdown
That, plus the fact that only some of a group can swing at once — a
hundred-strong swarm brings ten weapons to bear in a round, not a hundred —
is what keeps a deep-field intrusion survivable at all.
```

- [ ] **Step 3: Check the species schema doc**

Run: `rg -n "pack" assets/species/README.md`

Line 53 documents the per-move `ranged` flag ("A pack fights as species groups") and line 148 mentions habitat spawning. Neither states a size, so neither is falsified — leave them unless the surrounding wording implies the old cap. No schema field changed, so no other asset README needs touching.

- [ ] **Step 4: Verify no stale numbers remain**

Run: `rg -n "twelve|MAX_PACK_SIZE|PACK_SIZE_PER_ZONE|zone level \+ 1" README.md assets/*/README.md`

Expected: no hits.

- [ ] **Step 5: Final gate**

Run: `cargo test --workspace && cargo clippy --workspace && cargo fmt --check`

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add README.md
git commit -m "docs: describe swarm-sized groups and the per-round attacker cap"
```

---

## Notes for whoever plays this

Three things this plan deliberately does not change, all called out in the spec and worth watching the first time anyone actually plays a deep zone:

- **Levelling mid-fight fully heals.** `progression::add_xp` sets `stats.hp = stats.max_hp` on every level-up. Across a 400-kill swarm the player may level several times, each one a free full heal. That is what makes long swarms survivable, and it is also the most likely thing to feel broken.
- **Rewards scale linearly.** A cleared 400-swarm pays roughly 33× what a 12-pack paid.
- **Fight length.** A four-group swarm is hundreds of single-target actions unless the party brings `cascade_overflow` or `broadcast_storm`.
