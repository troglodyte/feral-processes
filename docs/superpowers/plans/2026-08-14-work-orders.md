# Work Orders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace hand-posting programs to machines with a queued work order — "3 Routine Disks" — that the base derives a production chain from and staffs itself against, upstream-first, from an explicit base-staff pool.

**Architecture:** A work order stores an item and a quantity and nothing else. Everything downstream of that — which machines are needed, in what order, who stands where, and what the status screen shows — is recomputed from live world state each tick by two pure functions (`can_progress`, `wants`) and one bevy system (`schedule_base_labour`), all in one new module under a new `game/base/` tree that Task 0 gathers the existing base subsystem into. The scheduler *drives* the existing `Task`/`assign_cronjob` mechanism rather than replacing it, so hauling, the `Stranded` marker, the walk-in and the save encoding all keep working untouched.

**Tech Stack:** Rust, standalone `bevy_ecs` 0.19, RON assets and RON save format, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-14-work-orders-design.md` — read it alongside this plan. The spec carries the *why* for every non-obvious choice here; this plan does not repeat those arguments.

## Global Constraints

Copied verbatim from the spec and from `CLAUDE.md`. Every task's requirements implicitly include this section.

- **No `SAVE_FORMAT_VERSION` bump.** Every new save field is additive, named, and `#[serde(default)]`. If you find yourself needing to remove a field or change a field's meaning under a name it keeps, stop and raise it — that needs a bump and real migration code.
- **`WorkOrder` is a named struct, never a positional tuple.** RON parses `(` in a struct position as the start of named fields, so a `Vec<(ItemId, u32)>` can never be widened. Two fields already shipped in that shape and both had to be drained into named successors.
- **The scheduler draws no RNG.** Not `GameRng`, not a local `StdRng`. Idle-staff movement is a deterministic function of `(tick, staff index)`. A per-tick draw would shift the shared stream that seeded combat and spawn tests depend on.
- **`Stock::input` may be read but never written from outside `assembler_system`.** `components::Stock`'s doc states the asymmetry that makes a chain flow one way. `can_progress` reads it; nothing in this feature writes it.
- **Reuse, don't reimplement.** `systems::produced_item`, `systems::assembly_recipe`, `collect::ORTHOGONAL`, `hauling::at_station`, `hauling::post_reach` and `tuning::INPUT_STOCK_BATCHES` already exist and are the single definitions of what they describe. A second copy of any of them is the drift trap `CLAUDE.md` records biting this repo four times. (After Task 0 the middle four live under `game::base::`; the plan writes them unqualified below.)
- **New tuning values go in `crates/engine/src/tuning.rs`** as documented `pub const`s, never inline in a formula.
- **Player-facing text says "sweep", never "raid".** The code, the enum and the `.ron` fields say raid; anything a player reads does not.
- **Gate before calling anything done:** `cargo test --workspace`, then `cargo clippy --workspace` and `cargo fmt`. Iterate with `cargo test -p feral-processes-engine <name>`.
- **TDD throughout.** Failing test first, watch it fail for the right reason, minimal implementation, watch it pass, commit. A test that passes with the fix removed is not a test — this repo shipped two vacuous ones on 2026-08-09.
- **Fixtures:** `crates/engine/src/tests/support.rs` has `work_node_parts()`, `park_at_post()`, `spawn_tamed`, `test_assets_dir`. A hand-spawned node short of `Stock` or `MachineStatus` is skipped by the query and silently produces nothing; a worker left where it was spawned is not at its station. Both read as a broken scheduler rather than a short fixture.
- **A hand-written `Task` against an assembler must set `required` to that machine's real `ticks_per_unit`.** A hand-written `1` means "finish a batch every tick".

---

## File Structure

The base subsystem gets its own directory, `crates/engine/src/game/base/`, created in Task 0 as a pure move before any new code lands.

This is a deliberate departure from the crate's existing idiom and should be understood as one. Combat is seven flat `combat_*.rs` files and the Stack five `stack_*.rs`; there is no nested subsystem directory in this crate today, and `game/base/` will be the first. The reason is anticipated growth in base interaction specifically, and the decision was the user's with the flat alternative on the table. **Do not take it as licence to nest the other subsystems** — if a later reader is weighing `game/combat/`, that is a fresh decision, not a precedent set here.

