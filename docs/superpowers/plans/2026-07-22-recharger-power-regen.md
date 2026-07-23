# Recharger power regeneration + Home rest gate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Recharger Node passively restore the player's Power across the base, and move the rest gate from the Recharger Node to Home.

**Architecture:** A new `#[serde(default)]` `power_regen` field on `StructureDef` drives a new `power_regen_system`, chained ahead of `needs_decay_system` so arriving at 0 Power costs no Integrity. Home's rest gate is pure data — `enables_rest` already works for any structure. No Rust code names either structure id.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, no Bevy app), RON asset files.

Spec: `docs/superpowers/specs/2026-07-22-recharger-power-regen-design.md`

## Global Constraints

- **Never commit.** `Bash(git commit *)` is in the **deny** list of `.claude/settings.local.json`, and CLAUDE.md forbids committing unless explicitly asked. Every task ends with a full-suite run instead. Leave the working tree dirty for the user to review and commit.
- **No hardcoded structure ids.** Per CLAUDE.md's moddability rule, no Rust code may branch on `"recharger_node"` or `"home"` for these behaviors. Both are expressed as `.ron` fields.
- **New `StructureDef`/`RestDef` fields get `#[serde(default)]`** so existing mod files keep parsing.
- **Update `assets/structures/README.md` in the same change** as any schema change — it is the modder-facing schema reference.
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate** for every task — not just the tests you wrote. Baseline is ~200 tests passing in ~1s.
- **No flaky tests.** No `sleep()`, no wall-clock, no unseeded RNG.
- Balance values, verbatim: Recharger Node `per_tick: 1.0`, `radius: 15`, `build_cost: [(CoreFragment, 10)]`. Home `enables_rest: Some((radius: 15))`. Both radii equal `MAX_BUILD_DISTANCE_FROM_HOME` (15).

---

### Task 1: The `power_regen` capability

Adds the schema field and the system that acts on it, registered in the shared schedule. Nothing in `assets/` uses it yet, so game behavior is unchanged — this task is verified entirely by unit tests against fixture data.

**Files:**
- Modify: `crates/engine/src/structures.rs` (add `PowerRegenDef` after `PassiveProcessDef` ~line 47; add field to `StructureDef` after `passive_process` ~line 96)
- Modify: `crates/engine/src/systems.rs` (constant near line 28; system after `passive_process_system` ~line 263; tests in `mod tests` from line 265)
- Modify: `crates/engine/src/lib.rs` (extract `build_schedule`, replacing the duplicated blocks at lines 550-557 and 663-670)

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - `structures::PowerRegenDef { per_tick: f32, radius: i32 }` — public, `Clone + Debug + Serialize + Deserialize`
  - `StructureDef::power_regen: Option<PowerRegenDef>`
  - `systems::power_regen_system` — public system fn
  - `Game::build_schedule() -> Schedule` — private associated fn
  - test helper `systems::tests::load_fixture_db(&[(&str, &str)]) -> StructureDb`

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/systems.rs`, add to the top of `mod tests` (after the existing `use` lines at 267-269):

```rust
    use crate::components::PLAYER_BASE_STATS;
    use std::sync::atomic::{AtomicU32, Ordering};
