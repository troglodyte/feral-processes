# Arena Rolled Encounters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let an arena scenario name a *context* — zone N field, or zone N Stack at depth D, on a chosen biome — and fight whatever the game's own spawn machinery rolls there, instead of an authored species-and-count list.

**Architecture:** A new `Scenario::encounter: Option<Encounter>`, mutually exclusive with `opponents`. `arena::stage` branches once: authored compositions keep going through `setup::build_opponents` uncapped, a rolled one goes through a new `arena::encounter::roll` which stamps the biome onto the player's tile, descends for real when a depth is named, calls the game's own spawn functions, and caps the result with `Game::group_pack`. Everything downstream of the groups — `begin_battle`, `Watch`, the transcript, the outcome — is unchanged and shared.

**Tech Stack:** Rust, `bevy_ecs` (engine only), `ron` for the scenario file, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-08-arena-rolled-encounters-design.md`. Read it before Task 1 — it carries the reasoning this plan only names.

## Global Constraints

- **Read `CLAUDE.md` first.** Its "Load-bearing seams" entries on `enter_frame`, `spawn_pack`, `Position` underground, and `start_battle`/`begin_battle` are all directly in the path of this change.
- **TDD.** Failing test first, minimal implementation, green, commit. Every task ends green.
- **No formula copies.** Where the spec says "call `X`", call it. A doc comment claiming to mirror another module's rule must be a call, not a copy.
- **The renderer never touches the ECS `World`.** All spawn logic lives in `crates/engine/src/arena/`; app-core and gui see only `Scenario`, `Encounter`, `RepRecord` and `App` accessors.
- **`#[serde(default)]` on every new scenario field**, so existing `dev-arenas/*.ron` keep parsing untouched.
- **Comments explain why, never what.**
- **Do not bump the version or write a `CHANGELOG.md` section on the branch.** Per `CLAUDE.md`'s release policy the bump, changelog section and tag happen once, at the merge.
- **Gates before calling any task done:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`. Iterate with `cargo test -p feral-processes-engine <name>` — the engine suite is ~3s.
- `balance_sim` is **not** a gate here; no tuning constant moves.

## File map

| File | Responsibility |
|---|---|
| `crates/engine/src/arena/scenario.rs` | `Encounter` enum, `Scenario::encounter`, the mutual-exclusion rule |
| `crates/engine/src/arena/encounter.rs` | **new** — stamp the biome, descend if asked, roll, group |
| `crates/engine/src/arena/mod.rs` | `stage` branches; seed install moves; `pub use Encounter` |
| `crates/engine/src/arena/report.rs` | `RepRecord::composition` |
| `crates/engine/src/game/stack.rs` | `descend_to` widens to `pub(crate)` |
| `crates/app-core/src/app/arena.rs` | encounter rows, the cycle, the biome picker |
| `crates/gui/src/render/arena.rs` | one composition line on the result screen |
| `crates/launcher/src/bin/arena.rs` | print the composition at `reps: 1` |
| `dev-arenas/README.md` | the schema and the three stated consequences |

---

### Task 1: The scenario field

**Files:**
- Modify: `crates/engine/src/arena/scenario.rs`
- Modify: `crates/engine/src/arena/mod.rs` (add `Encounter` to the `pub use` from `scenario`)
- Test: the existing `mod tests` at the bottom of `scenario.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum Encounter {
      Field { biome: Biome },
      Stack { biome: Biome, depth: u32 },
  }
  // on Scenario, #[serde(default)]:
  pub encounter: Option<Encounter>,
  ```
  `Biome` is `crate::world::Biome`; it already derives `Serialize`/`Deserialize`.
  Derive `Clone, Debug, PartialEq, Serialize, Deserialize` on `Encounter`, matching every other spec type in the file. `depth` takes no serde default — a Stack encounter with no depth is a typo, the same argument the file's header makes about an unnamed species id.

- [ ] **Step 1: Write the failing tests.** Four, in `scenario.rs`'s `mod tests`, named in this file's existing style:
  - `a_rolled_field_encounter_parses` — a scenario with `encounter: Some(Field(biome: Mainframe))` and no `opponents` loads, and `encounter` reads back as that value.
  - `a_rolled_stack_encounter_carries_its_depth` — `Some(Stack(biome: OpenGrid, depth: 5))` round-trips through `save`/`load` (follow `a_written_scenario_reads_back_as_itself`, which uses `std::env::temp_dir()` and removes the file).
  - `an_encounter_beside_opponents_is_an_err_naming_both` — a scenario with both populated is `Err`, and the message contains `encounter` and `opponents`.
  - `a_scenario_without_an_encounter_still_needs_opponents` — the existing empty-`opponents` error still fires when `encounter` is `None`. (Guards against relaxing `validate` too far.)

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine arena::scenario`. Expected: compile error on the unknown field, which is the failure.