**New:**

- `crates/engine/src/game/base/mod.rs` — declares the submodules, re-exports what the rest of `game` already imports so call sites change by path prefix only.
- `crates/engine/src/game/base/work_orders.rs` — the `WorkOrder` type, chain resolution, `can_progress`, `wants`, the scheduler, the status report.
- `crates/engine/tests/work_orders.rs` — integration tests.

**Moved in Task 0** (content unchanged):

| From | To |
| --- | --- |
| `game/building.rs` | `game/base/building.rs` |
| `game/hauling.rs` | `game/base/hauling.rs` |
| `game/upkeep.rs` | `game/base/upkeep.rs` |
| `game/collect.rs` | `game/base/collect.rs` |

**Modified:**

- `crates/engine/src/components.rs` — `BaseStaff`, `StandingJob`.
- `crates/engine/src/resources.rs` — `WorkOrders`.
- `crates/engine/src/save.rs` — additive fields on `CreatureSave`, `StructureSave`, `SaveData`.
- `crates/engine/src/game/mod.rs` — swap four `mod` lines for one `pub(crate) mod base;`.
- `crates/engine/src/game/lifecycle.rs` — load-path staff absorption.
- `crates/engine/src/views.rs` — widen `drawn_on_surface_map`; the report types.
- `crates/engine/src/systems.rs` — schedule registration and chaining.
- `crates/engine/src/tuning.rs` — the feature's constants.
- `crates/app-core/src/lib.rs` — `Mode` variants.
- `crates/app-core/src/app/group_menu.rs` — the `BASE_ROWS` table.
- `crates/app-core/src/app/building.rs`, `input.rs`, `lifecycle.rs` — handlers.
- `crates/gui/src/render/building.rs`, `mod.rs`, `manifest.rs` — screens.

---

### Task 0: The `game/base/` tree

A pure move. No behaviour change, no new types, no renamed functions. Done first so every later task lands in its final home and no task carries a move inside a feature diff.

**Files:**
- Create: `crates/engine/src/game/base/mod.rs`
- Move: the four modules in the table above
- Modify: `crates/engine/src/game/mod.rs`, plus every call site the compiler names

- [ ] **Step 1: Establish the baseline.** Run `cargo test --workspace` and record that it is green *before* you touch anything. A move done on an already-red suite is unattributable.

- [ ] **Step 2: Move the four files with `git mv`,** so history follows them. `git mv` rather than create-and-delete is what keeps `git log --follow` working on files carrying years of load-bearing doc comments.

- [ ] **Step 3: Write `base/mod.rs`.** Declare the four submodules. Its doc comment should say what the directory is for and carry the note above about not taking it as a precedent for nesting combat or the Stack.

- [ ] **Step 4: Let the compiler drive the call-site fixes.** `cargo check --workspace` — warm checks run about a second. Paths change from `game::building::X` to `game::base::building::X`; nothing else changes. Resist every temptation to tidy something you pass through. This task's diff should be imports and one new file.

- [ ] **Step 5: Verify nothing moved but paths.** `git diff --stat` should show the four moves as renames with near-zero content change. If a moved file shows real line changes, back them out — they belong to a later task.

- [ ] **Step 6: Gates.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`. The suite must be green with the same test count as Step 1.

- [ ] **Step 7: Commit.** `refactor(base): gather the base subsystem under game/base`

**Explicitly out of scope for this task:** `systems.rs` holds three base machine systems (`task_progress_system`, `assembler_system`, `idle_machine_system`) that arguably belong in `base/machines.rs`. Moving them means untangling shared imports and schedule registration from a 1,486-line file, and it is not needed by anything in this plan. Leave them. If the base tree earns them later, that is its own change with its own green baseline.

---

### Task 1: The base staff pool

A program assigned to the base, persisted, disjoint from `Party`. Nothing schedules anything yet — this task is only the pool and its save round trip.

**Files:**
- Modify: `crates/engine/src/components.rs` — add `BaseStaff` marker component
- Modify: `crates/engine/src/save.rs` — `CreatureSave` gains `#[serde(default)] pub staff: bool`
- Modify: `crates/engine/src/game/lifecycle.rs` — write and restore it; absorb legacy `Task` holders
- Modify: `crates/engine/src/game/party.rs` — the two entry points
- Test: `crates/engine/tests/work_orders.rs` (new file)

