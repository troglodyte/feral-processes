# Base bench: `chains` baseline and a first tune

## The claim

On the shipped `chains` template the base is mostly stalled, not busy: over
5000 ticks the Assembly Bay runs 4% of the time and is clogged 92%, mining
nodes run 1-14% (clogged 86-97%), the refinery runs 5%, and the compiler is
starved the whole run and makes nothing. Lines end at 4.0 and 2.0 units per
1000 ticks. A 25 x 16 CEM `tune` against three range targets reached zero
error on its search seeds and on hold-out seeds, in 3m20s. Replication of
nothing; all of this is new. The proposal is a demonstration of the
instrument on one small base, not a balance change (see below).

## How to reproduce it

```sh
cargo build --release --bin bench
cargo run --release --bin bench -- run --template chains --ticks 5000 --seed 1   # also 2, 3
cargo run --release --bin bench -- tune dev-tuning/economy.ron                    # search_seed 1337
```

Build `base-bench` branch, 16-core machine. `economy.ron`: 3000 ticks,
search seeds 1-3, hold-out 4-6, 25 iterations, population 16.

**Timing**, `run --template chains --ticks 5000`, release: 1.88 s, 1.88 s,
1.89 s (median 1.88 s, about 0.38 ms per tick). Tune wall time 3m20s (user
35m45s across threads), inside the 15-minute budget.

## The numbers

Baseline, 5000 ticks, seeds 1/2/3 (identical except where noted):

| machine (pos) | running | clogged | starved | units |
|---|---|---|---|---|
| recharger_node x3, research_node (0,1) | 100% | | | 0 |
| mining_node (2,0) | 10.6 / 11.4 / 14.2% | ~90% | | ~108 |
| mining_node (-1,0) | 3.9 / 1.5 / 1.3% | ~96% | | 20 |
| refinery (3,0) | 4.8% | 92.4% | 2.8% | 20 |
| assembly_bay (4,0) | 4.0% | 92.0% | 4.0% | 10 |
| winding_node (4,1) | 13.4% | 86.5% | 0.1% | 56 |
| power_conduit (4,2) | 7.9% | 92.1% | | 194 |
| compiler (-1,-2) | 0% | | 100% | 0 |

Lines: key "2,0" 4.0 per 1000 ticks, key "4,0" 2.0 per 1000. Items made:
core_fragment 128, power_cell 194, charge_coil 56, bytecode_block 20,
patch_routine 10, research_data 0. Seeds differ only in the first mining
node's yield: the base is close to deterministic.

**Labour.** `labour.mean_wanted` is 0.0 with 15 staff: `labour_demand()`
works, but nothing on `chains` queues work, so `wanted` and `unworked` are
empty. `economy.labour_unworked` is therefore a dead measure on this
template and is not a target. (The "-0.00" printed earlier was an empty f32
sum, fixed in its own commit.)

**Targets** (`dev-tuning/economy.ron`), chosen from the clog shape: every
machine is clogged on output, so the shipped values sit well below the bands.

| measure | range | shipped (3000 ticks, hold-out seeds) | proposed |
|---|---|---|---|
| running_share.assembly_bay | 0.15-0.40 | 0.067 | 0.187 |
| items.patch_routine | 25-60 | 10 | 28 |
| running_share.mining_node | 0.15-0.40 | 0.107 | 0.175 |

Tune: error before/after on search seeds 0.3116 -> 0.0000; hold-out 0.3259
-> 0.0000 ("holds up"). Knobs moved: mining_node `work.ticks_per_unit`
10 -> 19, assembly_bay `assembles.ticks_per_unit` 20 -> 30, assembly_bay
`capacity` 10 -> 28, refinery `capacity` 20 -> 39, assembly_bay
`power_draw` 3 -> 1.

## What it does not say

- **One small two-line base.** The proposal is tuned to `chains` alone: a
  demonstration of the instrument, not a balance change. A captured
  late-game factory should become the hold-out template when one exists.
- **A running-share target can be met by slowing a machine.** The search
  raised both `ticks_per_unit` values (a slower machine is busy longer per
  unit) and the share targets rose with them; only the patch_routine target
  pushes the other way, and the extra capacity is what delivered it. Do not
  apply the ticks changes without asking whether "busier" was what was
  wanted. Output-based targets are the safer kind.
- Hold-out seeds barely differ from search seeds on this near-deterministic
  base, so "holds up" is weak evidence of generalisation.
- Zero error means inside the ranges, which were my reading of the clog
  shape, not a design statement about what `chains` should do.
- Only asset-level numbers move; `tuning.rs` constants are out of reach.
- The compiler stays starved: no knob here feeds it.
