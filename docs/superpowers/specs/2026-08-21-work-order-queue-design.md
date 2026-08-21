# The work order queue

**Status:** approved, not implemented
**Date:** 2026-08-21

Six changes to the base's work order queue, in six independently landable
phases. The queue becomes a standing production policy rather than a
to-do list: several orders are worked at once in priority order, an order
can be a stock level the base holds forever, and the screen says which of
its orders is actually being worked and why the others are not.

## Why

Work orders shipped in 0.8.35 and deleted manual posting outright. What
landed is a **reconciliation loop**, not a job queue: `settle_orders`
re-derives the whole assignment from live world state every tick, and
`schedule_base_labour` computes a desired posting set and diffs it against
the actual one. `work_orders.rs`'s own header states the rule — *an order
is a target level, not a production run* — and the module is built around
it.

Three gaps sit between that rule and what the code does.

**A target level that deletes itself is not a level.** `settle_orders`
removes an order the moment `base_holding >= qty`. But `collect_adjacent`
moves `Stock::output` straight into the player's `Inventory`, so the shelf
empties every time the player walks past it, and the order that was meant
to keep it full is already gone. The loop the design describes cannot
close.

**All the bodies go to one order.** `settle_orders` returns the wants of
the *first* workable order and stops (work_orders.rs:1071). An order that
needs two bodies leaves the other four staff to standing jobs and dig
sites while three ready orders queue behind it. The base idles labour it
has been given work for.

**Priority exists and has no verb.** Queue position *is* priority — the
scheduler walks the Vec in order and `truncate(staff.len())` cuts from the
end. But the only edits are append and drop, so changing your mind means
cancel-and-refile, which lands the order you care about at the *bottom*.
The one control the player has over the base's attention is strictly
worse than useless.

The unifying frame is that the queue is a declaration of desired state and
the scheduler is a controller converging on it. Everything below follows
from taking that seriously; nothing below adds a second source of truth.

## What this is not

The queue was read as an SQS-shaped work queue during design, and most of
that vocabulary has nothing to attach to here. There are no messages, so
nothing can be lost, so there is no retry, no visibility timeout, no
at-least-once, and no dead-letter queue. An order that cannot be satisfied
does not fail — it stays unsatisfied and is re-evaluated next tick, which
is `stalled` and already exists.

Three ideas were considered and rejected on those grounds:

- **A separate error queue or stalled-order screen.** Stalled orders
  already do not block the queue (`settle_orders` skips them), so they
  cost attention rather than throughput. The stall *reason* is the errand
  the player needs, and moving it off the main screen hides it. Phase 5
  fixes the real problem — nobody is told — without a second surface.
- **Auto-cancelling a long-stalled order.** Silent deletion of a player's
  instruction, and a stall is usually a build errand rather than a
  mistake.
- **Retry counts, order ages, estimated completion.** Nothing fails, so
  there is nothing to count. An ETA is derivable from `Task::required`
  but is wrong the instant staffing shifts, and `CLAUDE.md` already
  refuses stored progress deliberately — `work_order_report` *calls*
  `wants` rather than reading a counter.

## Save format

Phases 2 and 3 each add one field to `WorkOrder`, which is saved at
`save.rs:653`. Both are additive and `#[serde(default)]`, and the save has
been field-named RON since well before this, so **neither earns a
`SAVE_FORMAT_VERSION` bump**. Phase 5 adds a third field that is
`#[serde(skip)]` and so costs nothing either.

`WorkOrder`'s own doc comment records that it was made a named struct
rather than a `(ItemId, u32)` tuple precisely so it could be widened
later, after `PlayerSave::fused_gear` and `SaveData::buyback` both had to
be drained into named successors. This is that widening.

---

## Phase 1 — Work orders concurrently

**The change.** `settle_orders` accumulates wants across *every*
unsatisfied, non-stalled order in queue order instead of returning at the
first one.

