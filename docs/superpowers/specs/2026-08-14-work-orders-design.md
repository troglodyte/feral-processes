# Work Orders

**Status:** approved 2026-08-14, not implemented.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

## The problem

The base is staffed one program at a time, by hand, forever. Posting is a
per-machine errand: open the base menu, pick a program, pick a structure,
repeat — and then repeat it again every time a machine clogs, a worker is
needed elsewhere, or a chain needs its upstream link fed before its
downstream one can run.

That is not a decision, it is bookkeeping. The interesting question a
production chain asks is *what do you want made*, and the game has never had
a way to say it. Everything below the answer — which machine needs a body
first, when to move that body downstream, what the base is short of — is
mechanical, derivable, and currently the player's job.

Work orders move that line. You say "3 Routine Disks"; the base works out
that this means twenty-four Core Fragments through a Lathe into a Disk
Press, and staffs itself in that order with whoever you have given it.

## What a work order is

A queue entry: an item and a quantity. The queue is worked front-first —
one order at a time, to completion, then the next.

An order is a **target level, not a production run**. "3 Routine Disks"
means *have three*; three already sitting in a Depot satisfies it
immediately. This follows from where completion is measured (below) and
matches how stock orders work in the genre. It is a deliberate choice, not
an oversight: an order says what the base should hold, and the scheduler's
whole job is closing the gap between that and what it holds now.

**Completion is measured across Depots and machine output buffers.** Not
the player's inventory, and not a count of units produced. What the base
holds is the question, and where it holds it does not matter.

## The design decision this rests on: derived, never stored

**A work order stores an item and a quantity. Nothing else.** No
per-machine plan, no unit targets, no progress counters.

Everything the scheduler and the status screen need is recomputed from live
world state each time it is asked. This is the same call this repo has made
five times already and documented every time: `Game::contract_board`,
`descriptions.rs`, `Game::build_radius`, `Game::wielded_program`, and the
Stack's regenerated frames. Each of those entries in `CLAUDE.md` records
the same payoff — the derivation cannot go stale, needs no save field, and
costs no migration when the thing it derives from moves.

The alternative was considered and rejected: multiply the recipe tree
through at queue time into fixed per-machine targets (Mining Node ×24,
Lathe ×6, Disk Press ×3) and count payouts against them. That produces a
trivially readable status bar and a plan that is confidently wrong the
moment a machine is demolished, upgraded, or fed from stock the plan did
not know about. It is the second copy that drifts, which `CLAUDE.md`
records biting this repo four times.

The two costs of deriving are real and are paid explicitly:

- **"Percent done" is not a stored number.** The status screen runs the
  same walk the scheduler runs, so what the player reads is what the
  scheduler believes *by construction* rather than by a comment claiming
  the two agree.
- **Staff could thrash between machines tick to tick.** The scheduler's
  assignment rule prevents it structurally: only *idle* staff are ever
  assigned, and a program already posted where a want still exists is never
  moved.

## What the mechanics already give us, free

Three existing behaviours decide this design's shape, and all three were
verified against the source rather than remembered.

**An unstaffed assembler does not pull.** `assembler_system` skips any
machine with no `TaskKind::GatherResource` worker on it — the pull phase is
behind that gate, not beside it. So a Lathe with nobody on it does not fill
its own input, and staffing a machine is what makes it draw from upstream.

**A machine feeds its neighbour before it feeds a Depot.** A hauler departs
only when its machine reads `MachineStatus::Clogged` (`haul_step_system`),
so a chain supplies itself first and only genuine surplus is carried away.

**Output buffers are finite.** A machine works until its output is full,
and then it can do nothing until something drains it.

Together these mean the base paces itself. A body stays on a machine until
that machine clogs; clogging is what releases it. The behaviour the player
described — work the deepest requirement until it is made, then move to the
next — is not something the scheduler has to sequence in phases. It falls
out of `can_progress` being false for a clogged machine.

