# What a stack costs once zone and depth both count

**Status:** measured, question posed, no shape chosen — **not approved, not implemented**. Audited 2026-09-02 against the source tree, not against this header.

`v0.13.21` fixed two real complaints — a zone-9 stack's first frame fielded a
lone program, and no lair shallower than depth 5 could field a hand-authored
apex. Both fixes are right and neither should be reverted. This document is
about the third thing that happened, which nobody asked for: the two changes
**multiply** at depth, and a stack's difficulty curve is now much steeper
than it was.

## The concern that was wrong, recorded so it is not re-raised

Before measuring, the worry was the **on-ramp**: a fresh run walks into its
first stack, meets an unscaled apex (`growth_multiplier: 2.0`, and
`spawn_pack` skips `BOSS_STAT_MULT` for an authored apex) plus a bigger
escort, and is wiped by the first thing the Stack shows them.

**The arena says no.** At zone 1, depth 2:

- A solo, level-1, ungeared player loses 300/300 on both Mainframe and
  OpenGrid, in ~4.2 rounds.
- The *same* player against the pre-change guardian — the toughest ordinary
  species, same build, same-build A/B — wins **1%** of the time.

So the on-ramp lair was never soloable by a bare level-1 with no party, and
the apex swap makes an already-lost fight faster rather than making a winnable
fight unwinnable. With two level-8 companions and **no crafted gear**, the
same fight wins **97%** at ~8.2 rounds with essentially no companion losses.
That is the party shape the game expects at a first stack. **Zone 1 needs no
softening, and the option to gate the apex behind "not zone 1" should be
dropped rather than kept in reserve.**

## What the measurement actually found

The pressure point is one step deeper, and it is the *escort*, not the
guardian.

| Fight | Wins | Rounds | Companions lost |
|---|---|---|---|
| zone 3, depth 2 lair, on-curve party | 100% | 11.1 | ~0 |
| zone 3, depth 3 lair, same party | **63%** | 35.3 | **1.89** |

The mechanism is legible in the scenario `composition` field and needs no
inference: at zone 3 depth 3, `danger_steps` is now **4** (zone step 2 +
depth step 2) where it was **2** (depth step alone). `max_group_size` is
`GROUP_SIZE_DISTANCE_GROWTH.pow(steps)`, so the group-size **ceiling** goes
from 4 to 16, and the measured escort balloons from 1 member at depth 2 to
7–13 at depth 3.

One frame of descent costs 37 points of win rate, triples the round count and
takes two companions with it. That is not a runaway — 63% is a fight, not a
wall — but it is a much bigger step than one frame used to buy, and the
player has no way to see it coming before committing to the descent.

## The question this spec exists to answer

**Is a single frame allowed to be worth that much?**

Three shapes, none chosen. They are not mutually exclusive.

1. **Accept it.** 63% at depth 3 in zone 3 is a real fight with a real cost,
   and the Stack is opt-in — the player chose to descend. The counter-argument
   is that the cost is invisible at the moment of the decision, and a
   one-way door at the bottom of each frame means "go back up" is not always
   the answer.
2. **Damp the sum rather than the terms.** The zone and depth steps both
   feed one exponent, and `GROUP_SIZE_DISTANCE_GROWTH` is 2, so summing the
   steps multiplies the ceiling by 4 per shared step. Halving the depth
   contribution underground (the "zone as a floor, depth adds half" option
   considered and passed over on 2026-08-24) keeps both commitments visible
   while flattening the compounding. **This is the smallest change that
   addresses the finding.**
3. **Split the curves.** Group *count* and group *size* both read
   `danger_steps` so they cannot disagree — that is a deliberate seam and
   worth keeping. But a lair's **escort** could take its own, gentler term
   without touching either curve, since the escort is already a distinct
   spawn (`pick_escort_species`). This targets exactly what the measurement
   blamed and leaves ordinary ambushes alone.

## What must not be undone in the course of answering it

- **The zone term itself.** Restoring "depth replaces zone" reintroduces the
  original bug: a zone-9 stack's first frame fielding one program.
- **The ungated lair apex.** A stack bottom without a real boss was the
  user's own report.
- **Linearity.** Every difficulty curve in the game is linear
  (`docs/seams.md`). A geometric enemy curve outruns a linear player curve
  wherever the coefficients are put. Note the *ceiling* here is already
  geometric in the step count via `GROUP_SIZE_DISTANCE_GROWTH.pow(steps)` —
  what stays linear is the step, and the roll is uniform in `1..=ceiling`.
  Any fix should move the step, not the base.
- **`danger_steps` as the single input both group curves read.** Shape 3
  adds a term beside it for one spawn site; it must not give the two curves
  independent inputs.

## Open questions

1. **Is 63% at zone 3 depth 3 the right number?** Nobody has played it. The
   arena cannot answer whether a 35-round fight that costs two companions
   reads as an epic or as a slog.
2. **Does the player have any way to read a frame's danger before
   descending?** If not, that is arguably the real defect, and a telegraph is
   a different and possibly better fix than a tuning change.
3. **Does the same compounding bite ordinary ambushes at depth?** The
   measurement covered zone 3 depth 1 (fine, 100% clear) and the depth-3
   *lair*. The depth-3 ordinary ambush was not measured.

## Blind spots in the measurement behind this

Stated in the measurement doc and repeated because they bound every number
above: arena figures compare **within one build only**. `lair-on-curve.ron`
itself moved from a documented 3.2 rounds on 2026-08-19 to 11.1 rounds in this
build, from commits unrelated to `v0.13.21` — so cross-report comparison is
invalid and the same-build A/B is the only trustworthy contrast here.
`balance_sim` gates none of this: it has no Stack term at all.

## What answering it would touch

Depends entirely on the shape chosen. Shape 2 is `Game::danger_steps` and one
tuning constant. Shape 3 is `spawn_pack`'s boss branch and a new term beside
`pick_escort_species`. Both need the arena rerun in the *same* build, and both
want a session before and after.
