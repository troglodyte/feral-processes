# Periodic Caravan Traders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A trader walks in from the sector, phases into base space through the anchor, stands beside the iso Market selling a rolled shelf, and walks back out — on a tunable interval with a randomness window.

**Architecture:** The *schedule and the shelf* are derived from `(BaseGrid::seed(), CARAVAN_SALT, visit_index)` exactly as the Broker board is derived, so they survive a reload with no save field, spend no `GameRng` draw, cannot be save-scummed, and rotate on their own. The *journey* — which of five stages the caravan is in and where it is standing — is entity state and is saved. Those are two different questions, which is what keeps them from becoming two sources of one truth. The caravan is a non-`Structure`, non-`Creature` entity, the third after `DigSite` to carry a base-space `Position`.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), RON assets, `serde`. Engine → app-core → gui, in that dependency order.

**Spec:** `docs/superpowers/specs/2026-08-24-periodic-caravan-traders-design.md` — read it first. It carries the argument; this plan carries the sequence.

## Global Constraints

Copied from `CLAUDE.md` and the spec. Every task's requirements implicitly include this section.

- **The renderer never touches the ECS `World`.** `Game` is the whole public API surface. `crates/gui` has `bevy_ecs` in its graph now, so nothing stops a `world_mut()` accessor except this rule.
- **`crates/gui/src/paint.rs` is the only file that names a graphics library.** Everything in `render/` draws through `Painter`.
- **Panes take their origin from the caller.** A literal `0.0` draws under the stock strip and no test sees it.
- **New content is data, not Rust.** A new trader is a `.ron` file. A malformed `.ron` is skipped with a logged warning, never a panic — follow `ContractDb::load_dir`.
- **`tuning.rs` holds what the engine hardcodes and never a copy of a `.ron` value.** Difficulty and economy knobs go there as documented `pub const` in a labelled section.
- **Additive save fields behind `#[serde(default)]` cost no `SAVE_FORMAT_VERSION` bump.** Do not bump it. Use a **named struct, never a positional tuple** — that is the one shape field-named RON does not save you from.
- **A `#[serde(skip)]` field stays green through a RON round trip.** Every new save field needs a real save→load test, not only the round trip.
- **No flaky tests.** No `sleep()`, no wall-clock, no unseeded RNG. Background systems will interfere with a naive assertion.
- **A test that passes with the fix removed is not a test.** For each new test, delete the implementation and confirm it fails before moving on.
- **Comments explain *why*, never *what*.** A doc comment claiming to mirror another formula must be a *call*, not a copy.
- **Commit on green.** Branch is `caravan-traders`; already created. Check `git branch --show-current` before every commit — a concurrent session has moved branches under this repo before. **Never push.**
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.

**Stale docs warning.** `CLAUDE.md`'s **The base** section and the matching `docs/seams.md` entries describe deleted code: `resources::Platform`, `Game::build_radius`, `Platform::covers`, `build_radius_bonus`, `hauling::walk_leg` and its `Leg` enum are all gone. The base is a phased-out interior pocket in `base_grid::BaseGrid` entered through an anchor. **Verify every seam against source before relying on it.** Do not "fix" those docs in this branch.

---

### Task 1: `CaravanDef`, `CaravanDb`, and the shipped traders

**Files:**
- Create: `crates/engine/src/caravans.rs`
- Create: `assets/caravans/README.md`
- Create: `assets/caravans/*.ron` — two shipped traders
- Modify: `crates/engine/src/lib.rs` (module declaration, re-export)
- Modify: `crates/engine/src/game/lifecycle.rs:~1758` — register beside `ContractDb::load_dir` and `MemoryDb::load_dir`
- Test: `crates/engine/src/tests/caravans.rs`, registered in `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `caravans::CaravanDef { id: String, name: String, description: String, glyph: char, color: GlyphColor, rows: u32, weights: CaravanWeights, min_zone: u32, max_zone: u32 }`; `CaravanWeights { gear: u32, routines: u32, programs: u32, materials: u32 }`; `CaravanDb` with `load_dir(&Path) -> std::io::Result<(Self, Vec<String>)>`, `get(&str) -> Option<&CaravanDef>`, `all() -> impl Iterator<Item = &CaravanDef>`, and `for_zone(u32) -> Vec<&CaravanDef>` returning defs sorted by `id`.

**Why sorted by id:** `for_zone` feeds a derived pick in Task 2. An unsorted iteration order makes the same seed choose different traders between runs — the exact fault `assembler_system` sorts machines by `(x, y)` to avoid, and the one that produced this repo's `species-habitat-lookup-unsorted-flake`.

- [ ] **Step 1: Read the pattern before writing anything.** Read `crates/engine/src/contracts.rs:422` (`ContractDb::load_dir`) and `crates/engine/src/memories.rs:108`. Match their shape: every field a mod might not have gets `#[serde(default)]`, a malformed file pushes a warning string and is skipped.