**Interfaces produced:**
- `components::BaseStaff` — unit marker struct, `#[derive(Component)]`
- `Game::assign_base_staff(&mut self, worker: Entity) -> Result<(), String>`
- `Game::release_base_staff(&mut self, worker: Entity) -> Result<(), String>`
- `Game::base_staff(&self) -> Vec<Entity>` — stable order, sorted by `Entity` index

- [ ] **Step 1: Write the failing tests.** Four, each asserting one thing: assigning a program you own marks it `BaseStaff` and drops it from `Party`; assigning one you don't own is refused; releasing clears the marker; a save round trip preserves the marker. Follow the existing `tests/trade.rs` idiom — `world.get::<Stats>(e).is_none()` is how this repo asks "is this entity gone", not `World::get_entity`.

- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-engine work_orders` — expected: does not compile, `BaseStaff` undefined. A compile failure is a legitimate first red here.

- [ ] **Step 3: Implement.** The ownership check is the same one `assign_cronjob` opens with (read `Tamed::owner`, compare to `player_entity()`) — copy its shape, not its body. Staff and party are disjoint sets, so assigning removes from `Party` exactly as `assign_cronjob` already does, with the same `log_base` line about standing down.

- [ ] **Step 4: Implement the save half.** `CreatureSave.staff` is additive and `#[serde(default)]`, so a save written before this feature loads with it false. **Do not bump `SAVE_FORMAT_VERSION`.**

- [ ] **Step 5: Load-path absorption.** In `Game::load`, any tamed program holding a `Task` is marked `BaseStaff` regardless of the saved flag. This is what keeps an existing base's workers working once the scheduler lands instead of standing them all down at once. Add a test that loads a save with a hand-posted cronjob and asserts the worker came back as staff, still on its machine.

- [ ] **Step 6: Run the gates.** `cargo test -p feral-processes-engine`, then `cargo test --workspace`. A save-shape change touches app-core's tests too.

- [ ] **Step 7: Commit.** `feat(base): a program can be assigned to the base staff pool`

---

### Task 2: Chain resolution and queue-time refusal

The queue exists and refuses impossible orders. Still no scheduler — an accepted order sits there doing nothing.

**Files:**
- Create: `crates/engine/src/game/base/work_orders.rs`
- Modify: `crates/engine/src/game/mod.rs` — `pub(crate) mod work_orders;`
- Modify: `crates/engine/src/resources.rs` — `WorkOrders`
- Modify: `crates/engine/src/save.rs` — additive `SaveData` field
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces consumed:** Task 1's `Game::base_staff`.

**Interfaces produced:**
- `pub struct WorkOrder { pub item: ItemId, pub qty: u32 }` — named fields, `Serialize`/`Deserialize`, in `work_orders.rs`
- `resources::WorkOrders(pub Vec<WorkOrder>)` — `#[derive(Resource, Default)]`
- `Game::queue_work_order(&mut self, item: ItemId, qty: u32) -> Result<(), String>`
- `Game::cancel_work_order(&mut self, index: usize) -> Result<(), String>`
- `Game::work_orders(&self) -> &[WorkOrder]`
- `pub(crate) fn producer_of(game: &Game, item: &ItemId) -> Option<Entity>` — the deployed structure whose def produces `item`, via `systems::produced_item`
- `pub(crate) fn chain_break(game: &Game, item: &ItemId) -> Option<String>` — `None` if the line is whole, otherwise the player-facing sentence naming the break

- [ ] **Step 1: Write the failing refusal tests.** One per refusal, each with its own fixture so a single missing piece can't satisfy two assertions:
  - No deployed machine produces the item → names the machine ("No Disk Press deployed — that is what presses a Routine Disk.")
  - A Disk Press deployed but not orthogonally adjacent to anything producing Blank Substrate → names *which link* is missing
  - An item nothing declares as `work.produces` or `assembles.item` → refused as unproducible
  - `research_data` → refused. **Assert the reason is the chain walk, not a hardcoded item id.** It is refused because a `banked` payout never reaches an `output`, so nothing can be fed from it; a test that would pass against a `if item == "research_data"` special case is testing the wrong thing.
  - A whole three-deep line correctly laid out → accepted
  - **Cancelling.** An order cancelled by index leaves the queue, the orders behind it shift up, and cancelling an out-of-range index is refused rather than panicking. There is nothing to unwind — no per-machine targets, no reserved stock — and that absence is worth one assertion so a later reader does not add an unwind path for a state that never existed.

