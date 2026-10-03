# 2026-10-02 — What a program's level points buy, and the mining spread

## The claim

**The default spend beats every single-attribute spend by a wide margin in
every cell measured, and the balance curves did not move.** A seated
program's level points are now placed like the player's. The default is the
species' Parity/Analysis split (`program_level_points`, 4 + 2 a level scaled
by `g`), and it won 42-64% of fights where all-Parity won 13-28%,
all-Footprint 5-15% and all-Bandwidth 0-28%, at levels 10 and 20, for
three species of different `g` (1.0, 1.25, 1.5). The cause is mostly
mechanical: each all-X spend gives up the Analysis half (all the attack a
level buys) or the HP half, so "default dominates" is what the model
predicts, not a finding about the points' prices. Nothing was retuned and
no per-point value changed; the numbers are reported because the plan's
checkpoint asked for them.

**`balance_sim` moved nothing.** `companion_stats` now seats and derives
(HP, ATK, mitigation equal the old flat growth for every species, zones
1-10 and levels 1-200, asserted), and companion crit and fumble come from
species Entropy. Every `[roster]`/`[no gear]` line the sim prints is
identical before and after; the median party species sits at the
baseline Entropy, so the term has nothing to move.

**Mining.** A posted program's success chance is
`0.4 + 0.1 x node level + 0.02 x (Analysis - 10)`. Across a species' ±3
Analysis spread that is a **0.12** band, and a program that levels on the
default spend gains 3 Analysis a level at `g` 1.5, so it reaches a
certain success by about level 8 on a Mk1 node. That is a consequence of
Analysis being both the attack stat and the mining stat; recorded, not
changed.

## How to reproduce it

Build `cargo build --release --bin arena` at `4ff93e6c` on branch
`derived-programs`. The arena runs with levelling frozen, so the leveling
the companions get is the staged spend only. The scenarios are scratch,
generated from this shape and not kept; `spend` is a `CompanionSpec` field
added for this run (`dev-arenas/README.md`):

```ron
(player: Fresh(level: 20, zone: 3),
 party: [(species: "crawler", level: 20, spend: Some("footprint")),
         (species: "crawler", level: 20, spend: Some("footprint")),
         (species: "crawler", level: 20, spend: Some("footprint"))],
 opponents: [(species: "rootkit", count: 4), (species: "rootkit", count: 4)],
 reps: 400, seed: 1)
```

`spend` omitted is the default. The player is a fresh level-L player on the
canonical spend, no gear, in every cell; only the party's spend varies. The
pack was chosen by probing so the default sits at 42-64%:

| level | species (g) | zone | opponents (two groups of) |
|---|---|---|---|
| 10 | glitch (1.0) | 2 | rootkit 4 |
| 10 | crawler (1.25) | 2 | rootkit 2 |
| 10 | rootkit (1.5) | 2 | rootkit 3 |
| 20 | glitch (1.0) | 4 | rootkit 4 |
| 20 | crawler (1.25) | 3 | rootkit 4 |
| 20 | rootkit (1.5) | 3 | rootkit 4 |

Levels are 6, 8 and 9 points a level for `g` 1.0, 1.25 and 1.5. The sim:
`cargo test -p feral-processes-engine balance_sim`.

Mining: the real `systems::mining_success_chance` called with Analysis 10
to 16 (rootkit authors 13, `spread: 3`), keen scavenger 0, no morale or
strain, and with the Analysis a default-spend rootkit holds at each level.

## The numbers

All new: programs had no spend before this branch. Win rate (mean rounds,
player HP left), 400 reps each, three companions of the species.

| level | species | default | all Parity | all Footprint | all Bandwidth |
|---|---|---|---|---|---|
| 10 | glitch | 48.8% (12.9, 33%) | 11.8% (13.0, 7%) | 14.7% (13.0, 8%) | 0.0% (9.0, 0%) |
| 10 | crawler | 55.2% (9.5, 43%) | 25.8% (10.9, 18%) | 11.2% (8.2, 10%) | 6.5% (13.4, 3%) |
| 10 | rootkit | 50.0% (9.1, 38%) | 28.0% (9.7, 23%) | 7.2% (7.3, 4%) | 19.8% (13.9, 11%) |
| 20 | glitch | 64.2% (12.9, 48%) | 22.5% (14.5, 15%) | 15.2% (10.5, 12%) | 0.0% (10.5, 0%) |
| 20 | crawler | 42.5% (14.0, 33%) | 13.0% (15.8, 8%) | 5.0% (11.0, 5%) | 0.0% (15.8, 0%) |
| 20 | rootkit | 62.2% (11.7, 38%) | 20.0% (12.4, 16%) | 6.2% (10.6, 2%) | 27.5% (20.0, 14%) |

Standard error near 50% is 2.5pp. Nothing but the default is within noise
of it in any row.

Mining success chance, Mk1 to Mk3 node (level 1 to 3), by Analysis:

| Analysis | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|
| Mk1 | 0.50 | 0.52 | 0.54 | 0.56 | 0.58 | 0.60 | 0.62 |
| Mk2 | 0.60 | 0.62 | 0.64 | 0.66 | 0.68 | 0.70 | 0.72 |
| Mk3 | 0.70 | 0.72 | 0.74 | 0.76 | 0.78 | 0.80 | 0.82 |

A default-spend rootkit on a Mk1 node: level 1 (Analysis 13) 0.56, level 5
(25) 0.80, level 10 (40) 1.00, level 20 (70) 1.00.

## What it does not say

- **The all-X spends are extremes nobody plays.** They drop the whole other
  half of what a level buys, so they lose to a mixed spend by construction.
  The question a player faces, Parity against Footprint at the margin, is
  not asked here; the 2026-09-29 sweep's mixed rows would be the model.
- **The bin plays All-Attack and invokes nothing.** Bandwidth (Max Power)
  has no lever in it, so all-Bandwidth's wipe is not a verdict on Bandwidth.
- **No gear on the party, one pack per cell, one seed base**; the pack is
  tuned to put the default near 50%, which is where a spend change shows
  most and says nothing about a fight at another difficulty.
- **Levelling is frozen** in the arena, and a program's mid-fight level-up
  (a full heal) is not modelled.
- **The mining table is the formula,** not a run, and the Analysis climb
  assumes the whole default spend lands, with no held points or gear.
- **Compare within one build.** Rows share a binary; another build
  reshuffles the RNG stream.

## Open questions

- Should Analysis stay both a program's attack stat and its mining stat?
  A default-spend program mines at a certain success by level 8.
- The Parity/Footprint trade for a program (mixed spends) is unmeasured.
