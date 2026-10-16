# Factory Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 17 one-recipe production structures with four configurable machine kinds (Extractor, Factory, Assembler, Printer) whose recipe the player sets per placed instance, plus a touch-to-use Workbench that replaces the four hand-compile benches.

**Architecture:** The recipe list lives on the machine's structure `.ron` (`machine: Some((role, recipes))`); a placed machine records its pick in a `MachineRecipe` component and `StructureSave::recipe`. Every "what does this structure make" reader goes through one view, `structures::Placed { def, recipe }`, so an unconfigured machine produces, draws, serves and staffs nothing by construction. The work is additive first (old structures keep working while the new ones are built and tests migrate), and the 17 old files are deleted last.

**Tech Stack:** Rust 2024, standalone `bevy_ecs` (engine), app-core state machine, bevy + bevy_egui (gui, through `Painter`), RON assets and saves.

**Spec:** `docs/superpowers/specs/2026-10-16-factory-overhaul-design.md`

**Crate tags:** each task heading says `[engine]`, `[app-core]`, `[gui]`, `[launcher]` or a mix, and a size `S`/`M`/`L`.

## Global Constraints

- Work only in `/home/trog/code/feral-processes-factory` (branch `factory-overhaul`). Never `git push`, `git checkout`, `git stash`, `git reset`; never `git add -A` — stage explicit paths. No `CHANGELOG.md`, root `README.md` or `docs/manual.md` edits; no version bump (release happens at deploy).
- Every commit builds and `cargo test --workspace` is green. Run with `FERAL_DEV_NO_SIEGES` **unset**. Don't pipe `cargo` (the exit code is lost); `cargo fmt` and `cargo clippy --workspace --all-targets` clean before each commit.
- New structure ids: `extractor`, `factory`, `assembler`, `printer`, `workbench`. All five buildable from the start (no research `unlocks_structures` names them).
- Power draw: Extractor 1, Factory 2, Assembler 3, Printer 2. Workbench 0 (it runs no job).
- Recipe table (`produces`, `ticks_per_unit`, `research`) — inputs are the item's own `craftable.cost`:

  | Machine (role) | produces | ticks | research | replaces |
  |---|---|---|---|---|
  | extractor (Extract) | core_fragment | 10 (`level: Some(1)`) | — | mining_node |
  | extractor | raw_trace | 10 (`level: Some(1)`) | routine_fabrication | log_scraper |
  | extractor | cache_grain | 14 (`level: Some(1)`) | cache_coherence | cache_tap |
  | extractor | power_cell | 6 (`level: None`, `tierless: true`) | power_grid | power_conduit |
  | factory (Assemble) | blank_substrate | 12 | routine_fabrication | lathe |
  | factory | bytecode_block | 12 | — | refinery |
  | factory | annealed_core | 12 | program_refactoring | annealing_node |
  | factory | logic_wafer | 12 | routine_fabrication | transcriber |
  | factory | charge_coil | 12 | — | winding_node |
  | assembler (Assemble) | routine_disk | 20 | routine_fabrication | disk_press |
  | assembler | honeypot | 20 | deception | decoy_bench |
  | assembler | recompile_kernel | 20 | program_refactoring | refactor_bench |
  | assembler | hardened_shell | 30 | armor_bench | armory |
  | assembler | trace_sniffer | 30 | weapon_bench | fabricator |
  | printer (Assemble) | ice_breaker | 8 | automation | compiler |
  | printer | patch_routine | 20 | — | assembly_bay |

- Build costs (near the cheapest replaced): Extractor `[("core_fragment", 12)]`, Factory `[("core_fragment", 16)]`, Assembler `[("core_fragment", 18)]`, Printer `[("core_fragment", 16)]`, Workbench `[("core_fragment", 16)]`.
- Upgrades: Extractor `max_tier: 5, cost: [("core_fragment", 10), ("cache_grain", 1)]` (Mining Node's); Workbench `max_tier: 5, cost: [("core_fragment", 12), ("cache_grain", 1)]` (Compiler's). Factory, Assembler, Printer have none.
- Output capacity per kind: Extractor 20, Factory 20, Assembler 15, Printer 15. All four and the Workbench carry `room_tags: ["workshop"]`.
- Glyphs: Extractor `'X'` Brown, Factory `'B'` Orange, Assembler `'A'` Magenta, Printer `'P'` Cyan, Workbench `'W'` Green.
- Hand-compile gates moved to the Workbench (`requires_structure: Some("workbench")` + `research`): annealed_core, recompile_kernel, bounds_check, buffer_extension, inline_cache → `program_refactoring`; bastion_lattice, hardened_shell, null_weave, nullsteel_plate, phase_carapace, static_mesh → `armor_bench`; the 15 fabricator items (arc_lance, black_ice_pick, entropy_damper, kernel_key, logic_probe, oracle_core, plasma_router, recursion_blade, reinitialization_protocol, scatter_lance, shim_blade, siege_compiler, singularity_matrix, sync_governor, trace_sniffer) → `weapon_bench`; blank_substrate, routine_disk, logic_wafer → `routine_fabrication`; honeypot → `deception`; patch_routine, bytecode_block, charge_coil → no research. Every `ResearchRecipe::requires_structure` naming a removed structure (18 entries across 11 research files) → `"workbench"` (its node already gates it).
- `SAVE_FORMAT_VERSION` 37 → 38. No migration.
- Copy rules: no "tick" in any player-facing string; rows select with lowercase letters/digits, actions are uppercase; plain language.
- Never hardcode content in Rust: recipes, gates, costs and the extraction gate are `.ron` data; new schema fields are `#[serde(default)]`; a malformed recipe entry is skipped with a warning, never a panic; the matching `assets/*/README.md` changes in the same commit.
- `crates/gui/src/paint.rs` is the only file naming a graphics library; `render/` draws through `Painter`.

## Review Focus

1. **Reconfiguring a staffed machine whose worker is carrying its output to a Depot.** Expect: worker unposted, its load on the floor (picked up later), nothing destroyed — test in Task 3.
2. **Reconfiguring a full machine in a base with no Depot.** Expect: every held input and output unit lands in a floor pile beside the machine; totals conserved — test in Task 3.
3. **A save whose machine names a recipe its def no longer lists** (a mod removed it, or a hand-edited template). Expect: loads unconfigured with a warning, no panic — test in Task 2.
4. **A standing work order whose only machine is switched to another recipe.** Expect: the order stalls (stays queued, not removed) and resumes when a machine is set back — test in Task 4.
5. **An upgraded Extractor switched core_fragment → power_cell → core_fragment.** Expect: tier kept (Mk3 stays Mk3), power_cell pays the tier-1 amount plus zone scaling, core_fragment's reliability level is the tier again — test in Task 3.

---

## File map

| File | Responsibility after this plan |
|---|---|
| `crates/engine/src/structures.rs` | `RecipeDef` (renamed `WorkDef`), `MachineRole`, `MachineDef`, `Production`, `Placed`, recipe validation; `assembles`/`AssembleDef` gone |
| `crates/engine/src/components.rs` | `MachineRecipe`, `MachineStatus::Unconfigured` |
| `crates/engine/src/game/base/machine_recipe.rs` (new) | `Game::placed`, `adjacent_machines`, `machine_recipe_view`, `set_machine_recipe` |
| `crates/engine/src/systems.rs` | production readers take `Placed` |
| `crates/engine/src/game/crafting.rs` | touch rule, `CraftableDef::research` gate, touched-bench tier |
| `crates/engine/src/contracts.rs`, `game/contracts.rs` | `Objective::Configure` |
| `crates/app-core/src/app/machine_recipe.rs` (new) | `Mode::MachineRecipe`/`MachineRecipeConfirm` |
| `crates/gui/src/render/machine_recipe.rs` (new), `render/base.rs` | recipe screen, `?` overlay |
| `assets/structures/{extractor,factory,assembler,printer,workbench}.ron` (new) | the five structures |

