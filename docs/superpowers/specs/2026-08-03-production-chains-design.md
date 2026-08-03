# Adjacency-fed production chains

Status: implemented 2026-08-03 on `factory-chains`, all twelve plan tasks
green. **Never played.** Two deviations from what is written below, both
recorded in the commits that made them: the intermediate's standalone value
is the Assembly Bay's `build_cost` rather than trade value (the Market's
`sell_rate` is flat per unit, so refining is a net loss at the counter and
there is no per-item price to raise), and `player_gather_system` redirects
into the buffer alongside the cronjob path (it shares `resolve_gather_cycle`,
and retiring the deposit pool would otherwise leave it unpaced).

## The problem

The base is idle income with one lever. `work` nodes produce from nothing,
tick regardless of where the player is, and drop output straight into the
player's inventory. `passive_process` can consume as well as produce, but
only while the player stands inside its radius — and no shipped structure
uses it at all. So the two halves of a factory live in different systems
and neither one has both:

| | inputs? | runs while you're away? |
|---|---|---|
| `work` | no | yes |
| `passive_process` | yes | no |

Nothing about a structure's *position* matters to any of this. Layout is
not a decision, chains are not expressible, and nothing ever backs up.

## What this builds

Machines with local storage that feed each other by touching. A chain is a
physical line across the base; a machine with two ingredients needs both
feeders adjacent to it; a machine nobody visits fills up and stops.

Three things gate a machine, and each has its own word in the base log:

- **starved** — input short, upstream too slow or not adjacent
- **clogged** — output full, downstream too slow or the player hasn't collected
- **idle** — no program assigned

## Design

### Stock

A `Stock` component on every deployed structure, holding an `input` and an
`output`, each a map of `ItemId` to count.

Neighbours may take from a machine's `output`. Nothing outside a machine
ever touches its `input`. That asymmetry is the whole of directionality —
it is why a chain flows one way without belts existing.

Output size comes from a top-level `capacity` field on `StructureDef`,
optional and defaulting to a `tuning.rs` constant. It has to be top-level
rather than the existing `work.capacity`, because an assembler declares
`assembles` and no `work` block at all and still needs an output size — and
because a storage building later will declare neither. Input size is
derived, not authored: a machine holds at most
`INPUT_STOCK_BATCHES` (2) times each ingredient's recipe amount, so a
greedy machine cannot drain a shared feeder dry. That constant lives in
`tuning.rs`.

### The `assembles` field

```ron
assembles: Some((item: "patch_routine", ticks_per_unit: 8)),
```

The machine runs *that item's own* `craftable.cost` as its recipe. There is
no second recipe format. A modder who adds a craftable item gets an
automatable one for free, and the two can never drift apart because there
is only one of them.

`craftable.cost` already supports multiple ingredients; nothing in the game
uses that yet. Every shipped recipe is N Core Fragments or N Portal
Fragments. So `x + y = n` needs no schema work — only content.

### The tick

Each tick, per machine, in two phases:

1. **Pull.** For each ingredient the recipe needs and the input lacks, take
   from the `output` of the four orthogonally adjacent structures, up to the
   input cap. Diagonals feed nothing.
2. **Work.** If the input covers a full batch *and* the output has room and
   a program is assigned, advance progress. On completion, spend the batch
   and add one unit to the output.

**Machines must be visited in a deterministic order.** Bevy query iteration
order is not stable, and two machines competing for one feeder's scarce
output would otherwise resolve differently between runs — a live source of
flaky tests and of a base that behaves differently after a reload. Sort by
`(x, y)` before the pull phase.

### Labor

Every machine needs an assigned program, assemblers included. Extractors
already do, so only assemblers gain the requirement.

This makes **programs the limiting resource, not fragments.** The pet cap is
`3 + pet_slot_bonus` summed over deployed structures, and a cronjob worker
counts against it while not fighting beside the player. So:

- A two-machine line costs 2 of a starting roster of 3.
- The first `x + y` assembler needs two feeder lines — five machines, five
  programs — which is unreachable until a Data Cache (+2) is standing.
- A second Data Cache is what buys back a party to adventure with.

Chain length is bought with roster capacity, and the growth lever already
ships. No new structure is needed to pace this.

