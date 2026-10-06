# Base bench: the `bench-economy` baseline and a tune that can be read

Follows [`chains` baseline](2026-10-05-base-bench-chains-baseline.md), which
ended with "a template with a drain, or nothing here is a statement about
flow". This is that template, and a tune on it.

## The claim

`dev-saves/bench-economy.ron`, a rearranged `chains`, sustains flow under
standing orders: seed 1, over the 4902 ticks before a siege stops the clock
(see below), the Assembly Bay makes 58 units (capacity 10), the Winding Node
176 (20), the Refinery 135 (20) and the Compiler 173 (20). Over a full 3000
ticks, seeds 1-3, the Bay makes 32-35, the Winding Node 97-108, the Refinery
81-86 and the Compiler 104-118. The output rises by the same amount every
thousand ticks. A tune on
it moved six machine speeds, produced a proposal that scores zero on hold-out
seeds, and is **not an artifact of the kinds the `chains` tune was** (no
buffer was enlarged, no machine slowed, no run stopped early). It is still a
"make everything faster" answer, because speed costs nothing in this model; do
not apply it.

Why `chains` stalled, each cause verified on that template and removed here:

| cause | in `chains` | in `bench-economy` |
|---|---|---|
| Depot room | Depots hold 50, `chains` stocks 150 Power Cells in each, so nothing can be shelved; the Mining Node's fragments never move and the Compiler never runs | the two stocked Depots hold 50; each production line has a 800-slot Depot Mk5 beside it |
| Power Cells on the shelf | with 300 cells shelved, `work_orders.rs:658` feeds the Winding Node from the shelf, so the Power Conduit is never wanted | 100 cells in all; three Conduits keep the shelf supplied once it drains |
| Route to a Depot | a one-tile corridor idle programs block, stranding the hauler, which holds the line's only worker slot | no machine has fewer than two free faces; the floor is 15 x 13 |
| Amenity | none; morale sinks and idle programs loiter | a Sandbox and a Defrag Bay with open ground around each |

Two further findings that cost most of the time and are not in the earlier
doc:

- **A single free face is a stall.** `hauling.rs` blocks every body's tile in
  the walk (`blocked_tiles`), so a machine whose one free neighbour is stood
  on by an idle program is `Stranded` for that moment, and every stranding
  writes a `stranded_at` memory (`memories.rs`, `note_strandings`) of about
  -5 morale. In the first rearrangements (one-tile faces, 9 x 9 floor) the
  on-shift staff fell from 15 to 2-5 within 3000 ticks and the grid
  collapsed with it.
- **One Power Conduit cannot carry the Bay line.** A Conduit makes 167 cells
  per 1000 ticks; the Winding Node can burn 250, and five Recharger Nodes burn
  50. With one Conduit the shelf drains, Rechargers go dry, the grid falls
  below the draw (the Winding Node reads `Unpowered`) and the line stops.

## How to reproduce it

```sh
cargo build --release --bin bench
for s in 1 2 3; do for n in 2000 5000; do
  cargo run --release --bin bench -- run --template bench-economy --ticks $n --seed $s \
    --order patch_routine:9999 --order bytecode_block:9999 --order ice_breaker:9999
done; done
cargo run --release --bin bench -- tune dev-tuning/economy-bench.ron   # search_seed 1337
```

Build `base-bench`, 16-core machine. Tune wall time 4m37s (user 50m47s).
A 3000-tick run is about 1.3 s.

## The numbers

Flow, seed 1, orders as above, items shelved at each 1000 ticks (all
shelves, so the running total of what the base holds). The run stops at 4902,
so the last row is the state there, not at 5000:

| tick | patch_routine | bytecode_block | ice_breaker |
|---|---|---|---|
| 1000 | 10 | 29 | 42 |
| 2000 | 20 | 55 | 83 |
| 3000 | 31 | 81 | 122 |
| 4000 | 41 | 110 | 158 |
| 4902 (the stop) | 51 | 135 | 189 |

Per machine, mean of seeds 1-3, 2000 ticks (the 5000-tick runs, which end
early, are in the next table):

| machine | units | running | starved | notes |
|---|---|---|---|---|
| mining_node (-4,-3) | 237 | 53% | | feeds the Compiler by adjacency |
| compiler (-3,-3) | 79 | 28% | 8% | |
| mining_node (3,0) | 221 | 52% | | |
| refinery (4,0) | 54 | 30% | 10% | |
| power_conduit x3 | 54-87 each | 6-8% | | idle between wants, not clogged |
| winding_node (4,-3) | 70 | 40% | 29% | cell-limited |
| assembly_bay (5,-3) | 22 | 21% | 2% | |
| recharger_node x5 | 0 | 98-100% | | dry for 20-80 ticks at most |

