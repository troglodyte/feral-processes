# Research as a Project Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the banked-points research economy with a single selected project the base works: Research Nodes feed progress into it, its materials become ordinary High-band work orders, and a material the base cannot make refuses the selection outright.

**Architecture:** A new saved `resources::ActiveResearch` holds the chosen node and a per-node progress map. The project supplies a want to `schedule_base_labour` the way an order does, so Research Nodes staff themselves while it runs and read `Idle` when none does — no new `MachineStatus`. `systems::deliver_payout` gains one branch ahead of its banked branch that routes the research currency into progress instead of the player's bank. Materials are filed through the ordinary `Game::queue_work_order` and carry a `for_research` provenance flag so they can be withdrawn when the project ends.

**Tech Stack:** Rust 2024, `bevy_ecs 0.19` (engine only), serde + RON for assets, bincode for the save. Four-crate Cargo workspace.

**Spec:** `docs/superpowers/specs/2026-09-11-research-as-a-project-design.md` — read it first; this plan argues from it and does not repeat its reasoning.

## Global Constraints

Copied verbatim from `CLAUDE.md` and the spec. Every task's requirements implicitly include this section.

- **No `SAVE_FORMAT_VERSION` bump.** The save is field-named RON inside bincode; every new field is additive behind `#[serde(default)]`. If you find yourself wanting a bump, you have gone off-plan — stop and say so.
- **Content stays data.** Nothing in this plan may hardcode an item, structure or research id in Rust. `ItemDb::research_currency()` is how you name Research Data; `work_orders::producers_of` is how you find a Research Node. `assets/research/*.ron` is untouched by every task except Task 11.
- **The engine's `Game` is the whole public API.** `crates/gui` never touches the ECS `World`. app-core and gui read derivations (`Game::research_nodes`, `views::*`), never world state.
- **`cargo fmt` and `cargo clippy --workspace` after every change.** Fix warnings, never silence them.
- **The full suite is the final gate of every task:** `cargo test --workspace`. Per-step, use `cargo test -p feral-processes-engine <name>` — it is ~6.7s where the workspace is minutes.
- **RNG:** nothing in this feature may draw from `resources::GameRng`. A test asserting the tick draws nothing is in Task 6.
- **Every refusal lands before anything is written**, and is asserted **per refusal** — a single test over one of six paths passes against the five that never write anyway.
- **Vocabulary:** you *run* or *invoke* a routine; the unit of health is Integrity; "Raid" is code's word and "GC Entropy Sweep" is the player's. Research Data keeps its name.
- **Commit at every green step.** Branch is `claude/research-as-project`; it already exists and holds the spec. **Do not push.**

**On this plan's shape:** code blocks are deliberately rare. `CLAUDE.md`'s
process-weight rule is that a plan hands a subagent the file list, the
interface it must produce, the intent of each test and the gates to run —
**not finished code it will merely re-emit**. Where a block does appear
(Task 4's scheduler line, Task 5's gate ordering) it is because the thing is
genuinely non-obvious: an ordering constraint or a placement whose
correctness is not visible from the surrounding code. Everything else is
yours to write, against the neighbouring code the task names.

## File structure

| File | Responsibility after this plan |
|---|---|
| `crates/engine/src/resources.rs` | `ActiveResearch` — the chosen node and the per-node progress map |
| `crates/engine/src/save.rs` | two additive `SaveData` fields and their round trip |
| `crates/engine/src/systems.rs` | `deliver_payout`'s research-currency branch |
| `crates/engine/src/game/unlocks.rs` | `select_research`, `abandon_research`, `settle_research`, `research_nodes` — the whole research door |
| `crates/engine/src/game/base/work_orders.rs` | `WorkOrder::for_research`, `with_research`, `withdraw_research_orders`, `research_wants` and its place in the ladder |
| `crates/engine/src/game/base/stock.rs` | `spend_from_base` — the base-wide take |
| `crates/engine/src/game/inspection.rs` | the attention row for idle Research Nodes |
| `crates/engine/src/views.rs` | `ResearchStatus`, `ResearchState::Active` |
| `crates/engine/src/game/lifecycle.rs` | dropping a legacy save's banked Research Data |
| `crates/app-core/src/app/progression.rs` | `handle_research_key` — select and abandon |
| `crates/gui/src/render/progression.rs` | the research screen and graph view |

## Task order and why

