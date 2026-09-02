# Departure memories, and the structures that carry them

**Status:** brainstorm parked, three questions open — **not approved, not implemented**. No departure memory exists in `assets/memories/`. Audited 2026-09-02 against the source tree, not against this header.

An owned program remembers when another program is sold, decompiled or
killed, and holds it against the bench it happened at. One new structure —
an Archive — is the price you can pay to spare them that. Morale's reach
widens by one rule.

## Where the brainstorm started, and where it went

The opening question was "what base structures could evoke positive or
negative memories". The negative half turned out to need **no new
structures at all**: the game already has two benches whose whole purpose
is doing something invasive to a program you own, and one function every
such departure already runs through. The new structures are only worth
building as the *answer* to that, not as invented hazards.

Rejected on the way, and why:

- **Slag Pit** (a hazard whose neighbours' workers form a grudge) — the
  departure hook does the hazard job better, and with content that already
  ships.
- **Rollback Console** (spend an item, delete a program's worst grudge) —
  a second, more general verb than the Archive, and it competes with it.
  Reconsider only if the Archive proves too narrow.
- **`impressions:` as a data-driven `StructureDef` block** — a trigger
  vocabulary (`ParkedAt` / `PostedAdjacent` / `PostedHere`) invented from
  two samples, which `memories.rs`'s own module doc argues against. Grow
  into it at five or six samples, not two.

## What is already there (verified 2026-08-24)

- `Game::dissolve_tamed_program` (`crates/engine/src/game/trade.rs:420`) is
  documented as **"the one function sale, extraction, battle death and a
  raid defender's death already agree through"**. Callers:
  `sell_companion` (`trade.rs:474`), `routines::extract_routine`
  (`game/routines.rs:601`), `combat_teardown.rs:283`, and the raid-defender
  branch at `game/base/upkeep.rs:413`. `fuse_companions` does its own
  `retain`/`despawn` and is deliberately **not** a caller — see the
  two-destruction-paths seam in `CLAUDE.md`.
- The two benches that earn the grudges already ship:
  **iso Market** (`assets/structures/black_market.ron`, "sell a program for
  a tenth of its power") and **Compiler** (`assets/structures/compiler.ron`,
  "the bench that extracts a routine out of a program you own").
- Memory reaches the game at exactly **two** places today:
  `park_idle_staff`'s third rejection (`opinion_of` vs
  `MEMORY_AVOIDANCE_THRESHOLD`, `game/base/work_orders.rs`) and
  `morale_shift` into `systems::mining_success_chance`.
- `MemorySubject::Program(ProgramId)` (`components.rs:1307`) carries a
  value, not an `Entity`, so it survives the despawn of the program it
  names.

## The seam: a `Departure` reason

`dissolve_tamed_program` takes the reason. Each caller knows its own;
nothing else has to.

| Departure | Subject | Sign |
|---|---|---|
| `Sold { at }` | `Structure` (the Market) | strong grudge, long half-life — this one is purely the player's choice, for money |
| `Decompiled { at }` | `Structure` (the Compiler) | strong grudge |
| `KilledInBattle` | `Program` (the lost one) | modest — grief, not blame |
| `KilledDefendingTheBase` | `Program` (the lost one) | same def; a separate "died for us" positive is content bloat until asked for |

Written to **every other program on the roster**. Three properties fall out
without a branch at the call site: the player has no `Memories` and so is
excluded structurally (`Game::remember` no-ops without a store), hostiles
likewise, and the departing program's own store is discarded with it.

The grief def pairs with `bonded_in_battle`, which is positive and already
`Program`-subject, so *fought beside them, then lost them* nets out on one
subject.

Two things the trigger has to get right:

1. **Write before the despawn.** The survivors are read off the roster, and
   the roster is what `dissolve_tamed_program` is about to shrink.
2. **It is an edge, not a stretch** — `swept_here`'s shape, not
   `settled_in`'s. A departure is an event, so it writes once, at the
   moment, and never on `MEMORY_POSTING_PERIOD`.

Both benches then inherit the two existing hooks for free: a deep enough
grudge makes `park_idle_staff` refuse to stand a body near them, and the
grudge nets against `settled_in`/`jammed_here` on the same `Structure`
subject.

## The one new structure: the Archive

A machine with an input buffer like any other, so it plugs into work
orders, hauling and depots for free. When a program departs, if the Archive
holds a unit, it spends one and the survivors form a positive `Nothing`
memory instead of the grudge. Empty, it does nothing.

The shape is load-bearing. A permanent "programs don't mind any more" flag
would nullify the mechanic it sits beside; a **price paid per departure out
of the production chain** is a decision the player can be caught short on,
and it is a real sink. It is also the lever the brainstorm ranked first:
build cost plus a fed buffer, and roster attrition stops poisoning the base.

Expressed as a `#[serde(default)]` flag on `StructureDef` — the shipped
pattern that `stores`, `issues_contracts` and `raidable` already follow —
and not as a data-driven effect block, per the rejection above.

### Deferred to a later slice: the Idle Dock

The layout half, kept out of slice 1. A structure flagged `parks_staff`
becomes the `center` `park_idle_staff` lays its ring around instead of
Home, and standing on that ring writes a positive `Structure` fondness
about the Dock.

**Trap 1 — the parking hook is a refusal, not a preference.**
`park_tile(home, index, tick)` (`game/base/work_orders.rs`) is a pure
function of its three arguments: an idle body is *assigned* a slot on a
Chebyshev ring of `IDLE_STAFF_RING_TILES = 3` around Home, rotating every
`IDLE_STAFF_STEP_TICKS = 6`. The memory check only lets a body *decline*
what it was handed. An amenity cannot attract anyone by being pleasant —
moving the ring's centre is the only verb available, and it is the better
one anyway.

**Trap 2 — a `BaseTile` fondness on the parking ring would gut the store.**
A ring of 3 is a 24-tile perimeter and `MEMORY_CAP_PER_PROGRAM` is 12. A
program parked a few hundred ticks would hold twelve weak one-strike tile
fondnesses and would have evicted `mauled_by`, `bonded_in_battle` and every
work memory to make room. Any amenity that writes from a *rotating*
position must use a `Structure` subject — one entry, reinforced — and never
`BaseTile`.

## Widening morale, a little

One rule, and the split falls out of a branch that already exists:

> **Where a work cycle can fizzle, morale moves the odds. Where it cannot,
> morale moves the clock.**

- **Extraction** can fizzle, and keeps its existing `morale_shift` term on
  reliability in `systems::mining_success_chance`. Unchanged.
- **Assembly** and **digging** always land — an assembler consumes its
  inputs and produces, and rock cannot dodge (`Game::strike_rock`, and
  mining deliberately does not go through `battle::resolve_attack`) — so
  morale moves their rate instead, through one new shared helper beside
  `morale_shift`, with its own cap constant in `tuning.rs`.

No double-counting: each job type gets exactly one morale term.
`work_ticks_for` (`game/base/building.rs:495`) already matches on
`(&def.work, &def.assembles)`, so the branch is there to read.

**The decision inside this one:** `work_ticks_for` bakes `Task::required`
at posting time, so a morale term placed there is read once and never
again — a stable base would never see the Archive take effect, because
`schedule_base_labour` only re-posts on a diff. Recommendation: apply it
**live in `task_progress_system`**, where progress accrues, so acting on
morale visibly changes throughput.

`balance_sim` models no base production at all, so none of this is gated
numerically. The instruments are `--template chains` and a session.

## Open questions — answer these before a spec is written

1. **Does the Archive read right**, or should the departure grudge just
   stand with no counter at all?
2. **Battle death** — grief on the roster at all, or should only the
   player's *choices* (sale, extraction) leave a mark?
3. **Is "odds for extraction, clock for assembly and digging" the amount of
   widening** meant by "widen it a little"?

## Process note

Architectural by `CLAUDE.md`'s weight rule — a `StructureDef` schema change
plus engine hooks — so the path after these three answers is: finish this
into a spec, then `writing-plans`. No save-format bump is expected: the new
memory entries are ordinary `Memories` records, `Departure` is a parameter
and never stored, and a new `StructureDef` field is `#[serde(default)]`.
Assets touched: three or four new `.ron` files in `assets/memories/`, one in
`assets/structures/`, plus rows in `MEMORY_TRIGGERS`
(`crates/engine/src/tests/assets.rs:1887`) or the build fails.