**A corollary that must not be quietly fixed later:** nothing pulls out of
a Depot back into a machine's input. Intermediates hauled to a Depot are
dead to the chain and an order re-makes them. Making haulers restock
machine inputs is a separate feature with its own consequences and is
explicitly out of scope here.

## Data model

Three additions. All are additive, named fields, so per `CLAUDE.md`'s save
entry they cost **no `SAVE_FORMAT_VERSION` bump, no migration code, and no
tool**: a file written before this feature loads with them defaulted.

| Addition | Shape | On disk |
| --- | --- | --- |
| `components::BaseStaff` | marker on a tamed program assigned to the base | `#[serde(default)] staff: bool` on `CreatureSave` |
| `resources::WorkOrders` | `Vec<WorkOrder { item: ItemId, qty: u32 }>` | additive `#[serde(default)]` field on `SaveData` |
| `components::StandingJob` | `{ work: bool, guard: bool }` on a **structure** | two additive `#[serde(default)]` bools on `StructureSave` |

`WorkOrder` is a **named struct, never a tuple.** `CLAUDE.md`'s save entry
records why: RON parses a `(` in a struct position as the start of named
fields, so a `Vec<(ItemId, u32)>` can never be widened and cannot be
converted to a named struct either. Two fields shipped in that shape
already and both had to be drained into named successors. A third is not
being added.

`StandingJob` lives on the structure entity rather than in a resource keyed
by tile, which is the deliberate opposite of `BuybackLedger`. A shelf
outlives its building on purpose; a job order must not — a Shield rebuilt
on the footprint of a demolished one should not inherit a standing guard
nobody asked for.

Staff and party are **disjoint sets**. `assign_cronjob` already drops a
worker out of `Party`, and both draw from the same `pet_capacity` roster of
three plus bonuses. `tuning.rs` already states the tension this formalises:
*"every program at a machine is one absent from the party"*. Work orders
make that split an explicit, visible choice rather than a side effect of a
menu action.

## The scheduler

One new module, `crates/engine/src/game/work_orders.rs`: two pure functions
and one system, chained before `task_progress_system` so a body assigned
this tick progresses this tick.

### `can_progress(machine) -> bool`

Output has room, **and** for an assembler, its input holds at least one
batch of each ingredient *or* every shortfall is available in an
orthogonally adjacent feeder's output.

This is the predicate that releases a worker. A clogged machine cannot
progress, so it stops wanting a body, so the body goes somewhere useful.

### `wants(order) -> Vec<(Entity, depth)>`

The recursive walk, and the whole of the priority rule.

Find the deployed machine producing the ordered item. If it can progress,
it wants a body at the current depth. If it cannot, recurse into each
ingredient it is short of at depth + 1. Sort deepest-first.

So with nothing in the base, an order for Routine Disks yields Mining Node
before Lathe before Disk Press — the player's "lowest required structures
first". The walk terminates because the recipe tree is finite and output
buffers are finite.

**A machine reached twice is kept once, at its deepest position.** No
shipped assembler recipe has more than one ingredient, so a branching chain
that arrives at the same feeder down two paths is unreachable today — but
the engine's multi-input support is real and mods may ship two-ingredient
assemblers, which is exactly the case `chains::a_machine_short_one_of_its_
two_ingredients_stays_starved` already walks with a modded machine. Keeping
the deepest is what stops a shared feeder being staffed second on behalf of
one branch while the other still needs it first.

### `schedule_base_labour`

Per tick:

1. Take the front order. If base storage already holds its quantity, the
   order is complete — pop it, announce it, and take the next.
2. Build `wants` for that order. If the walk yields nothing at all, the
   order is **stalled**, not complete: take the next order instead and
   leave the stalled one in the queue, marked. Append standing jobs after
   whatever order is being worked, in a stable tile order, at the lowest
   priority — so a Research Node or a guarded Shield is filled only by a
   body no order needs.
3. Leave in place any staff already posted where a want still exists.
4. Unpost any staff whose machine no longer wants a body.
5. Fill the remaining wants, deepest first, from idle staff.