- [ ] **Step 2: Write the failing tests.** In `tests/caravans.rs`:
  - The shipped directory loads and yields at least two defs, with no warnings.
  - A def whose `.ron` is malformed is skipped and produces exactly one warning, and the other files in the directory still load. Write the malformed file to a temp dir — do **not** mutate `assets/`.
  - `for_zone` returns only defs whose `[min_zone, max_zone]` window contains the argument, sorted by `id`.
  - A census over the real `assets/caravans/`: every def has `rows >= 1`, at least one non-zero weight, and `min_zone <= max_zone`.

- [ ] **Step 3: Run and confirm they fail.** `cargo test -p feral-processes-engine caravan` — expect compile failure (module does not exist).

- [ ] **Step 4: Implement `caravans.rs`, declare the module, register the load.** Registration goes beside the other `load_dir` calls in `game/lifecycle.rs`, inserting `CaravanDb` as a resource and routing warnings the same way its neighbours do.

- [ ] **Step 5: Write the two shipped traders and the README.** One gear-weighted, one program-weighted, so "which trader visits" is visibly a different shelf and not a reroll of one table. Both author `glyph: 'Ω'`. Pick a `color` no base fixture uses — census `assets/structures/*.ron` and the `Glyph` writers in `crates/engine/src` first, and record the census result in the README. The README is the schema reference and must document **every** field, per the project's schema-docs rule.

- [ ] **Step 6: Verify green, then prove the tests bite.** `cargo test -p feral-processes-engine caravan`. Then, for the census and the `for_zone` sort, break the implementation (unsort the iteration; relax the window check), confirm each test fails, restore. `git diff --quiet assets/` before believing any result.

- [ ] **Step 7: `cargo fmt`, `cargo clippy --workspace`, commit.** Explicit paths only — never `git add -A`.

---

### Task 2: The derived schedule

**Files:**
- Create: `crates/engine/src/game/caravan.rs`
- Modify: `crates/engine/src/game/mod.rs` (module declaration)
- Modify: `crates/engine/src/tuning.rs` — a new labelled section
- Test: `crates/engine/src/tests/caravans.rs`

**Interfaces:**
- Consumes: `CaravanDb::for_zone` (Task 1); `base_grid::BaseGrid::seed() -> u32`; `contracts::fold(u64, &[u8]) -> u64`.
- Produces: `CaravanVisit { visit: u64, def_id: String, arrival_tick: u64, depart_tick: u64, bearing: u8 }`; `Game::visit_index(&self) -> u64`; `Game::visit_seed(&self, visit: u64) -> u64`; `Game::scheduled_visit(&self) -> Option<CaravanVisit>` — `None` when no iso Market stands or the current tick is outside the visit's window.

**The formula.** This is the one part worth spelling out, because both halves are load-bearing:

```
visit_index = current_tick / CARAVAN_VISIT_INTERVAL_TICKS
seed        = fold(fold(BaseGrid::seed() as u64, &CARAVAN_SALT.to_le_bytes()),
                   &visit_index.to_le_bytes())
arrival     = visit_index * CARAVAN_VISIT_INTERVAL_TICKS
              + reduce(seed, CARAVAN_ARRIVAL_JITTER_TICKS)
depart      = arrival + CARAVAN_STAY_TICKS
```