---

### Task 1: Recipe schema and the one resolution door `[engine] M`

**Files:**
- Modify: `crates/engine/src/structures.rs` (types at 12-95, `StructureDef` 316-750, `runs_a_job`/`category`/`needs_program` 874-993, `structure_levels` 787-871, `cycle_ticks` 1132, tests module)
- Modify: `crates/engine/src/components.rs` (add `MachineRecipe` beside `ResourceNode` ~863; doc mention of `WorkDef` at 865)
- Modify: `crates/engine/src/game/base/mod.rs` (register `machine_recipe`), `crates/engine/src/game/lifecycle.rs:3871` (call validation)
- Modify: every `WorkDef` mention (`rg -n WorkDef crates`) — rename only

**Interfaces:**
- Produces (structures.rs):
  - `pub struct RecipeDef { pub produces: ItemId, pub ticks_per_unit: u32, #[serde(default)] pub level: Option<u32>, #[serde(default)] pub flat_payout: bool, #[serde(default)] pub research: Option<ResearchId>, #[serde(default)] pub tierless: bool }` — `WorkDef` renamed; field `produces` kept so `research_node.ron`'s `work:` block and modded ones still parse.
  - `pub enum MachineRole { Extract, Assemble }` (Serialize/Deserialize/Copy/Eq).
  - `pub struct MachineDef { pub role: MachineRole, pub recipes: Vec<RecipeDef> }`.
  - `StructureDef` gains `#[serde(default)] pub machine: Option<MachineDef>` and `#[serde(default)] pub extracts_research: Option<ResearchId>`; `work: Option<RecipeDef>` stays (the fixed-recipe Research Station).
  - `#[derive(Clone, Copy)] pub struct Production<'a> { pub recipe: &'a RecipeDef, pub extracts: bool }`.
  - `#[derive(Clone, Copy)] pub struct Placed<'a> { pub def: &'a StructureDef, pub recipe: Option<&'a ItemId> }` with `pub fn production(self) -> Option<Production<'a>>` (`work` wins and extracts; else a machine whose `recipe` is listed; `Assemble` → `extracts: false`) and `pub fn is_unconfigured(self) -> bool` (a machine def with no production).
  - `impl StructureDef { pub fn productions(&self) -> impl Iterator<Item = Production<'_>>; pub fn machine_recipe(&self, item: &ItemId) -> Option<&RecipeDef>; pub fn extracts(&self) -> bool }` — `productions` is the def-level "every recipe this kind could run" (work + every machine recipe); `extracts` is `work.is_some()` or role `Extract`.
  - `impl StructureDb { pub fn validate_machines(&mut self, items: &ItemDb) -> Vec<String> }` — drops (and warns per entry) a recipe whose item is unknown, an `Assemble` recipe whose item has no `craftable`, `ticks_per_unit == 0`, or a duplicate `produces` within one machine.
  - `pub(crate) fn cycle_ticks(placed: Placed<'_>) -> Option<u32>` (signature change; old callers pass `Placed { def, recipe: None }` until Task 4).
  - components.rs: `#[derive(Component, Clone, Debug, Default, PartialEq, Eq)] pub struct MachineRecipe(pub Option<ItemId>);` exported from `lib.rs` beside `ResourceNode`.
- `runs_a_job()` also true for `machine.is_some()`; `category()` maps role `Extract` → `StructureCategory::Extractor`, `Assemble` → `StructureCategory::Assembler` (checked right after the existing `work`/`assembles` arms); `structure_levels` builds its `producers` map and `consumed_items` from `productions()` (def-level on purpose: a build site has no recipe yet).

- [ ] **Step 1: Write the failing tests** in `structures.rs`'s test module, parsing RON literals:
  - `a_machine_def_parses_with_role_and_recipes` — `machine: Some((role: Extract, recipes: [(produces: "ore", ticks_per_unit: 10, level: Some(1))]))` parses; `research` is `None`, `tierless` false.
  - `placed_production_reads_the_instance_recipe` — for a two-recipe Assemble machine, `Placed { def, recipe: Some(&"b") }.production().unwrap().recipe.ticks_per_unit` is b's; `recipe: None` → `None` and `is_unconfigured()`; a recipe not in the list → `None`.
  - `a_fixed_work_block_wins_over_any_recipe` — a def with `work` gives `extracts: true` whatever `recipe` is.
  - `a_machine_runs_a_job_and_groups_by_role` — `runs_a_job()`, `needs_program()` true; Extract → `StructureCategory::Extractor`, Assemble → `Assembler`.
  - `validation_skips_a_bad_recipe_and_keeps_the_rest` — an unknown item, a non-craftable Assemble item and a zero cycle each yield one warning and leave the valid sibling.
  - `structure_levels_count_every_recipe_a_machine_lists` — an extractor kind (ore) feeding a factory kind (ore → plate) gets levels 0 and 1.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine structures::` — Expected: FAIL (types missing).
- [ ] **Step 3: Implement** the types, `Placed::production`, `productions`, `validate_machines` (called in `lifecycle.rs` right after `structures.compute_levels(&items)`, its warnings extended into `warnings`), the `runs_a_job`/`category` arms, `structure_levels` over `productions()`, the `cycle_ticks` signature, `MachineRecipe`. Rename `WorkDef` → `RecipeDef` everywhere.
- [ ] **Step 4: Update `assets/structures/README.md`** — document `machine`, `role`, `recipes` entry fields (`produces`, `ticks_per_unit`, `level`, `flat_payout`, `research`, `tierless`), `extracts_research`, and that a bad entry is skipped with a warning.
- [ ] **Step 5: Run** `cargo test -p feral-processes-engine structures::` then `cargo test --workspace` — Expected: PASS.
- [ ] **Step 6: Commit** `structures.rs components.rs lib.rs game/lifecycle.rs game/base/mod.rs assets/structures/README.md` + renamed-mention files: "Structures: a machine def lists its recipes, and Placed is the one door to what a placed one makes".

### Task 2: The four machine files, a placed machine's lifecycle, and the save `[engine] M`

**Files:**
- Create: `assets/structures/extractor.ron`, `factory.ron`, `assembler.ron`, `printer.ron` (values from Global Constraints; descriptions say the role, e.g. "Pulls a raw resource out of nothing. Set what it pulls beside it with R.")
- Modify: `crates/engine/src/game/base/building.rs:852-934` (`spawn_structure`), `crates/engine/src/game/lifecycle.rs:1196-1265` (load) and `:2947` (save)
- Modify: `crates/engine/src/save.rs:1413` (`StructureSave`), `:2117` (`SAVE_FORMAT_VERSION` + its doc entry)
- Modify: `crates/engine/src/components.rs:1014` (`MachineStatus`), `crates/engine/src/systems.rs:888` (`set_machine_status` text), `:1254` (`idle_machine_system`)
- Modify: version pins at `tests/outposts.rs:155`, `tests/research.rs:1880`, `tests/caravans.rs:1023` and `:1709` (37 → 38; `rg -n 'SAVE_FORMAT_VERSION,' -A1 crates | rg 37` finds them all)
- Modify: `crates/gui/src/render/marks.rs:706` (`machine_color` arm, `palette::FAINT`)
- Test: `crates/engine/src/tests/machines.rs` (new, registered in `tests/mod.rs`), `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: Task 1's `MachineDef`, `Placed`, `MachineRecipe`.
- Produces:
  - `StructureSave { …, #[serde(default)] pub recipe: Option<ItemId> }`.
  - `MachineStatus::Unconfigured` appended last; `as_str()` → `"unconfigured"`; log/alert text `"The {name} has nothing set to make — stand beside it and press R."`.
  - `spawn_structure` inserts `MachineRecipe::default()` on every `def.machine.is_some()`; load inserts `MachineRecipe(s.recipe)` when the def lists that recipe, else `MachineRecipe(None)` and pushes the warning `"a {kind} at {x},{y} was set to {item}, which it no longer makes — it loads with nothing set"` into `Game::load`'s `load_warnings` (`lifecycle.rs` ~1495, logged at ~1794; the structure-restoring helper returns its warnings alongside `structure_positions`).
  - Load inserts `ResourceNode { resource, level }` from `Placed { def, recipe }.production()` when it `extracts` (replacing the `def.work` read); the existing tier re-derive below it stays.
  - `idle_machine_system` writes `Unconfigured` for a machine with no production, before the worker check.

