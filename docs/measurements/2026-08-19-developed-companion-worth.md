# What a developed companion is worth

## The claim

Opening every Kernel Ring on a companion roughly **doubles its power** —
level 6 to level 12 is 177 → 345 on `Stats::power()` for a Scrapper — and a
fully spent generic talent tree adds **9%** on top of that (345 → 376), because
at most half a tree's nodes may be `Stat` and each is capped at
`MAX_TALENT_STAT_PERCENT`. So the *ring* is the whole of the power and the
*talents* are the shape. Against a zone-3 group a fully ringed party of three
clears 18% faster (7.6 → 6.2 rounds) at the same 100% win rate, and against a
depth-5 lair it still loses **every** rep, surviving 7.4 rounds instead of 4.8.

Two decisions follow. The sale needs no `PurchasedTiers`-shaped receipt: the
part a player can *buy* (rings) grants no stats at all, and the part that raises
`program_payout` (a `Stat` node) is 9% rather than the 10x a bought Recompile
Kernel chain was. And **development does not fix Stack depth 5** — the 32-vs-4
volume fault recorded in `docs/measurements/2026-08-12-stack-lair-reachability.md`
is untouched by a stronger party, which is what a level-half instrument can
say and a talent-half one cannot.

## How to reproduce it

Same build for every number below; arena numbers compare within one build only.

```sh
# The level half, 50 reps each. The control is the same file with the party
# at `level: 6`, which is where the arena used to clamp them.
cargo run --bin arena -- dev-arenas/developed-companion.ron --out developed.ron
sed 's/level: 12, equip/level: 6, equip/' dev-arenas/developed-companion.ron > control.ron
cargo run --bin arena -- control.ron --out control.ron.out

# Depth 5, the same pair.
cargo run --bin arena -- dev-arenas/stack-depth-5.ron
sed 's/level: 12/level: 6/' dev-arenas/stack-depth-5.ron > d5-control.ron
cargo run --bin arena -- d5-control.ron

# The power figures, printed by the bound that pins them.
cargo test -p feral-processes-engine \
  tests::talents::a_developed_programs_stats_stay_a_small_multiple -- --nocapture
```

## The numbers

Zone 3, player level 20, three Scrappers with standard weapon and armour, four
rootkits, seed 7, 50 reps:

| Party | Win rate | Rounds (mean/median) | Player HP left | Companions down |
|---|---|---|---|---|
| Level 6 (control) | 100% | 7.6 / 8 | 100% | 0.02 |
| Level 12 (every ring) | 100% | 6.2 / 6 | 99% | 0.00 |

Depth-5 lair, same player and party, 50 reps:

| Party | Win rate | Rounds (mean/median) | Companions down |
|---|---|---|---|
| Level 6 (control) | 0% | 4.8 / 4 | 2.92 |
| Level 12 (every ring) | 0% | 7.4 / 6 | 2.42 |

`Stats::power()` for the fixture Scrapper:

| State | Power | Against the cap |
|---|---|---|
| Level 6 (`CREATURE_MAX_LEVEL`) | 177 | — |
| Level 12 (`absolute_companion_level_cap()`) | 345 | 1.95x |
| Level 12 + every `Stat` node the generic tree offers | 376 | 2.12x |

New: all of it. Nothing here reproduces a prior belief, except the depth-5
result, which reproduces the reachability finding from 2026-08-12 under a
party twice as strong.

## What it does not say

- **Nothing about three of the four talent node kinds.** The `arena` bin plays
  the game's own All-Attack every round, so no companion Special ever fires:
  `Ability`, `Affinity` and `RoutineSlot` nodes are invisible to it, exactly as
  they are invisible to `balance_sim`. The only instrument for those is playing
  `dev-arenas/developed-companion.ron` on the arena screen
  (`FERAL_DEV_ARENA=1 cargo run`, `[R]` Arena, `[L]`), and **that has not been
  done**.
- **Nothing about how long a ring takes to get.** One Privilege Ring per lair
  guardian, and ring N costs N of them, so a fully developed program is six
  guardians — six Stack runs. Whether that reads as a chase or a grind is a feel
  question and needs a session.
- **Nothing about the shipped class trees.** Every figure above is the *generic*
  tree, whose `Stat` nodes are the ones the fixture species can reach. The five
  class trees are weighted further toward options, so 9% is a ceiling on the
  stat half rather than a typical figure.
- **Nothing about a party mixing developed and undeveloped programs**, which is
  what a real run looks like for most of its length.
- The arena's own blind spots stand: no world, no Trace, no hunger, and a
  player who never uses an item.

## Open questions

- Does the played half change the 18%? Three companions each holding a granted
  routine and a sharpened affinity is exactly what the bin cannot see, and it is
  plausible that the option half is worth more than the number half — which is
  the design bet the trees are weighted on.
- The arena's companion clamp moved from `CREATURE_MAX_LEVEL` to
  `absolute_companion_level_cap()` in this change, so **every existing scenario
  authoring `level: 12` now fields level-12 companions where it silently got
  level 6 before**. `full-group.ron`, `class-mirror.ron`, `gear-passives.ron`,
  `lair-on-curve.ron` and `stack-depth-5.ron` all do. Their old reports are not
  comparable to new ones; the files were left alone rather than edited down,
  because 12 is what they were authored to mean (they predate `HP_PER_LEVEL`'s
  `K = 2` halving of the cap).