`labour.mean_wanted` 3.5, `mean_staffed` 13-14.5, `mean_unworked` 0.0: unlike
`chains`, `economy.labour_unworked` has something to say here (it says zero).

**Siege-free window.** A siege opens and stops the clock at tick 3789-4902 of
a run with these orders (seeds 1-5; seed 6 reaches 5000). So 5000 ticks is not
available and 3000 is, with 790 to spare. `stopped_at` does not depend on the
machines: the shipped and tuned asset sets stop at the same ticks.

| seeds | ticks | stopped_at |
|---|---|---|
| 1, 2, 3 | 5000 | 4902, 3840, 3789 |
| 4, 5, 6 | 5000 | 4560, 4578, none |
| 1-6 | 3000 | none |

A stopped run reports over the ticks it actually ran (`ticks` in the report is
4902, 3840 and 3789 for seeds 1-3): statuses, running shares, output per 1000
and labour means all use that count. Before the fix that landed with this
note they used the requested 5000, which stretched the last status over the
unplayed tail and read every rate low; figures from stopped runs in earlier
notes carry that error.

## The tune

`dev-tuning/economy-bench.ron`: 3000 ticks, the three orders at 9999, search
seeds 1-3, hold-out 4-6, 25 iterations x 16, search seed 1337. Four targets,
read off the shipped numbers at 3000 ticks and set above them:

| measure | want | shipped (hold-out) | proposed (hold-out) |
|---|---|---|---|
| items.patch_routine | 45..70 | 33.3 | 48.0 |
| items.ice_breaker | 140..200 | 115.0 | 181.3 |
| items.bytecode_block | 100..150 | 83.3 | 107.7 |
| running_share.assembly_bay | 0.30..0.50 | 0.216 | 0.317 |

None of the tune's runs stopped (3000 ticks is inside the siege-free range
for the search and hold-out seeds 1-6, and the stop-aware search now refuses a baseline that stops),
so the stopped-run fold error does not touch it and its result stands.

Error before/after on the search seeds 0.7633 -> 0.0006; hold-out 0.6828 ->
0.0000 ("holds up"). Three more seeds (7, 8, 9), which the search never saw:
patch_routine 47.0, ice_breaker 171.3, bytecode_block 105.3, bay share 0.305,
all inside their ranges (shipped on the same seeds 33.3 / 113.0 / 81.7 / 0.214).

| knob | shipped | proposed | bounds |
|---|---|---|---|
| mining_node `work.ticks_per_unit` | 10 | 5 | 5..14 |
| power_conduit `work.ticks_per_unit` | 6 | 3 | 3..9 |
| winding_node `assembles.ticks_per_unit` | 12 | 6 | 6..16 |
| assembly_bay `assembles.ticks_per_unit` | 20 | 21 | 8..30 |
| compiler `assembles.ticks_per_unit` | 8 | 6 | 4..12 |
| refinery `assembles.ticks_per_unit` | 12 | 12 | 6..16 |

### Reading it for artifacts

- **Slowing a machine or enlarging a buffer: no.** Every move is faster (or
  1 tick slower on the bay, below); no `capacity` was a knob.
- **Stopping runs: no.** The tuned set stops at exactly the same ticks as the
  shipped set (table above), and no hold-out run stopped at 3000.
- **Drift on unobserved knobs: one, small.** The refinery did not move. The
  bay's 20 -> 21 is noise: reverting only that file gives patch_routine 45.0
  and bay share 0.293 against the proposal's 48.0 and 0.317, so a one-tick
  change swings the result by three units through the hauling dynamics alone.
  That is the evaluation's noise floor, and it is about as wide as the
  margin by which patch_routine clears its minimum (48 against 45).
- **Three knobs sit on a bound** (mining 5, conduit 3, winding 6). The search
  wants more than the range offers, so the proposal is set by the bounds I
  chose, not found by the search. Reverting each one on its own, mean of
  seeds 4-6: mining 5 -> 10 costs 4 patch routines and 55 ICE Breakers;
  conduit 3 -> 6 costs 5 patch routines; winding 6 -> 12 costs 13 patch
  routines. So each carries real throughput and none is idle drift.
