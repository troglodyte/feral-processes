# What stacked affixes are worth

**Measured 2026-08-24**, on the `gear-affix-stacking` branch at `a8a73f41`
(before the merge, so the numbers are from the build that introduced the
feature).

## The claim

Stacking affixes is a **real power move, not a data change**. On one
authored fight, four copies of the commonest loud weapon affix took the
player from a 60% win rate to 90%, and from 48% mean Integrity remaining
to 81%. That is a bigger swing than the whole rare-tier ladder buys on the
same fight — Ordinary to Prismatic is 60% → 83% — and the affixes are
reached by fusing four spares, while Prismatic is a roll nothing can farm.

Held at a **fixed fusion tier**, which is the comparison a player actually
faces, the effect is smaller but still large: a tier-2 copy carrying four
affixes wins 86.7% against 66.7% for a tier-2 copy carrying none. Four
affixes is exactly what a tier-2 copy can hold — `ITEM_FUSION_COST` is 2,
so a T2 is four tier-0 copies and each of those may carry one — so this is
the ceiling at that tier rather than a contrived figure.

`balance_sim` sees none of this. It models gear stats but no fusion, so a
green suite is not evidence here and was never going to be; this file is
what stands in its place.

## How to reproduce it

Three scenarios, identical but for the weapon's `affixes` row, live in
`dev-arenas/`:

```sh
cargo build --release --bin arena
./target/release/arena dev-arenas/affix-stack-none.ron
./target/release/arena dev-arenas/affix-stack-one.ron
./target/release/arena dev-arenas/affix-stack-four.ron
```

The rarity and fusion-tier rows below were run from a scratch scenario of
the same shape, varying only the `equip` row:

```ron
(
    player: Fresh(level: 12, zone: 3),
    equip: [ (item: "arc_lance", <the row under test>) ],
    party: [ (species: "scrapper", level: 12) ],
    opponents: [(species: "rootkit", count: 4)],
    reps: 30,
    seed: 7,
)
```

The rows under test were `rarity: Silver` / `Gold` / `Prismatic`, `tier: 2`,
and `tier: 2, affixes: ["overdriven", ...×4]`.

The fight is the `gear-passives.ron` shape — a level-12 player at zone 3
with one Scrapper against four Rootkits — chosen because
`2026-08-18-gear-passive-worth.md` established it as the on-curve band
where plain gear wins about 70% of the time. A fight nobody was ever in
danger of losing shows a defensive or offensive bonus nothing.

The affix is `overdriven` (+3 ATK, +1–3 damage), and **the same one four
times on purpose**: affix stats are added to the item's base before every
scaling axis, so four of one isolates the stacking from which affixes were
picked. A mixed set would measure the pool as much as the mechanic.

## The numbers

All new. Nothing here reproduces a previously believed figure — fusion has
never been measured in an arena before, because until this branch two
copies had to be identical to fuse at all.

Tier 0, Ordinary, varying the affix count:

| affixes | win rate | rounds (mean) | player Integrity left | companions down |
|---|---|---|---|---|
| 0 | 60.0% (18/30) | 17.1 | 48% | 0.30 |
| 1 | 70.0% (21/30) | 16.9 | 66% | 0.13 |
| 4 | 90.0% (27/30) | 14.6 | 81% | 0.07 |

The whole rare-tier ladder on the same fight, for scale, at tier 0 and no
affix:

| rarity | win rate | player Integrity left |
|---|---|---|
| Ordinary | 60.0% | 48% |
| Silver | 73.3% | 69% |
| Gold | 76.7% | 68% |
| Prismatic | 83.3% | 72% |

And at the fusion tier four affixes actually arrive with:

| copy | win rate | player Integrity left |
|---|---|---|
| tier 2, 0 affixes | 66.7% | 64% |
| tier 2, 4 affixes | 86.7% | 76% |

Two things worth reading off these. **One affix is worth roughly one rung
of the rare ladder** (60 → 70 against 60 → 73), which is the calibration
`assets/affixes/README.md` claims and is now checked rather than asserted.
And **the fourth affix is worth as much as the first three**, because the
bonus is flat and added before four multiplicative scaling axes — there is
no diminishing return anywhere in the chain.

## What it does not say

- **One authored fight, not a run.** Thirty reps at one seed, one matchup,
  one level, one zone. Nothing here says what four affixes do to pacing
  across a zone, to how long a player stays on found gear, or to whether
  the Fabricator still has a customer.
- **It does not model how often a player accumulates four affixes.** A
  drop rolls at most one affix at `GEAR_AFFIX_CHANCE`, and four on one copy
  means four affixed spares of the *same item at the same rare tier* — the
  90% row is a ceiling a player reaches occasionally, not a new baseline.
  Nothing was run to find out how occasionally.
- **One affix, on one weapon.** `overdriven` is an offensive prefix on a
  weapon with a damage band; a mitigation affix on armour compounds through
  `effective_mitigation`'s cap instead and would read differently.
- **Arena absolutes compare within this build only.** Any RNG-stream shift
  moves every figure here, so a later run's number is comparable to these
  only as a delta. See
  `memory/arena-numbers-compare-within-one-build-only.md`.
- **No cap was tested**, because there is none: the spec's decision was
  explicitly no ceiling, no diminishing returns and no burn-out. This file
  is the evidence for revisiting that, not a revisit of it.

## Open questions

Whether the no-cap decision survives play. The design argument for it is
that fusion is the game's only sink for spare gear and a cap would put the
sink back where it was; the argument against is the fourth row of the first
table. The instrument cannot settle it — it does not know how often four
matching affixed spares actually turn up — so this stays open until someone
plays a zone with the feature in.
