# Work order queue implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the base's work order queue from a serial to-do list into a
standing production policy — several orders worked at once in priority
order, orders that hold a stock level forever, and a screen that says
which order has the base's attention and why the others do not.

**Architecture:** Six independently landable phases against one engine
module (`crates/engine/src/game/base/work_orders.rs`), its app-core
handlers and its gui screen. Nothing introduces a second source of truth:
priority stays queue position, order state stays derived from
`settle_orders`' own rule, and the two new saved fields are labels on
`WorkOrder` rather than plans. Each phase is green on the full workspace
suite before the next begins.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), serde/RON saves,
`cargo test --workspace`.

**Spec:** `docs/superpowers/specs/2026-08-21-work-order-queue-design.md` —
read it before Task 1. It carries the arguments this plan only cites: why
the SQS reading was dropped for a reconciliation one, why priority is an
insert position rather than a second sort, and why the standing-order
refill needs no hysteresis.

## Global Constraints

- **Follow `CLAUDE.md`.** It is loaded every turn and its seam rules bind
  here. The work-order seams it already names are in the "The base"
  section; correct that file *and* `docs/seams.md` in the same change if a
  phase moves one.
- **TDD, failing test first**, every phase. No exceptions for the "obvious"
  ones.
- **Mutation-proof every new test**: delete the fix, watch the test fail,
  restore. Record it. Tasks 1 and 2 both have fixtures that pass by
  accident if the precondition is not asserted — see their steps.
- **Gate:** `cargo test --workspace` green before any commit that ends a
  task. `cargo fmt` and `cargo clippy --workspace` clean, warnings fixed
  rather than silenced.
- **No `SAVE_FORMAT_VERSION` bump.** Tasks 2 and 3 add
  `#[serde(default)]` fields to `WorkOrder`; Task 5 adds a
  `#[serde(skip)]` one. The save is field-named RON, so all three are free.
  If any task finds itself wanting to bump it, stop and raise it.
- **No new tuning constants** unless a task names one. Magic numbers go in
  `crates/engine/src/tuning.rs` if they appear at all.
- **Do not push.** Commit freely on the branch; releasing is the user's
  call.
- **The spec's "What this is not" section is binding.** A dead-letter or
  error queue, a separate stalled-order screen, auto-cancelling a stalled
  order, retry counts, order ages and estimated completion were all
  considered and rejected with reasons. Do not add them helpfully.
- **`docs/manual.md`, root `README.md` and `TODO.md` are carved out** of
  the documentation obligation. `CHANGELOG.md` is not.

## File structure

The whole change lives in six files plus their tests. No new modules.

| File | Responsibility | Tasks |
|---|---|---|
| `crates/engine/src/game/base/work_orders.rs` | `WorkOrder`, `settle_orders`, `schedule_base_labour`, `queue_work_order`, `work_order_report` | 1–6 |
| `crates/engine/src/views.rs` | `WorkOrderReport` — the screen's read-only shape | 4 |
| `crates/engine/src/resources.rs` | `WorkOrders`; the new labour-demand cache | 6 |
| `crates/app-core/src/app/building.rs` | the three work-order key handlers | 2, 3 |
| `crates/app-core/src/lib.rs` | `App` fields for the pending order's flags | 2, 3 |
| `crates/gui/src/render/building.rs` | `draw_work_orders`, `work_order_lines`, `draw_work_order_quantity` | 2, 3, 4, 6 |
| `crates/engine/src/tests/work_orders.rs` | engine coverage (1621 lines today) | 1–6 |
| `crates/app-core/src/tests/building.rs` | handler coverage | 2, 3 |

**The gui file has a width trap.** `work_order_lines` is a pure function
of the row *specifically so a headless test can measure how wide its
widest line runs* — `draw_row` clamps vertically and never horizontally,
so a line that outgrows the popup body is lost in silence. Tasks 4 and 6
add text to that screen and must extend the existing width test, not just
keep it passing.

---

### Task 1: Work orders concurrently

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — `settle_orders`
  (line 1038) and `schedule_base_labour`'s five-step doc contract
  (line 596)
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Consumes: `wants(game: &Game, order: &WorkOrder) -> Vec<(Entity, u32)>`
  (line 387), unchanged.
- Produces: `settle_orders(&mut self) -> Vec<(Entity, u32)>` — same
  signature, new contract. It now returns the accumulated wants of **every**
  unsatisfied non-stalled order in queue order, deepest-first within each
  order, deduplicated by `Entity` keeping the first occurrence. Tasks 2–6
  all build on this contract.