- **The model has no cost for speed.** Machine `power_draw` and `build_cost`
  were not knobs, and no target penalises a faster machine, so "everything as
  fast as the bounds allow" is the optimum of the question asked. A proposal
  to apply would need those costs in the objective.
- **The bay target is the weak one.** The share range was chosen to be
  reached by output rising, and it was (0.216 -> 0.317); it could also have
  been reached by slowing the bay, which the patch_routine range guards
  against.

Verdict: a legitimate answer to the question asked, and not an artifact in
the earlier sense; but the question was "which machines are slower than they
could be", and the answer is "all of them, to the edge of the range". Read it
as a ranking of which speeds bind throughput (winding, mining, conduit, in
that order of patch_routine cost) and do not apply it.

## What it does not say

- **One layout.** Throughput here depends on where things stand: the first
  rearrangements of the same machines were stalled by two-tile gaps. The
  numbers are for this layout and this floor, not for the machines.
- **Fifteen programs, all present from tick 0.** Nothing here says how a base
  with fewer would flow.
- **Noise floor of about 3 units** per 3000 ticks on patch_routine from a
  one-tick knob change (RNG stream and hauling order shift), with three seeds
  per candidate. A target margin smaller than that is not resolved.
- **Siege caps the window** at about 3800 ticks; anything slower than that
  (research, settlement growth) cannot be seen on this template.
- **Only `assets/` structure numbers move.** `tuning.rs` constants such as
  `POWER_UPKEEP_TICKS` and `HAUL_CARRY_CAPACITY` are out of reach, and
  those, not machine speeds, set the Power Cell budget.
- **The template is not `chains`.** It drops the Shields, Market, Research
  Nodes and Patch Nodes and widens the floor; it is a fixture for `bench`,
  not a base a player would build.

## Open questions

- Whether `work_orders.rs` should keep a Power Conduit wanted while the
  shelf holds cells for a burner but a consumer is draining it faster than
  the Conduit refills; here that is solved by building three.
- Whether a stranding should cost as much morale when the cause is another
  program standing on the only free face.
- Whether the sieges-on baseline survives `play_with` always inserting the
  `DevSieges` resource (a new Resource can shift query order and the RNG
  stream). It does: re-run on the staff branch, seed 1, 3000 and 5000 ticks,
  release, sieges on. The 5000-tick run stops at 4902 as above and its items
  are the figures in the claim (Bay 58, Winding Node 176, Refinery 135,
  Compiler 173); the 3000-tick run reads 35, 108, 81 and 118, each inside the
  seeds 1-3 ranges quoted there. Nothing moved.

## After floor items (branch `floor-items`, T5)

Re-run exactly as in "How to reproduce it" (release, seeds 1-3, 2000 and 5000
ticks, the same three orders). A load a carrier used to lose, or set down in
the nearest Depot when stranded, is now dropped on the floor and fetched by an
idle hauler. The siege stops are unchanged (4902, 3840, 3789).

| run | Bay | Winding | Refinery | Compiler | before (same run) |
|---|---|---|---|---|---|
| seed 1, 4902 ticks | 54 | 164 | 138 | 196 | 58 / 176 / 135 / 173 |
| mean seeds 1-3, 2000 ticks | 22.7 | 69.0 | 56.3 | 76.7 | 22 / 70 / 54 / 79 |

Shelved items at the stop, seed 1: patch_routine 54, bytecode_block 138,
ice_breaker 196 (before: 51, 135, 189).

Staff measures (new here; this note had none, and the staff baseline's
`bench-economy` run is a different length and sieges setting, so there is no
like-for-like "before"): `on_shift_share` 0.97 / 0.92 / 0.95 at 2000 ticks
and 0.93 / 0.90 / 0.93 at the stops, seeds 1-3; `morale_mean` 1.0-1.4 at 2000
and 2.1-3.8 at the stops; `mean_staffed` 13.5-14.6, `mean_unworked` 0.

Reading: at 2000 ticks nothing moved beyond the noise floor already recorded
(a one-tick knob change swings patch_routine by about 3 per 3000). At the
stop, seed 1 moved by up to -12 (Winding) and +23 (Compiler); one seed and
five thousand ticks of changed hauling order cannot say whether that is drift
or noise, and I did not run enough seeds to separate them. The "Nothing moved"
claim of the last open question is therefore not renewed for the 4902-tick
figures; the 2000-tick figures and the staff measures show no collapse
(on shift never below 0.90, staffed never below 13.5).