- [ ] **Step 3: Implement.** Add the enum and the field; add the field to the hand-written `Default` impl (`None`). In `validate`, add the mutual-exclusion check and make the existing empty-`opponents` error conditional on `self.encounter.is_none()`. Leave the `Fresh`-only loadout rule exactly as it is — a rolled encounter says nothing about the player.

- [ ] **Step 4: Green.** `cargo test -p feral-processes-engine arena`, then `cargo test --workspace` (the three shipped `dev-arenas/*.ron` must still parse; `crates/app-core` and the launcher construct `Scenario` with `..Scenario::default()` and should be unaffected).

- [ ] **Step 5: Commit.** `feat(arena): a scenario may name a rolled encounter`

---

### Task 2: Seed the composition, not just the fight

**Files:**
- Modify: `crates/engine/src/arena/mod.rs` (`stage`)
- Test: the existing `mod tests` in `arena/mod.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `stage` installs `GameRng(StdRng::seed_from_u64(seed))` immediately after `setup::build_player`, before any opponent is spawned. Task 3 depends on this ordering — a roll before the seed is installed would draw from `Game::new(0)`'s stream and be identical at every seed.

This is a deliberate behaviour change and the spec's "The seeding change this forces" section is the argument. Today authored opponents are spawned before the seed lands, so their potential rolls are the same in every rep.

- [ ] **Step 1: Write the failing test.** `the_seed_varies_the_opponents_it_spawns` in `arena/mod.rs`'s `mod tests`: stage the same authored scenario at two different seeds and assert the opponents' total HP differs. Use a species with a wide potential band and a count of about 6 so the collision probability is negligible; read the HP by summing `Stats::hp` over `staged.watch`'s opponents — if `Watch` exposes no accessor, query `With<Hostile>` on `staged.game.world` (arena tests already reach into `world` freely).

- [ ] **Step 2: Run it and watch it fail.** `cargo test -p feral-processes-engine arena::tests::the_seed_varies` — expected: equal totals.

- [ ] **Step 3: Implement.** Move the `insert_resource(GameRng(..))` call above `setup::build_opponents`. Update its comment: it now says the seed covers the composition *and* the fight, and why.

- [ ] **Step 4: Green.** `cargo test -p feral-processes-engine arena`, then `cargo test --workspace`. **Expect other arena tests to move.** Any test asserting a specific outcome, round count or transcript content at a fixed seed may now report a different fight. Re-baseline them; do **not** loosen an assertion into vacuity to make it pass — if a test asserted "the companion swung", it must still assert that.

- [ ] **Step 5: Commit.** `fix(arena): seed the opponents as well as the fight` — and say in the body that pinned loss seeds from older reports no longer replay.

---

### Task 3: Rolling an encounter

**Files:**
- Create: `crates/engine/src/arena/encounter.rs`
- Modify: `crates/engine/src/arena/mod.rs` (`mod encounter;`, and `stage` branches)
- Modify: `crates/engine/src/game/stack.rs` (`descend_to` → `pub(crate)`)
- Test: a `mod tests` in `encounter.rs`, plus one addition to `arena/mod.rs`'s tests

**Interfaces:**
- Consumes: `Encounter` (Task 1); the seed ordering (Task 2).
- Produces:
  ```rust
  pub(crate) fn roll(game: &mut Game, encounter: &Encounter)
      -> Result<Vec<crate::battle::EnemyGroup>, String>
  ```

**What it does, in order.** Every one of these is an existing function; none of their rules are to be reimplemented here.

1. Read the player's `Position`. Stamp the tile: `WorldMap::set_override(x, y, Tile { biome, walkable: biome.walkable() })`.
2. *Field* — `pick_habitat_species(x, y, true)`, then `spawn_pack(&species, is_boss, x, y, SpawnEscalation::surface())`. These are the two halves of `try_spawn_habitat_creature` minus its nest branch; the doc comment must say **why** the nest roll is skipped (a nest is not a fight, and a roll that placed one would leave the arena with nobody to fight).
   *Stack* — `let frames = game.frames_at((x, y)).max(depth);` then `game.descend_to(depth, frames, (x, y));` then `game.stack_encounter_pack()`.
3. An empty pack is `Err` naming the biome and saying nothing lives there — never an empty battle.
4. Return `game.group_pack(pack)`. Document the asymmetry: a rolled pack is the game's own fight so it takes the game's own capping, where `build_opponents` deliberately leaves an authored composition uncapped.

**The `descend_to` widening.** It is `fn descend_to` in `game/stack.rs`; make it `pub(crate)` and leave its body alone. Add a line to its doc naming the arena as the second caller and stating that this is still the one way into a frame, via `enter_frame`. Do **not** add a new descent path.

`stage` branches once:

```rust
let (groups, warnings) = match &scenario.encounter {
    Some(encounter) => (encounter::roll(&mut game, encounter)?, Vec::new()),
    None => setup::build_opponents(&mut game, &scenario.opponents)?,
};
```

A rolled encounter warns about nothing — nothing was asked for past a ceiling, because nothing was asked for. Say that in a comment where the empty `Vec` is built.

- [ ] **Step 1: Write the failing tests** in `encounter.rs`'s `mod tests`. Build the game with `setup::build_player` on a `Fresh` scenario and `test_assets_dir()`, as `setup.rs`'s own tests do. Intent of each:
  - `a_field_roll_fields_only_that_biomes_residents` — roll `Field { biome }` for a biome with a known non-empty pool, and assert every spawned member's species lists that biome in `SpeciesDef::habitats`. Asserted over the real assets, so it stays honest when the roster changes.
  - `a_stack_roll_leaves_the_party_underground_at_the_depth_asked_for` — after `roll`, `game.stack_pos()` is `Some` with `depth` equal to the ask, and every member carries `StackSpawn`.
  - `a_deeper_stack_roll_hits_harder` — the determinism argument matters, so build it exactly this way: two games, **same seed, same biome**, depths 1 and 5. Same seed means the same RNG draws, so the species picked and the group size rolled are identical; the only difference is `SpawnEscalation::stat_mult`. Assert the depth-5 pack's total `Stats::hp` is strictly greater. If the two packs come back with different species or different lengths, the test is wrong rather than the code — say so in an assertion message.
  - `a_rolled_pack_is_capped_by_the_zones_own_ceilings` — no group returned exceeds `game.group_size_ceiling()`, and there are no more groups than `game.enemy_group_ceiling()`.
  - `zone_one_field_fields_only_the_opening_ring` — at zone 1, every species rolled satisfies `balance_sim::beatable_by_a_fresh_player`. This pins the spec's first stated consequence rather than leaving it to be rediscovered.
  - `a_biome_nothing_lives_in_is_an_err_naming_it` — roll on `Biome::Platform` (no shipped species lists it) and assert the error names the biome.
  - In `arena/mod.rs`'s tests: `staging_a_rolled_encounter_opens_a_fight_with_no_warnings` — `stage` on a rolled scenario leaves `has_active_battle()` true, `watch.rounds() == 0`, and `warnings` empty.
  - In `arena/mod.rs`'s tests: `staging_then_running_a_rolled_encounter_matches_at_the_same_seed` — the parity property the existing `staging_then_running_matches_run_at_the_same_seed` asserts for authored scenarios, now for a rolled one. It is what stops the played half and the measured half rolling different packs, which is exactly the divergence the `stage`/`run_rep` split exists to prevent. Follow that test: `run` a one-rep scenario and compare `report.reps[0]` to `test_fight(&s, seed)`.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine arena::encounter`.