Steps 3 and 4 are the anti-thrash rule and are not optional. A scheduler
that rebuilt every posting each tick would walk the whole roster across the
base whenever a buffer changed by one unit.

**A stalled order must not block the queue.** Queue-time refusal catches a
broken line when the order is placed, but a machine can be demolished, or
raided to destruction, after that — so the front order can become
unfillable mid-run. Skipping it rather than blocking is what stops one dead
order freezing a base that could still work the three behind it. It stays
in the queue, and the status screen says which machine went missing; the
player cancels it or rebuilds.

**Cancelling an order unwinds nothing**, because nothing was wound. There
are no per-machine targets to roll back and no reserved stock to release —
the next tick simply derives a different answer. That is the
derived-never-stored decision paying out somewhere it was not designed for,
which is the usual pattern with it.

**Zero base staff is a valid, quiet state.** Orders queue and report
normally, and nothing is posted. The status screen says the base has nobody
in it rather than that the order is stalled — those are different errands.

**The assignment itself reuses `assign_cronjob`'s body.** Same `Task`, same
`CronjobSave`, same hauling, same `work_ticks_for` rate baked in at
assignment. The scheduler drives the mechanism that already exists; it does
not replace it. That is what makes an existing save's postings survive and
what keeps the walk-in, the depot errand and the `Stranded` marker working
without being reasoned about again.

## Idle staff

Staff with no want to fill park on a ring around the Home, one tile per
staff index, stepping on a cycle keyed to `(tick, index)`.

**Deterministic, drawing no `GameRng`.** This is not fastidiousness.
`CLAUDE.md` records three separate occasions where a shifted RNG stream
silently rewrote the outcome of a seeded test in an unrelated file, and
world generation is barred from that stream outright for the same reason. A
milling draw taken every tick for every idle program would shift the shared
stream harder than anything currently in the game.

`views::drawn_on_surface_map` widens from "is this worker away from its
post" to "is this program's `Position` honest". It stays one function,
called by both `render/base.rs` and `Game::find_target_in_direction`, so
what the map draws and what `x` can name remain the same set — the property
that entry in `CLAUDE.md` exists to protect.

The parking ring must respect the same rules a posted worker's field does:
no tile a `Structure` stands on, and inside the build radius.

## What is removed

`Assign a cronjob` and `Post a guard` leave `BASE_ROWS`. `assign_cronjob`
and `assign_guard` become `pub(crate)` — the scheduler's, not a menu's.
Making them private is the barrier; removing the menu row is only the
convention, and `CLAUDE.md` is explicit that the private-field kind of
barrier is what actually holds a rule.

Two rows arrive:

- **Work orders** — queue an order, cancel one, read status.
- **Base staff** — move programs into and out of the base pool.

Standing jobs are a toggle on the structure screen, not a menu row: the
question "should this machine always be running" belongs to the machine.

`Work a structure yourself` is untouched. The player is not staff.

**On load, any tamed program already holding a `Task` is marked
`BaseStaff`.** An existing base keeps its workers, and they are absorbed
into the pool rather than standing down. This is a load-path rule, not a
migration, and costs no version bump.

## Refusal at queue time

`Game::queue_work_order(item, qty)` walks the chain before accepting
anything, and names the break rather than accepting an order that silently
never moves. Fail fast, per `CLAUDE.md`.

The refusals:

- **Nothing deployed makes it.** "No Disk Press deployed — that is what
  presses a Routine Disk."
- **The line is physically broken.** A Disk Press not orthogonally adjacent
  to something producing Blank Substrate can never be fed, however much
  substrate the base holds. Name which link is missing.
- **The item is not producible at all.** Nothing declares it as a
  `work.produces` or an `assembles.item`.

**Research is excluded without a special case.** `research_data` is
`banked`, and `deliver_payout` sends a banked item straight to the player's
bank — it reaches no `output`, so no machine can ever be fed from it and
nothing can hold a stock of it. The chain walk refuses it on its own terms.
The Research Node is still staffed, as a standing job.