- [ ] **Step 2: Run and watch each fail** for the right reason. `cargo test -p feral-processes-engine work_orders`

- [ ] **Step 3: Implement `producer_of` and `chain_break`.** `chain_break` walks the item's recipe tree via `systems::assembly_recipe`, and for each assembler in it checks that some orthogonal neighbour (`game::base::collect::ORTHOGONAL`) produces each ingredient. Recursion terminates at an item with no `craftable`.

  The one non-obvious part is the banked exclusion, which must fall out rather than be special-cased. A banked item reaches no `output` (`deliver_payout` sends it straight to the bank), so `producer_of` finding a machine is not sufficient — the machine must be able to *feed* a neighbour, which is exactly what `systems::produced_item`'s doc already narrows for. Read that doc before writing this.

- [ ] **Step 4: Implement the queue.** `queue_work_order` runs every refusal *before* pushing anything. Same ordering argument `use_symlink` makes about `clear_stack` and `install_routine` makes about the disk: nothing is written until every check has passed.

- [ ] **Step 5: Save round trip.** Additive `#[serde(default)]` field on `SaveData`. Two tests: orders round-trip; and a save file written before this feature — strip the field back out of a real one rather than hand-writing a string, the way `a_save_written_before_contracts_existed_still_loads` does — still loads.

- [ ] **Step 6: Gates.** `cargo test --workspace`

- [ ] **Step 7: Commit.** `feat(base): queue a work order, refusing a broken production line`

---

### Task 3: `can_progress` and `wants`