```

Then add these helpers inside `mod tests`:

```rust
    /// Writes `files` (filename, RON body) into a scratch dir and loads
    /// them through `StructureDb::load_dir` — `StructureDb`'s map is
    /// private outside its own module, so a fixture db has to come from
    /// disk. The counter disambiguates the directory per call: the pid
    /// alone repeats for every test in a run, so two tests loading
    /// fixtures in parallel would delete each other's directory mid-read.
    fn load_fixture_db(files: &[(&str, &str)]) -> StructureDb {
        static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "feral_structure_fixture_{}_{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        let (db, warnings) = StructureDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            warnings.is_empty(),
            "fixture should parse cleanly: {warnings:?}"
        );
        db
    }

    /// A structure that only regenerates Power. `per_tick` and `radius`
    /// are deliberately unlike the shipped Recharger Node's (1.0 / 15) so
    /// a test asserting on them can't accidentally pass against real game
    /// data instead of this fixture.
    fn load_test_recharger() -> StructureDb {
        load_fixture_db(&[(
            "test_recharger.ron",
            r#"(
                id: "test_recharger",
                name: "Test Recharger",
                glyph: 'z',
                color: Orange,
                build_cost: [],
                work: None,
                power_regen: Some((
                    per_tick: 2.0,
                    radius: 3,
                )),
            )"#,
        )])
    }

    /// A player at the origin with `hunger` Power, plus one structure of
    /// `kind` at each of `structure_positions`.
    fn power_regen_world(
        db: StructureDb,
        kind: &str,
        hunger: f32,
        structure_positions: &[(i32, i32)],
    ) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(db);
        world.insert_resource(MessageLog::default());
        let player = world
            .spawn((
                Player,
                Position { x: 0, y: 0 },
                Needs {
                    hunger,
                    fatigue: 100.0,
                },
                PLAYER_BASE_STATS,
            ))
            .id();
        for (x, y) in structure_positions {
            world.spawn((
                Structure {
                    kind: kind.to_string(),
                },
                Position { x: *x, y: *y },
            ));
        }
        (world, player)
    }

    /// Runs `power_regen_system` alone for one tick and returns the
    /// player's resulting Power.
    fn run_regen_once(db: StructureDb, kind: &str, hunger: f32, at: &[(i32, i32)]) -> f32 {
        let (mut world, player) = power_regen_world(db, kind, hunger, at);
        let mut schedule = Schedule::default();
        schedule.add_systems(power_regen_system);
        schedule.run(&mut world);
        world.get::<Needs>(player).unwrap().hunger
    }