**What changes.** Today the loop returns at the first workable order
(line 1071). Make it accumulate instead: keep completing and removing
satisfied orders and skipping stalled ones exactly as now, but push each
workable order's `wants` onto one list and carry on to the next index
rather than returning.

**Why priority needs no new code.** `schedule_base_labour` truncates
`wanted` to `staff.len()`, and its own doc comment states the rule: *"the
priority **is** the position in this list"*. Order 1's machines are at the
front of the accumulated list and get first refusal on every body; order 2
fills from what is left; standing jobs and dig wants are still appended
after all orders and stay lowest. Do not add a sort or a score.

**The dedupe is an ordering constraint, not an optimisation.** A machine
wanted by two orders would otherwise occupy two slots in `wanted` against
one post, silently shortening the truncation for everything below it. Keep
the **first** occurrence so the higher-priority order holds the position:

```rust
let mut seen = std::collections::HashSet::new();
list.retain(|&(machine, _)| seen.insert(machine));
```

**The doc comment is load-bearing and goes stale.** Steps 1 and 2 of
`schedule_base_labour`'s five-step contract describe taking *the front
order*. Rewrite both, and check whether the "The base" section of
`CLAUDE.md` and the matching entry in `docs/seams.md` make the same claim.

- [ ] **Step 1: Write the failing tests**

Four, in `crates/engine/src/tests/work_orders.rs`. Use `work_node_parts()`
and `park_at_post()` from `tests/support.rs` for any hand-spawned node or
posted program — omitting either reads as a payout curve that moved rather
than as a fixture short something.

1. *A base with spare staff works the second order too.* Stand up two
   independent chains, queue an order against each, give the base more
   staff than the first order's wants. Assert a body is posted to a machine
   belonging to the **second** order. **Assert the precondition**: the
   first order is unsatisfied and its wants are strictly fewer than the
   staff count. Without that assertion this test passes against today's
   code whenever the first order happens to be already satisfied.
2. *A machine wanted by two orders is posted once.* Two orders whose
   chains share a feeder. Assert exactly one `Task` targets that machine.
3. *The front order still fills first when staff are scarce.* Two orders,
   fewer staff than the first order's wants. Assert every posted body is on
   a first-order machine and none on the second's.
4. *Standing jobs and digs still come last.* One unsatisfied order plus a
   standing job plus a marked dig site, with staff enough for the order
   only. Assert the order's machines are staffed and the standing job and
   dig site are not.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-engine work_orders`

Tests 1 and 2 must fail. Tests 3 and 4 may pass already — that is fine,
they are regression guards for the rule this task must not break. Note in
the commit which of the four were red.

- [ ] **Step 3: Implement**

Accumulate in `settle_orders`; dedupe as above; rewrite the two doc
comment steps.

- [ ] **Step 4: Confirm they pass, then mutation-prove**

`cargo test -p feral-processes-engine work_orders`, then revert the
accumulation (restore the early `return list`), confirm tests 1 and 2 go
red, restore the fix. Then remove **only** the dedupe line and confirm
test 2 goes red. Record both in the commit body.

- [ ] **Step 5: Gate**

`cargo test --workspace`, then
`cargo test -p feral-processes-engine balance_sim`. The balance run is not
evidence about base throughput — `balance_sim` has no base term and cannot
see it — it confirms this moved no curve it *does* watch. `cargo fmt`,
`cargo clippy --workspace`.

- [ ] **Step 6: Commit**

Note in the body that this makes a staffed base materially more productive
and that **nothing in the suite gates that**; it is a pacing question for
play. Do not claim it is balanced.

---

### Task 2: Standing orders

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — `WorkOrder`
  (line 42), `settle_orders` (line 1038), `queue_work_order` (line 1082)
- Modify: `crates/app-core/src/lib.rs` — a new `App` field beside
  `pending_order` (line 1520)
- Modify: `crates/app-core/src/app/building.rs` —
  `handle_work_order_pick_key` (line 529) and
  `handle_work_order_quantity_key` (line 544)
- Modify: `crates/gui/src/render/building.rs` —
  `draw_work_order_quantity` (line 272) and its dispatch in
  `crates/gui/src/render/mod.rs:666`
- Test: `crates/engine/src/tests/work_orders.rs`,
  `crates/app-core/src/tests/building.rs`

**Interfaces:**
- Consumes: Task 1's accumulating `settle_orders`.
- Produces: `WorkOrder { item, qty, standing: bool }` with
  `#[serde(default)]` on the new field, and
  `Game::queue_work_order(&mut self, item: ItemId, qty: u32, standing: bool)
  -> Result<(), String>`. Task 3 widens this signature again — expect that.