Each task leaves `cargo test --workspace` green. The old purchase path (`Game::unlock_research`, paying out of the bank) stays alive and working through Task 5, is starved by Task 6, and is deleted in Task 8. Do not reorder: cutting the bank before the project can complete leaves the tree unreachable, and no test would catch it because every research test would still be asserting the old path.

---

### Task 1: `ActiveResearch` and its save round trip

**Files:**
- Modify: `crates/engine/src/resources.rs` — add the resource beside `Research`
- Modify: `crates/engine/src/game/lifecycle.rs` — insert it at `Game::new`, write it in `Game::save`, restore it in `Game::load`
- Modify: `crates/engine/src/save.rs` — two `SaveData` fields
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `resources::ActiveResearch { pub id: Option<ResearchId>, pub progress: HashMap<ResearchId, u32> }`, `#[derive(Resource, Default)]`
  - `SaveData::active_research: Option<ResearchId>` and `SaveData::research_progress: Vec<(ResearchId, u32)>`, both `#[serde(default)]`

**What to build**

The resource holds the chosen node and a progress figure **per node**, not just the active one — abandoning a 540-cost project and returning to it is not destructive. Progress is in units of the research currency, the same denomination `ResearchDef::cost` is in.

`research_progress` is **sorted by id on write**, for the reason `SaveData::researched` is sorted: the encoded bytes must not differ run to run. Read `save.rs:1254-1270` for the existing comment and match it.

Nothing reads either field yet. This task exists on its own because a save-shape change is the one thing in this plan a reviewer should be able to reject independently.

**Steps**

- [ ] **Step 1: Write the failing tests.** In `crates/engine/src/tests/research.rs`, two tests:
  - `an_active_project_and_its_progress_survive_a_save_round_trip` — set `ActiveResearch` by hand on a `Game::new`, save to a temp path, load, assert both the id and a progress entry come back. Use the save/load fixtures the module's neighbours already use; **do not** write a RON round-trip test, which cannot see a skipped field and would be vacuous here.
  - `a_save_written_before_this_change_loads_with_no_project` — build a `SaveData` with the two fields defaulted, load it, assert `ActiveResearch::default()` and that `resources::Research` is untouched.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine research::an_active_project` — expect a compile failure naming `ActiveResearch`.
- [ ] **Step 3: Add the resource** in `resources.rs`, doc-commented with why progress is per node.
- [ ] **Step 4: Wire `Game::new`, `Game::save`, `Game::load`** in `game/lifecycle.rs`. Follow exactly how `resources::Research` is handled three lines away — insert, write sorted, restore.
- [ ] **Step 5: Add the two `SaveData` fields** in `save.rs`, each `#[serde(default)]`, each doc-commented, and add them to the `SaveData` constructor around line 1807.
- [ ] **Step 6: Green.** `cargo test -p feral-processes-engine research`, then `cargo fmt && cargo clippy --workspace`, then `cargo test --workspace`.
- [ ] **Step 7: Commit.** `feat(research): ActiveResearch — the chosen node and per-node progress`

---

### Task 2: `WorkOrder::for_research`

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — the `WorkOrder` struct (around line 89), its `impl` (around line 130), and a new withdrawal function
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `WorkOrder::for_research: bool`, `#[serde(default)]`, public
  - `WorkOrder::with_research(self) -> Self` — a setter beside `with_priority`
  - `Game::withdraw_research_orders(&mut self)` — `pub(crate)`, drops every queue entry carrying the flag

**What to build**

One bool on `WorkOrder`. The module header's rule is *"a work order stores what was asked for, never how it will be done"* — this is **provenance, not a plan**: it says who asked, never which machines run or how far along they are. Say that in the field's doc comment, because the next reader will check.

It is `#[serde(default)]` and **not** `#[serde(skip)]` — unlike `announced_stalled` directly above it. A project abandoned after a reload has to be able to take its orders with it, so the flag has to survive the round trip. Note the trap that makes this worth a test of its own: a `#[serde(skip)]` field is invisible to a RON round-trip test, so **assert this with a save→load test, not a RON one**.

