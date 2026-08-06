# Depots and hauling programs — design

**Date:** 2026-08-06
**Status:** designed, not implemented.

## The problem

A tamed program that isn't in your party is invisible as an actor. It keeps
its `Position` and its `Glyph`, so it is drawn on the map — but taming strips
`WanderAi` (`game/combat_rewards.rs:386`), and `assign_cronjob` never touches
`Position`. So every program you have ever tamed sits frozen on the tile you
caught it on, forever, including the ones nominally working your base. The
base produces, but nothing in it moves.

Separately, `MachineStatus::Clogged` is a hard brake. A machine fills its
20-unit `output` and stops until the player personally stands orthogonally
adjacent and collects. There is no way for the base to unstick itself, so
`DEFAULT_OUTPUT_CAPACITY` is literally "how long a base runs unattended"
(`tuning.rs:975`).

These are one feature. Give a worker somewhere to put a full buffer and it
has a reason to walk.

## What this is for, in order

Ranked deliberately, because the three pull in different directions and the
ranking is what decides the tie-breaks below.

1. **Liveliness.** Tamed programs visibly do something. The depot is the
   excuse for the errand.
2. **Consolidation.** One place to collect from instead of walking the line.
3. **Uptime.** The base keeps producing while you are in the Stack. Wanted,
   but *bounded* — a consequence to be paid for, not a goal to maximise.

Uptime ranking last is why the walk costs production rather than running
alongside it, and why the depot is a build rather than a freebie.

## Decisions

**The machine's own worker hauls.** No courier post, no new `TaskKind`, no
drone spawned by the depot. The program posted to a machine leaves that post,
carries a load, and comes back. Its production time pays for the trip.

The alternative considered and rejected was a dedicated courier posted to the
depot. It puts the cost somewhere cleaner, but it spends a whole program on
logistics and gives the map one actor per depot rather than one per machine.

**The worker physically carries the goods, so there is no third reach rule.**
`Stock`'s `output`/`input` asymmetry and `collect::ORTHOGONAL` still govern
machine-to-machine pulls and player collection exactly as documented. A
carrier is not a reach extension, and the CLAUDE.md invariant survives
without a caveat.

**A worker hauls from any machine whose output is full**, not only from
end-of-line machines. Consequence, accepted knowingly: a machine that clogs
is over-producing relative to downstream, and removing part of its buffer
deprives the next machine of reserve it would have eaten during the round
trip. The carry cap below reduces this to leaving 15 of 20 behind, and every
shipped assembler recipe is a single ingredient in a straight line, so this
bites only bases the player has mis-laid.

**The trigger is `MachineStatus::Clogged`**, not a new threshold. That state
already exists, `set_machine_status` already announces it on transition only,
and the machine has already stopped producing when it fires — so the walk
costs nothing that was not already lost.

**A worker walks to its post when assigned**, rather than snapping there.
This is a visible change to an existing feature: posting a program tamed 40
tiles away now costs 40 ticks before production starts. Accepted, because
goal 1 is the ranked priority and the first errand being "go to work" is the
cheapest possible way to teach the player that programs walk.

**Walking workers collide with nothing but terrain and structures.** They
share tiles with the player and with each other. This avoids a worker parking
in a one-tile gap and walling the player into their own base, and avoids the
frozen-actor trap `NEST_TETHER_RADIUS` already sprang once, where a guardian
displaced past its tether had no legal move and stopped for the rest of the
run.

**Any number of depots; nearest wins.** Placement is an optimisation the
player makes — a second depot across a sprawled base halves half the routes.
"Nearest" is the same rule whether there is one depot or five, so no
special-casing.

"Nearest" means Chebyshev distance from the worker, ties broken by the
depot's `(x, y)`. Deliberately *not* `walk_field` path cost: that would be a
second field per worker per tick for a difference that only shows up when a
wall sits between two near-equidistant depots, and the tie-break exists for
the same reason `assembler_system` sorts by position — Bevy's query
iteration order is not stable, and a base that picks a different depot after
a reload is a flaky test waiting to happen.

## Structural shape

### `Carrying { item: ItemId, qty: u32 }`

The only stored state. Present on a worker exactly while it holds a load.
Everything else is derived:

