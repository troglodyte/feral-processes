# 2026-09-29 — What a level's six stat points buy, and where each class starts

## The claim

**Where the six points a level go matters a great deal, and the canonical
spend (4 Parity, 2 Analysis) is not the best one the arena can see.**
Against a zone-3 pack at player level 15 and 20, all-Parity beats the
canonical spend by 23pp and 15pp (46.5% vs 23.5%, 81.0% vs 66.2%); 5 Parity
+ 1 Analysis sits between. Any spend that leaves Parity out (all-Bandwidth,
all-Analysis) is a wipe, because Parity and Footprint are the only sources
of HP and the player dies in about two rounds. All-Footprint is roughly the
canonical spend's equal at win rate (it trades a lengthened fight for
mitigation and HP: 27-34 rounds against 16-20).

**A second, stranger finding, unexplained:** at level 15 and 20, swapping
the canonical spend's 2 Analysis for 2 Bandwidth (4 Parity + 2 Bandwidth)
*raises* the win rate (23.5 -> 40.8% at 15, 66.2 -> 82.0% at 20) while the
player's mean hit drops from ~48 to ~11 at level 20. The bin plays
All-Attack and Bandwidth moves nothing it can see, so this is Analysis's
Atk hurting the player in the arena, not Bandwidth helping. Cause not
found (see "What it does not say"); it is reported, not fixed.

**Class starts now differ enormously** (spec: class attributes are kept as
authored, no net-zero rule). At level 1 a Bastion or Medic wins 98% of a
fight an attribute-less baseline wins 24% of, and a Saboteur wins none of
it. That is what the authored numbers say and the spec asked for it to be
reported; it is **a dominance result**: Bastion, Medic, Fabricator and
Decompiler are strictly ahead of the baseline; Invoker and Saboteur are
not viable in the opening fight; Striker and Leech sit below the baseline.

**Nothing was retuned.**

## How to reproduce it

Everything is one build (`cargo build --bin arena`, branch
`level-up-stat-allocation` at `5dd88fa6`+docs), scenarios written by hand;
the scratch files are not kept. `player_spend` is the **per-level** pattern
and must sum to `STAT_POINTS_PER_LEVEL` (6), or the scenario is refused —
verified: `None` and an explicit `{"parity":4,"analysis":2}` give identical
numbers to the digit, and `entropy` is refused as `NotBuyable`.

Spend sweep, `reps: 400, seed: 1`, player + 2 `glitch` companions at the
player's level, no gear, class `None`:

```ron
(player: Fresh(level: 15, zone: 3), player_spend: Some({"parity": 6}),
 party: [(species: "glitch", level: 15), (species: "glitch", level: 15)],
 opponents: [(species: "rootkit", count: 4), (species: "rootkit", count: 4)],
 reps: 400, seed: 1)
```

| level | zone | opponents |
|---|---|---|
| 10 | 2 | `crawler` 4 + 4 + 2 |
| 15 | 3 | `rootkit` 4 + 4 |
| 20 | 3 | `rootkit` 4 + 4 |

The packs were picked by probing so the canonical spend sits at 18-66%
rather than at a walkover (4 + 4 crawlers was 90%, 4 + 4 + 4 was 6%; 4
rootkits alone were 96-100% at 15/20).

Class starts, `reps: 1000, seed: 1`, `character: (class: Some(X))`, no
`player_spend`:

- A: `Fresh(level: 5, zone: 1)`, 2x `glitch` L5, `sub_process` 5 + 5
  (the `player-class-*.ron` pack; zone 1 fields at most 2 per group, so the
  bin warns and it is 2 + 2 in effect, as it was for the 2026-09-01 shapes).
- B: `Fresh(level: 1, zone: 1)`, solo, `sub_process` x2 (fielded as 1).

Below level 8 a Striker's second swing is not in play
(`EXTRA_ATTACK_LEVEL`), so neither shape is confounded by it.

## The numbers

All new: the attribute-allocation model did not exist before this branch.

### Spend sweep — win rate (rounds, player HP left)