- [ ] **Step 3: Implement** `encounter.rs`, the `descend_to` widening, and the `stage` branch.

- [ ] **Step 4: Green.** `cargo test -p feral-processes-engine arena`, then `cargo test --workspace`.

- [ ] **Step 5: Commit.** `feat(arena): roll the encounter a zone and depth would field`

---

### Task 4: Report what was fought

**Files:**
- Modify: `crates/engine/src/arena/report.rs` (`RepRecord`)
- Modify: `crates/engine/src/arena/watch.rs` and/or `mod.rs` — wherever the groups are in scope at `Watch::new`
- Modify: `crates/launcher/src/bin/arena.rs` (`print_result`)
- Modify: `crates/gui/src/render/arena.rs` (`draw_arena_result`)
- Modify: `crates/gui/src/render/mod.rs` if `draw_arena_result`'s signature changes
- Test: `report.rs` or `watch.rs` tests; `render/arena.rs`'s existing `mod tests`

**Interfaces:**
- Consumes: Task 3's groups.
- Produces: `RepRecord.composition: Vec<(String, u32)>` — one entry per `EnemyGroup`, in formation order, `(species id, member count)`. Read off the groups at staging time, before the fight, because `BattleState` is gone by the time the record is built (the same argument the existing `opponents: Vec<Entity>` capture makes).