The two pure functions the scheduler and the status screen both run. Testable with no scheduler and no staff.

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs`
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces consumed:** Task 2's `producer_of`, `WorkOrder`.

**Interfaces produced:**
- `pub(crate) fn can_progress(game: &Game, machine: Entity) -> bool` — `&Game`, not `&World`, so all five functions in this module take the same first argument
- `pub(crate) fn wants(game: &Game, order: &WorkOrder) -> Vec<(Entity, u32)>` — machine and depth, deepest first, each machine appearing once
- `pub(crate) fn base_holding(game: &Game, item: &ItemId) -> u32` — units across every Depot (`StructureDef::stores`) and every machine `output`

- [ ] **Step 1: Write the failing tests.**
  - `can_progress` is false for a machine whose `output_room()` is 0
  - `can_progress` is true for an extractor with room
  - `can_progress` is false for an assembler with empty input and no adjacent feeder holding the ingredient
  - `can_progress` is **true** for an assembler with empty input whose adjacent feeder *does* hold it — this is the case that matters, because staffing the machine is what makes it pull
  - `wants` on an empty base orders Mining Node before Lathe before Disk Press
  - `wants` returns a shared feeder once, at its deepest position — build this with a **modded two-ingredient assembler** under `test_assets_dir`, since no shipped recipe is more than one ingredient
  - `base_holding` counts a Depot and a machine output, and does not count the player's inventory

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement `can_progress`.** Output has room, and for an assembler, input holds at least one batch of each ingredient *or* every shortfall is available in an adjacent feeder's output. The batch size is `per_batch * tuning::INPUT_STOCK_BATCHES` — read it from the constant, do not restate the number.

- [ ] **Step 4: Implement `wants`.** Find the producer of the ordered item; if it can progress it wants a body at the current depth, otherwise recurse into each ingredient it is short of at depth + 1. Dedupe keeping the deepest, then sort deepest-first. Sort must be **total and stable** — `(depth, x, y, entity)` — for the reason `assembler_system` sorts its machines: bevy's query iteration order is not stable, and two machines at equal depth resolving differently between runs is a flaky test and a base that behaves differently after a reload.

- [ ] **Step 5: Run, watch pass, then check the tests aren't vacuous.** Delete the dedupe and confirm the shared-feeder test fails; delete the sort and confirm the ordering test fails. If either still passes, the test is asserting nothing.

- [ ] **Step 6: Gates.** `cargo test --workspace`

- [ ] **Step 7: Commit.** `feat(base): derive which machines a work order needs staffed`

---

### Task 4: The scheduler

Staff get posted and unposted automatically. This is the task where the feature becomes real.

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — `schedule_base_labour`
- Modify: `crates/engine/src/game/base/building.rs` — extract the posting body
- Modify: `crates/engine/src/systems.rs` — register and chain
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces consumed:** Tasks 1–3.

**Interfaces produced:**
- `pub(crate) fn post_worker(...) -> Result<(), String>` in `building.rs` — everything `assign_cronjob` does after its refusals: `Position` write, `work_ticks_for`, `Party` removal, `displace_task_holder`, `Task` insert. `assign_cronjob` becomes its refusals plus a call to this; the scheduler calls it directly.
- `pub(crate) fn schedule_base_labour(...)` — a bevy system

- [ ] **Step 1: Write the failing behaviour tests.** Assert *behaviour*, never that a function was called — the regression to head off is a later path skipping the scheduler, and only behaviour catches that.
  - One staff member, empty base, three-deep chain → posted to the Mining Node
  - That machine clogs → the same body ends up on the Lathe, then the Disk Press
  - Two and three staff distribute across the chain without duplicating a machine
  - An order whose quantity is already in a Depot completes on the first tick, posting nobody, and is popped from the queue with a log line
  - **Anti-thrash:** a posted worker whose machine still wants a body does not move when an unrelated buffer changes
  - **A stalled front order does not block the queue:** demolish a machine mid-order and the order behind it is still worked, the stalled one still listed
  - Zero base staff → orders queue and report, nothing is posted, nothing panics

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Extract `post_worker` first, as a pure refactor.** Run the full suite before touching anything else and confirm it is still green. Doing this as its own green step is what makes the next step's failures attributable.

- [ ] **Step 4: Implement the scheduler.** The five steps are in the spec and their order is load-bearing: complete-or-stall the front order, build wants, leave settled staff alone, unpost the unwanted, fill from idle only. Steps 3 and 4 are the anti-thrash rule and are not optional.

- [ ] **Step 5: Register it chained before `task_progress_system`,** so a body assigned this tick progresses this tick. Bevy can see the `Task` write conflict but not the disjointness — an arbitrary-but-fixed order is not the same as a stated one, which is why `task_progress_system` and `assembler_system` are already `.chain()`ed.

- [ ] **Step 6: Run the full suite and expect unrelated failures.** Registering a system and a resource shifts query iteration order, and this repo has latent unsorted-query tests that surface exactly then. A failure in an untouched subsystem here is probably that, not your regression — read the failing test before assuming otherwise.

- [ ] **Step 7: Gates.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`

- [ ] **Step 8: Commit.** `feat(base): staff machines automatically from the work order queue`

---

### Task 5: Standing jobs

Research, trace, power and guard posts — filled only by a body no order needs.

**Files:**
- Modify: `crates/engine/src/components.rs` — `StandingJob { work: bool, guard: bool }`
- Modify: `crates/engine/src/save.rs` — two additive `StructureSave` bools
- Modify: `crates/engine/src/game/base/work_orders.rs` — append to the want list
- Modify: `crates/engine/src/game/base/building.rs` — the setter
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces produced:**
- `Game::set_standing_job(&mut self, structure: Entity, work: bool, guard: bool) -> Result<(), String>`
- `Game::standing_job(&self, structure: Entity) -> Option<(bool, bool)>`

- [ ] **Step 1: Write the failing tests.**
  - A standing work job on a Research Node is filled when no order needs the body
  - It is dropped the moment an order does need it, and re-filled when the order completes
  - A standing guard produces nothing but survives a sweep, and re-fills after one
  - Setting a guard job on an unraidable structure (Home) is refused — `assign_guard` already carries this refusal and its reason; reuse it rather than restating it
  - Round trip: the flags survive a save, and a structure demolished and rebuilt on the same tile comes back **without** them

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement.** `StandingJob` lives on the structure entity, deliberately the opposite of `BuybackLedger` — a shelf outlives its building on purpose, a job order must not.

- [ ] **Step 4: Append standing jobs to the want list** after the worked order's wants, in a stable tile order, at the lowest priority.