- destination = nearest depot if `Carrying` is present, else `Task.target`'s
  tile
- "at post" = standing on the destination
- "in transit" = not standing on it

A `HaulState` enum with `ToPost`/`AtPost`/`ToDepot`/`Returning` was
considered and rejected: three of its four variants must be hand-synced with
`Position`, and a desynced pair is a worker standing at its machine insisting
it is still walking. Fields on `Task` were rejected because `Task` is
deliberately generic across job kinds, and a `Guard` task would carry a
permanently-`None` cargo field.

The single `(item, qty)` pair is only honest because of the carry cap.
`Stock::output` is a `BTreeMap<ItemId, u32>` and can hold more than one item
id — the component doc says so explicitly. A whole-buffer drain would have
needed `Carrying(BTreeMap<ItemId, u32>)` and a matching map in the save. With
a cap the worker takes `HAUL_CARRY_CAPACITY` units of *one* item, chosen as
the first key in `BTreeMap` order — deterministic, and already load-bearing
for exactly this reason (`Stock` keys by `ItemId` in a `BTreeMap` so the pull
phase and the save encoding are stable run to run).

### `walk_field`

`pursuit_field` (`game/pursuit.rs`) cannot be reused as-is: it excludes
`Biome::Platform` on top of the walkability check, because that rule exists
to keep a nest swarm off the base slab. A hauler needs to walk *across* the
base slab, which is the exact tile set that filter removes.

Extract the Dijkstra body into:

```rust
walk_field(map: &mut WorldMap, origin: (i32, i32), radius: i32,
           step_allowed: impl Fn(&Tile) -> bool) -> HashMap<(i32, i32), u32>
```

`pursuit_field` becomes a one-line wrapper that keeps its name, its doc
comment and its `Platform` rule. Copying the walk is what CLAUDE.md warns
against; a shared walk with a per-caller predicate keeps "a second
walkable-but-off-limits rule belongs in that filter" true for both callers.
Two implementations exist today, so this is not speculative extraction.

## Mechanics

**Departure.** `task_progress_system`'s clogged branch (`systems.rs:414`)
already holds `progress` at `required` and calls `set_machine_status`. It
gains one action: take `min(HAUL_CARRY_CAPACITY, qty)` of the first item in
`output` into `Carrying`, and leave. `progress` stays held at `required`, so
the machine pays out on the very next tick after the worker is back — the
existing comment's reasoning extends unchanged.

**Stepping.** A new `haul_step_system`, `.chain()`ed with
`task_progress_system` and `assembler_system` for the reason CLAUDE.md
already gives about those two: Bevy can see the `Stock` conflict but not the
disjointness, and an arbitrary-but-fixed order is not the same as a stated
one. Each tick, for every worker with a `Task` not standing on its
destination: build `walk_field` from the destination bounded by the build
radius, step to the lowest-cost neighbour. Only workers in transit pay for a
field, and most workers on most ticks are at their posts.

**Arrival.** On a depot tile with `Carrying`: move the load into the depot's
`output`, drop `Carrying`. The return leg starts next tick because the
destination flips back automatically. No arrival event and no state
transition to write — that is the payoff of deriving state from position.

**Production gating.** `task_progress_system` advances `progress` only when
the worker is on its machine's tile. This is what makes the walk cost real
rather than free uptime, and it is the same predicate that picks the
destination, so the two cannot disagree.

**`MachineStatus::Unstaffed`.** New variant: the worker is en route, or
cannot reach its machine. Free to add — the component is deliberately not
saved. It must win over both `Clogged` and `Running` whenever the worker is
off its tile: after shedding 5 units the machine is no longer full, and
without the precedence rule the pane would claim a machine is running while
nothing is producing.

## The depot

One `.ron` in `assets/structures/`. No Rust content, per the moddability
rule.

- `work: None`, `assembles: None`, `pet_slot_bonus: 0`
- `output_capacity: 100` — five machines' worth of full buffers
- `build_cost: [("core_fragment", 12)]`, just above the Data Cache's 10, so
  it is an early build but not the first one

`accepts_a_program` needs **no change**: it is `runs_a_job()` (a `work` or
`assembles` def) or the presence of a `ResourceNode`, and a depot has
neither. It falls out of the existing predicate that a depot never enters the
cronjob menu — a depot is delivered to, not worked.

