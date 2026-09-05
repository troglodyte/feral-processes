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
