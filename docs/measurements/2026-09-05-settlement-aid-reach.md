# 2026-09-05 — Do the settlement aid radii reach anything?

## The claim

Both radii the settlement aid ladder hangs on were **dead by geometry**, not
by tuning. Settlements are derived one per `SETTLEMENT_REGION_CHUNKS`-chunk
region — 256 tiles across, 45% occupied, each town inset 24 tiles from its
region border. Against that spacing the median distance from the anchor to
the nearest town is **147 tiles**. `SETTLEMENT_GARRISON_RADIUS` shipped at a
flat 40, which finds a town in **1.6%** of worlds; `ROUTE_PREDATION_RADIUS`
shipped at 15, which found a town beside a trade lane in **0 of 2,000**.
Neither feature could be evaluated at the keyboard because in almost every
world there was nothing there to evaluate.

The fix is to express both against `placement::REGION_TILES` rather than as
flat numbers: garrison at **half a region** (128) reaches 39% of worlds,
predation at **a quarter** (64) catches a town beside 8% of second-nearest
lanes and 18% of third-nearest.

The second finding is the one that could not have been guessed: **a lane to
the *nearest* town is unpreyable at any radius.** It is short and points away
from everywhere else, so even a 128-tile band catches something in only 5.8%
of worlds, and 128 is half a region — at that width "beside the lane" has
stopped meaning anything. Route risk is therefore a property of hauling
*past* somebody, which only a trip to a farther market does. That is a
design fact about the placement derivation, not a number to tune.

## How to reproduce it

```sh
cargo test -p feral-processes-engine aid_reach_probe -- --ignored --nocapture
```

`crates/engine/src/tests/settlement_aid_reach.rs`. 2,000 worlds, seeds
`i * 2_654_435_761 + 12_345` for `i` in `0..2000`. Towns are read straight
off `placement::settlement_at` over the regions within 3 (garrison) or 4
(lanes) of the anchor's own, against the shipped `assets/settlements/`
catalogue. The anchor is taken as the origin, which is where the zone spawn
point sits in nearly every world.

The three non-ignored tests in the same file are the gates: each fails, with
the measured share in its message, if either constant is flattened back.

## The numbers

Chebyshev distance from the anchor to the nearest town, 2,000 worlds:

| p10 | p25 | p50 | p75 | p90 |
|----:|----:|----:|----:|----:|
| 71 | 105 | 147 | 190 | 227 |

Share of worlds with any town inside a given garrison radius — **new**:

| radius | share | note |
|-------:|------:|------|
| 40 | 1.6% | what shipped |
| 60 | 6.8% | `SETTLEMENT_NOTICE_RADIUS` |
| 96 | 20.4% | |
| **128** | **39.2%** | `REGION_TILES / 2`, adopted |
| 160 | 58.3% | |
| 256 | 91.6% | one whole region |

Share of lanes carrying at least one other town inside a given predation
radius, by how far out the destination market is — **new**:

| radius | nearest | 2nd-nearest | 3rd-nearest |
|-------:|--------:|------------:|------------:|
| 15 | 0.0% | 0.8% | 3.0% |
| 32 | 0.0% | 1.8% | 6.7% |
| **64** | **0.1%** | **8.2%** | **17.6%** |
| 96 | 1.1% | 21.2% | 32.8% |
| 128 | 5.8% | 37.6% | 48.6% |

Median distance from the nearest *other* town to the lane: 216 tiles for a
nearest-town destination, 148 for second-nearest, 130 for third.

## What it does not say

- **These are candidate cells, not resolved tiles.** `settlement_at` answers
  where a town wants to stand; `ensure_local_settlements` then walks up to
  `SETTLEMENT_SITE_SEARCH_TILES` (24) outward for standable ground, and a
  region that is mostly Data Void leaves its town unplaced entirely. So the
  distances carry ±24 of slop and the shares are an **upper bound** — the
  real ones are slightly lower.
- **It says nothing about how often either thing actually fires.** Reach is
  the geometry gate only. A garrison additionally needs that town at `Warm`
  or better; predation needs a `Hostile` town the party has already found,
  plus a `ROUTE_PREDATION_CHANCE` roll per leg. The 39% and 18% are ceilings
  on how often the seed *permits* the feature, not rates.
- **The anchor is the origin.** A player who founds their base a long way
  from spawn re-rolls their own garrison odds, and this does not model that.
- **Nothing here is a feel judgement.** Whether 39% is the right share of
  worlds to have a friendly garrison is a play question; this only
  establishes that 1.6% was not a share anyone chose.
- **`ROUTE_PREDATION_CHANCE` and `ROUTE_PREDATION_LOSS` remain unmeasured**,
  as do `SETTLEMENT_WARM_GARRISON` and `SETTLEMENT_ALLIED_GARRISON`. This run
  moved the reach of the aid ladder, not its magnitudes.

## Follow-up, 2026-09-06 — the pitch halved and the ratios were re-run

`SETTLEMENT_REGION_CHUNKS` went 8 to 4 on a play note that towns were too far
to walk to, so a region is now **128 tiles** across rather than 256. The
sweep above was re-run unchanged against the new geometry. Both radii are
fractions of `REGION_TILES`, so both moved with it: garrison 128 to **64**,
predation 64 to **32**.

Chebyshev from the anchor to the nearest town, 2,000 worlds:

| | p10 | p25 | p50 | p75 | p90 |
|---|----:|----:|----:|----:|----:|
| 256-tile regions | 71 | 105 | 147 | 190 | 227 |
| **128-tile regions** | **42** | **55** | **71** | **88** | **102** |

The walk roughly halved, which is what the change was for. The tail is the
part that mattered: a quarter of worlds used to ask for a 190-tile hike
before the player had met anybody.

**The garrison ratio survived the retune and the predation ratio did not.**
That is the finding, and it is not what the ratio-not-value rule predicted:

| | 256 pitch | 128 pitch |
|---|----:|----:|
| worlds inside `SETTLEMENT_GARRISON_RADIUS` | 39.2% | **39.6%** |
| third-nearest lanes inside `ROUTE_PREDATION_RADIUS` | 17.6% | **12.7%** |
| second-nearest lanes | 8.2% | 3.3% |

The cause is `REGION_EDGE_INSET`, which is a **flat 24 and did not scale**.
It was 9% of a region's width on each side and is now 19%, so the usable span
a town is jittered within fell from 208 tiles to 80 — placement is markedly
more grid-like than it was, and a lane between two points on a regular grid
passes through the gaps rather than past the towns. A radial measure like the
garrison share cannot see this; a
"does the line from A to B pass near C" measure sees it directly.

So the gate in `settlement_aid_reach.rs` still passes — 12.7% against its
`>= 0.10` — but on a thinner margin than the number it was written against,
and the margin will keep thinning if the pitch is halved again without the
inset following it. **Scaling `REGION_EDGE_INSET` off `REGION_TILES` is the
open question this leaves**; it was not changed here, because the gate holds
and a second unmeasured constant moving in the same commit is how the flat 40
happened in the first place.

One consequence worth naming for play rather than for the gate: two towns in
adjacent regions can now stand as close as `2 * REGION_EDGE_INSET` — 48 tiles
— which is about one map viewport. That floor was always 48; what the halved
pitch changes is how often placement lands near it. "Never visible from the
last one" is now a claim about the common case rather than about every case.