`with_research` is a setter, not a third constructor, for the documented reason `with_priority` is one: a band and a provenance are values *on* an order, not kinds of order.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/work_orders.rs`:
  - `a_research_order_keeps_its_provenance_across_a_save` — file one through `queue_work_order`, save, load, assert the flag survived.
  - `withdrawing_research_orders_leaves_the_players_own` — queue one research-flagged order and one ordinary order for the **same item**, withdraw, assert exactly the ordinary one remains. This is the test that matters; matching by item instead of by flag is the bug it exists to catch.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Add the field, the setter and `withdraw_research_orders`.** The withdrawal is a `retain` on `resources::WorkOrders`. It logs nothing — withdrawal is bookkeeping the player did not ask for, and `cancel_work_order`'s own line would be a lie about who cancelled it.
- [ ] **Step 4: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 5: Commit.** `feat(work-orders): an order can be filed on behalf of research`

---

### Task 3: `select_research` and its six refusals

**Files:**
- Modify: `crates/engine/src/game/unlocks.rs`
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Task 1's `ActiveResearch`, Task 2's `with_research`.
- Produces:
  - `Game::select_research(&mut self, id: &str) -> Result<(), String>`
  - `Game::abandon_research(&mut self) -> Result<(), String>`
  - `Game::research_block(&self, def: &ResearchDef) -> Option<String>` — `pub(crate)`; the shared "can this base ever work this node" question, so the screen and the refusal cannot disagree

**What to build**

`select_research` refuses in this order, **every refusal before anything is written**:

1. `is_game_over` / `has_active_battle` / `require_base` — as `unlock_research` does today, plus `require_base` because this now reads which machines are standing and they stand in base space (`queue_work_order`'s own reason).
2. Unknown id; already researched.
3. Missing prerequisites (`Game::missing_prereqs`), then the zone gate (`Game::research_zone_gate`). The existing order, unchanged — the spec explains why zone comes after prereqs and before cost.
4. A project is already active. Name it, and say that abandoning is how you change your mind.
5. **No Research Node deployed.** `work_orders::chain_break` cannot answer this: it refuses every banked item by construction and names `research_data` in its own doc as the example. So this is `work_orders::producers_of(self, &self.research_currency()).is_empty()`, reported with the two-sentence split `makeable_by` already uses — *"No Research Node deployed — that is what makes Research Data."*
6. **Each material through `work_orders::chain_break`**, in the order the file lists them, refused with that function's sentence **verbatim**. Do not reword it: the same sentence appears on the work-order screen, and two spellings of one refusal is the drift this repo keeps recording.

Refusals 5 and 6 are what `research_block` returns, so Task 7's screen marks a row blocked with the same call that refuses it — `Game::orderable_items`' rule one rung up.

On success: write `ActiveResearch::id`, then file one `WorkOrder::batch(item, need).with_priority(OrderPriority::High).with_research()` per material line through `Game::queue_work_order`, then log. Filing goes **through** `queue_work_order` and not around it, so every log line and every refusal that door owns still applies.

**Filed once, at selection — never topped up.** Do not add a per-tick refile; it would make `cancel_work_order` a no-op on exactly the orders a player most wants to intervene in.

`abandon_research` clears `ActiveResearch::id`, calls `withdraw_research_orders`, **keeps** the progress entry, and logs. It refuses only when no project is active.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/research.rs`. One test **per refusal** — six of them — each asserting the refusal *and* that nothing was written: `ActiveResearch::id` still `None` **and** `resources::WorkOrders` still empty. A single test over one path passes against the five that never write anyway; that is why this is six tests. Plus:
  - `selecting_files_one_high_order_per_material_line` — assert count, item, qty, `priority == High`, `for_research`.
  - `a_material_with_no_producer_names_the_missing_machine` — assert the message is `chain_break`'s own string for that item, fetched by calling `chain_break` in the test rather than hardcoding the prose.
  - `building_the_missing_machine_makes_the_same_selection_succeed` — the reachability half. Without this the refusal could be permanent and every test above would still be green.
  - `abandoning_withdraws_the_projects_orders_and_keeps_its_progress` — and leaves a player's own order for the same item standing.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement `research_block`**, then `select_research`, then `abandon_research`. Leave `unlock_research` exactly as it is — it is deleted in Task 8, and until then it is what keeps the tree reachable.
- [ ] **Step 4: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 5: Commit.** `feat(research): select a project, and refuse one the base cannot work`

---

### Task 4: the scheduler staffs the project

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — a new `Game::research_wants` and one line in `schedule_base_labour`
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Consumes: Task 1's `ActiveResearch`.
- Produces: `Game::research_wants(&self) -> Vec<(Entity, TaskKind)>` — private to the module.