- [ ] **Step 1: Write failing tests** in `tests/machines.rs`:
  - `a_placed_machine_starts_with_nothing_set` — `place_now(g, "factory", 1, 0)`; the entity carries `MachineRecipe(None)`, no `ResourceNode`, and after one tick `MachineStatus::Unconfigured`.
  - `a_machines_recipe_survives_save_and_load` — set `MachineRecipe(Some(bytecode_block))` directly on a placed factory (the setter lands in Task 3), `Game::save` → `Game::load`; the reloaded factory at that tile carries it. Save → load through the file, not a RON round trip.
  - `a_saved_recipe_the_def_no_longer_lists_loads_unconfigured` (Review Focus 3) — write a save, edit its text to `recipe: Some("honeypot")` on the factory, load: `MachineRecipe(None)`, no panic.
  - `a_reloaded_extractor_rebuilds_its_node_from_the_saved_recipe` — extractor set to `core_fragment` at tier 3 reloads with `ResourceNode { resource: core_fragment, level: Some(3) }`.
  In `tests/assets.rs`:
  - `the_four_machines_carry_the_recipe_table` — for every row of the Global Constraints table, `db.get(kind).machine.recipes` has that `produces` with that `ticks_per_unit`, `research`, `level`, `tierless`; and the machine has exactly those recipes.
  - `every_machine_recipe_research_names_a_shipped_node` and `every_assemble_role_recipe_is_a_single_ingredient`.
  - `the_machines_cost_and_draw_what_the_spec_says` — build cost, power draw, capacity, upgrade per Global Constraints.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine machines` and `… assets::the_four` — Expected: FAIL.
- [ ] **Step 3: Implement** the four `.ron` files, the component insertion in `spawn_structure` and the loader (both lists — `seam`: they are hand-written copies of each other), `StructureSave::recipe` read/write, the version bump with its doc entry ("37 → 38: configurable machines. `StructureSave::recipe` is written and 17 structure kinds are gone; a v37 base would load with its production structures missing."), the `Unconfigured` status, its log text, `machine_color` arm, and every exhaustive match the new variant breaks.
- [ ] **Step 4: Run** the new tests, then `cargo test --workspace` — Expected: PASS.
- [ ] **Step 5: Commit**: "Machines: Extractor, Factory, Assembler and Printer stand with nothing set, and a save remembers what each makes (save v38)".

### Task 3: Setting a recipe — the Game API `[engine] M`

**Files:**
- Create: `crates/engine/src/game/base/machine_recipe.rs`
- Modify: `crates/engine/src/views.rs` (new view types near `RigToolView` ~4100), `crates/engine/src/tuning.rs` (pace bands), `crates/engine/src/game/base/depot_filter.rs` (auto-filter helper beside `auto_filter_depots` ~149)
- Modify: `crates/engine/src/tests/support.rs` (helpers below)
- Test: `crates/engine/src/tests/machines.rs`

**Interfaces:**
- Consumes: `Placed`, `MachineRecipe`, `stock::return_to_depots(game, item, qty) -> u32`, `floor::spawn_floor_pile(world, pos, item, qty)`, `floor::drop_load(world, who)`, `collect::ORTHOGONAL`, `Game::is_researched`, `unlock_research_chain` (tests).
- Produces:
  - `impl Game { pub(crate) fn placed(&self, structure: Entity) -> Option<Placed<'_>>; pub fn adjacent_machines(&self) -> Vec<Entity>; pub fn machine_recipe_view(&self, machine: Entity) -> Option<views::MachineRecipeView>; pub fn set_machine_recipe(&mut self, machine: Entity, item: &ItemId) -> Result<(), String> }`. `adjacent_machines` is `adjacent_teardown_rigs`' shape (orthogonal to `base_pos`, sorted by tile) filtered on `def.machine.is_some()`.
  - `views::MachineRecipeView { pub tile: (i32, i32), pub name: String, pub current: Option<ItemId>, pub held: u32, pub rows: Vec<MachineRecipeRow> }` — `held` is the units a change would empty (input + output).
  - `views::MachineRecipeRow { pub item: ItemId, pub name: String, pub inputs: Vec<(String, u32)>, pub pace: RecipePace, pub current: bool }` — researched recipes only, in the def's authored order; inputs empty for an Extract recipe.
  - `views::RecipePace { Quick, Steady, Slow, VerySlow }` with `pub fn label(self) -> &'static str` ("quick", "steady", "slow", "very slow"), banded on `ticks_per_unit` by `tuning::RECIPE_PACE_QUICK_MAX = 8`, `RECIPE_PACE_STEADY_MAX = 14`, `RECIPE_PACE_SLOW_MAX = 20` (documented `pub const`s; >20 is `VerySlow`).
  - `set_machine_recipe` — guards as `install_rig_tool` (game over, battle, `require_base`); refusals, worded once: not a machine → `"That structure has nothing to set."`; not listed → `"A {Kind} can't make {Item}."`; unresearched → `"{Item} needs {Research name} researched first."`; already set → `"It is already making {Item}."`. On success, in order: empty `Stock` input and output through `stock::return_to_depots`, the remainder via `floor::spawn_floor_pile` on the first walkable orthogonal neighbour (machine tile if none); for the posted worker (`systems::worker_of`) `floor::drop_load` then remove its `Task` (the scheduler re-posts it and bakes the new pace — no re-bake here); write `MachineRecipe(Some(item))`; insert `ResourceNode { resource: item, level: recipe.level.map(|_| tier) }` for an Extract role (tier = `StructureTier` or 1), remove it for Assemble; if the previous recipe was `None`, call `Game::auto_filter_beside_machine(machine)`; log `"The {Kind} is set to make {Item}."` (append `" What it held goes to the Depots."` / `" What it held is left on the floor beside it."` when it held anything).
  - `pub(crate) fn auto_filter_beside_machine(&mut self, machine: Entity)` in `depot_filter.rs` — each orthogonal Depot **with no `DepotFilter`** becomes allow-only for the machine's buffered output; a Depot with a filter is untouched. `auto_filter_depots` (Depot placement) stays and now reads instance outputs once Task 4 lands.
  - tests/support.rs: `pub(super) fn place_machine(game: &mut Game, kind: &str, recipe: &str, dx: i32, dy: i32) -> Result<Entity, String>` (`place_now` + `unlock_research_chain` for the recipe's gate + `set_machine_recipe`); `pub(super) fn spawn_machine_at(game: &mut Game, kind: &str, recipe: &str, x: i32, y: i32) -> Entity` (`Game::spawn_structure(def, x, y, None)` + the same).

- [ ] **Step 1: Write failing tests** in `tests/machines.rs`:
  - `the_menu_lists_only_researched_recipes` — a fresh extractor's view lists core_fragment only; after `unlock_research_chain(g, "power_grid")` it also lists power_cell; locked rows never appear.
  - `a_recipe_pace_is_a_band_never_a_number` — power_cell `Quick`, core_fragment `Steady`, routine_disk `Slow`, hardened_shell `VerySlow`.
  - `setting_an_unresearched_recipe_is_refused` / `setting_an_unlisted_recipe_is_refused` / `setting_the_same_recipe_again_is_refused` — exact sentences above; component unchanged.
  - `changing_a_recipe_empties_the_machine_into_a_depot` — factory holding 3 input core_fragment + 2 output bytecode_block beside a Depot; set to charge_coil: buffers empty, Depot holds all 5 units, progress of the posted worker's task gone.
  - `changing_a_recipe_with_no_depot_leaves_the_units_beside_it` (Review Focus 2) — same with no Depot: a `FloorPile` on an orthogonal neighbour holds all 5.
  - `changing_a_recipe_unposts_its_worker_and_loses_nothing` (Review Focus 1) — worker carrying 2 bytecode_block: worker has no `Task`, a floor pile holds the 2, base total unchanged.
  - `an_extractor_keeps_its_tier_across_recipes` (Review Focus 5) — Mk3 extractor: core_fragment → `ResourceNode.level == Some(3)`; power_cell → `level == None`; back → `Some(3)`; `StructureTier(3)` throughout.
  - `the_first_recipe_filters_only_unfiltered_depots_beside_it` — Depot A (no filter) and Depot B (denies one item) beside an unset factory; set bytecode_block: A accepts only bytecode_block; B's denied set unchanged. Then switch to charge_coil: A unchanged (it now has a filter).
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine machines` — Expected: FAIL.
- [ ] **Step 3: Implement** the module, views, tuning constants, auto-filter helper and the two support helpers.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine machines`, then `cargo test --workspace` — Expected: PASS.
- [ ] **Step 5: Commit**: "Machines: the player sets a recipe, and a change empties the machine and unposts its worker".

### Task 4: Every production reader goes per instance; an unset machine does nothing `[engine] L`

**Files (each reader verified by `rg`; line numbers approximate):**
- `crates/engine/src/systems.rs` — `node_is_flat_payout` 528, `assembly_recipe` 764, `intake_recipe` 792, `produced_item` 811, `feeds_ingredient`/`feeds_fuel`/`feeds` 833-860, `task_progress_system` 1445 (flat payout and `tierless` → pass `tier: None`), `assembler_system` 2137 (query gains `Option<&MachineRecipe>`; product from `production()`), `player_gather_system` 1839 (tierless)
- `crates/engine/src/game/base/building.rs` — `work_ticks_for` 1679, `cycle_ticks_for` 1733, `accepts_a_program` 1768 (a machine also needs a production), `build_quality` 1987 (`def.extracts()`)
- `crates/engine/src/game/party.rs:916` (`def.extracts()`; a machine's build preview is `BuildEffect::NoCycle`)
- `crates/engine/src/game/base/work_orders.rs` — `producers_of` 177, `makeable_by` 196, `no_producer` 205, `can_progress` 473, `walk_feeders`/`break_at` 370-640, `machine_shortfall` 3204
- `crates/engine/src/game/base/lines.rs:164,348,355`; `hauling.rs:1162,1171,1240`; `depot_filter.rs:37,198,205-225`; `stock.rs:62` (`producible`); `power.rs:106` (`grid_rung`) and `ledger` (draw 0 for an unconfigured machine)
- Def-level readers switched to `productions()`: `game/catalog.rs:156,172,246`, `game/crafting.rs:268-280,480`, `game/contracts.rs:711`, `game/unlocks.rs:592`
- `crates/engine/src/game/lifecycle.rs:1230` already done in Task 2
- Test: `crates/engine/src/tests/machines.rs`

**Interfaces:**
- Consumes: `Placed`, `Game::placed`, Task 3's `place_machine`/`spawn_machine_at`.
- Produces (signatures every later task uses):
  - `pub(crate) fn produced_item<'a>(placed: Placed<'a>) -> Option<&'a ItemId>`
  - `pub(crate) fn assembly_recipe<'a>(placed: Placed<'_>, items: &'a ItemDb) -> Option<&'a [(ItemId, u32)]>` (None for an extracting production)
  - `pub(crate) fn intake_recipe(placed: Placed<'_>, items: &ItemDb) -> Option<Vec<(ItemId, u32)>>`
  - `pub(crate) fn feeds(a: Placed<'_>, b: Placed<'_>, items: &ItemDb) -> bool` (and `feeds_ingredient`/`feeds_fuel` alike)
  - `pub(crate) fn grid_rung(placed: Placed<'_>, chain: &HashMap<ItemId, u32>) -> Option<u32>`
  - `pub(crate) fn buffered_item<'a>(placed: Placed<'a>, items: &ItemDb) -> Option<&'a ItemId>`, `item_depot_takes(placed, items, filter)`
  - **User decision (2026-10-16): an order for an item no machine is set to make is ACCEPTED and waits.** `queue_work_order` no longer refuses on "no machine is set" (it still refuses when no machine kind could ever make the item — unchanged). The queued order reports `OrderState::Stalled` with `blocked_by = Some("No way to make this — set a {Kind} to {Item}.")`, and starts working on its own once a machine is set to the item.
- The old `work`/`assembles` structures keep working unchanged through `Placed { recipe: None }` (fixed `work`) and the `assembles` arm, until Task 14 deletes them: `Placed::production` gains a temporary third arm for `assembles` **in this task**, removed with the field in Task 14 — say so in its doc comment.

- [ ] **Step 1: Write failing tests** in `tests/machines.rs`, all with real shipped machines:
  - `a_configured_line_runs_like_the_structures_it_replaces` — extractor (core_fragment) beside factory (bytecode_block), both staffed: within 200 ticks the factory's output holds bytecode_block, and `Task::required` on the factory's worker equals what a Refinery's worker bakes at the same speed (the pace is the replaced structure's). No equal-output comparison — the extractor's chance roll makes that an RNG-stream test.
  - `an_unconfigured_machine_is_never_staffed` — unset factory, idle programs, a standing order for bytecode_block queued (and Stalled); after 200 ticks no `Task` targets it; `accepts_a_program` false; `set_standing_job(f, true, false)` refused.
  - `an_unconfigured_machine_draws_no_power` — base supply covers exactly the configured machines; an unset Assembler beside them darkens nothing and is not in the ledger's draw.
  - `an_unconfigured_machine_pulls_nothing_from_its_neighbours` — unset factory beside a stocked extractor output: extractor output unchanged after 100 ticks.
  - `an_order_is_served_only_by_machines_set_to_its_item` — two factories, one set bytecode_block, one charge_coil; order bytecode_block: only the first gets a body.
  - `an_order_for_an_item_no_machine_is_set_to_make_waits_stalled` — `queue_work_order(batch(bytecode_block, 5))` with only an unset factory → `Ok`; its `WorkOrderReport` has `state == OrderState::Stalled` and `blocked_by == Some("No way to make this — set a Factory to Bytecode Block.")`; set the factory to bytecode_block and within 200 ticks the state leaves `Stalled`.
  - `a_standing_order_stalls_when_its_machine_is_switched_and_resumes_when_set_back` (Review Focus 4) — standing order for bytecode_block; switch the factory to charge_coil: order still queued and `standing_of` is `Stalled`; set back: `Wants` again.
  - `power_cell_pays_tier_one_on_an_upgraded_extractor` — Mk4 extractor on power_cell pays `node_payout(1, zone)` per cycle; on core_fragment pays `node_payout(4, zone)` (Review Focus 5's payout half).
  - `a_depot_placed_beside_a_configured_machine_filters_for_its_output` — `auto_filter_depots` reads the instance output.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine machines` — Expected: FAIL.
- [ ] **Step 3: Implement** the signature changes above at every listed site; let the compiler find the rest (`cargo build -p feral-processes-engine`), building each `Placed` from `Structure` + `Option<&MachineRecipe>` (systems) or `Game::placed` (Game methods).
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine machines`, then `cargo test --workspace` and `cargo test -p feral-processes-engine balance_sim` — Expected: PASS with no other test changed (the old structures resolve identically).
- [ ] **Step 5: Commit**: "Production: every reader asks what a placed machine is set to make, so an unset one does nothing".

### Task 5: A placed machine is named by what it makes `[engine] S`

**Files:**
- Modify: `crates/engine/src/structures.rs` (`Placed::label`), `systems.rs` (stall names in `idle_machine_system`, `task_progress_system`, `assembler_system`), `game/inspection.rs:1469` (`EntityView`) and `:1603` (`structure_report`), `views.rs:1074` (`EntityView`) and `:1726` (`StructureReport`)
- Test: `crates/engine/src/tests/machines.rs`

**Interfaces:**
- Produces:
  - `impl Placed<'_> { pub fn label(self, items: &ItemDb) -> String }` — `"{def.name} ({item name})"` for a configured machine, `def.name` otherwise (non-machines unchanged).
  - `EntityView { …, pub unconfigured: bool }`, `StructureReport { …, pub recipe: Option<ItemId> }`; both `label`s use `Placed::label`.
- [ ] **Step 1: Failing tests**: `a_set_machine_is_labelled_by_its_product` (`structure_report` label `"Factory (Bytecode Block)"`, `recipe == Some(bytecode_block)`); `an_unset_machine_reads_as_its_kind_and_is_flagged` (`"Factory"`, `EntityView::unconfigured`); `a_stall_line_names_the_product` (clogged factory logs `"The Factory (Bytecode Block) is clogged — its output buffer is full."`).
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine machines` — FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, then `cargo test --workspace` — PASS (gui/app-core fixtures that build `EntityView`/`StructureReport` by hand gain the fields).
- [ ] **Step 5: Commit**: "Machines: a placed machine is named by what it makes".

### Task 6: The Workbench, the touch rule and recipe research gates `[engine] L`

**Files:**
- Create: `assets/structures/workbench.ron` (Global Constraints values; `extracts_routines: true`, `extracts_programs: true`, `extracts_research: Some("automation")`, description "Where gear and bench parts are compiled by hand. Stand beside it to compile.")
- Modify: `assets/structures/compiler.ron` (drop `extracts_routines`/`extracts_programs` — moved)
- Modify: `crates/engine/src/items_db.rs:97-102` (`CraftableDef::research`), `crates/engine/src/game/crafting.rs` (`craft_recipes` 61-123, `player_craft_order` 217, new `touched_bench`), `crates/engine/src/game/routines.rs:633` (`can_extract_routines`), `crates/engine/src/game/extraction.rs:120-160` (`standing_extraction_bench`), `crates/engine/src/game/unlocks.rs:584-660` (`research_conversions`/`research_unlocks` name what a node gates)
- Modify: the 33 item files and 18 research recipe entries listed in Global Constraints (`requires_structure` → `"workbench"`, plus `research:` on item craftables); `assets/items/README.md`, `assets/research/README.md`, `assets/structures/README.md`
- Modify: tests that compiled at a removed bench or extracted at the Compiler (`tests/crafting.rs`, `tests/research.rs`, `tests/routines.rs`, `tests/extraction*.rs`, `tests/assets.rs`'s `the_teardown_rig_is_also_an_extraction_bench_and_the_compiler_still_is` → `…_and_the_workbench_is`, `items_db.rs:1250` fixture)
- Test: `crates/engine/src/tests/workbench.rs` (new)

**Interfaces:**
- Produces:
  - `CraftableDef { …, #[serde(default)] pub research: Option<ResearchId> }`.
  - `impl Game { pub(crate) fn touched_bench(&self, kind: &str) -> Option<(Entity, u32)> }` — the orthogonally adjacent standing structure of `kind` with the highest tier (`StructureTier` or 1), ties to the lowest tile; `None` outside base space.
  - `craft_recipes`: a row naming `requires_structure` is offered only while `touched_bench(kind)` is `Some`; an item row with `research` only once researched; benchless rows unchanged (anywhere, including the field).
  - `player_craft_order.bench_tier` = the touched bench's tier (1 when benchless) — replaces `best_structure_tier` here only.
  - Extraction (`can_extract_routines`, `standing_extraction_bench`) counts a flagged def only when its `extracts_research` is `None` or researched; still ownership, not touch.
  - `research_unlocks`/`research_conversions` also name the machine recipes (`def.productions()` with `research == node`) and item craftables (`craftable.research == node`) the node gates, e.g. `"Unlocks: Printer: ICE Breaker"`.
- [ ] **Step 1: Failing tests** in `tests/workbench.rs`:
  - `a_bench_recipe_is_offered_only_beside_a_workbench` — workbench placed at (1,0), `armor_bench` researched: party beside it → hardened_shell listed; party two tiles away → not listed; party in the field → not listed.
  - `a_benchless_recipe_compiles_anywhere` — ice_breaker and power_cell listed in the field with no workbench.
  - `the_touched_workbench_sets_the_quality_floor` — Mk3 workbench touched, a Mk5 one elsewhere in the base: `craft_quality_floor(order)` = `QUALITY_BASE + 2 * QUALITY_BENCH_PER_TIER` (+ perk 0, not careful).
  - `a_workbench_recipe_keeps_the_gate_of_the_structure_it_came_from` — beside a workbench, blank_substrate is absent until `routine_fabrication`, honeypot until `deception`; bytecode_block always present.
  - `extraction_needs_automation_and_a_workbench_somewhere` — workbench standing, automation not researched → `can_extract_routines()` false; researched → true while standing anywhere in the base.
  - In `tests/assets.rs`: `every_bench_recipe_moved_to_the_workbench_keeps_its_gate` asserting the Global Constraints gate list item by item, and `no_shipped_recipe_names_a_removed_bench` (no `requires_structure` in items or research names one of the four old bench ids or the 13 production ids).
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine workbench` — FAIL.
- [ ] **Step 3: Implement** the field, `touched_bench`, the crafting filter and order, the extraction gate, the research display, `workbench.ron`, and re-point the assets. Retire `tests/assets.rs::no_shipped_assembler_builds_another_benchs_product` (its premise — one structure per recipe — is what this removes); say why in the commit body.
- [ ] **Step 4: Fix the tests that compiled at a removed bench** to stand beside a placed `workbench` (`place_now(g, "workbench", …)` + `unlock_research_chain`); a changed expected number must trace to the tier/touch rule, otherwise stop and report.
- [ ] **Step 5: Run** `cargo test -p feral-processes-engine workbench`, `cargo test -p feral-processes-engine balance_sim`, then `cargo test --workspace` — PASS.
- [ ] **Step 6: Commit**: "Workbench: one hand-compile bench you stand at, its tier sets quality, and every moved recipe keeps its research gate".

### Task 7: Contracts — a machine set to make an item `[engine] M`

**Files:**
- Modify: `crates/engine/src/contracts.rs:205-390` (`Objective`, `ObjectiveState`, `target`, `asks_for_none`, `already_met`), `crates/engine/src/game/contracts.rs` (state at ~60, `objective_hint` ~1098, `objective_line` ~1194), `crates/engine/src/settlements/mod.rs:135` (`Specialty::Gear`)
- Modify: `assets/contracts/break_ground.ron`, `tutorial_break_ground.ron`, `keep_the_lights_on.ron` (`Build(structure: "extractor")`, descriptions name the Extractor and the R key), `stand_up_a_refinery.ron` (`Configure(item: "bytecode_block")`, name "Make Bytecode Blocks", description "Raw stock is worth more processed. Set a Factory to make Bytecode Blocks."), `commission_an_armory.ron` (`Configure(item: "hardened_shell")`, name "Commission Plating", description "The Broker would rather buy plate from this sector than haul it in. Set an Assembler to make Hardened Shells."); ids unchanged; `assets/contracts/README.md`
- Modify: `crates/engine/src/tests/assets.rs:3700,3878` (census arms), `tests/contracts.rs` where it enumerates variants
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Produces: `Objective::Configure { item: ItemId }` (target 1, never "asks for none"); `ObjectiveState { …, pub configured: Vec<ItemId> }` (items some standing machine is set to make, from `Game::placed`); `already_met` → `state.configured.contains(item)`; line `"Set a machine to make {Item}"`; hint `"Place a machine, stand beside it and press R to choose what it makes."`.
- [ ] **Step 1: Failing tests**: `a_configure_contract_completes_when_a_machine_is_set_to_its_item`; `an_unset_machine_does_not_meet_a_configure_contract`; `a_machine_set_to_another_item_does_not_meet_it`; assets census `every_configure_objective_names_an_item_some_machine_can_make`; the existing objective-line width census covers the new line.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine contracts` — FAIL.
- [ ] **Step 3: Implement** the variant, every match it breaks, the state field, the five files and the README.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine contracts`, `… assets`, then `cargo test --workspace` — PASS.
- [ ] **Step 5: Commit**: "Contracts: a job can ask for a machine set to make an item".

### Task 8: The recipe menu `[app-core] M`

**Files:**
- Create: `crates/app-core/src/app/machine_recipe.rs` (modelled on `app/rig_tool.rs`)
- Modify: `crates/app-core/src/lib.rs` (`Mode::MachineRecipe`, `Mode::MachineRecipeConfirm`, the blocking-screen list ~2414-2440, `App::machine_recipe` field ~3300, re-exports), `app/mod.rs`, `app/input.rs:262` (dispatch), `app/playing.rs:527` (`GameKey::Char('R') => self.open_machine_recipe()` beside `F`/`P`/`I`)
- Modify: `assets/help/20-controls.md` (`- R — choose what the machine beside you makes`), `assets/help/62-production-lines.md` (one paragraph: place a machine, press R beside it, a change empties it)
- Test: `crates/app-core/src/tests/machine_recipe.rs` (registered in `tests/mod.rs`)

**Interfaces:**
- Consumes: `Game::adjacent_machines`, `machine_recipe_view`, `set_machine_recipe`, `views::MachineRecipeView`.
- Produces: `pub struct MachineRecipeScreen { pub machine: Entity, pub view: MachineRecipeView, pub of: usize, pub pending: Option<ItemId> }`; `App::open_machine_recipe(&mut self) -> bool` (refuses `"There is no machine here to set."` via `App::refuse`), `cycle_machine_recipe` (Tab), `handle_machine_recipe_key`, `handle_machine_recipe_confirm_key`, `leave_machine_recipe`.
- Keys: lowercase letter/digit (`selected_index`) or Up/Down + Enter picks a row; on an unset machine it sets at once, on a set one it moves to `MachineRecipeConfirm` with `pending`; confirm takes `App::yes_no` (yes sets, no returns to the list), Esc cancels; uppercase never picks a row. Confirm prompt names what is emptied: `"Switch the {label} to {Item}? It empties {held} units to the Depots."` (`held` from the view; omit the second sentence when 0).
- [ ] **Step 1: Failing tests**: `r_beside_a_machine_opens_its_recipe_menu`; `r_with_no_machine_beside_refuses`; `picking_a_row_on_an_unset_machine_sets_it_without_asking`; `picking_a_row_on_a_set_machine_asks_first`; `declining_the_confirm_leaves_the_recipe`; `the_menu_shows_only_researched_rows`; `an_uppercase_letter_never_picks_a_row`; `the_screen_closes_when_the_machine_is_demolished`.
- [ ] **Step 2: Run** `cargo test -p feral-processes-app-core machine_recipe` — FAIL.
- [ ] **Step 3: Implement.** Help-page census (`no_shipped_help_page_names_a_hidden_key`) must stay green — `R` is not hidden.
- [ ] **Step 4: Run** the tests, then `cargo test --workspace` — PASS (gui's `ALL_MODES` length fails to compile until Task 9: **do Tasks 8 and 9 as one commit** if the census is a compile-time array — it is, `[Mode; 152]`).
- [ ] **Step 5: Commit** together with Task 9.

### Task 9: The question mark and the recipe screen `[gui] M` (commits with Task 8)

**Files:**
- Create: `crates/gui/src/render/machine_recipe.rs` (modelled on `render/rig_tool.rs`)
- Modify: `crates/gui/src/render/mod.rs` (`mod machine_recipe;`, draw arms ~1190, `ALL_MODES` 1677 → 154 entries, `NEEDS_PENDING_STATE` 1905 if the screens need `App::machine_recipe`), `crates/gui/src/render/base.rs` (~1500, after the outline and before the job mark)
- Test: in-file `#[cfg(test)]` of both files, plus `render/mod.rs`'s `every_screen_draws_a_refusal_exactly_once`
- Modify (user decision 2026-10-16): the Work Orders screen rows (`render/building.rs` `work_order_screen_rows`, and `render/base_report/stock.rs` where `OrderState::Stalled` reads "stalled") — a `Stalled` order wears a red `!` (`hud::palette` danger/red token, through `Painter`) and shows its `blocked_by` sentence ("No way to make this — set a {Kind} to {Item}.") on its row.

**Interfaces:**
- Consumes: `EntityView::unconfigured`, `MachineRecipeScreen`.
- Produces: `fn draw_unconfigured_mark(painter: &Painter, px: f32, py: f32, tile_px: f32)` in `render/base.rs` — a `?` through `Painter::map` in `hud::palette::ATTENTION`, top-right quarter of the tile; `pub(super) fn draw_machine_recipe(…)` and `draw_machine_recipe_confirm(…)` with rows `"[a] Bytecode Block  4 Core Fragment  steady"`, the current row marked.
- [ ] **Step 1: Failing tests**: `an_unset_machine_wears_a_question_mark` and `a_set_machine_wears_none` (headless painter census as the existing base-map tests do, asserting the `?` glyph is or is not drawn on that cell); `a_stalled_work_order_wears_a_red_bang_and_its_reason` (row for an order with `OrderState::Stalled`, `blocked_by` set: `!` drawn in the red token and the sentence on the row; a `Queued` order wears none); `the_widest_recipe_row_fits_the_popup` (the widest shipped row — `hardened_shell`'s — within the popup body, `rig_tool.rs`'s `widest_screen` pattern); mutation-check: stub `draw_machine_recipe` and see `every_screen_draws_a_refusal_exactly_once` fail.
- [ ] **Step 2: Run** `cargo test -p feral-processes-gui` filtered on those names — FAIL.
- [ ] **Step 3: Implement**; nothing in `render/` names a backend type.
- [ ] **Step 4: Run** `cargo test --workspace` — PASS. Screenshot check: `cargo run -- --template chains --keys "R" --screenshot /tmp/claude-1000/recipe.png` once Task 13 has machines in the template (skip until then; note it in the commit body).
- [ ] **Step 5: Commit** Tasks 8+9: "Recipe menu: R beside a machine picks what it makes; an unset machine wears a question mark".

### Task 10: Migrate base-labour engine tests to the machines `[engine] L`

**Files:** `crates/engine/src/tests/support.rs` (rename `spawn_mining_node` → `spawn_extractor`, kind `"extractor"` plus `MachineRecipe(Some(core_fragment))`; `run_one_full_gather_cycle` and lines 950-2800 helpers), `tests/lines.rs`, `tests/work_orders.rs`, `tests/hauling.rs`, `tests/chains.rs`, `tests/depot_feed.rs`, `tests/depot_filter.rs`, `tests/stock.rs`, `tests/power.rs`, `tests/transfer.rs`, `tests/floor.rs`.

**Mapping** (old id → kind + recipe, via `place_machine`/`spawn_machine_at`): mining_node → extractor/core_fragment; log_scraper → extractor/raw_trace; cache_tap → extractor/cache_grain; power_conduit → extractor/power_cell; lathe → factory/blank_substrate; refinery → factory/bytecode_block; annealing_node → factory/annealed_core; transcriber → factory/logic_wafer; winding_node → factory/charge_coil; disk_press → assembler/routine_disk; decoy_bench → assembler/honeypot; refactor_bench → assembler/recompile_kernel; armory → assembler/hardened_shell; fabricator → assembler/trace_sniffer; compiler → printer/ice_breaker; assembly_bay → printer/patch_routine. A test using an old bench **as a bench** uses `workbench`.

- [ ] **Step 1:** Replace every removed id in these files per the mapping; `find_structure_by_kind(g, "mining_node")` becomes the new kind (or a tile lookup where two of one kind now stand).
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine` filtered per file. An expected figure that moves must be explained by a Global Constraints change (power draw, capacity, build cost, glyph, the per-instance label); write the new figure with that reason in a comment only where the old comment named the old value. Anything else is a regression: stop and report.
- [ ] **Step 3: Gate** `rg -n '"(mining_node|log_scraper|cache_tap|power_conduit|lathe|refinery|annealing_node|transcriber|winding_node|disk_press|decoy_bench|assembly_bay|compiler|armory|fabricator|refactor_bench)"'` over these files returns nothing; `cargo test --workspace` PASS.
- [ ] **Step 4: Commit**: "Tests: base-labour fixtures stand configured machines".

### Task 11: Migrate the remaining engine tests `[engine] L`

**Files:** every other `crates/engine/src/tests/*.rs` the gate finds (`building`, `raids`, `inspection`, `construction`, `memories`, `base_space`, `assets`, `catalog`, `research`, `save_roundtrip`, `ruins`, `base_ledger`, `alerts`, `sabotage`, `routines`, `prison`, `supply`, `siphon`, `duties`, `bonds`, `zone`, …) and in-module tests: `structures.rs:1269,1281,1442`, `base_ledger.rs:583`, `alerts.rs:168-210` (subject strings only), `rooms.rs:519`, `research.rs:423-500`, `items_db.rs:1250`.

- [ ] **Step 1:** Same mapping and rules as Task 10.
- [ ] **Step 2: Run** per file, then `cargo test --workspace` — PASS.
- [ ] **Step 3: Gate** — the Task 10 `rg` over `crates/engine/src` returns only `assets`-file-reading code that Task 14 deletes with the files (list it in the commit body).
- [ ] **Step 4: Commit**: "Tests: the engine's fixtures stand machines and a Workbench".

### Task 12: app-core, gui and launcher fixtures; the bench tuner `[app-core][gui][launcher] M`

**Files:**
- `crates/app-core/src/tests/{support,building,group_menus,sprite_forge}.rs`; `crates/gui/src/render/{test_support.rs:371, mod.rs:2041-2104, base.rs:3980, building.rs:2741-3093, hud/panes.rs:732, marks.rs, transfer.rs:538}` — fixtures that read real assets switch to the new kinds; hand-built view fixtures whose labels are only strings may keep them (they test layout, not content).
- `crates/engine/src/bench/report.rs:82` (`MachineReport { …, #[serde(default)] pub recipe: Option<String> }`; `measure("economy.running_share.<key>")` matches `kind == key || recipe == key`), `crates/engine/src/bench/mod.rs:79` (carry `StructureReport::recipe`)
- `crates/launcher/src/bench/knob.rs` (`FIELDS`: replace `assembles.ticks_per_unit` with `machine.recipes.<item>.ticks_per_unit`; `read_back` and `locate` find the `recipes` entry whose `produces` is `<item>`), `bench/objective.rs`, `bench/search.rs:590`, `bench/mod.rs:84`
- `dev-tuning/economy.ron`, `dev-tuning/economy-bench.ron` (knobs → `structures/extractor.ron` `machine.recipes.core_fragment.ticks_per_unit`, `…power_cell…`, `structures/factory.ron` `…charge_coil…`/`…bytecode_block…`, `structures/printer.ron` `…patch_routine…`/`…ice_breaker…`, capacity knobs → `structures/printer.ron`; targets `economy.running_share.patch_routine` / `.core_fragment`)
- [ ] **Step 1: Failing tests** in `knob.rs`: `a_recipe_knob_reads_and_patches_one_recipes_cycle` (patching `machine.recipes.patch_routine.ticks_per_unit` on `printer.ron` changes only that entry's number); `a_recipe_knob_on_a_missing_item_is_an_error`; in `bench/report.rs`: `running_share_matches_a_recipe_key`.
- [ ] **Step 2: Run** `cargo test -p feral-processes knob` (launcher package) — FAIL.
- [ ] **Step 3: Implement** and migrate the fixtures.
- [ ] **Step 4: Run** `cargo test --workspace` — PASS. Add `bench/objective.rs` test `the_shipped_economy_objectives_check_against_the_assets`: `Objective::load` each of `dev-tuning/economy.ron` and `economy-bench.ron` and `knob::check` every knob against `assets/` — nothing else validates these files.
- [ ] **Step 5: Commit**: "Fixtures and bench tuner speak machines and recipes".

### Task 13: Recreate every dev-saves template `[launcher] M`

**Files:** `dev-saves/*.ron` (18 with removed kinds; `tactical.ron` has none), `dev-saves/README.md`, `crates/launcher/src/dev_template.rs` tests (610-740: find the bay as the printer whose `recipe` is `patch_routine`; capacity 15 from `assets/structures/printer.ron`).

Templates are dumps; edit them as text (the established v31/v33/v34/v36 route), with a throwaway script in the scratchpad, never in the repo:
- Each `kind: "<old>"` StructureSave → the mapped kind plus `recipe: Some("<item>")`; `tier` kept.
- **Compiler by intent:** in `bench-economy`, `breeding`, `rooms` (an ICE Breaker line measured with `--order ice_breaker`) and `deep-lair` (staffed) → `printer` set to `ice_breaker`; everywhere else → `workbench` (tier kept), and any `cronjob` targeting its tile becomes `None` (the program turns idle staff). `deep-lair`'s armory → `workbench`. Every template that gains a Workbench already has `automation` researched (it owned a Compiler) — assert in Step 2.
- [ ] **Step 1: Failing tests** in `dev_template.rs`: `no_template_stands_a_removed_structure` (every `StructureSave::kind` is a loaded def id); `every_template_machine_is_set_to_a_recipe_it_lists`; `every_template_with_a_workbench_has_automation_researched`.
- [ ] **Step 2: Run** `cargo test -p feral-processes dev_template` — FAIL.
- [ ] **Step 3: Rewrite** the templates; update `dev-saves/README.md`: a "Migrated at v38: configurable machines" section (the mapping and the Compiler rule), and every template row that names an old structure renamed in place with its intent unchanged (e.g. chains: "Extractor (Core Fragment) `(2,0)` → Factory (Bytecode Block) `(3,0)` → Printer (Patch Routine) `(4,0)`", extraction: "A Workbench stands at tier 1, so `can_extract_routines()` is true").
- [ ] **Step 4: Run** `cargo test -p feral-processes dev_template` (includes `every_checked_in_template_still_loads`, `the_chains_template_starts_with_a_chain_that_actually_runs`, `the_bench_economy_template_sustains_flow_under_orders`, `every_checked_in_templates_base_can_power_itself`), then `cargo test --workspace` — PASS. Take the Task 9 screenshot now and Read the PNG.
- [ ] **Step 5: Commit**: "dev-saves: every template stands configured machines and a Workbench".

### Task 14: Delete the 17 structures and the `assembles` field `[engine][app-core][gui] M`

**Files:**
- Delete: `assets/structures/{mining_node,log_scraper,cache_tap,power_conduit,lathe,refinery,annealing_node,transcriber,winding_node,disk_press,decoy_bench,assembly_bay,compiler,armory,fabricator,refactor_bench}.ron` — 16 files. The spec says 17, but its own removal table lists these 16 and no seventeenth file exists; nothing else is deleted.
- Modify: `crates/engine/src/structures.rs` (delete `assembles`, `AssembleDef`, the temporary `assembles` arm in `Placed::production` and in `category`, `cycle_ticks`' old split), `FALLBACK_BASIN_EXIT`'s literal if it names `assembles`, `tests/assets.rs` censuses on `assembles` (rewrite `every_shipped_assembler_recipe_is_a_single_ingredient`/`every_shipped_assembles_names_an_item_that_declares_a_recipe`/`an_assembler_runs_the_named_items_own_craftable_recipe`/`a_structure_that_assembles_nothing_resolves_no_recipe` over `machine.recipes`; retire `every_assembling_structure_describes_its_conversion_in_that_order`), `every_research_material_is_reachable_through_that_nodes_own_prerequisites` (an item is makeable at a node when some machine recipe producing it has `research` `None` or within the node's prerequisite closure).
- Modify research: drop removed ids from `unlocks_structures` in `automation`, `power_grid`, `routine_fabrication`, `armor_bench`, `weapon_bench`, `deception`, `cache_coherence` (keeps `line_driver`), `program_refactoring`; rewrite their `description`s to name the recipes and Workbench they gate (plain language, no structure that no longer exists).
- Modify help: `assets/help/50-routines.md`, `60-your-base.md`, `62-production-lines.md`, `64-implants.md`, `70-supplies.md`, `75-extraction.md` — every removed structure name rewritten to the machine + recipe or Workbench.
- Modify READMEs: `assets/structures/README.md` (remove `assembles`; examples use the machines), `assets/items/README.md`, `assets/research/README.md`, `assets/perks/README.md`, `assets/tools/README.md`, `assets/species/README.md`, `assets/classes/README.md`, `assets/abilities/README.md` wherever they name a removed structure.
- Modify: `crates/launcher/src/bench/knob.rs` (`assembles.ticks_per_unit` arm gone if Task 12 left it).
- Test: `crates/app-core/src/tests/building.rs`
- [ ] **Step 1: Failing test** `the_build_menu_offers_the_machines_and_the_workbench_and_none_of_the_removed` (app-core: from a fresh base, `buildable_structure_defs` ids include the five and none of the 16).
- [ ] **Step 2: Run** `cargo test -p feral-processes-app-core the_build_menu_offers` — FAIL.
- [ ] **Step 3: Delete and rewrite** as listed; achievements, talents and `dev-arenas/` name none of the removed ids (verified when this plan was written — re-run the gate below to confirm).
- [ ] **Step 4: Gate**: `rg -n '\b(mining_node|log_scraper|cache_tap|power_conduit|lathe|refinery|annealing_node|transcriber|winding_node|disk_press|decoy_bench|assembly_bay|armory|fabricator|refactor_bench)\b|"compiler"' crates assets dev-saves dev-tuning dev-arenas` returns only the `fabricator` *class* (`assets/classes/fabricator.ron` and its readers) and prose uses of "compiler"; `rg -n 'Mining Node|Log Scraper|Cache Tap|Power Conduit|Lathe|Refinery|Annealing Node|Transcriber|Winding Node|Disk Press|Decoy Bench|Assembly Bay|Refactor Bench|Armory' assets crates/*/src --glob '!**/tests/**'` returns only hand-built gui layout fixtures. Then `cargo test --workspace` and `cargo test -p feral-processes-engine balance_sim` — PASS.
- [ ] **Step 5: Commit**: "Structures: the sixteen one-recipe machines and benches are gone".

### Task 15: Seams — rules, skill and graph `[docs] S`

**Files:** `.claude/rules/seams-base.md`, `.claude/rules/seams-items.md`, `.claude/rules/seams-instrumentation.md`, `.claude/skills/seams/references/{base-structures,base-digging,base-instrumentation,hud,ground,sorties,combat-tactical-core}.md`, the memory graph (`memory_search(…, subsystem: "seams")`).
- [ ] **Step 1:** Invoke the `seams` skill and follow its order (graph, skill, rules). Moved rules: "A machine's recipe is the assembled item's own `craftable.cost`, via `systems::assembly_recipe`" (now per placed instance through `Placed`); "Every shipped `assembles` recipe is one ingredient" (now every `Assemble`-role recipe); "`Game::auto_filter_depots` fires once, from `spawn_structure` and only for the Depot" (plus `auto_filter_beside_machine` on a machine's first recipe); "An attached building…" (example names); seams-items "a `work.produces` structure makes its item out of nothing" (an `Extract` recipe). New seams: "**`structures::Placed` is the one door to what a placed structure makes**, and an unset machine produces, draws, serves and staffs nothing because every reader asks it"; "**A machine's recipe is the player's: `Game::set_machine_recipe` is the only writer of `MachineRecipe`**, and no order, auto-setup or load ever sets one".
- [ ] **Step 2: Verify** each changed rule against source (`rg` the named function).
- [ ] **Step 3: Commit** the `.claude/` files: "Seams: production is per placed machine, and only the player sets a recipe".

---

## Out of scope (flagged, not done)

- `docs/*-gen.py` and the pages they generate (`docs/structures.md`, `docs/items.md`, `docs/research.md`, `docs/perks.md`) are hand transcriptions and will go stale; `docs/measurements/` are historical records and stay as written. Raise with the user after landing.
- `CHANGELOG.md`, the release and the version bump happen at deploy.
