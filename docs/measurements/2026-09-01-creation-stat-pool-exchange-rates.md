# 2026-09-01 — Are the creation stat pool's exchange rates commensurate?

## The claim

**Attack is worth roughly 1.2–1.8x Integrity per point of the creation
pool, not the 2.5x the rates imply on paper — and the gap closes further
than that once you look past win rate.** `tuning::CREATION_STAT_POINTS` is
5, and one point buys `+1 atk` or `+6 max_hp`. On a level-1 player
(`PLAYER_BASE_STATS`: 6 atk, 90 max HP) that is +16.7% offense against
+6.7% survivability, which reads as Attack being about two and a half times
the better buy. Measured over 1,000 fights at each of three shapes, it is
not: Attack raises the win rate by 1.16x, 1.18x and 1.82x what Integrity
raises it by, depending on the fight.

The reason is that the two axes move the *length* of the fight in opposite
directions, and length is itself a cost. Attack shortens it (16.5 rounds
against a 18.3-round control), so the pack gets fewer swings; Integrity
lengthens it (20.1 rounds), so the pack gets more. The extra Integrity
absorbs those swings for the player, and the party pays for them —
`companions down` went **up** with Integrity, from a control's 1.17 to
1.28, while Attack took it down to 1.04. So "tankier is safer" is not
simply true for a party of three, and a win rate on its own hides that.

**Two further findings this run produced.** The `Def` axis is measurable
and does nothing: 5 points into one point of Mitigation (it costs 3, so 2
are stranded) produced outcomes **byte-identical to the control** at 200
reps — same win rate, same round count, same HP left. The axis lands
(`a_character_spec_stat_spend_reaches_the_players_stats` asserts it in
`Stats`), it just moves nothing a fight can see, because one percentage
point on a base of two rounds away against damage of this size. And
`Decompiler` is likewise identical to the control, correctly — it prices
capture odds and a staged fight captures nothing, which makes it a useful
null control for the instrument itself.

**No change was made to `tuning.rs` on the strength of this.** It is a
measurement, not a retune.

> **Superseded in part, same day.** The Mitigation finding below *was*
> acted on: `CREATION_COST_DEF` went 3 → 1 and `CREATION_STAT_POINTS` 5 →
> 9. Every number in this report was taken at the five-point pool with Def
> at three, so the rows no longer reproduce — the two `dev-arenas/player-
> points-*.ron` files have been moved to the nine-point pool and will read
> differently. What survives the retune is the *ratio* between Attack and
> Integrity (they were not repriced against each other) and the finding
> that one percentage point of Mitigation is invisible to the instrument.

## How to reproduce it

The two shipped rows, and the control, in one build:

```sh
cargo run --bin arena -- dev-arenas/player-points-atk.ron
cargo run --bin arena -- dev-arenas/player-points-integrity.ron
```

The control is either of those with `stats: (0, 0, 0, 0)`, or with the
whole `character` row deleted — the same player. The other axes and the
other two shapes were run by copying one of those files and editing
`character.stats`, `player`, `party` and `opponents`:

```ron
character: (stats: (5, 0, 0, 0)),   // MainStat::all() order:
                                    // Atk, Def, Integrity, Decompiler
```

Shapes, all at `reps: 1000, seed: 1` unless noted:

| | player | party | opponents |
|---|---|---|---|
| A | `Fresh(level: 5, zone: 1)` | 2x `glitch` L5 | `sub_process` 5 + 5 |
| B | `Fresh(level: 1, zone: 1)` | none | `sub_process` 2 |
| C | `Fresh(level: 10, zone: 2)` | 2x `glitch` L10 | `crawler` 4 + 4 |

Shape A is the pack `dev-arenas/player-class-*.ron` also use, deliberately:
one fight for both questions, so a shape artefact shows up in both places or
in neither. B is the opening fight made survivable enough to have a spread.
C checks the ratio at a level where +5 atk and +30 HP are a smaller
fraction of the base.

**`reps: 1000`, not 20.** The difference between the two axes is a few
points of win rate, and twenty fights cannot see it — at 200 reps A read
71.0% against 75.5% (Integrity *ahead*), which reversed at 1,000. That
20-rep noise floor is a standing trap in this repo, not a new one.

`character` is a `Fresh`-only field added with character creation; a save
or a template carries its own answer. `stats` is **units bought**, never
points spent, so `(0, 1, 0, 0)` is one point of Mitigation for three of the
five pool points.

## The numbers

All new. Nothing here reproduces a prior belief; the pool did not exist
before this branch.

### Shape A — level 5, zone 1, two companions, `sub_process` 5 + 5