**What to build**

`research_wants` returns every deployed Research Node while a project is active, and an empty `Vec` when none is. Find them with `producers_of(self, &self.research_currency())`, which already sorts by tile, so the result is deterministic without a second sort.

It returns `(Entity, TaskKind)` with `TaskKind::GatherResource` — the shape `standing_wants` and `build_wants` return, **not** the `(Entity, u32)` depth pairs `settle_orders` returns. A Research Node is the top of its own line and has no recipe tree behind it to measure a depth against.

**Placement — the one load-bearing line in this task.** In `schedule_base_labour` (from line 840), the ladder is `build_wants` → `fuel_wants` → `settle_orders` → `standing_wants` → `dig_wants`, and **the priority is the position in that list** because `truncate` cuts from the end. Research goes **between `fuel_wants` and `settle_orders`**:

```rust
wanted.extend(self.fuel_wants());
// The player picked this node; it outranks the queue they filed and
// forgot. Below a build and below keeping the lights on, above every
// order and every standing job.
wanted.extend(self.research_wants());
wanted.extend(self.settle_orders().into_iter().map(...));
```

**Do not touch `queue_is_empty`** (around line 1137). It is tempting: when a project ends, the want disappears and the early return leaves the body standing at the Research Node. That is the documented, deliberate behaviour of a base whose queue has run dry — the same thing that happens when a work order completes — and the body being already in place when the next project is picked is a feature. Changing it stands a whole base down on the first tick after a load. The guard's own comment block says this; read it before deciding you know better.

Note also that `standing_wants`' loop already skips an entity present in `wanted`, so a hand-posted `StandingJob` on a Research Node dedupes against this for free. It stays legal and still works.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/work_orders.rs`:
  - `a_research_node_is_staffed_while_a_project_runs` — deploy a Research Node with `spawn_structure_at`, give the base a body, select a project, tick, assert a `Task` on the body targeting that node.
  - `no_project_staffs_no_research_node` — the same base with nothing selected; assert the node's `MachineStatus` is `Idle` and no body is posted there. This is the assertion that keeps the "a Research Node has no full state" seam intact.
  - `a_research_want_outranks_a_work_order` — one body, one Research Node, one ordinary order whose machine is also staffable; assert the body lands on the Research Node.
  - `ending_a_project_frees_its_body_while_the_queue_still_has_work` — abandon with another order outstanding; the Research Node leaves `wanted`, the pass proceeds past `queue_is_empty`, and the diff frees the body onto the order. This is the ordinary case.
  - `ending_a_project_on_a_run_dry_base_leaves_the_body_standing` — abandon with **nothing else queued**; assert the body stays. It looks like a bug and is not: it is the documented run-dry behaviour the early return exists for, and the body being already in place when the next project is picked is what it buys. Written as a test so nobody "fixes" it later. Read `queue_is_empty`'s comment block before touching either of these.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement `research_wants` and insert the one line**, with the comment above.
- [ ] **Step 4: Green**, per-crate then workspace, with fmt and clippy. Watch `crates/engine/src/tests/hauling.rs` and `base_space.rs` — a new want in the ladder shifts what gets posted on bases those tests build.
- [ ] **Step 5: Commit.** `feat(research): the active project staffs the base's Research Nodes`

---

### Task 5: the bill is paid from the base, and the project completes

**Files:**
- Modify: `crates/engine/src/game/base/stock.rs` — `spend_from_base`
- Modify: `crates/engine/src/game/unlocks.rs` — `settle_research`
- Modify: `crates/engine/src/game/lifecycle.rs` — call it from `tick_inner`
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces:
  - `stock::spend_from_base(game: &mut Game, bill: &[(ItemId, u32)]) -> bool` — `pub(crate)`; takes the whole bill or nothing
  - `Game::settle_research(&mut self)` — `pub(crate)`; one tick of the completion check

**What to build**

`spend_from_base` walks `stock::output_buffers` in the order that function already fixes and routes **every** take through `hauling::take_from`, which is the one way a unit leaves a `Stock`. It checks the whole bill against `work_orders::base_holding` **before a single unit moves** — `commit_caravan_basket`'s rule, and the reason this is two passes and not a take-as-you-go loop that would strand goods on a shortfall. Returns `false` and moves nothing if any line is short.

