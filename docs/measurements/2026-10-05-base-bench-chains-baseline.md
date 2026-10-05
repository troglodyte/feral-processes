# Base bench: `chains` baseline and a first tune

## The claim

On the shipped `chains` template nothing drains machine output: each machine
fills its output buffer once and then clogs for the rest of the run. Over
5000 ticks the Assembly Bay makes 10 units (its capacity is 10) and runs 4.0%
of the time; the refinery makes 20 (capacity 20) and runs 4.8%; the compiler
is starved throughout and makes nothing. So `units` is about the machine's
capacity and `running_share` is about `capacity x ticks_per_unit / ticks`,
which is an artifact of run length, not a property of the economy.

The 25 x 16 CEM `tune` in `dev-tuning/economy.ron` reached zero error, but
only by enlarging and slowing a fill-once buffer (the tuned
`patch_routine` count, 28, is the proposed assembly_bay capacity, 28).
**Do not apply that proposal.** `chains` cannot be tuned for flow until a
template has something that drains its output or the measures are taken in
steady state. The instrument works; this template is the wrong subject.

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

**Targets** (`dev-tuning/economy.ron`) were chosen from the clog shape, and
they only watch fill-once artifacts:

| measure | range | shipped (3000 ticks, hold-out seeds) | proposed |
|---|---|---|---|
| running_share.assembly_bay | 0.15-0.40 | 0.067 | 0.187 |
| items.patch_routine | 25-60 | 10 | 28 |
| running_share.mining_node | 0.15-0.40 | 0.107 | 0.175 |

Tune: error before/after on search seeds 0.3116 -> 0.0000; hold-out 0.3259
-> 0.0000 ("holds up"). Knobs moved: mining_node `work.ticks_per_unit`
10 -> 19, assembly_bay `assembles.ticks_per_unit` 20 -> 30, assembly_bay
`capacity` 10 -> 28, refinery `capacity` 20 -> 39, assembly_bay
`power_draw` 3 -> 1. "Holds up" here only means the artifact was
reproduced on other seeds.

## What the tune did, read plainly

- **The targets are run-length artifacts.** `items.patch_routine` equals the
  assembly bay's capacity, so raising the capacity is the only way to raise
  it; no flow is being improved.
- **Slower machines score as busier.** A longer `ticks_per_unit` keeps a
  machine running longer per unit, so `running_share` rises with it. The
  search found that and used it.
- **`power_draw` 3 -> 1 is drift.** No target observes power, so the knob had
  no gradient and the move carries no information.
- The refinery capacity change is likewise unobserved by any target except
  through the same fill-once chain.

## What it does not say

- **One small two-line base** with no output drain. Nothing here is a
  statement about a base whose machines are emptied.
- Hold-out seeds barely differ from search seeds on this near-deterministic
  base, so "holds up" is weak evidence even of the artifact generalising.
- Only asset-level numbers move; `tuning.rs` constants are out of reach.
- The compiler stays starved: no knob here feeds it.

## What would make it usable

A template with a drain (something that consumes each machine's output), or
a measure taken after the buffers have filled (steady state), so that
`running_share` and `items` reflect throughput rather than capacity. Until
then `economy.ron` is a demonstration of the mechanics, not an objective.

## With orders (`--order item:qty`)

`bench run` and `Objective.orders` now queue batch work orders before tick 0.
Only posted workers haul, and workers are posted only on machines an order
wants, so a huge quantity (9999) keeps demand alive. The question was
whether that gives `chains` a sustained flow.

**Baseline**, orders `patch_routine`, `bytecode_block`, `ice_breaker` at
9999, release. `labour.mean_wanted` is now 2.1-2.5 (was 0.0), so the
scheduler posts workers. Per-machine, seed 1:

| ticks | bay units | winding units | refinery units | bay starved |
|---|---|---|---|---|
| 500 | 4 | 12 | 15 | 0 of 500 |
| 1000 | 8 | 25 | 20 | 0 |
| 2000 | 10 | 30 | 29 | 884 |
| 4000 | 10 | 30 | 39 | 1545 |

**Flow check: it is not sustained.** The line moves for about 1000 ticks
and then stops at a number set by the starting stock, not by the machines:
the winding node stops at 30 charge coils, the bay at 10 patch routines
(3 coils each), and the bay is starved for the rest of the run. Seeds 2 and
3 land on exactly 10 units by 1000 ticks and stay there. The power conduit
reads "idle" for the entire run with 0 units (it made 194 with no orders),
and the compiler never runs even with an `ice_breaker` order. The mining
node (2,0) and the refinery keep running (units 81 -> 117 -> 156), but their
output feeds nothing downstream that moves. I did not find why the conduit
is never wanted or hauled from; the bay passing its capacity once (11 units
at seed 1, 2000 ticks, `patch_routine` order alone) shows the drain is real
but short-lived. Extra orders for `power_cell` and `charge_coil` change
nothing.

Separately, 5000-tick runs with orders stop with "clock stopped" for seeds
2 and 3 (about tick 4000-5000 of the run; seed 1 survives to ~4500), so the
5000-tick window the first baseline used is not available with orders. That
is the bench refusing a run where a battle opens, as designed.

**No tune was run and `economy.ron` was not rewritten.** There is no flowing
window to take targets from: anything measured at 1000 ticks is the first
batch, and at 2000 or more it is the starting stock again. Tuning on it would
reproduce the same artifact as before. `economy.ron` stays a mechanics demo.

## What would make `chains` tunable

Find out why the conduit never runs under orders (its output looks full and
unhauled, or the scheduler skips it because a depot holds a batch of power
cells that is never delivered to the winding node), and fix the template or
the engine. Until a run shows units far above capacity at 4000 ticks, no
target on this base is a statement about flow.

## Later

The questions above were answered by building a different template, not by
changing `chains`: see
[the `bench-economy` baseline](2026-10-05-base-bench-economy-baseline.md).