**The one correctness point.** A satisfied standing order is **skipped**
(`index += 1`, the branch a stalled order already takes at line 1068), not
removed and not returned. Returning its empty wants would starve every
order below it forever.

**No hysteresis.** Re-arm at `base_holding < qty`, full stop. The spec
argues why: `collect_adjacent` empties the whole output buffer, so the
drain is bursty rather than a trickle and there is nothing to oscillate
around. Do not add a `refill_at` field or a `tuning.rs` fraction.

**No log line on top-up.** Today `settle_orders` announces "Work order
complete" and removes (line 1057). A standing order must not say that —
"complete" is a lie for something that is not complete, and detecting the
transition needs state the order does not have. Task 4's dormant tag
carries the news instead. A one-shot order keeps the line unchanged.

**The player's gesture** mirrors the careful-craft flag exactly:
`crates/app-core/src/app/crafting.rs:43` toggles `careful_craft` on
`[C]` in the quantity handler, and `crafting.rs:16` clears it when the
pick page opens. Copy that shape — `[S]` on
`Mode::WorkOrderQuantity`, cleared in `handle_work_order_pick_key`
alongside `order_quantity_input.clear()`. It must not outlive its order.

**`base_holding` does not count the player's cargo** (work_orders.rs:523
sums `Stock::output` over deployed structures only). Add a line to
`draw_work_order_quantity` saying the figure is what the base holds. The
screen already has a sentence of this kind at building.rs:288 — extend
that idiom rather than inventing a second voice.

- [ ] **Step 1: Write the failing tests**

Engine (`tests/work_orders.rs`):
1. *A satisfied standing order stays in the queue.* Assert it is still
   present and that no `MessageKind::Complete` line was logged. **Assert
   the precondition** that `base_holding >= qty` — without it this passes
   against an order that was never satisfied at all.
2. *An order below a satisfied standing order is worked.* Assert bodies
   are posted to the second order's machines.
3. *A standing order re-arms after the shelf drains.* Satisfy it, call
   `collect_adjacent`, tick, assert its machines are wanted again.
4. *A one-shot order still completes and is removed*, and still logs
   `Complete`.
5. *A save written before this field loads every order as one-shot.* A
   save→load round trip asserting `standing == false`.

app-core (`tests/building.rs`):
6. *`[S]` toggles the pending flag and Enter files a standing order.*
7. *Opening the pick page clears the flag*, so it cannot outlive its
   batch.

- [ ] **Step 2: Run and confirm they fail**

`cargo test -p feral-processes-engine work_orders` and
`cargo test -p feral-processes-app-core building`.

- [ ] **Step 3: Implement**

Field, `settle_orders` skip branch, widened `queue_work_order`, the
`App` field and two handlers, the gui row and its dispatch argument.

- [ ] **Step 4: Confirm and mutation-prove**

Revert the skip branch to the removal, confirm engine tests 1–3 go red,
restore. Then remove the pick-page clear and confirm app-core test 7 goes
red. Record both.

- [ ] **Step 5: Gate and commit**

`cargo test --workspace`, `cargo fmt`, `cargo clippy --workspace`.

---

### Task 3: Priority bands

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — `WorkOrder`
  (line 42), `queue_work_order` (line 1082)
- Modify: `crates/app-core/src/lib.rs`, `app/building.rs`,
  `crates/gui/src/render/building.rs` + `render/mod.rs:666` as in Task 2
- Test: `crates/engine/src/tests/work_orders.rs`,
  `crates/app-core/src/tests/building.rs`

**Interfaces:**
- Produces: `pub enum OrderPriority { High, #[default] Normal, Low }` —
  `Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize`,
  exported from the engine root the way `WorkOrder` is so app-core and gui
  can name it. `WorkOrder` gains `priority: OrderPriority` with
  `#[serde(default)]`. `Game::queue_work_order(&mut self, item: ItemId,
  qty: u32, standing: bool, priority: OrderPriority) -> Result<(), String>`.

**Priority is an insert position, not a second sort.** `queue_work_order`
inserts after the last order of equal-or-higher priority rather than
pushing. The Vec stays in effective order, so `settle_orders`,
`cancel_work_order` (whose `index` is a raw Vec index, line 1132) and
`work_order_report` are all untouched, and the screen keeps indexing
straight into the report. **Do not sort at scheduling time** — that makes
Vec order and effective order diverge and every index in the system then
has to know which one it holds.