`settle_research` runs once a tick from `Game::tick_inner`. Both gates:

```
progress >= def.cost  &&  spend_from_base(self, &def.materials)
```

`&&` short-circuits, so the bill is only spent once the progress gate has passed — get the order right or a project spends its materials while still half-researched.

On completion, in this order: insert into `resources::Research`; log `"Research complete: {name}."`; run the **existing** `unlocks_abilities` and `unlocks_tools` loops **unchanged** (lift them out of `unlock_research` into a shared private helper both call, rather than copying them — a copy here is the pattern `CLAUDE.md` records biting this repo four times); clear `ActiveResearch::id`; **remove** the node's `progress` entry; call `withdraw_research_orders`.

Where in `tick_inner`: beside `schedule_base_labour`, **after** it, so a body assigned this tick has already delivered before the check runs.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/research.rs`:
  - `progress_alone_does_not_complete_a_project` — full progress, empty shelves; assert not researched and **nothing consumed**.
  - `a_full_bill_alone_does_not_complete_a_project` — full shelves, zero progress; assert not researched and nothing consumed. These two are the "both gates" pair and neither is redundant.
  - `a_completed_project_consumes_its_bill_from_a_depot_the_player_is_nowhere_near` — the whole point of moving off the pack. Stand the player somewhere else entirely.
  - `completing_clears_the_project_drops_its_progress_row_and_withdraws_its_orders`.
  - `completing_grants_the_nodes_abilities_and_tools` — the shared-helper regression. Pick a shipped node that declares both.
  - `settling_research_draws_no_rng` — snapshot `resources::GameRng`'s state, tick a base with an active project, assert it is unchanged. Nothing in this feature may shift the seeded stream.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement `spend_from_base`.**
- [ ] **Step 4: Extract the unlock side effects** out of `unlock_research` into a private helper, leaving `unlock_research` calling it. Run the existing research suite — it must stay green, since nothing about the old path changed.
- [ ] **Step 5: Implement `settle_research` and call it from `tick_inner`.**
- [ ] **Step 6: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 7: Commit.** `feat(research): a project completes on progress and a bill the base pays`

---

### Task 6: Research Data becomes progress

**Files:**
- Modify: `crates/engine/src/systems.rs` — `deliver_payout` (line 476)
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Task 1's `ActiveResearch`.
- Produces: no new signature. `deliver_payout` gains a parameter or a caller-side branch — see below.

**What to build**

This is the cut: after this task the bank stops filling and `unlock_research` becomes unpayable. It sits here, after Task 5, because the project path has to be able to complete before the purchase path is starved — otherwise the tree is unreachable and no test catches it.

The new branch goes **ahead of** the existing banked branch, because it is narrower: the research currency is banked, so an order-of-checks mistake sends it to the bank and the feature silently does nothing. Read `deliver_payout`'s current shape before editing; it takes `bank: Option<&mut Inventory>` and is called from a bevy system with no `Game`. The research currency is `ItemDb::research_currency()`, which that system already has the `ItemDb` to ask.

Behaviour:
- Research currency **and** a project active → add to `ActiveResearch::progress[id]`, **saturating at the active node's `cost`**. Return what landed.
- Research currency, **no** project → return 0. The existing "landed nothing" path, already what a full `Stock` does. Silent by design: the node is `Idle` in every case but a hand-posted `StandingJob`, and that is the player's own instruction.
- Anything else → unchanged.

`ActiveResearch` is a `Resource`, so the system needs it in its parameter list. Adding a `Resource` to a system's signature **shifts bevy's query iteration order** — that is a known trap in this repo and it surfaces as seed-luck failures in unrelated tests. Expect them; do not chase them as bugs until you have confirmed the failure is real by probing to ground.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/research.rs`:
  - `a_posted_program_feeds_the_active_project` — deploy a Research Node, staff it, select a project, tick, assert `progress` rose and the player's `Inventory` count of the research currency did **not**.
  - `progress_saturates_at_the_nodes_cost` — run well past it.
  - `with_no_project_a_research_cycle_lands_nowhere` — hand-post via `set_standing_job`, tick, assert no progress anywhere and no bank.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement the branch.**