**Why priority survives for free.** `schedule_base_labour` documents the
mechanism: *"the priority **is** the position in this list"*, because
`truncate(staff.len())` cuts from the end. Order 1's machines land at the
front of the accumulated list and get first refusal on every body; order
2 fills from what is left; standing jobs and dig wants are still appended
after all of them and so stay lowest. No sort, no scoring, no new rule —
the existing one simply has more list to act on.

**The trap is duplicate machines.** Two orders can want the same feeder,
and a machine appearing twice in `wanted` would consume two staff slots
against one post and silently shorten the truncation for everything
below. Dedupe by `(Entity, TaskKind)` keeping the **first** occurrence, so
the higher-priority order is what holds the position.

**The second trap is the doc comment.** Steps 1 and 2 of
`schedule_base_labour`'s five-step contract describe taking *the front
order*. That prose is load-bearing documentation of a rule this phase
changes, and leaving it stale is exactly the drift `CLAUDE.md` names.
Rewrite it in the same change.

**Ungated.** This makes a staffed base substantially more productive.
`balance_sim` has no base term at all, the arena models player combat, and
no test in the suite can see base throughput against the zone curve. The
risk is stated here rather than mitigated, because there is no instrument
to mitigate it with; it is a pacing question for play.

**Tests.** A base with more staff than the front order wants posts bodies
to the second order's machines. A machine wanted by two orders is posted
once. The front order still fills before the second when staff are
scarce. Standing jobs and digs still come last.

## Phase 2 — Standing orders

**The change.** `WorkOrder` gains `standing: bool`. In `settle_orders`, a
satisfied standing order is **skipped** (`index += 1`) rather than
removed — the same branch a stalled order already takes.

**Skipped, not returned.** Returning a satisfied standing order's (empty)
wants would starve every order below it forever. This is the one
correctness point in the phase.

**No hysteresis, deliberately.** The instinct is that re-arming at
`qty - 1` thrashes: one unit leaves, the chain wakes, bodies are yanked
off the next order to make one unit. That is the failure
`schedule_base_labour`'s anti-thrash rule exists to prevent one level up,
so the worry is fair. It does not apply, because **`collect_adjacent`
empties the whole output buffer.** The drain is bursty by construction —
the player collects, holding goes to zero, the order runs the full `qty`,
sleeps. There is no trickle to oscillate around. The one genuine trickle
case is a standing order on an intermediate that a downstream assembler
eats a batch at a time, and there the downstream order's own `wants` walk
already staffs that same machine, so the bodies land in the same places.

No `refill_at` field, no `tuning.rs` fraction. If play shows oscillation,
`qty.saturating_sub(batch)` is a one-line addition later. Building it now
is designing against a failure the collect rule already prevents.

**No log line on top-up.** `settle_orders` currently announces "Work order
complete" and removes. A standing order cannot say that every tick, and
"complete" is a lie for something that is not complete. Detecting the
transition needs stored state the order does not have. The dormant tag
from Phase 4 carries the news on screen instead.

**`base_holding` does not count the player's cargo.** It sums
`Stock::output` across deployed structures only. "Keep 20 Cache Grain"
therefore means 20 *on the shelf*, and 40 in the player's pocket are
invisible to it. That is the correct reading — the order is a statement
about the base — but `have 0/20` while carrying 40 will read as a bug the
first time it happens, so the screen says which figure it is showing.

**Where the player says it.** A toggle on `Mode::WorkOrderQuantity`,
mirroring the careful-craft flag's shape. One-shot orders stay: "make me
3 Routine Disks" and "always hold 3 Cache Grain" are different errands.

**Tests.** A satisfied standing order stays in the queue and is not
announced complete. An order below a satisfied standing order is worked.
A standing order re-arms after `collect_adjacent` drains the shelf. A
one-shot order still completes and is removed.

## Phase 3 — Priority bands

**The change.** `WorkOrder` gains a `priority` enum (High / Normal /
Low, `#[derive(Default)]` on Normal). `queue_work_order` **inserts** after
the last order of equal-or-higher priority instead of pushing to the end.