```

Now the tests, also inside `mod tests`:

```rust
    #[test]
    fn power_regen_restores_per_tick_while_in_range() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[(0, 0)]);
        assert_eq!(hunger, 52.0, "an in-range structure should add its per_tick");
    }

    #[test]
    fn power_regen_clamps_at_full_power() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 99.0, &[(0, 0)]);
        assert_eq!(hunger, 100.0, "Power must never exceed the 0..=100 range");
    }

    #[test]
    fn power_regen_ignores_a_structure_past_its_radius_on_either_axis() {
        for at in [(4, 0), (0, 4), (-4, 0), (0, -4)] {
            let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[at]);
            assert_eq!(
                hunger, 50.0,
                "a structure at {at:?} is outside radius 3 and should do nothing"
            );
        }
    }

    #[test]
    fn power_regen_applies_at_exactly_the_radius_boundary() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[(3, 3)]);
        assert_eq!(hunger, 52.0, "radius is inclusive, matching passive_process");
    }

    #[test]
    fn power_regen_stacks_across_in_range_structures() {
        let hunger = run_regen_once(
            load_test_recharger(),
            "test_recharger",
            50.0,
            &[(0, 0), (1, 1)],
        );
        assert_eq!(hunger, 54.0, "each in-range structure adds its own per_tick");
    }

    #[test]
    fn a_structure_without_power_regen_does_not_restore_power() {
        let hunger = run_regen_once(load_test_capacitor(), "test_capacitor", 50.0, &[(0, 0)]);
        assert_eq!(
            hunger, 50.0,
            "a def that sets no power_regen must be inert here"
        );
    }

    #[test]
    fn power_regen_runs_before_decay_so_arriving_drained_costs_no_integrity() {
        let (mut world, player) =
            power_regen_world(load_test_recharger(), "test_recharger", 0.1, &[(0, 0)]);
        let mut schedule = Schedule::default();
        schedule.add_systems((power_regen_system, needs_decay_system).chain());
        schedule.run(&mut world);

        let stats = *world.get::<Stats>(player).unwrap();
        let needs = *world.get::<Needs>(player).unwrap();
        assert_eq!(
            stats.hp, stats.max_hp,
            "regen must cover the player before decay can starve them"
        );
        assert!(
            (needs.hunger - (0.1 + 2.0 - HUNGER_DECAY_PER_TICK)).abs() < 1e-5,
            "expected regen then decay, got {}",
            needs.hunger
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine power_regen`

Expected: FAIL to compile — `cannot find function power_regen_system in this scope`, and the fixture RON's `power_regen` field is rejected by `StructureDef`'s deserializer.

- [ ] **Step 3: Add `PowerRegenDef` and the `StructureDef` field**

In `crates/engine/src/structures.rs`, immediately after the `PassiveProcessDef` struct (ends line 47):

```rust
/// A structure's power-regeneration capability — see
/// `StructureDef::power_regen` and `systems::power_regen_system`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PowerRegenDef {
    /// Power (`components::Needs::hunger`) restored per tick while the
    /// player is in range. Stacks additively across every in-range
    /// structure that sets it.
    pub per_tick: f32,
    /// Chebyshev distance (in tiles) the player must be within for this to
    /// run, same box-radius style as `PassiveProcessDef::radius`.
    pub radius: i32,
}
```

In the same file, add to `StructureDef` directly after the `passive_process` field (line 96):

```rust
    /// If set, this structure restores the player's Power every tick while
    /// they stand within `radius` tiles — no assigned worker and no input
    /// item, unlike `work` and `passive_process`. `#[serde(default)]` so
    /// existing structure files (including mods) written before this field
    /// existed still parse (defaulting to no regeneration).
    #[serde(default)]
    pub power_regen: Option<PowerRegenDef>,
```

- [ ] **Step 4: Add the system**

In `crates/engine/src/systems.rs`, alongside the other decay constants (after line 28):

```rust
/// The ceiling for Power, per the 0..=100 range documented on
/// `components::Needs`.
const MAX_POWER: f32 = 100.0;
```

Then add the system immediately after `passive_process_system` ends (line 263):

```rust
/// Restores the player's Power once per tick for every in-range structure
/// whose def sets `power_regen` — no worker and no input item, unlike
/// `task_progress_system` and `passive_process_system`.
///
/// Chained ahead of `needs_decay_system` (see `Game::build_schedule`), and
/// that order is load-bearing: run the other way round, a player limping
/// into range at 0.1 Power is driven to 0 first, docked an Integrity point
/// and shown the "power reserves are critical!" warning on the very tick
/// the structure was about to cover them.
pub fn power_regen_system(
    mut player: Query<(&Position, &mut Needs), With<Player>>,
    structures: Query<(&Structure, &Position)>,
    structure_db: Res<StructureDb>,
) {
    for (player_pos, mut needs) in &mut player {
        let player_pos = *player_pos;
        for (structure, pos) in &structures {
            let Some(regen) = structure_db
                .get(&structure.kind)
                .and_then(|def| def.power_regen.as_ref())
            else {
                continue;
            };
            if (pos.x - player_pos.x).abs() > regen.radius
                || (pos.y - player_pos.y).abs() > regen.radius
            {
                continue;
            }
            needs.hunger = (needs.hunger + regen.per_tick).min(MAX_POWER);
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine power_regen`

Expected: PASS, 7 tests (`power_regen_restores_per_tick_while_in_range`, `power_regen_clamps_at_full_power`, `power_regen_ignores_a_structure_past_its_radius_on_either_axis`, `power_regen_applies_at_exactly_the_radius_boundary`, `power_regen_stacks_across_in_range_structures`, `a_structure_without_power_regen_does_not_restore_power`, `power_regen_runs_before_decay_so_arriving_drained_costs_no_integrity`).

- [ ] **Step 6: Rewire the existing capacitor fixture onto the shared loader**

`load_test_capacitor` (systems.rs line ~278) duplicates the scratch-dir dance and carries the same parallel-collision hazard. Replace its whole body, keeping the RON verbatim:

```rust
    /// A conversion that consumes a banked currency (no cargo cost) and
    /// produces ordinary cargo — unlike any shipped recipe, this can
    /// actually grow cargo usage, so it's the only way to observe the
    /// buffer-overflow bug a net-zero recipe like the real Terminal can't
    /// expose.
    fn load_test_capacitor() -> StructureDb {
        load_fixture_db(&[(
            "test_capacitor.ron",
            r#"(
                id: "test_capacitor",
                name: "Test Capacitor",
                glyph: 'C',
                color: Cyan,
                build_cost: [],
                work: None,
                passive_process: Some((
                    consumes: ResearchData,
                    produces: CoreFragment,
                    ticks_per_unit: 1,
                    radius: 5,
                )),
            )"#,
        )])
    }
```

- [ ] **Step 7: Extract the shared schedule and chain the ordering**

In `crates/engine/src/lib.rs`, add this private associated fn to the `impl Game` block, directly above `pub fn load` (line ~659):

```rust
    /// The system schedule every tick runs, shared by `new` and `load` so
    /// the two can't drift — the chained pair below is exactly the kind of
    /// constraint that gets added to one copy and forgotten in the other.
    fn build_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            (systems::power_regen_system, systems::needs_decay_system).chain(),
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));
        schedule
    }
```

Then replace **both** duplicated blocks — lines 550-557 in `new` and 663-670 in `load`, each of which reads:

```rust
        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::needs_decay_system,
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));
```

with:

```rust
        let schedule = Self::build_schedule();