- [ ] **Step 4: Fix the fallout.** The existing `tests/research.rs` tests that assert the bank fills (`a_banked_item_is_not_an_inventory_row` and its neighbours around lines 76–160) now fail. **Do not delete them and do not use `grant_research_data` to prop them up.** The banked-item *mechanism* is still real and still worth testing — retarget them at the research currency's `banked` flag and the inventory filter, which is what they were actually about, or move them to a fixture item. Say in your report which you did.
- [ ] **Step 5: Green**, per-crate then workspace, with fmt and clippy. Expect churn in seeded tests; `cargo test -p feral-processes-engine` and `--workspace` are different builds and may disagree — run both.
- [ ] **Step 6: Commit.** `feat(research): Research Data is a project's progress, not a bank`

---

### Task 7: the screen's data

**Files:**
- Modify: `crates/engine/src/views.rs` — `ResearchStatus`, `ResearchState`
- Modify: `crates/engine/src/game/unlocks.rs` — `research_nodes`
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Tasks 1, 3, 5.
- Produces:
  - `ResearchState::Active` — a fourth variant
  - `ResearchStatus::progress: u32` and `ResearchStatus::blocked_by: Option<String>`
  - `ResearchMaterial::have` re-sourced to `work_orders::base_holding`

**What to build**

**Additive only.** `ResearchStatus::affordable` stays for now and is removed in Task 9, once gui stops reading it — removing it here breaks the workspace build and nothing can be committed green.

`ResearchState` gains `Active`, and `research_nodes` sorts it **first**: `Active`, `Available`, `Locked`, `Unlocked`. A variant rather than a flag beside the enum, for the reason `Locked`'s two reasons are *fields* rather than variants, read the other way round: these four really are disjoint, so a `bool` alongside would be a second thing to keep in step.

`blocked_by` is filled from `Game::research_block` — the same call Task 3 refuses on. Not a re-derivation: that is how the screen comes to offer a row the selection turns down.

`ResearchMaterial::have` moves from `Game::research_material_held` (pack plus *adjacent* shelves) to `work_orders::base_holding` (machine and depot buffers, base-wide), so the figure the screen draws and the figure Task 5's gate reads are one call. Leave `research_material_held` in place; Task 8 deletes it with its last caller.

**Steps**

- [ ] **Step 1: Write the failing tests:**
  - `the_active_project_is_the_first_row` — and carries its progress.
  - `a_blocked_node_carries_the_sentence_that_would_refuse_it` — assert equality against `Game::research_block`'s own return, not against hardcoded prose.
  - `a_materials_have_column_counts_a_depot_across_the_base` — the player standing nowhere near it.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 5: Commit.** `feat(research): the screen sees the project, its progress and what blocks it`

---

### Task 8: cut over app-core, and retire the purchase

**Files:**
- Modify: `crates/app-core/src/app/progression.rs` — `handle_research_key` (line 56)
- Modify: `crates/engine/src/game/unlocks.rs` — delete `unlock_research` and `research_material_held`
- Test: `crates/app-core/src/tests/research.rs`, `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Task 3's `select_research` / `abandon_research`.
- Produces: nothing new.

**What to build**

In `handle_research_key`, both the list view's row selectors and the graph view's `Enter` call `Game::select_research` instead of `unlock_research`, reporting through `App::refuse` / `self.report` exactly as they do now — that is the one door a refusal reaches both the popup's status line and the log through.

Add `[A]` for abandon. **Uppercase**, because lowercase letters are row selectors and a lowercase key would both pick a row and fire the action on one press. Put the check **before** `selected_index`, where the existing `[G]` check sits and for the reason its comment gives.

Then delete `Game::unlock_research` and `Game::research_material_held`. Nothing may be left behind — no shim, no `#[allow(dead_code)]`, no "removed" comment. Rewrite the engine tests that called them; `crates/app-core/src/tests/research.rs:14` has a `research_data_held` helper reading the bank that goes with them.

**Steps**