- [ ] **Step 5: Gates.** `cargo test --workspace`

- [ ] **Step 6: Commit.** `feat(base): standing jobs keep a machine or a guard post filled`

---

### Task 6: Idle staff on the map

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — the parking walk
- Modify: `crates/engine/src/views.rs` — widen `drawn_on_surface_map`
- Modify: `crates/engine/src/tuning.rs` — the parking constants
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces produced:**
- `pub(crate) fn park_tile(center: Position, index: usize, tick: u64) -> Position`
- `views::drawn_on_surface_map(is_tamed: bool, position_is_honest: bool) -> bool` — **signature change**, same two call sites

- [ ] **Step 1: Write the failing tests.**
  - `park_tile` is a pure function of its arguments — same inputs, same output, called twice
  - Two staff at the same tick get different tiles
  - A parked staff member's tile is inside the build radius and not on a tile a `Structure` occupies
  - An idle staff member is drawn on the surface map; a party companion still is not
  - **`Game::find_target_in_direction` can now name an idle staff member** — the map and the inspector must stay the same set, which is the whole reason `drawn_on_surface_map` is one function
  - **No RNG:** run the same seeded fixture with and without idle staff present and assert an unrelated seeded roll is unchanged. This is the test that catches the stream shift, and it is the one that would have caught the three that already happened.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement `park_tile`** as a deterministic function of `(center, index, tick)`. `game::stack::ring_offset` is this repo's single definition of a Chebyshev ring — read it and reuse it rather than writing ring maths again. **No `GameRng`, no `StdRng`, no `Date`/clock.**

- [ ] **Step 4: Widen `drawn_on_surface_map`.** Change the parameter's meaning from "away from post" to "position is honest", and update both call sites — `render/base.rs` and `Game::find_target_in_direction`. Its doc comment states the two-callers rule; update the doc in the same edit.

- [ ] **Step 5: Gates.** `cargo test --workspace`. Watch for map-pane and examine tests specifically.

- [ ] **Step 6: Commit.** `feat(base): idle base staff loiter on the map instead of being invisible`

---

### Task 7: The status report

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs`
- Modify: `crates/engine/src/views.rs` — the report types
- Test: `crates/engine/tests/work_orders.rs`

**Interfaces produced:**
- `pub struct WorkOrderReport { pub item: ItemId, pub label: String, pub have: u32, pub target: u32, pub stalled: bool, pub machines: Vec<WorkOrderMachine> }`
- `pub struct WorkOrderMachine { pub label: String, pub worker: Option<String>, pub short_of: Option<String>, pub depth: u32 }`
- `Game::work_order_report(&self) -> Vec<WorkOrderReport>`

- [ ] **Step 1: Write the failing tests.** The report's machine list is the same list `wants` returns, in the same order, for the same world — assert them against each other rather than against a hardcoded expectation, so the two cannot drift. A stalled order reports `stalled: true` and names the missing machine. A base with no staff reports the orders normally rather than reporting them stalled — those are different errands and the screen must not conflate them.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement by calling `wants`,** not by walking the chain a second time. Per `CLAUDE.md`, a claim that two places use one rule has to be a call and not a comment; the copy that drifts is the one nobody runs.

- [ ] **Step 4: Gates.** `cargo test --workspace`

- [ ] **Step 5: Commit.** `feat(base): work order status derived from the scheduler's own walk`

---