```

- [ ] **Step 8: Verify the whole suite is green**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no clippy warnings; all tests pass. Game behavior is unchanged so far — no shipped `.ron` sets `power_regen`.

- [ ] **Step 9: Do not commit**

Leave the changes staged-free and dirty. Committing is denied by settings; the user reviews and commits.

---

### Task 2: Home becomes a rest gate

Home gains `enables_rest`. The Recharger Node keeps its own `enables_rest` for now, so nothing that works today stops working and the suite stays green — Task 3 removes it.

**Files:**
- Modify: `assets/structures/home.ron`
- Modify: `crates/engine/src/lib.rs` (test helper at line 7294 and its three call sites at 6781, 6800, 8723; test at 8748; new test)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: test helper `spawn_rest_structure_at_player(&mut Game)` — replaces `spawn_recharger_node_at_player`, same signature.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/lib.rs`, inside `mod tests`, next to the other structure-schema tests (after `recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent`, line ~8746):

```rust
    #[test]
    fn home_enables_rest_across_the_whole_base_footprint() {
        let game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "home")
            .expect("home.ron should load");
        assert_eq!(
            def.enables_rest
                .as_ref()
                .expect("Home should be the rest gate")
                .radius,
            MAX_BUILD_DISTANCE_FROM_HOME,
            "Home's rest radius should cover exactly the base footprint"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine home_enables_rest`

Expected: FAIL — panics on `Home should be the rest gate` (`enables_rest` is `None`).

- [ ] **Step 3: Add the field to `home.ron`**

Rewrite `assets/structures/home.ron` in full:

```ron
(
    id: "home",
    name: "Home",
    glyph: 'H',
    color: Green,
    build_cost: [(CoreFragment, 5)],
    work: None,
    teleport_cost: Some([(PowerCell, 4)]),
    enables_rest: Some((radius: 15)),
)
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine home_enables_rest`

Expected: PASS.

- [ ] **Step 5: Repoint the rest test helper at Home**

In `crates/engine/src/lib.rs`, replace `spawn_recharger_node_at_player` (line 7294) and its doc comment with:

```rust
    /// Deploys a Home directly on the player's current tile — `Game::rest`
    /// requires a rest-enabling structure nearby, so tests exercising
    /// `rest` need one in place first. Spawned directly rather than
    /// through `place_structure` to sidestep its cost and one-Home-only
    /// requirements, which aren't what these tests are about.
    fn spawn_rest_structure_at_player(game: &mut Game) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
        ));
    }
```

Update all three call sites — lines 6781, 6800 and 8723 each read `spawn_recharger_node_at_player(&mut game);` and become:

```rust
        spawn_rest_structure_at_player(&mut game);
```

- [ ] **Step 6: Retarget the no-rest-available test**

Replace the test at line 8748 (`rest_is_a_no_op_without_a_nearby_recharger_node`) with:

```rust
    #[test]
    fn rest_is_a_no_op_without_a_nearby_rest_structure() {
        let mut game = Game::new(401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut needs = game.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 10.0;
        }

        game.rest();

        let needs = *game.world.get::<Needs>(player).unwrap();
        assert_eq!(
            needs.fatigue, 10.0,
            "resting with no Home in range shouldn't restore anything"
        );
    }
```

- [ ] **Step 7: Verify the whole suite is green**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no clippy warnings; all tests pass, including the three rest tests now resting near Home.

- [ ] **Step 8: Do not commit**

---

### Task 3: The Recharger Node becomes a power source

Swaps the Recharger Node's role: drops `enables_rest`, adds `power_regen`, raises the cost to 10. This is the task where the feature goes live end-to-end.

**Files:**
- Modify: `assets/structures/recharger_node.ron`
- Modify: `crates/engine/src/lib.rs` (`structure_description` ~line 4541; schema test at 8732; new tests)

**Interfaces:**
- Consumes: `StructureDef::power_regen` and `power_regen_system` from Task 1; `spawn_rest_structure_at_player` from Task 2.
- Produces: nothing later tasks depend on in code.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`, replace the test at line 8732 (`recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent`) with:

```rust
    #[test]
    fn recharger_node_loads_as_a_permanent_base_wide_power_source() {
        let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "recharger_node")
            .expect("recharger_node.ron should load");
        assert_eq!(def.build_cost, vec![(ItemId::CoreFragment, 10)]);
        let regen = def
            .power_regen
            .as_ref()
            .expect("the Recharger Node should regenerate Power");
        assert_eq!(regen.per_tick, 1.0);
        assert_eq!(
            regen.radius, MAX_BUILD_DISTANCE_FROM_HOME,
            "the Recharger Node should cover the whole base"
        );
        assert!(
            def.enables_rest.is_none(),
            "resting moved to Home; the Recharger Node is no longer a rest gate"
        );
        assert!(
            def.temporary.is_none(),
            "the Recharger Node should be a permanent structure"
        );
    }