The stored field is a label: it decides where an order lands and lets the
row show its band. Position remains what the scheduler reads.

**Ties break by insertion order**, which falls out of inserting *after*
the last equal-priority order rather than before the first.

**Set at file time only.** No reorder verb in this task — see the spec's
"what is knowingly left open". `[P]` cycles the band on
`Mode::WorkOrderQuantity` (digits belong to quantity), cleared on the pick
page beside the standing flag. This is what fixes cancel-and-refile:
refiling restores the band instead of dropping the order to the bottom.

- [ ] **Step 1: Write the failing tests**

Engine:
1. *A High order files above an existing Normal one.* Assert Vec position.
2. *Two orders of one band keep insertion order.*
3. *A Low order files below everything.*
4. *`cancel_work_order(index)` still drops the row at the screen's index*
   after a High order has been inserted mid-queue.
5. *A save written before this field loads every order as Normal.*

app-core:
6. *`[P]` cycles High → Normal → Low → High and the filed order carries
   the band shown.*
7. *Opening the pick page resets the band to Normal.*

- [ ] **Step 2: Run and confirm they fail**

`cargo test -p feral-processes-engine work_orders` and
`cargo test -p feral-processes-app-core building`.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Confirm and mutation-prove**

Replace the insert with a push, confirm tests 1 and 3 go red, restore.
Change the insert to land *before* the first equal-priority order and
confirm test 2 goes red, restore.

- [ ] **Step 5: Gate and commit**

`cargo test --workspace`, `cargo fmt`, `cargo clippy --workspace`.

---

### Task 4: Four states on the screen

**Files:**
- Modify: `crates/engine/src/views.rs` — `WorkOrderReport` (line 757)
- Modify: `crates/engine/src/game/base/work_orders.rs` —
  `work_order_report` (line 1156)
- Modify: `crates/gui/src/render/building.rs` — `work_order_lines`
  (line 208) and its width test (around line 1001)
- Test: `crates/engine/src/tests/work_orders.rs`, the gui test module

**Interfaces:**
- Produces: `pub enum OrderState { Working, Queued, Dormant, Stalled }` in
  `views.rs`, and `WorkOrderReport::state: OrderState`. The existing
  `stalled: bool` is **replaced**, not kept beside it — two fields
  answering one question is the drift this module exists to avoid. Update
  the gui's `STALLED` suffix at building.rs:212 accordingly.

**Derived from `settle_orders`' rule, not recomputed.**
`work_order_report` already exists so the screen shows what the scheduler
believes *by construction* rather than by a comment claiming the two
agree, and it calls `wants` for exactly that reason. Keep that: `Working`
means this order contributed machines that fit inside `staff.len()`;
`Queued` means it wants bodies there were none left for; `Dormant` means a
standing order with `base_holding >= qty`; `Stalled` means `wants` was
empty. A second copy of "is this order being worked" is the copy that
drifts.

**The width trap.** `work_order_lines` is pure specifically so a headless
test can measure its widest line, and `draw_row` never clips horizontally.
Extend that test to cover the new state token on the longest shipped item
name.

- [ ] **Step 1: Write the failing tests**

1–4. *Each of the four states is reachable and reported* — one test per
state, each asserting the other three are not reported for that order.
5. *A dormant standing order is not reported stalled.* This is the
   regression the enum exists to prevent; today they are indistinguishable.
6. *gui:* the widest row with a state token still fits the popup body.

- [ ] **Step 2: Run and confirm they fail**

`cargo test -p feral-processes-engine work_orders` and
`cargo test -p feral-processes-gui building`.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Confirm and mutation-prove**

Collapse `Dormant` into `Stalled`, confirm test 5 goes red, restore.

- [ ] **Step 5: Gate and commit**

`cargo test --workspace`, `cargo fmt`, `cargo clippy --workspace`.

---

### Task 5: Announce a stall once

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` — `WorkOrder`
  (line 42) and `settle_orders`' stalled branch (line 1064)
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Produces: `WorkOrder { .., announced_stalled: bool }` with
  `#[serde(skip)]`. Engine-internal; no app-core or gui change.

**Copy `DigSite::announced_stuck`**, whose implementation is in this same
file at `can_walk_to_dig` (lines 826–860). It has two halves and the
second is the one that gets forgotten: it sets the latch and logs on entry
into the stuck state, **and clears the latch the moment a route exists
again** — without the clear, a second stall is silent forever. Do the same
around the stalled branch, using the sentence `chain_break` already
produces so the log and the screen cannot word one break differently.