A refusal must be checked *before* anything is spent or written, the same
ordering argument `use_symlink` makes about `clear_stack` and
`install_routine` makes about the disk.

## Status

`Game::work_order_report()` runs the same `wants` walk the scheduler runs.
Not a walk that mirrors it — the same one, called. Per `CLAUDE.md`, a claim
that two places use one rule has to be a call and not a comment.

Per order: the item, `have / target`, and its position in the queue. Per
machine in the chain: its name, who is posted on it, and what it is short
of. A stalled base therefore says *which* machine is starved and of what,
rather than only that nothing is happening.

Row counts are owned by app-core and drawn by gui, as the history screen
and the structure roster already are, so the screen cannot open on a row
that is not drawn.

## Testing

Engine-side, all deterministic — the scheduler draws no RNG, so nothing
here can flake.

- **Chain arithmetic.** An order for a three-deep item resolves to the
  right machines in the right depth order.
- **Each refusal**, individually: no machine, broken adjacency, unproducible
  item, banked item.
- **Priority under scarcity.** One staff member on a three-machine chain
  works upstream first and moves downstream as each machine clogs. Two and
  three staff distribute correctly.
- **Completion.** An order whose quantity is already in a Depot completes on
  the first tick without staffing anything.
- **Standing jobs fill only from spare bodies**, and are dropped the moment
  an order needs the body.
- **A guard standing job survives a raid** and re-fills.
- **Anti-thrash.** A posted worker whose machine still wants a body is not
  moved when an unrelated buffer changes.
- **A stalled front order does not block the queue.** Demolish a machine
  mid-order and the order behind it is still worked.
- **Zero base staff** queues and reports without panicking or posting.
- **A shared feeder in a branching chain is staffed once**, at its deepest
  position — walked with a modded two-ingredient assembler, since no
  shipped recipe can reach the case.
- **Load absorption.** A save with a hand-posted cronjob loads with that
  worker as `BaseStaff`, still on its machine.
- **Round trip.** A save written before this feature loads clean; one
  written with orders round-trips them.

Fixtures must use `work_node_parts()` and `park_at_post()` from
`crates/engine/src/tests/support.rs`. A hand-spawned node short of `Stock`
or `MachineStatus` is skipped by the query and silently produces nothing,
and a worker left where it was spawned is not at its station — both read as
a scheduler that does not work rather than as a fixture short something.

A test fixture hand-writing a `Task` against an assembler must set
`required` to that machine's real `ticks_per_unit`; a hand-written `1` means
"finish a batch every tick".

## What is not gated

`balance_sim` is RNG-free and models no base at all — it cannot see a work
order, a posted program, or a production chain. The arena models player
combat, not the base. So **nothing in this feature is covered by an
automatic balance gate**, and the pacing question it raises — whether
automating staffing makes the base too productive for the zone curve — is
answerable only by play.

This is stated rather than discovered: the same gap already covers the three
species base jobs, which shipped on argument alone.

## Scope boundaries

Out of scope, each for a stated reason:

- **Haulers restocking machine inputs from a Depot.** A separate feature
  with its own consequences; without it, Depot stock stays dead to the
  chain and an order re-makes intermediates.
- **Orders for banked items,** including research. Structurally excluded by
  the chain walk, not special-cased.
- **Several orders worked at once.** The queue is front-first. Splitting
  staff across live orders makes it impossible to read why a machine is
  idle.
- **Auto-building missing machines.** An order refuses when the line is
  broken; it does not offer to fix it.

## Crates touched

- **engine** — `components.rs`, `resources.rs`, `save.rs`, `views.rs`,
  `systems.rs`, and the new `game/work_orders.rs`.
- **app-core** — `Mode` variants, the `BASE_ROWS` table, key handlers.
- **gui** — one new screen.

Three crates and a save-schema change, which is what puts this on the full
spec-and-plan pipeline rather than an inline build.