- [ ] **Step 1: Write the failing tests** in `crates/app-core/src/tests/research.rs`:
  - `enter_selects_the_highlighted_node` — in both views.
  - `a_refused_selection_lands_on_the_status_line` — one refusal is enough here; the six live in the engine.
  - `capital_a_abandons_and_lowercase_a_still_picks_a_row` — the key-collision regression, and the reason `[A]` is capital.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-app-core research`
- [ ] **Step 3: Rewire `handle_research_key`.**
- [ ] **Step 4: Delete `unlock_research` and `research_material_held`**, and rewrite every test that called them onto `select_research` plus a tick to completion.
- [ ] **Step 5: Green.** `cargo test -p feral-processes-app-core`, `cargo test -p feral-processes-engine`, then workspace, with fmt and clippy. Note `crates/app-core/src/tests/creation` is a known flake that fails on an unmodified binary with a varying test name — re-run the module rather than chasing it inside this work.
- [ ] **Step 6: Commit.** `feat(research): pick a project from the screen; the purchase is retired`

---

### Task 9: the renderer

**Files:**
- Modify: `crates/gui/src/render/progression.rs` — `draw_research_menu` and the graph view
- Modify: `crates/engine/src/views.rs` — remove `ResearchStatus::affordable`
- Modify: `crates/engine/src/game/unlocks.rs` — stop computing it
- Test: `crates/gui/src/` — beside the existing research render tests

**Interfaces:**
- Consumes: Task 7's view shape.
- Produces: nothing.

**What to build**

Draw, on each row: the progress figure against `cost` for the active project, the blocked sentence on the detail panel (wrapped — `chain_break`'s sentences run to 158 characters and a `LogLine` is never wrapped, which is why this belongs on the panel), and the active project marked. Both the list and the graph view.

The row colour rule reads `blocked_by` where it read `affordable`. Then remove `affordable` from `ResearchStatus` and stop computing it in `research_nodes`: it meant "the player can pay `cost` and every material line right now", a question nobody asks any more.

**The screen has no scroll, so height and width are both layout constraints.** The tallest shipped node's detail panel now carries a `chain_break` sentence it did not before. Write the census test the way `the_tallest_shipped_notification_fits_its_screen` is written, and **verify it by mutation** — shrink the pane until it fails, then put it back. A census that cannot fail is not a census. Popup row width is measurable headlessly via `paint::with_painter`; it does not need a display.

**Steps**

- [ ] **Step 1: Write the failing census test** — the widest and tallest shipped research row fits its panel at 1280x720.
- [ ] **Step 2: Run it and watch it fail** (or pass — if it passes, mutate the pane size to prove it can fail, then restore).
- [ ] **Step 3: Draw the progress, the block and the active mark**, in both views.
- [ ] **Step 4: Remove `affordable`** from the view and from `research_nodes`.
- [ ] **Step 5: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 6: Commit.** `feat(research): the screen draws progress, the active project and what blocks a row`

---

### Task 10: attention, and what an old save carries

**Files:**
- Modify: `crates/engine/src/game/inspection.rs` — `attention` (line 1498)
- Modify: `crates/engine/src/game/lifecycle.rs` — `Game::load`
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: Tasks 1 and 4.
- Produces: nothing new.

**What to build**

**Attention:** a row when the base has a Research Node standing and no project selected. `Game::attention` is the one derivation of what needs the player, read by three surfaces — adding it here is what puts it on all three. Order it with the existing rows' rule (Home first, then by def id, then nearest) rather than inventing a position.

**Migration:** leftover banked Research Data in a legacy save is dropped in `Game::load`, with a log line. The stock strip folds in every `ItemDef::banked` pool **by the flag**, so a leftover pool would sit across the top of every base screen for the rest of the run with nothing to spend it on. The fold itself **stays** — it is written against the flag and never against a name, and a mod may ship another banked item. Do not narrow it.

**Steps**

- [ ] **Step 1: Write the failing tests:**
  - `a_research_node_with_no_project_asks_for_attention` — and stops once one is selected.
  - `a_legacy_saves_banked_research_data_is_dropped_on_load` — build a save with a stocked bank, load, assert the count is zero and that ordinary cargo in the same `Inventory` is untouched.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement both.**
- [ ] **Step 4: Green**, per-crate then workspace, with fmt and clippy.
- [ ] **Step 5: Commit.** `feat(research): an idle Research Node asks for the player, and an old bank is dropped`

---

### Task 11: assets, docs and the three seam writes

**Files:**
- Modify: `assets/items/research_data.ron` — the description
- Modify: `assets/research/README.md` — what `cost:` and `materials:` now mean
- Modify: `docs/research.md`, `docs/structures.md` — regenerate
- Modify: `assets/help/60-your-base.md` and any other help page naming Research Data
- Modify: `CHANGELOG.md`, root `Cargo.toml`
- Modify: `CLAUDE.md`, `.claude/skills/seams/references/base.md`, and the memory graph

**Interfaces:** none.

**What to build**

**`research_data.ron`:** the description currently reads *"Spent on the research tree. Banked rather than carried, so it never counts against your cargo."* Both halves are now false. Rewrite it as what it is: what a Research Node produces and what a research project runs on. **`banked: true` stays** — it is what keeps the payout out of every `Stock` buffer, which is still exactly what is wanted, and `ItemDb::validate` requires the `ResearchCurrency` role to exist.

**`docs/research.md` and `docs/structures.md`:** `docs/*-gen.py` are **hand transcriptions, not parsers** — one was a node short for a whole release and was regenerated confidently anyway. Read the output against the assets before trusting it.

**`docs/manual.md` is carved out** of the documentation obligation and stays stale. Do not touch it.

**Version:** this is a feature landing on a branch; per repo policy the bump, the `## X.Y.Z` section and the tag happen **at the merge**, not on the branch. Write the changelog section, leave the version alone, and say so in your report. Which digit moves is `CHANGELOG.md`'s preamble's call; "breaking" means a player's save stops loading, and this one does not.

**The three seam writes**, in the order the `seams` skill documents — argument to the graph, trap to the skill, rule to `CLAUDE.md`:

1. **New:** *"Research is one project at a time, and `Game::select_research` is the one door — every refusal before anything is filed."*
2. **New:** *"A research project's materials are ordinary work orders, and `WorkOrder::for_research` is provenance rather than a plan."*
3. **Amended:** the Research Node status seam. *"A banked resource can never clog, so a Research Node has no full state"* still holds; what is added is that with no project selected it reads `Idle` rather than anything new, because the scheduler simply does not staff it.

Each rule in `CLAUDE.md` is **one sentence — the rule alone**. That file is loaded on every turn and reached 151 KB once by letting each trap creep back in beside its rule; the trap goes in `references/base.md` and the argument goes in the graph.

**Steps**

- [ ] **Step 1: Rewrite the item description and the research README.**
- [ ] **Step 2: Regenerate the two docs pages and read them against the assets.**
- [ ] **Step 3: Grep the help pages** for claims this change falsifies — `rg -l "Research Data" assets/help/` — and fix each.
- [ ] **Step 4: Write the changelog section.** No version bump on the branch.
- [ ] **Step 5: Write the three seams**, argument → trap → rule.
- [ ] **Step 6: Green.** `cargo test --workspace` — `tests/assets.rs` holds censuses over the real assets and an edited `.ron` can fail one.
- [ ] **Step 7: Commit.** `docs(research): the project model in the assets, the docs and the seams`

---

## Final gate

- [ ] `cargo test --workspace` green from a clean checkout of the branch
- [ ] `cargo clippy --workspace` clean
- [ ] `cargo fmt --check` clean
- [ ] `cargo test -p feral-processes-engine balance_sim` — unaffected in principle, since `balance_sim` models no base, but it is the regression gate whenever `tuning.rs` is touched
- [ ] A whole-branch review on **opus**, given the diff **as a file** and this plan's ledger. Per-task gates are skipped in this plan; the final review is not optional, and dropping the per-task gate is exactly why it must see everything.
- [ ] **Nothing is pushed.** Landing is the user's call.

## Known traps, collected

Each of these has bitten this repo before. They are listed once here so no task has to repeat them.

- **A test that passes with the fix removed is not a test.** Delete the implementation and watch each new test fail before you believe it.
- **Adding a `Resource` to a bevy system shifts query iteration order**, which surfaces as seed-luck failures in unrelated tests. Task 6 will do this. Probe to ground before theorising.
- **`cargo test -p feral-processes-engine` and `cargo test --workspace` are different builds** and shift the RNG stream differently. Run both.
- **Cargo's exit code is lost through a pipe.** Never `cargo test | tail`.
- **A `#[serde(skip)]` field is invisible to a RON round trip.** Task 2's flag needs a save→load test.
- **`crates/app-core/src/tests/creation` flakes** on an unmodified binary, with a varying test name. Re-run the module; do not fix it inside this work.
- **Mass `NotFound` failures on an assets path are stale build artifacts**, not bugs. `cargo clean -p feral-processes-engine -p feral-processes-app-core`, never a full `cargo clean`.
- **Agents cannot playtest** — there is no display in this environment. A green suite is not evidence of play; say so once, plainly, in the final report.