This is `systems::set_machine_status`' rule one subsystem over: entering a
state is news, staying in it is not.

**`#[serde(skip)]`, not a default.** A reload should say it again. Note
that a skipped field is invisible to the RON round-trip test, so the
reload case needs its own save-then-load assertion — a round trip alone
would pass against a field that was never skipped.

- [ ] **Step 1: Write the failing tests**

1. *A stall logs once, not per tick.* Tick several times, assert exactly
   one matching log line.
2. *A stall that resolves and recurs logs again.* Rebuild the missing
   machine, tick, demolish it, tick, assert a second line.
3. *A reload re-announces.* Save with a stalled order, load, tick, assert
   the line appears. Use `save_path()` from the top of the test file for a
   process-unique scratch path.

- [ ] **Step 2: Run and confirm they fail**

`cargo test -p feral-processes-engine work_orders`.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Confirm and mutation-prove**

Remove the latch *clear*, confirm test 2 goes red, restore. This is the
half the precedent warns about and the one a green suite would otherwise
hide.

- [ ] **Step 5: Gate and commit**

`cargo test --workspace`, `cargo fmt`, `cargo clippy --workspace`.

---

### Task 6: Say how short of bodies the base is

**Files:**
- Modify: `crates/engine/src/resources.rs` — a new resource beside
  `WorkOrders` (line 518)
- Modify: `crates/engine/src/game/base/work_orders.rs` —
  `schedule_base_labour` (line 596)
- Modify: `crates/gui/src/render/building.rs` — `draw_work_orders`
  (line 179) and the width test
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Produces: `resources::LabourDemand { wanted: usize, staff: usize }`,
  written once per tick by `schedule_base_labour` **before** its
  `truncate`, and a `Game` accessor returning it for the screen.

**Why a cached resource rather than a derivation.** The two figures live
inside `schedule_base_labour`, which is `&mut self` and has side effects —
`settle_orders` removes completed orders and Task 5's latch logs. A screen
must not call that. This is precisely the situation `resources::Platform`'s
cached radius already solves, and `CLAUDE.md` records the reasoning: the
footprint must be readable from `&self` while the derivation needs
`&mut self`. Follow that precedent. **Not saved** — it is rewritten every
tick.

**Write it before the `truncate`**, or it reports the post-cut figure and
the shortfall is always zero. Write it also on the early-return path where
`staff.is_empty()` (line 629), which is a valid quiet state and the one a
player is most likely to be looking at the screen for.

**The header line** goes in `draw_work_orders` above the existing
key-hints row. It answers "why is nothing happening" from a different
direction than Task 4's tag: the tag says which order has the base's
attention, this says whether the base has anyone to give it. Say nothing
when there is no shortfall — a line that always shows is a line nobody
reads.

- [ ] **Step 1: Write the failing tests**

1. *The shortfall is zero when staff outnumber wants.*
2. *The shortfall is the difference when wants outnumber staff.*
3. *A base with no staff at all reports its wants against zero* rather
   than reading as an error or reporting zero wants.
4. *gui:* the header row fits the popup body at the largest plausible
   figures.

- [ ] **Step 2: Run and confirm they fail**

`cargo test -p feral-processes-engine work_orders` and
`cargo test -p feral-processes-gui building`.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Confirm and mutation-prove**

Move the write below the `truncate`, confirm test 2 goes red, restore.
Delete the write on the `staff.is_empty()` path, confirm test 3 goes red,
restore.

- [ ] **Step 5: Gate and commit**

`cargo test --workspace`, `cargo fmt`, `cargo clippy --workspace`.

---

## Closing out

After Task 6 is green:

- [ ] **Update `CHANGELOG.md`** with a section for the release. Which digit
  moves is decided by that file's own preamble — read it. No save format
  broke here, so this is not a breaking change under its definition.
- [ ] **Update `CLAUDE.md`'s "The base" section and the matching
  `docs/seams.md` entries.** Three seam statements there are falsified by
  this work: that a work order stores an item and a quantity *and nothing
  else*, that the scheduler decides the whole assignment from the front
  order, and that priority is position with no way to set it. `CLAUDE.md`
  and `AGENTS.md` are gitignored twins — edit `CLAUDE.md`, then `cp` it.
- [ ] **Do not touch `docs/manual.md`, root `README.md` or `TODO.md`.**
- [ ] **Version bump and tag are the user's call.** Do not push.