**Priority is an insert position, not a second sort.** The obvious build
is a `priority` field plus a sort at scheduling time, and it is the
expensive one: `cancel_work_order` takes a raw index into the Vec and
`work_order_report` returns in Vec order for the screen to index straight
into, so a sort at scheduling time makes Vec order and effective order
diverge and every index in the system has to know which it holds. Keeping
the Vec in effective order means `settle_orders`, `cancel_work_order`,
`work_order_report` and the screen are all untouched. The stored field is
a **label** — it decides where an order lands and lets the row show its
band — while position remains what the scheduler reads. One source of
truth.

**Set at file time, and that is why there is no reorder verb.** The band
goes on `Mode::WorkOrderQuantity` beside the standing toggle, cycled by a
key since the digits belong to quantity. This is what fixes
cancel-and-refile: refiling restores the band, so it puts the order back
where it was meant to be instead of at the bottom. Ties within a band
break by insertion order.

**What is knowingly left open.** Reordering *within* a band. Three orders
all at Normal and wanting the second one first means refiling it as High,
which is blunter than moving it up one. Three bands is likely more
resolution than a short queue needs; if it bites, `move_work_order` is
~20 lines and composes with bands.

**Tests.** A High order files above a standing Normal one. Two orders of
one band keep insertion order. `cancel_work_order`'s index still names
the row the screen shows. A defaulted save loads every order as Normal.

## Phase 4 — Four states on the screen

**The change.** `WorkOrderReport` gains a state:
`Working` / `Queued` / `Dormant` / `Stalled`, **derived from
`settle_orders`' own rule** rather than recomputed.

`work_order_report` already exists precisely so the screen shows what the
scheduler believes by construction rather than by a comment claiming the
two agree, and this keeps that. A second copy of "is this order being
worked" is the copy that drifts.

Standing orders are what make this necessary rather than merely nice: a
dormant order and a stalled one look identical today, and after Phase 2
one of them is the normal healthy state.

**Tests.** Each of the four states is reachable and reported. A dormant
standing order is not reported stalled.

## Phase 5 — Announce a stall once

**The change.** A `#[serde(skip)]` latch on `WorkOrder`, logged once on
transition into stalled: *"Work order stalled: 3 x Routine Disk — no Lathe
deployed"*, with the sentence `chain_break` already produces.

**Modelled on `DigSite::announced_stuck`**, which `work_orders.rs` already
cites. Same rule as `set_machine_status`: entering a state is news,
staying in it is not. **The latch is not saved** — a reload should say it
again, which is what `#[serde(skip)]` buys rather than merely omitting a
default. Note that a skipped field is invisible to the RON round-trip
test, so the reload case needs its own save-then-load assertion.

This is the useful half of a dead-letter queue and the whole of it: the
problem was never that stalled orders block anything, it is that nobody
is told without opening a screen on purpose.

**Tests.** A stall logs once, not per tick. A stall that resolves and
recurs logs again. A reload re-announces.

## Phase 6 — Say how short of bodies the base is

**The change.** A header line on the work order screen: how many posts the
queue wants against how many staff exist.

`schedule_base_labour` truncates `wanted` to `staff.len()` and the posts
that fall off the end vanish silently — the screen says "no one" per
machine but never says *you are four bodies short*. Both figures are
already computed in that function; this exposes them.

It answers "why is nothing happening" from a different direction than
Phase 4's tag: the tag says which order has the base's attention, this
says whether the base has anyone to give it.

**Tests.** The shortfall reports zero when staff outnumber wants, and the
difference when they do not. A base with no staff at all still reports
rather than reading as an error.

## Testing standard

Every phase is TDD with the failing test first, and each phase must be
green on `cargo test --workspace` before the next begins.

New coverage is **mutation-proved**: delete the fix, watch the test fail,
restore. Phases 1 and 2 both change `settle_orders`' control flow in ways
a naive assertion passes against by accident — a test that a second order
is worked passes trivially if the first order was already satisfied, and a
test that a standing order survives passes against an order that was never
satisfied in the first place. Both fixtures must assert the precondition.

`cargo test -p feral-processes-engine balance_sim` after Phase 1, not
because it can see base throughput — it cannot — but to confirm this
changed no curve it *does* watch.