| spend per level | L10 z2 | L15 z3 | L20 z3 |
|---|---|---|---|
| canonical (4 Parity, 2 Analysis) | 18.0% (19.9, 12%) | 23.5% (16.0, 20%) | 66.2% (19.5, 56%) |
| all Parity | 19.5% (25.5, 18%) | **46.5%** (21.6, 45%) | **81.0%** (24.1, 77%) |
| all Footprint | 20.5% (27.3, 18%) | 23.8% (28.1, 23%) | 63.2% (34.0, 60%) |
| all Bandwidth | 4.5% (5.9, 4%) | 0.0% (2.0, 0%) | 0.2% (2.0, 0%) |
| all Analysis | 9.8% (6.9, 8%) | 0.8% (2.1, 1%) | 0.5% (2.1, 0%) |
| 5 Parity + 1 Analysis | 17.2% | 37.0% | 74.5% |
| 4 Parity + 2 Footprint | 19.2% | 41.5% | 73.8% |
| 3 Parity + 3 Footprint | 18.8% | 39.5% | 62.0% |
| 2 Parity + 4 Analysis | 13.8% | 13.8% | 35.5% |
| 4 Parity + 2 Bandwidth | 15.0% | 40.8% | **82.0%** |

At 400 reps the standard error near 50% is 2.5pp, so L10 is one flat row
(13.8-20.5% except the two no-Parity wipes) and only L15/L20 separate.

### Class starts — win rate (rounds, player HP left)

| class | A: L5 z1 + 2 glitch | B: L1 z1 solo |
|---|---|---|
| none (catalogue bases) | 51.7% (18.5, 28%) | 23.9% (9.5, 6%) |
| Bastion | 92.3% (24.8, 61%) | 98.2% (14.7, 52%) |
| Medic | 90.9% (24.5, 59%) | 98.7% (14.7, 53%) |
| Fabricator | 84.0% (23.6, 52%) | 90.7% (14.3, 37%) |
| Decompiler | 80.7% (23.1, 49%) | 90.1% (14.3, 36%) |
| Leech | 48.8% (17.9, 26%) | 18.1% (8.8, 5%) |
| Striker | 44.3% (17.1, 23%) | 11.0% (7.7, 3%) |
| Invoker | 26.2% (14.1, 15%) | 1.5% (4.8, 0%) |
| Saboteur | 10.8% (9.1, 7%) | 0.0% (1.2, 0%) |

Authored attributes (Parity / Footprint / Bandwidth) explain the order:
Medic 76 / 44 / 48, Bastion 60 / 64 / 46, Saboteur 40 / 30 / 52, Invoker
44 / 40 / 70. HP is 6 per Parity point and 2 per Footprint point, and
Footprint also gives 1 mitigation a point, all measured from the catalogue
base (Parity 50, Footprint 45). Analysis is authored at the base (10) on
every class, so none of them starts with an Atk edge. The Bandwidth 72 /
Persistence 58 seen in one capture are Striker's and Decompiler's authored
values in `assets/classes/`, i.e. intended by the spec, not drift.

## What it does not say

- **The bin plays All-Attack and invokes nothing.** Bandwidth (Max Power)
  and Persistence (status resist) have no lever here, so "all-Bandwidth
  loses" says only that HP matters more than a stat the bin cannot use; it
  is not a verdict on Bandwidth. Classes' affinities are likewise unseen.
- **Player death ends the fight.** A build with less HP is measured mostly
  as a build that dies early, which flatters HP-heavy spends over the
  damage-heavy ones a real, defended player could survive with.
- **The Analysis anomaly is unexplained.** Damage output per hit is ~4x
  higher in the canonical spend and the fight is no shorter and the player
  takes more damage. Not a targeting effect via `slot_aggro_weight` (it
  ignores Atk); I did not trace the enemy policy features or
  `combat_policy.rs`. It could be something real about how enemies pick
  targets, or a defect; it wants a reader who can step a single seed.
- **One pack per level, one party, no gear, one seed base.** The L10 pack
  is at a cliff (4+4 -> 90%, 4+4+4 -> 6%), so its flat row is a weak read.
- **Class shapes are two fights against one species** and only the
  opening levels. Whether the gap survives the level-up spend is unrun.
- **Compare within one build.** The rows share a binary; another build
  reshuffles the RNG stream and moves the absolutes.

## Open questions

- Should class attribute totals be normalised? The spec keeps them as
  authored, so this is a design decision, not a bug; the numbers above are
  the evidence for it.
- Why does Analysis's Atk lower the arena win rate?
- Is all-Parity the intended dominant spend, or should Parity's 6 HP a
  point be re-priced against Analysis's 1 Atk + 1 Decompiler?