Two constraints on it. Use `contracts::fold`, not a new scheme — `FrameSpec::salted`'s rule is that there is one salting scheme, and a second could collide with the Stack's. Reduce with the **high bits** (Lemire's reducer, as `descriptions.rs` does), never `%` — a modulo on a fold makes consecutive visit indices anti-correlate, which is this repo's `description-selection-reads-high-bits` trap. Derive each of arrival offset, trader pick and bearing from a *separately re-folded* seed so they are independent.

**`BaseGrid::seed()`, not `WorldMap::seed()`:** the base's seed is minted at `Game::new` and travels across a breach; the world seed is re-minted per zone. The rhythm belongs to the base.

- [ ] **Step 1: Add the `tuning.rs` section.** `CARAVAN_VISIT_INTERVAL_TICKS`, `CARAVAN_ARRIVAL_JITTER_TICKS`, `CARAVAN_STAY_TICKS`, `CARAVAN_SPAWN_DISTANCE_TILES`, `CARAVAN_MARKUP`, and `CARAVAN_SALT`. Each a documented `pub const` saying what it does and what breaks if it moves. **`rows` is not here** — it is `CaravanDef::rows`. Pick `CARAVAN_STAY_TICKS` long enough to outlast a field trip, per the spec.

- [ ] **Step 2: Write the failing tests.** Intent, one test each:
  - Exactly one visit falls in each interval.
  - Across many visit indices, every arrival offset lands inside `[0, CARAVAN_ARRIVAL_JITTER_TICKS)` and the set of observed offsets has more than one member — a jitter that is constant passes a bounds-only test.
  - Two consecutive visits differ in at least one of trader, offset, bearing.
  - `scheduled_visit` is `None` with no Market standing and `Some` with one.
  - **The derivation survives save/load**: schedule, save, load, schedule again, assert equal.
  - **It draws no `GameRng`**: sample the RNG stream, call `scheduled_visit` several times, sample again, assert the stream did not move. This is the property that keeps the feature from shifting every other roll in the run.

- [ ] **Step 3: Run and confirm failure.** `cargo test -p feral-processes-engine caravan`.

- [ ] **Step 4: Implement `game/caravan.rs`.** Schedule only — no entity, no shelf, no movement yet.

- [ ] **Step 5: Verify green and prove the tests bite.** Swap the high-bit reducer for `%` and confirm the "consecutive visits differ" test degrades; restore. Delete the `has_market` guard and confirm the `None` test fails; restore.

- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 3: The derived shelf and its prices

**Files:**
- Modify: `crates/engine/src/game/caravan.rs`
- Modify: `crates/engine/src/views.rs` — the row view type
- Test: `crates/engine/src/tests/caravans.rs`

**Interfaces:**
- Consumes: `CaravanVisit` (Task 2); `Game::item_value`; `Game::copy_bonus` and the gear-roll helpers behind `Game::grant_gear_drop`; `stack_market`'s program appraisal; `ZoneLevel`.
- Produces: `views::CaravanOffer { index: usize, name: String, detail: String, kind: CaravanOfferKind, unit_cost: u32, qty: u32 }` with `CaravanOfferKind { Gear(GearCopy), Routine(String), Program(String), Material(ItemId) }`; `Game::caravan_shelf(&mut self, visit: &CaravanVisit) -> Vec<CaravanOffer>`.

**Pricing, and the two floors that fence it:** unit cost is `item_value × CARAVAN_MARKUP`, scaled by `ZoneLevel`, and for anything craftable floored strictly above what its recipe's ingredients cost. Programs are priced by power — **call** `stack_market`'s existing appraisal rather than restating it; a doc comment claiming to mirror it is exactly the failure this repo has hit four times.

- [ ] **Step 1: Write the failing tests.**
  - Row count equals the visiting def's `rows`.
  - A gear-weighted def yields mostly gear rows and a program-weighted def mostly program rows — assert the bias, not an exact composition, or the test pins the RNG stream rather than the feature.
  - The same visit index yields an identical shelf twice, and across a save/load.
  - Zone scaling: the same offer costs strictly more at a higher `ZoneLevel`.
  - Craft premium: for every craftable on a shipped shelf, unit cost exceeds the summed value of its recipe ingredients.
  - **Census over the real assets: a Portal Fragment can never appear on any caravan shelf**, at any zone, for any shipped def, across many visit indices. Model it on the contracts census that keeps `Reward::PortalFragments` absent.
  - Reading the shelf draws no `GameRng` — same stream-position assertion as Task 2.

- [ ] **Step 2: Run and confirm failure.**

- [ ] **Step 3: Implement `caravan_shelf` and the view type.**

- [ ] **Step 4: Verify green, then prove they bite.** Remove the Portal Fragment exclusion and confirm the census fails. Remove the `ZoneLevel` scale and confirm that test fails. Restore both.

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 4: The component, the space predicate, and the save

**Files:**
- Modify: `crates/engine/src/components.rs`
- Modify: `crates/engine/src/game/inspection.rs:218` — `stands_in_base_space`
- Modify: `crates/engine/src/save.rs` — new `CaravanSave`, new field on `SaveData:597`
- Test: `crates/engine/src/tests/caravans.rs`, `crates/engine/src/tests/save.rs`

**Interfaces:**
- Consumes: nothing from Tasks 2–3 at runtime.
- Produces: `components::CaravanStage { Approaching, Docking, Crossing, Docked, Leaving }` with `fn in_base_space(self) -> bool` (true for `Crossing` and `Docked`; `Docking` and `Leaving` are the transition ticks — decide each explicitly and state it in a doc comment); `components::Caravan { stage: CaravanStage, visit: u64, arrival_tile: (i32, i32), stage_ticks: u32 }`; `save::CaravanSave` as a **named struct**.

**The predicate.** `stands_in_base_space` is currently `Structure || Tamed`. Its third arm reads the caravan's stage, so the answer is per-stage rather than per-entity. This is the first entity besides the player to change spaces, and it is why the arm reads a component field rather than testing for the component's presence.

`DigSiteSave` (`save.rs:481`) is the precedent to follow: a non-`Structure` base-space entity, saved as its own named struct.

- [ ] **Step 1: Write the failing tests.**
  - `view_entities` shows the caravan on the surface while `Approaching` and on the base map while `Docked` — asserted **through `view_entities` from both locales**, never by calling the predicate directly. The map and the predicate must be tested against each other.
  - **Save→load**, not only the RON round trip: stand a caravan mid-journey, save, load, assert stage, position, visit and `arrival_tile` all survive. A `#[serde(skip)]` would leave a round-trip test green.
  - A save written before this field loads without error and yields no caravan.
  - `SAVE_FORMAT_VERSION` is unchanged — assert on the constant.

- [ ] **Step 2: Run and confirm failure.**

- [ ] **Step 3: Implement the component, the predicate arm, and the save field** behind `#[serde(default)]`.

- [ ] **Step 4: Verify green, then prove the tests bite.** Delete the `stands_in_base_space` arm and confirm the two-locale test fails. Mark the save field `#[serde(skip)]` and confirm the save→load test fails while the RON round trip stays green — that contrast is the point of writing both. Restore.

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 5: The journey

**Files:**
- Modify: `crates/engine/src/game/caravan.rs`
- Modify: `crates/engine/src/game/turn.rs:136` — one call in `tick_inner`
- Test: `crates/engine/src/tests/caravans.rs`

**Interfaces:**
- Consumes: `scheduled_visit` (Task 2); `Caravan`/`CaravanStage` (Task 4); `pursuit::walk_field(origin, radius, step_allowed) -> HashMap<(i32,i32), u32>`; `hauling::step_to_post(grid, from, target, blocked, pocket_radius) -> Result<Option<Position>, NoPost>`; `Game::anchor_position() -> Option<(i32,i32)>`; the anchor's landing cell in `game/base_space.rs`.
- Produces: `Game::caravan_tick(&mut self)`.

**Where the call goes in `tick_inner`, and why.** Place it **after `self.schedule.run(&mut self.world)` and before `self.raid_check()`**. After the schedule because base systems' commands have just flushed and the caravan reads structure positions; before `raid_check` so a caravan cannot be caught by a raid resolved the same tick. State the reasoning in a comment beside the call, as every one of its neighbours does.

**Reuse, do not copy.** `walk_field` is the one Dijkstra walk on the surface and takes its step rule as a parameter — pass a caravan rule, do not write a second walk. `step_to_post` is already the shared base-space answer to "which way", shared between `haul_step_system` and `run_dig_crew`; this is its third caller.

**Determinism:** a base with more than one Market resolves by sorting candidates by `(x, y)` and taking the first. Bevy's query iteration order is not stable, and a caravan that docked at a different Market between two loads of one save would be reporting iteration order rather than the base.

**Two stuck cases, each announced once**, on `DigSite::announced_stuck`'s latch idiom — a per-tick line is what makes a latch necessary:
- No surface route to the anchor → give up, despawn, visit is a miss.
- No base-space route to the Market → wait at the landing cell, leave at the end of the stay.

- [ ] **Step 1: Write the failing tests.** Use a `dev-saves/` template or a fixture base with a Market standing rather than a bare `Game::new` where practical.
  - Each of the four stage transitions fires, driven by ticking rather than by hand-writing the stage.
  - The caravan reaches a cell orthogonally adjacent to the Market.
  - It departs after `CARAVAN_STAY_TICKS` and despawns at `arrival_tile`.
  - Both stuck cases log **once**, not per tick — tick well past the stall and count log lines.
  - A base with two Markets docks at the same one across repeated loads of one save.
  - The caravan advances while the party is underground — ticks run regardless, and there must be no locale special case.
  - Market destroyed mid-visit → the caravan leaves early.
  - Arrival logs one line and departure logs one line, both `Info`, neither `Raid`.

- [ ] **Step 2: Run and confirm failure.**

- [ ] **Step 3: Implement `caravan_tick` and wire it into `tick_inner`.**

- [ ] **Step 4: Verify green, then prove they bite.** Remove a latch and confirm the once-only test fails. Remove the `(x, y)` sort and confirm the two-Market test becomes unreliable — if it still passes, the test is pinning something else and needs rewriting. Restore.

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 6: Examine, and the glyph on the map

**Files:**
- Modify: `crates/engine/src/views.rs` — `drawn_on_surface_map`
- Modify: `crates/engine/src/game/inspection.rs` — `find_target_in_direction`
- Test: `crates/engine/src/tests/caravans.rs`

**Interfaces:**
- Consumes: `Caravan` (Task 4).
- Produces: no new public signature; the examine ray gains a caravan case.

The ray is a **one-tile-wide** scan at `tuning::EXAMINE_RANGE_TILES` whose ties are a total `(step, kind, entity)` order. A caravan carries neither `Creature` nor `Structure`, which is exactly why the ray looks through nests, surface links and zone portals today — this arm closes part of that gap. `views::drawn_on_surface_map` and `find_target_in_direction` must both change: they are one rule read from two places.

- [ ] **Step 1: Write the failing tests.** An inbound caravan on the player's row is named by the examine ray. Nothing can target it as a combat participant. The map and the ray agree — assert them **against each other**, never against a string.

- [ ] **Step 2: Run and confirm failure.**

- [ ] **Step 3: Implement both arms.**

- [ ] **Step 4: Verify green; delete one arm and confirm the map/ray agreement test fails; restore.**

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 7: Reach, transactions, and within-visit depletion

**Files:**
- Modify: `crates/engine/src/game/caravan.rs`
- Modify: `crates/engine/src/resources.rs` — `CaravanMemory`
- Modify: `crates/engine/src/save.rs` — save `CaravanMemory`
- Modify: `crates/engine/src/game/zone.rs` — wipe by name in `enter_next_zone`, beside `BuybackLedger` and `StackMemory`
- Test: `crates/engine/src/tests/caravans.rs`

**Interfaces:**
- Consumes: `caravan_shelf` (Task 3); `Caravan` (Task 4).
- Produces: `CaravanReach { NoCaravan, NotDocked, AtCaravan }`; `Game::caravan_reach(&mut self) -> CaravanReach`; `views::CaravanView { trader: String, description: String, offers: Vec<CaravanOffer>, sells: Vec<CaravanSellRow>, credits: u32, currency: String, ticks_left: u32 }` and `views::CaravanSellRow { copy: GearCopy, name: String, held: u32, unit_price: u32 }`; `Game::caravan_view(&mut self) -> Option<views::CaravanView>`; `Game::buy_caravan_offer(&mut self, index: usize) -> Result<(), String>`; `Game::sell_to_caravan(&mut self, copy: GearCopy, qty: u32) -> Result<(), String>`; `resources::CaravanMemory { visit: u64, bought: BTreeSet<usize> }`.

**One call answers two questions.** `caravan_view` returns `None` when there is nothing to show, so no screen asks "is there a trader" and "what is on the shelf" separately and then disagrees — `Game::stack_market`'s contract. `caravan_reach` has three states rather than two booleans for `NoPost::BoxedIn`'s reason: the three leave the player different errands. `AtCaravan` measures **base space**, exactly as `broker_reach` does via `base_pos().is_some()`; the walk is visibility, not a gate.

**Every refusal lands before anything is spent.** `buy_market_offer`'s whole ordering, and the reason it is worth reading before writing this: a purchase that took the Credits and then failed is the one bug the player cannot undo, and a caravan has no buyback.

**Depletion is not a buyback shelf.** `CaravanMemory` records which of the caravan's *own* rows it has sold. Keyed by visit index so it self-clears when the index moves — no explicit reset anywhere.

- [ ] **Step 1: Write the failing tests.**
  - The three reach states, on the `broker_reach_reports_the_three_states` model.
  - A bought row is gone for the rest of the visit and present again next visit.
  - **Selling to a caravan leaves `BuybackLedger` untouched** — assert on the ledger, not on a screen.
  - Selling pays the iso Market's rate.
  - Every refusal (no caravan, not docked, not enough Credits, row already bought, qty 0) leaves Credits and inventory exactly as they were.
  - `enter_next_zone` wipes `CaravanMemory` and despawns the caravan.
  - `CaravanMemory` survives save→load, and `SAVE_FORMAT_VERSION` is still unchanged.

- [ ] **Step 2: Run and confirm failure.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Verify green, then prove they bite.** Move one refusal below the Credit spend and confirm the corresponding test fails. Remove the `enter_next_zone` wipe and confirm that test fails. Restore.

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 8: The screen in app-core

**Files:**
- Create: `crates/app-core/src/app/caravan.rs`
- Modify: `crates/app-core/src/lib.rs:801` — two new `Mode` variants
- Modify: `crates/app-core/src/app/mod.rs` — module, dispatch, and the modifier fold
- Modify: `crates/app-core/src/app/group_menu.rs:304` — `base_menu_rows`
- Test: `crates/app-core/src/tests/caravan.rs`, registered in `crates/app-core/src/tests/mod.rs`

**Interfaces:**
- Consumes: `caravan_view`, `caravan_reach`, `buy_caravan_offer`, `sell_to_caravan` (Task 7).
- Produces: `Mode::Caravan`, `Mode::CaravanQuantity`; `caravan_row(idx, offers, sells) -> Option<CaravanRow>` with `CaravanRow { Offer(usize), Sell(usize) }`.

Model the file on `crates/app-core/src/app/stack_market.rs`, whose header states why a second counterparty gets its own screen rather than a third section bolted into `Mode::Trade`. Two sections, so `caravan_row` has one offset — mirror `market_row`, which exists precisely because offset arithmetic is where a multi-section screen goes wrong and is the part testable without a trader standing in front of you.

**The row index carried is the row's position in the drawn list, not its shelf index** — a bought row leaves the list and the two stop agreeing the instant anything is bought. Resolve one to the other through the view, as `handle_stack_market_key` does.

The menu row must come from `base_menu_rows` and nowhere else — a group menu's rows are hidden dynamically and a second source drifts. It is **not** `surface_only`.

- [ ] **Step 1: Write the failing tests.**
  - `caravan_row` resolves both sections and returns `None` past the end.
  - The base menu row appears only when `caravan_reach()` is `AtCaravan`, and the test calls `caravan_reach`, not `caravan_view` — the heavier call would make the row test pay for rolling a shelf.
  - Esc from each mode returns to the right place.
  - Buying the last row does not leave the cursor pointing past the list.

- [ ] **Step 2: Run and confirm failure.** `cargo test -p feral-processes-app-core caravan`

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Verify green; break the offset in `caravan_row` and confirm the resolution test fails; restore.**

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 9: The renderer, the censuses, and the full gate

**Files:**
- Create: `crates/gui/src/render/caravan.rs`
- Modify: `crates/gui/src/render/mod.rs` — module, the `draw` arm, and the refusal census at `:1217`
- Test: `crates/gui/src/render/mod.rs` test module

**Interfaces:**
- Consumes: `Mode::Caravan`, `Mode::CaravanQuantity` (Task 8); `views::CaravanView` (Task 7).
- Produces: no new public signature.

Draw through `Painter` only — `paint.rs` is the sole file naming a graphics library. Take the pane origin from the caller; a literal `0.0` draws under the stock strip and no test sees it. The category tag on a row is a **column**, not a substring — pass it as `Row::Item::tag` so `draw_row` lays the row out as three `ui_runs` pieces.

**This page has no scroll.** `draw_popup` pages a `Row::Item` span; a row past the bottom is dropped in silence. Both censuses below are what say it fits.

- [ ] **Step 1: Write the failing tests.**
  - The refusal census at `mod.rs:1217` drives all `Mode`s through `draw` and counts what is painted — confirm both new modes draw a refusal **exactly once**. This census has previously caught both a screen showing it nowhere and one showing it twice.
  - Height: the tallest possible caravan page fits its popup, on `the_tallest_gear_page_fits_its_popup`'s model.
  - Width: no caravan row overflows its popup, measured with `paint::with_painter` — real text metrics, headless. A width test that skips non-`Item` rows measures nothing; the UI font is DejaVu Sans Mono, not the map's unscii.

- [ ] **Step 2: Run and confirm failure.** `cargo test -p feral-processes-gui`

- [ ] **Step 3: Implement `render/caravan.rs` and the `draw` arm.**

- [ ] **Step 4: Verify green; drop the refusal argument on one arm and confirm the census fails; restore.**

- [ ] **Step 5: Update `CHANGELOG.md`.** A new `## X.Y.Z` section describing the feature in the player's vocabulary. **Do not bump the workspace version and do not tag** — that happens once, at the merge, so a rebase cannot invalidate a version already tagged. Do **not** touch `docs/manual.md`, the root `README.md`, or `TODO.md`.

- [ ] **Step 6: The full gate.** In order:
  - `cargo fmt`
  - `cargo clippy --workspace` — zero warnings
  - `cargo test -p feral-processes-engine balance_sim` — prices moved, so this must be looked at. A moved curve means progression changed; that is the signal, not a broken test.
  - `cargo test --workspace` — the real gate. Passing only the tests you wrote is not evidence of correctness.
  - `git diff --quiet assets/` — confirm no test left a shipped asset mutated.

- [ ] **Step 7: Commit.** Then report what you actually ran and what it printed. Do not push.

---

## Notes for whoever executes this

- **Do not reach for a fresh `Game::new` when a `dev-saves/` template would do.** `cargo run --bin savetool -- template` lists them. Testing a docked caravan by hand otherwise starts with an hour of play.
- **`FERAL_DEV_REVEAL=1`** draws the whole Stack frame; irrelevant here but in the same family as the dev switches you may want.
- If many tests fail at once with `NotFound` on an asset path, that is stale build artifacts from the old `petmud` directory name, not real failures. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — **not** a full `cargo clean`, which discards ~4 GB.
- A single-crate `-p` run and a `--workspace` run are different builds and shift the RNG stream. A seeded test can pass in one and fail in the other; that is the known trap, not a new bug.
- Registering a new `Resource` shifts bevy's query iteration order. If an untouched subsystem's test goes red right after Task 7, suspect a latent unsorted-query test there before suspecting this feature.