**Program stats do not affect machine output in v1.** A worker is a token
spent, not a choice made. Making stats matter is the obvious v2 and is
where this wants to go, but it multiplies the balance surface of a system
nobody has played, and the chain has to be proven fun before it is worth
tuning who staffs it.

### Extractors deposit into their own stock

`work` nodes write to their `output` instead of the player's inventory.

This is the largest felt change in the design. Fragments stop appearing in
the player's pocket while they are away; the player comes home and harvests.
It is also the only thing that makes clogging real — without it the first
stage of every chain is an infinite source and nothing upstream can ever
back up.

It is the piece to reverse if the rhythm reads as a chore. Nothing else in
the design depends on it being irreversible.

### Collection

One key, emptying the `output` of every structure orthogonally adjacent to
the player into their inventory. The player pulls by the same rule machines
do, and like a machine can never reach another's `input`.

Structures block movement (`game/turn.rs:369`), so the player always stands
beside a machine, never on it — which is what makes the symmetry work.
Standing in the crook of an L empties three buildings; standing at the end
of a sprawled line empties one. Collection ergonomics become part of the
layout puzzle rather than a separate system.

The key must be one not already bound in app-core.

### Deletions

- **`passive_process`** — the field, `PassiveProcessor`,
  `passive_process_system`, its tests, and its README section. Zero shipped
  users, superseded entirely by `assembles`.
- **`work.capacity`'s deposit-pool mechanic** — the refilling reserve a node
  is "mined down" from. Output stock is what paces a node now, so the pool
  is redundant pacing. `capacity` keeps its name and becomes output size.
  `StructureSave::resource_amount` goes with it.

### Save format

`Stock` is per-structure state and must persist. `StructureSave` gains it
and loses `resource_amount`. `SAVE_FORMAT_VERSION` goes 19 → 20.

### Surfacing it

The player cannot play this without seeing buffers. Two places:

- **Base log** — a machine entering *starved*, *clogged* or *idle* says so,
  as `MessageSource::Base`. Entering, not every tick; a stalled base must not
  flood the pane.
- **`Game::structure_report`** — each structure's stock and stall state.
  Per `CLAUDE.md`, the row count is owned by app-core and drawn by gui, so
  any per-row transform belongs in the engine.

## Content

One complete chain, end to end. Concrete numbers are set during
implementation against `cargo test -p feral-processes-engine balance_sim`;
the shape is what this spec fixes.

| stage | structure | produces | from |
|---|---|---|---|
| extract | Mining Node *(exists)* | `core_fragment` | — |
| extract | Power Conduit *(exists)* | `power_cell` | — |
| refine | Refinery *(new)* | `bytecode_block` *(new)* | core fragments |
| refine | Winding Node *(new)* | `charge_coil` *(new)* | power cells |
| assemble | Assembly Bay *(new)* | `patch_routine` *(new)* | block + coil |

The two-machine line (Mining Node → Refinery) has to be worth building on
its own, since it is the only part affordable at a starting roster. Its
output is the intermediate, so the intermediate needs standalone value —
trade value is enough, and needs no new mechanism.

**Existing single-input recipes are left alone.** The chain's terminal
product is a new item, so this lands beside the current economy instead of
retuning it, and keeps the existing balance curves out of the blast radius.
An armour piece as a second terminal product is desirable and was asked for,
but it is the one part that can move a `balance_sim` curve — add it only
after the consumable chain is green, and treat a moved curve as the signal
it is meant to be.

## Testing

Engine unit tests:

- a diagonal neighbour feeds nothing
- a starved machine does not advance progress
- a clogged machine does not consume its input
- a machine with no program assigned does neither
- input stock caps at two batches
- a full three-stage chain produces the terminal item over N ticks
- pull order is stable: two machines competing for one scarce feeder resolve
  the same way across runs

Shipped-assets test: every `assembles` names an item that actually declares
a `craftable` recipe. Without it a typo'd mod builds a machine that can
never run, and says nothing.

Save round-trip: a base with partially filled buffers survives dump/pack.

A `dev-saves/` template capturing a mid-chain base, so the next session
testing this does not start with an hour of play.

## Deliberately not now

Storage buildings, transport programs, belts. All three are the same shape
as what is above — a structure with a buffer and a movement rule — so this
design is their foundation rather than an obstacle to them. Storage is the
one exception to the labor rule when it lands: a buffer needs no program to
sit there. Building none of them now is what keeps this to one spec.
