# 2026-09-29 — What a level's six stat points buy, and where each class starts

## The claim

**Where the six points a level go matters, and no one spend dominates.**
The canonical spend (4 Parity, 2 Analysis) is within noise of the best
Parity/Analysis mixes at every level measured. Footprint-heavy spends win
more (all-Footprint 74% vs 54% at L15, 56% vs 46% at L20) by lengthening
the fight: 25-37 rounds against 13-18, which the All-Attack bin rewards
and a player with an item or a routine to spend those rounds on might not.
Any spend that leaves both Parity and Footprint out (all-Bandwidth,
all-Analysis) is a wipe: they are the only sources of HP.

**These are the second set of numbers.** The first run of this sweep
(`4b68e510`) let the player level mid-fight. A level-up refills HP
(`progression::add_xp`) and kill XP scales with the victim's threat
against the player's own Atk (`Game::kill_xp` -> `threat_to`), so a
build with *less* Atk earned about twice the XP a kill, levelled about
three times as often (1021 level-ups in 400 reps against 346) and was
healed in the middle of the fight. That made 4 Parity + 2 Bandwidth
look better than the canonical spend (82% vs 66% at L20) and all-Parity
look dominant. The arena now freezes levelling in a staged fight
(`LevellingFrozen`, `9e9db415`), and both results are gone.
**In play the effect is real**: hitting harder earns less XP a kill, and
a level earned mid-fight is a full heal. Recorded, not changed.

**Class starts now differ enormously** (spec: class attributes are kept as
authored, no net-zero rule). At level 1 a Bastion or Medic wins 98% of a
fight an attribute-less baseline wins 24% of, and a Saboteur wins none of
it. That is what the authored numbers say and the spec asked for it to be
reported; it is **a dominance result**: Bastion, Medic, Fabricator and
Decompiler are strictly ahead of the baseline; Invoker and Saboteur are
not viable in the opening fight; Striker and Leech sit below the baseline.

**Nothing was retuned.**

## How to reproduce it

Everything is one build (`cargo build --release --bin arena`, branch
`level-up-stat-allocation` at `9e9db415`), scenarios written by hand;
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
| 10 | 2 | `crawler` 4 + 4 + 1 |
| 15 | 3 | `rootkit` 4 + 1 |
| 20 | 3 | `rootkit` 4 + 4 |

The packs were picked by probing so the canonical spend sits at 35-54%
rather than at a walkover or a wipe (L10: 4 + 4 was 79%, 4 + 4 + 2 was 4%;
L15: 4 was 92%, 4 + 2 was 10%). They are lighter than the first run's
because the mid-fight heals are gone.

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
| canonical (4 Parity, 2 Analysis) | 35.0% (19.6, 22%) | 54.0% (13.4, 38%) | 46.0% (17.9, 28%) |
| all Parity | 28.0% (25.7, 14%) | 55.0% (19.8, 34%) | 37.2% (27.3, 20%) |
| all Footprint | 31.5% (27.8, 16%) | **74.2%** (25.1, 55%) | **56.2%** (36.9, 34%) |
| all Bandwidth | 1.0% (6.2, 1%) | 0.5% (2.7, 0%) | 0.0% (2.0, 0%) |
| all Analysis | 11.5% (6.5, 9%) | 1.8% (2.6, 2%) | 0.8% (2.1, 1%) |
| 5 Parity + 1 Analysis | 33.2% (23.4, 18%) | 60.3% (16.5, 38%) | 47.8% (21.2, 28%) |
| 4 Parity + 2 Footprint | 29.8% (26.4, 15%) | 64.8% (21.3, 40%) | 48.2% (33.0, 29%) |
| 3 Parity + 3 Footprint | 30.2% (26.7, 15%) | 70.2% (22.2, 45%) | 54.8% (38.1, 35%) |
| 2 Parity + 4 Analysis | 28.2% (12.7, 20%) | 30.5% (8.6, 23%) | 24.5% (9.9, 19%) |
| 4 Parity + 2 Bandwidth | 15.0% (20.1, 9%) | 33.0% (15.6, 19%) | 18.8% (19.1, 11%) |

At 400 reps the standard error near 50% is 2.5pp, so the Parity/Analysis mixes
(canonical, all-Parity, 5 + 1) are one result at L10 and L15; at L20
all-Parity trails the canonical spend by 9pp.

### Class starts — win rate (rounds, player HP left)

Re-run after the levelling freeze and unchanged to within 0.2pp (these
fights are short and rarely levelled anyone).

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
- **XP and levelling are switched off in the arena.** A number here is
  what the staged build does, not what the same fight does in play, where
  a low-Atk build levels (and heals) more often.
- **One pack per level, one party, no gear, one seed base.** The L10 pack
  is at a cliff (4+4+1 -> 35%, 4+4+2 -> 4%).
- **Class shapes are two fights against one species** and only the
  opening levels. Whether the gap survives the level-up spend is unrun.
- **Compare within one build.** The rows share a binary; another build
  reshuffles the RNG stream and moves the absolutes.

## Open questions

- Should class attribute totals be normalised? The spec keeps them as
  authored, so this is a design decision, not a bug; the numbers above are
  the evidence for it.
- Should kill XP stop shrinking as the player's Atk grows, so building for
  damage does not cost levels? A design question, not an arena one.
- Footprint outlasts rather than outfights. Is a long fight the intended
  price of mitigation, or should Footprint be re-priced against Parity?