```

Add these three tests next to it:

```rust
    /// Deploys a Recharger Node `dx`/`dy` tiles from the player, bypassing
    /// `place_structure`'s Home and cost requirements — this is about the
    /// regen system, not the build rules.
    fn spawn_recharger_node(game: &mut Game, dx: i32, dy: i32) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position {
                x: player_pos.x + dx,
                y: player_pos.y + dy,
            },
        ));
    }

    #[test]
    fn a_recharger_node_in_range_nets_power_upward_on_a_real_tick() {
        let mut game = Game::new(403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        spawn_recharger_node(&mut game, 0, 0);

        game.wait();

        let hunger = game.world.get::<Needs>(player).unwrap().hunger;
        assert!(
            (hunger - 50.85).abs() < 1e-4,
            "expected +1.0 regen less 0.15 decay, got {hunger}"
        );
    }

    #[test]
    fn a_recharger_node_past_the_base_footprint_does_not_reach_the_player() {
        let mut game = Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        spawn_recharger_node(&mut game, MAX_BUILD_DISTANCE_FROM_HOME + 1, 0);

        game.wait();

        let hunger = game.world.get::<Needs>(player).unwrap().hunger;
        assert!(
            (hunger - 49.85).abs() < 1e-4,
            "expected decay only, got {hunger}"
        );
    }

    #[test]
    fn reaching_a_recharger_node_while_drained_costs_no_integrity() {
        let mut game = Game::new(405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 0.1;
        let before = *game.world.get::<Stats>(player).unwrap();
        spawn_recharger_node(&mut game, 0, 0);

        game.wait();

        let after = *game.world.get::<Stats>(player).unwrap();
        assert_eq!(
            after.hp, before.hp,
            "regen runs before decay, so arriving drained must not cost Integrity"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine recharger`

Expected: FAIL — `recharger_node_loads_as_a_permanent_base_wide_power_source` panics on the build-cost assertion (`5` vs `10`), and the three behavior tests fail because `power_regen` is unset (no regen applied; Power falls by decay alone and the drained player takes a point of damage).

- [ ] **Step 3: Swap the Recharger Node's role in data**

Rewrite `assets/structures/recharger_node.ron` in full:

```ron
(
    id: "recharger_node",
    name: "Recharger Node",
    glyph: 'z',
    color: Orange,
    build_cost: [(CoreFragment, 10)],
    work: None,
    power_regen: Some((
        per_tick: 1.0,
        radius: 15,
    )),
)
```

- [ ] **Step 4: Describe the capability in the build menu**

In `crates/engine/src/lib.rs`, in `structure_description`, add this clause immediately after the `passive_process` clause (which ends at line 4497, before the `bench_for` block):

```rust
        if let Some(regen) = &def.power_regen {
            parts.push(format!(
                "recharges {} Power per tick within {} tiles",
                regen.per_tick, regen.radius
            ));
        }
```

The existing assertion at line 5901, `assert!(describe("recharger_node").contains("recharge"))`, is satisfied by "recharges" and needs no change.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine recharger`

Expected: PASS, 4 tests.

- [ ] **Step 6: Verify the whole suite is green**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no clippy warnings; all tests pass. Note `a_structure_named_by_no_research_file_is_buildable_from_the_start` (line 5166) still lists `recharger_node` — neither structure's research gating changed, so it needs no edit.

- [ ] **Step 7: Do not commit**

---

### Task 4: Documentation

The schema docs are the modder-facing reference and CLAUDE.md requires them updated in the same change. The player-facing docs currently tell you to build a Recharger Node in order to rest, which is now wrong in both directions.

**Files:**
- Modify: `assets/structures/README.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: the final shipped values from Tasks 2 and 3.
- Produces: nothing.

- [ ] **Step 1: Document `power_regen` in the structure schema**

In `assets/structures/README.md`, insert after the `passive_process` block (which ends at line 48, before the `teleport_cost` comment):

```ron
    // Optional; can be left out entirely (defaults to no regeneration).
    // If set, the structure restores `per_tick` Power to the player every
    // tick that they're standing within `radius` tiles of it — no assigned
    // worker and no input item, unlike `work` and `passive_process`.
    // Stacks additively across every in-range structure that sets it, and
    // clamps at full Power. This is how the Recharger Node works:
    // `power_regen: Some((per_tick: 1.0, radius: 15))`, a radius chosen to
    // cover a whole base (structures must be built within 15 tiles of
    // Home).
    power_regen: Some((
        per_tick: 1.0,
        radius: 15,
    )),
```

- [ ] **Step 2: Re-point the `enables_rest` example at Home**

In `assets/structures/README.md`, replace the `enables_rest` comment at lines 102-107 with:

```ron
    // Optional; can be left out entirely (defaults to no rest capability).
    // If set, `Game::rest` (recharge/overnight rest) is only allowed while
    // the player stands within `radius` tiles of this structure — resting
    // has no other way to happen. This is how Home works:
    // `enables_rest: Some((radius: 15))`, which covers the whole base.
    enables_rest: Some((radius: 15)),
```

- [ ] **Step 3: Fix the player-facing docs**

In `README.md`, make these five replacements.

Line 99, the `r` keybind row:

```markdown
| `r` | Recharge overnight (restores Fatigue and Integrity, costs Power) — requires standing within your base, near Home (see [Structures](#structures)) |
```

Line 251, the Power row of the Stats table:

```markdown
| **Power** | Your hunger-equivalent. Drains over time; hits 0 and you start taking Integrity damage each tick. Below 50%, your Attack also starts weakening — a linear falloff to half strength at 0 Power, on top of (not instead of) the tick damage. Restored by draining a Power Cell (`e`), standing near a cooking Terminal, or passively anywhere in a base with a Recharger Node. |
```

Line 526, the Home row of the structures table:

```markdown
| Home | 5 Core Fragments | `u` ("use symlink") instantly teleports you to it from anywhere on the map, for 4 Power Cells; also lets you `r` (recharge/rest) anywhere in the base |
```

Line 530, the Recharger Node row:

```markdown
| Recharger Node | 10 Core Fragments | Passively refills your Power anywhere within 15 tiles — the whole base |
```

Lines 556-557, the structure-category prose:

```markdown
Recharger Node is a **passive power source** — a fourth category: it
refills your Power every tick you're inside its 15-tile radius, with no
worker and no input item. Home doubles as the **rest gate**: `r` only
works within 15 tiles of it, which is exactly the base footprint.
```

- [ ] **Step 4: Add the changelog entry**

In `CHANGELOG.md`, insert a new section directly after line 3 (`Release notes for [feral-processes](README.md).`) and before `## 2026-07-21`:

```markdown
## 2026-07-22

- **The Recharger Node actually recharges you now**: instead of gating
  rest, it passively restores 1 Power per tick anywhere within 15 tiles —
  the whole base footprint — with no assigned worker and no input item.
  Being home means never watching your reserves drain. Its cost rises from
  5 to 10 Core Fragments to match. Power Cells and the Terminal are now
  expedition gear rather than daily upkeep.
- **Resting moved to Home**: `r` (recharge overnight) now works anywhere
  within 15 tiles of Home rather than within 2 tiles of a Recharger Node,
  so the base you already built is the place you rest. Existing saves need
  no migration and nobody is locked out — Home has always had to be built
  before anything else. A Recharger Node deployed under the old rules
  simply stops gating rest and starts regenerating Power, at the price you
  already paid — see [Structures](README.md#structures).
```

- [ ] **Step 5: Verify docs and suite**

Run: `grep -n "Recharger Node" README.md assets/structures/README.md CHANGELOG.md`

Expected: no remaining claim that the Recharger Node is required for resting, or that it costs 5 Core Fragments. Lines 389, 392 and 589-591 of `README.md` describe what resting *does* (healing tamed programs) rather than where it happens — leave them alone.

Run: `cargo test --workspace`

Expected: all tests pass.

- [ ] **Step 6: Do not commit**

---

## Final verification

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --workspace` — no warnings
- [ ] `cargo test --workspace` — all tests pass
- [ ] `git status` shows exactly: 2 `.ron` files, 3 engine `.rs` files, 3 docs, plus the spec and this plan. No stray scratch files.
- [ ] Report to the user what was run and what the output actually was.