Every existing `RepRecord` literal in tests and fixtures needs the new field; `crates/gui/src/render/arena.rs`'s `record()` fixture is one of them.

- [ ] **Step 1: Write the failing tests.**
  - Engine: `a_rep_records_what_it_fought` — run a one-rep authored scenario of two groups and assert `composition` equals the species and counts asked for, in order.
  - Gui: extend `render/arena.rs`'s result-screen tests with `the_result_screen_names_the_composition` — the drawn rows contain the species id and count. Follow the existing test that checks the warnings line; `paint::with_painter` measures real text headlessly.

- [ ] **Step 2: Run them and watch them fail.**

- [ ] **Step 3: Implement.** Thread the composition from the staged groups into `Watch` (or into `RepRecord` at `finish`, whichever keeps `Watch::new`'s existing shape). In the bin, print it in the `reps: 1` branch above the transcript; leave the aggregate summary alone. On the result screen, draw one line under the seed line.

- [ ] **Step 4: Green.** `cargo test --workspace`.

- [ ] **Step 5: Commit.** `feat(arena): a rep records the composition it fought`

---

### Task 5: The builder screen

**Files:**
- Modify: `crates/app-core/src/app/arena.rs`
- Test: `crates/app-core/src/tests/arena.rs`

**Interfaces:**
- Consumes: `Encounter` (Task 1), re-exported from `feral_processes_engine::arena`; `feral_processes_engine::world::Biome`.
- Produces: new `ArenaRowKind` variants `Encounter`, `EncounterBiome`, `EncounterDepth`; a new `ArenaPickKind::EncounterBiome`; `ArenaCatalog` gains `biomes: Vec<Biome>`.

**The catalogue's biome list is data-driven.** Build it in `ArenaCatalog::load` from the already-loaded `SpeciesDb`: every biome appearing in any `SpeciesDef::habitats`, filtered by `Biome::walkable()`, deduplicated, sorted by `{:?}` for a stable order. That is exactly the pair of conditions `Game::habitat_pools` early-returns on, so the picker cannot offer a biome the roll would refuse. Do not hardcode a list of variants — a mod that adds the first `StaticField` resident should get it offered for free, and `Platform` should stay absent because no species lives on a base slab.

**Rows.** `arena_builder_rows` gains, immediately above the `Against:` block:
- `Encounter: Authored` / `Field` / `Stack` — `ArenaRowKind::Encounter`
- when not `Authored`: `  Biome: <id>` — `ArenaRowKind::EncounterBiome`
- when `Stack`: `  Depth: N` — `ArenaRowKind::EncounterDepth`

and the `Against:` rows plus `+ add an opponent group` are emitted **only** when `encounter.is_none()`. This is the same dynamic-hiding rule the `Fresh`-only loadout rows already follow, and for the same reason: `validate` refuses a file holding both.

**The cycle** (`adjust_arena_row`, `ArenaRowKind::Encounter`): `Authored → Field → Stack → Authored`, using the same wrapping `rem_euclid` shape as `cycle_arena_player_source`. Entering a rolled state clears `scenario.opponents`; returning to `Authored` with an empty list pushes one `OpponentSpec` — the catalogue's first species at a count of 1, the same thing the picker would append. Both directions exist so every state the cycle can reach is one `save` will accept. A new rolled encounter starts at the catalogue's first biome and, for `Stack`, depth 1.

`EncounterDepth` steps with `step(depth, delta, 1, u32::MAX)`. `EncounterBiome` opens the picker on Enter, via `open_arena_picker`.

- [ ] **Step 1: Write the failing tests** in `crates/app-core/src/tests/arena.rs`, following its existing style:
  - `cycling_to_a_rolled_encounter_hides_the_opponent_rows` — after cycling, no row label starts with `Against:` and none is `AddOpponent`.
  - `cycling_back_to_authored_restores_an_opponent_row` — the scenario has exactly one opponent again, and `scenario.save` would accept it (assert `opponents` is non-empty and `encounter` is `None`).
  - `a_stack_encounter_shows_a_depth_row_and_a_field_one_does_not`.
  - `the_biome_picker_offers_only_biomes_something_lives_in` — the pick rows for `EncounterBiome` are non-empty, contain a known biome, and contain neither `Platform` nor an unwalkable biome.
  - `picking_a_biome_replaces_the_encounters_biome_and_keeps_its_depth` — the depth beside it is the tuning dial and must survive an id change, the rule the existing pick handler already follows for counts and levels.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-app-core arena`.

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Green.** `cargo test --workspace`. `crates/gui` needs no change here — it draws `arena_builder_rows`' labels and `arena_pick_rows`' rows generically.

- [ ] **Step 5: Commit.** `feat(arena): pick a zone context on the builder screen`

---

### Task 6: Documentation

**Files:**
- Modify: `dev-arenas/README.md`

**Interfaces:** consumes Tasks 1–5; produces nothing code depends on.

- [ ] **Step 1: Add `encounter` to the schema table** — default `None`, meaning "which context to roll, instead of naming `opponents`".

- [ ] **Step 2: Add an `### encounter` section** after `### opponents`, covering: the two variants and their RON syntax; that it is mutually exclusive with `opponents`; that the zone comes from the player row and why; that `reps` now samples a distribution rather than repeating one composition; and the three stated consequences from the spec — zone 1 field is the opening ring, a field roll is one habitat spawn roll, and a biome nothing lives in cannot be picked.

- [ ] **Step 3: Update the "Playing one" section** with the new builder rows and the biome picker.

- [ ] **Step 4: Re-read the file end to end** for claims this change falsifies — in particular the `opponents` section's "Required, and must be non-empty" and its "There is no per-enemy level" paragraph, which now needs to say the rolled path gets its scaling from the zone and the depth.

- [ ] **Step 5: Commit.** `docs(arena): document rolled encounters`

---

## Finishing

- [ ] `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt` — all clean.
- [ ] Play it: `FERAL_DEV_ARENA=1 cargo run`, main menu `[R]`, build a `Stack(biome: Mainframe, depth: 5)` against a levelled `Fresh` player and fight it. A green suite is not evidence that a screen reads well, and this feature's whole point is the fight you get.
- [ ] Run a rolled scenario through the bin at `reps: 50` and read the composition lines — that the distribution looks like the zone is the only end-to-end check of the roll.
- [ ] Merge with the version bump, the `CHANGELOG.md` section and the annotated tag, per `CLAUDE.md`'s release policy. Which digit moves is decided by `CHANGELOG.md`'s preamble; no save format changes here.