The player collects from it through `collect_adjacent` with **no code
change**: it has a `Stock` with a populated `output`, which is the only thing
that function has ever asked about. Goal 2 costs nothing.

`assets/structures/README.md` gets the new file documented if any field
meaning changes; adding a structure alone needs no schema note.

## Tuning

`pub const HAUL_CARRY_CAPACITY: u32 = 5;` in `tuning.rs`, in the base/work
section. With `DEFAULT_OUTPUT_CAPACITY` at 20 and no per-structure overrides
shipped, steady state is one round trip per 5 production cycles rather than
one per 20 — four times the visible motion, and 15 units left in the buffer
for a downstream neighbour throughout.

## Bounds and failure modes

Throughput becomes `capacity / (cycle_ticks * capacity + 2 * distance)`. The
walk shortens with depot placement and never disappears. **No depot built
leaves today's behaviour exactly intact**, so the feature is opt-in and
existing saves are unaffected until the player builds one.

Each failure mode is decided, not left to emerge:

- **No depot** → nothing changes; the clogged branch holds as it does today.
- **Every depot full** → the worker returns to its machine and pours the load
  back into `output`, re-clogging it. The base stalls loudly rather than
  losing goods.
- **Depot destroyed mid-walk** → re-target the nearest remaining depot; if
  none, as above.
- **Machine unreachable** → `Unstaffed`, no production. Visible, not silent.
- **Task cleared mid-haul** → `remove_structure` (`game/building.rs`) and
  `damage_structure` (`game/upkeep.rs`) already clear worker `Task`s, and
  both must also drop `Carrying`, or a worker keeps a load with nowhere to
  put it. This is the "destroying a structure has two paths" trap from
  CLAUDE.md and it applies unchanged.

## Save format

**23 → 24, breaking.** `CreatureSave` gains
`carrying: Option<(ItemId, u32)>`; bincode is positional, so existing `.bin`
saves stop loading. `dev-saves/` templates are RON and update in place.
`position` and `cronjob` are already saved, so nothing else moves.

Per CLAUDE.md this is a minor version bump and a CHANGELOG entry under a
**Breaking** heading.

## Testing

Engine tests. Any hand-spawned work node needs `work_node_parts()`, or
`task_progress_system`'s query skips it and the fixture silently produces
nothing.

- a clogged machine's worker departs carrying `HAUL_CARRY_CAPACITY` units,
  and `progress` stays at `required`
- a full 20-unit buffer takes four trips to clear
- production does not advance while the worker is off its machine's tile
- the worker reaches the depot, the depot's `output` holds the goods,
  `Carrying` is gone, and `collect_adjacent` beside the depot yields them
- nearest-depot selection with two depots **spawned in the opposite order to
  their positions**, the same trick `assembler_system`'s sort test uses, so
  it cannot pass on iteration order alone
- a full depot round-trips and re-clogs the machine
- a destroyed depot re-targets; the last depot destroyed re-clogs
- `remove_structure` and `damage_structure` each drop `Carrying`
- `Unstaffed` wins over `Running` while the worker is away
- posting a program walks it to its machine before production starts
- `walk_field`: `pursuit_field` still refuses `Biome::Platform`, and a haul
  route still crosses it

`balance_sim` is untouched — it models battle, not work — so this throughput
change is ungated there, as the rest of the base economy already is.

## Deliberately not in scope

- No hauling between machines. A worker delivers to a depot, never to another
  machine's `input`; that would be a genuine second reach rule.
- No depot-to-depot balancing.
- No worker inventory UI. `Carrying` shows up nowhere the player reads except
  implicitly, as a program walking with a load.
- No hauling *inside* a Stack frame. Frames have no structures, and a
  worker's `Position` is a surface coordinate. Workers do keep hauling on the
  surface while the party is underground — the base ticks either way, and
  that is exactly the bounded uptime goal 3 asks for.
- No item filters or depot priorities. Nearest wins, full stop.

## Not playtested

Every number here is an unplayed judgement call: the carry cap of 5, the
depot's capacity and build cost, and the walk-to-post delay on assignment.
The suite passing is not evidence that a base *feels* alive or that the
walk-to-post delay reads as intent rather than as a bug.