| spend | won | rounds (mean) | player HP left | companions down |
|---|---|---|---|---|
| control, unspent | 57.9% | 18.3 | 33% | 1.17 |
| 5 Attack | **71.6%** | 16.5 | 45% | 1.04 |
| 5 Integrity | 69.5% | 20.1 | 42% | 1.28 |

Attack +13.7pp, Integrity +11.6pp. Ratio **1.18x**.

### Shape B — level 1, zone 1, solo, `sub_process` x2

| spend | won | rounds (mean) | player HP left |
|---|---|---|---|
| control, unspent | 24.2% | 9.5 | 6% |
| 5 Attack | **56.1%** | 9.0 | 17% |
| 5 Integrity | 51.6% | 12.0 | 15% |

Attack +31.9pp, Integrity +27.4pp. Ratio **1.16x**. This is the shape the
paper arithmetic was computed on, and it is the shape where the two axes
are *closest* — the opposite of what the arithmetic predicts.

### Shape C — level 10, zone 2, two companions, `crawler` 4 + 4

| spend | won | rounds (mean) | player HP left | companions down |
|---|---|---|---|---|
| control, unspent | 52.4% | 27.6 | 40% | 0.59 |
| 5 Attack | **60.4%** | 26.1 | 46% | 0.53 |
| 5 Integrity | 56.8% | 29.4 | 43% | 0.80 |

Attack +8.0pp, Integrity +4.4pp. Ratio **1.82x** — the widest of the three,
and the direction to expect: HP grows with level, so a flat +30 is a
smaller fraction of the bar at 10 than at 1, while +5 atk keeps compounding
over a fight that runs 27 rounds.

### The two axes that move nothing

Shape A, 200 reps (this row was measured before the 1,000-rep pass and not
re-run, because the result is a zero):

| spend | won | rounds (mean) | player HP left | companions down |
|---|---|---|---|---|
| control, unspent | 58.0% | 17.9 | 32% | 1.09 |
| 1 Mitigation (3 points; 2 stranded) | 58.0% | 17.9 | 32% | 1.09 |
| 5 Decompiler | 58.0% | 17.9 | 32% | 1.09 |

Identical in every column, to every digit. For Decompiler that is correct
and is what makes it a null control: it prices a capture and a staged fight
captures nothing. For Mitigation it is the finding — the axis is priced at
3x the others precisely because it is the one levelling never raises, and
at the bottom of the ladder that scarcity is worth nothing you can measure.

## What it does not say

- **The bin plays All-Attack.** `run_rep` uses `PartyPlan::AllAttack`, the
  game's own `[A]`, which invokes no routine. So none of this measures a
  build that spends rounds on its starter routine, and none of it can see a
  **class** at all — a class is a spread of multipliers over authored
  routine power, and the ordinary swing never touches
  `Game::ability_affinity`. That is what `dev-arenas/player-class-*.ron`
  and the played arena are for, and it is why the pool is left unspent in
  those five files.
- **Nothing here is a per-point curve.** Every row spends the whole pool on
  one axis. Whether 3 Attack + 2 Integrity beats 5 of either is unmeasured,
  and mixed builds are what a player will actually make.
- **Three shapes are three shapes.** All are surface fights against one
  species per group, with `glitch` companions and no gear. Gear scales by
  zone and locks in at equip, and a geared player's atk is a different
  denominator; a Stack fight has its own depth multiplier. Neither was run.
- **Compare within one build.** Every number here came from the same
  binary. A moved baseline in a later report is a reshuffled RNG stream
  before it is a difficulty change, and the control row is what tells the
  two apart — re-run it rather than trusting the 57.9%.
- **Win rate is a coarse instrument at these margins.** 1,000 reps puts the
  standard error on a win rate near 50% at about 1.6pp, so a difference of
  2.1pp (shape A) is barely outside noise and the 1.18x should be read as
  "about the same", not as a measured ratio. Shape C's 3.6pp gap is the
  only one comfortably clear of it.

## Open questions

- **Should Integrity be repriced up, or Mitigation down?** ~~Open.~~
  **Answered 2026-09-01: Mitigation down**, to one point like the other
  three axes, on the strength of the zero below. Unmeasured at the new
  price — a full-Def build now reaches 11% mitigation, and whether *that*
  is visible to the instrument has not been run. This run says
  the two cheap axes are close enough that the *player* will not
  distinguish them, which is arguably fine, and that Mitigation at 3 points
  is a trap row — it costs the most and does the least at the level the
  choice is made. Nobody has played the screen yet, so there is no read on
  whether that is felt as a trade-off or as a mistake.
- **Does the companion cost of Integrity survive a real party?** The
  `companions down` swing (1.04 / 1.17 / 1.28) is the most interesting
  number here and rests on a two-`glitch` party at one shape. It predicts
  that a tanky player with fragile companions is worse off than the win
  rate suggests, which would be a genuine design lever — but it has been
  measured once.