### Task 8: app-core — menus and handlers

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode::WorkOrders`, `Mode::BaseStaff`; remove `Mode::Cronjob`, `Mode::Guard`
- Modify: `crates/app-core/src/app/group_menu.rs` — the `BASE_ROWS` table
- Modify: `crates/app-core/src/app/building.rs`, `input.rs`, `lifecycle.rs`
- Modify: `crates/engine/src/game/base/building.rs` — narrow `assign_cronjob`/`assign_guard` to `pub(crate)`
- Test: app-core's existing test module

**Interfaces consumed:** Tasks 1–7.

- [ ] **Step 1: Write the failing tests.** The base menu no longer offers "Assign a cronjob" or "Post a guard"; it offers "Work orders" and "Base staff". Both new rows are `surface_only: true`. A row is hidden when the screen behind it would be empty — the `available` closure asks the same question the screen asks, which is the rule that table exists to keep checkable.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Make `assign_cronjob` and `assign_guard` `pub(crate)`.** The compiler is the barrier; removing the menu row is only the convention, and `CLAUDE.md` is explicit that the private-visibility kind of barrier is what actually holds a rule. Expect this to break app-core's build — that breakage is the point, and it is your list of call sites to convert.

- [ ] **Step 4: Rewire the handlers** to the new engine entry points.

- [ ] **Step 5: Check `surface_only` against the engine.** Any new action reaching zone-map state through `Position` must be a `Game::require_surface` caller *and* carry `surface_only: true`, because `Position` is pinned to the surface entrance tile while the party is underground. `queue_work_order` and the staff assignment both qualify. `work_order_report` is read-only and does not act — but it *claims something about where the base is*, which is the test the `find_target_in_direction` entry in `CLAUDE.md` states, so treat it the same way.

- [ ] **Step 6: Gates.** `cargo test --workspace`

- [ ] **Step 7: Commit.** `feat(base): work order and staff menus replace manual posting`

---

### Task 9: gui — the screens

**Files:**
- Modify: `crates/gui/src/render/building.rs` — the work order and staff screens
- Modify: `crates/gui/src/render/mod.rs` — mode dispatch
- Modify: `crates/gui/src/render/manifest.rs` — if it names the removed modes

**Interfaces consumed:** Task 7's report types, Task 8's modes.

- [ ] **Step 1: Draw through `Painter` only.** `crates/gui/src/paint.rs` is the one file that names a graphics library; the ~3,000 lines under `render/` know nothing about the backend, which is what made the macroquad→Bevy swap touch five files and no drawing code. Do not reintroduce a direct backend call.

- [ ] **Step 2: Row counts come from app-core, rows are drawn by gui.** The history screen and the structure roster already work this way. A renderer that rebuilt the list itself would be right until the first hidden row and then open a different screen from the one under the highlight.

- [ ] **Step 3: Reuse `render/mod.rs::fusion_color` and `popup.rs::fusion_row`** if any row shows a fused program. Eleven menus already call `fusion_row`; a twelfth goes through it.

- [ ] **Step 4: Check row width headlessly.** `paint::with_painter` measures real text, and `draw_row` clamps vertically but never horizontally — two shipped screens already overflow their popup because nobody measured. The widest row here is a machine line carrying a machine name, a worker name and a shortfall; measure it against the popup body before calling this done.

- [ ] **Step 5: Gates.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`

- [ ] **Step 6: Commit.** `feat(gui): work order queue, status and base staff screens`

---

### Task 10: Documentation and release

- [ ] **Step 1: `CHANGELOG.md`** — a new `## X.Y.Z` section. Which digit moves is decided by the changelog's own preamble; read it rather than guessing. "Breaking" means a player's save stops loading, and this feature deliberately does not.
- [ ] **Step 2: Bump the workspace version** in the root `Cargo.toml`. One release per change that lands on `main`.
- [ ] **Step 3: `CLAUDE.md` load-bearing seams.** Add an entry for the derived-never-stored decision and one for the scheduler's anti-thrash rule; **amend** the existing `drawn_on_surface_map` entry, which currently states the narrower rule this task widened, and the `assign_cronjob` entry describing a posting the player makes by hand.
- [ ] **Step 4: Do not touch `docs/manual.md` or the root `README.md`.** Both are explicitly carved out of the doc obligation.
- [ ] **Step 5: Grep for claims this change falsifies** — anything describing cronjob posting as a player action.
- [ ] **Step 6: Full gate.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`
- [ ] **Step 7: Commit.** `docs: work orders`

---

## What is not gated, and must be played

`balance_sim` is RNG-free and models no base — it cannot see a work order, a posted program or a production chain. The arena models player combat, not production. **Nothing in this feature is covered by an automatic balance gate**, and the question it raises — whether automating staffing makes the base too productive for the zone curve — is answerable only by playing it.

A green suite is not evidence of play. Say so plainly when reporting this done, and offer to launch it:

```sh
cargo run -- --template chains     # a base with production lines already up
```

Three things to watch for that no test here can catch: whether a body visibly cycling between machines reads as *working* or as *dithering*; whether the status screen answers "why is nothing happening" on the first read; and whether losing manual posting costs a kind of control that mattered.
