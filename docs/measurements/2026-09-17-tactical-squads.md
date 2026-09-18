# What five folded rootkits are worth against five loose ones

**This supersedes a contaminated first run at `0b264583`.** That commit
measured a squad that could barely act: its own four footprint cells were
walls in its own movement field, so it could never step right or down (17 of
87 offered destination anchors were illegal and the walk silently aborted
mid-approach), and the tactical AI measured reach anchor-to-anchor while the
attack door measured footprint-to-footprint, so a squad standing adjacent
along its bottom or right edge declined to attack and tried to walk instead
— which the first bug then prevented outright. Both defects were found by
review and fixed on this branch (`51fd5128`, `4684a29b`, plus the squad
fixes around them). The old file's numbers are not a measurement of
`swing_share`; they are a measurement of a squad that could not reliably
move or swing, and must not be used to retune anything. The old file also
read its own numbers backwards: it called a fight that ended **sooner** and
cost the party **less** HP "the direction the spec predicted", which is
exactly wrong — a fight that resolves faster with the party healthier means
the folded squad was the *weaker* side, not a stronger one.

**The claim, re-measured on fixed code (`35db4357`).** At `swing_share =
0.5` (the shipped `FORMATIONS[0]` value), a folded squad of five rootkits is
still the *weaker* opponent against this mid-grade party than five loose
rootkits of (near-)equal total power — the fight against it resolves about
**0.66 rounds sooner** (8.04 against the control's 8.70) and costs the party
about **1.9 points more HP left** (92.9% against 91.0%) — but the gap is
roughly half the size the contaminated run reported (0.98 rounds / 3.7 HP
points), because the squad can now actually move to and swing from every
anchor it is offered. Both fights are still **walkovers**: 100% win rate in
both, 50/50 reps. The control side is numerically **identical** to the old
run (rounds, HP, spread all match to the decimal) — expected, since
`squad-control.ron` never folds and so never touches the code the fix
changed; that identity is itself a sanity check that only the squad side of
the comparison moved. `swing_share`'s own doc comment
(`crates/engine/src/tuning.rs`) says a fresh squad's turn is meant to deal
what its five members would deal fighting alone, with folding then buying a
further edge (less overkill, less mitigation waste, no split damage) on top
of parity — measured, the squad still falls short of parity, which reads as
`swing_share` sitting a little low rather than the comparison being flat.

## How to reproduce it

Built once, both scenarios run from that same binary (arena figures compare
within one build only — CLAUDE.md, `dev-arenas/README.md`):

```sh
git rev-parse HEAD
# 35db4357d9b27a15d5334fad6d1197e242be3ff8

cargo build --release --bin arena
./target/release/arena dev-arenas/squad.ron --out /tmp/squad-report.ron
./target/release/arena dev-arenas/squad-control.ron --out /tmp/squad-control-report.ron
```

Both scenarios are unchanged from the first run and are still sound: five
rootkits folded (`squad.ron`) against `tactical-full-group.ron`'s own party
— `Fresh(level: 20, zone: 3)` player geared with Plasma Router / Bastion
Lattice / Singularity Matrix, three level-12 Scrappers each carrying an Arc
Lance and Hardened Shell — versus four rootkits plus one virus that cannot
fold (`squad-control.ron`, same party, same zone, same seed). `model:
Tactical`, `approach: Some(East)`, `reps: 50`, `seed: 3` for both.

## The numbers

### Verification at low reps, before trusting the aggregate

Ran both at `reps: 1` first, since **arena scenario fields fail silently**
in this repo and the whole point of this re-run is that the squad side was
previously crippled without any warning firing.

- `squad.ron` at `reps: 1`: `report.warnings` is `[]`. The transcript shows
  one unlabeled `Rootkit` (no hex id, unlike the control's `Rootkit 3`)
  taking every hit and dealing every hostile swing, one HP pool draining
  across all eight rounds, then five separate "The rogue program crashes
  and deletes itself!" lines printed together on its single death — one
  pool, five payouts, exactly the feature under test. `fought: rootkit x5`,
  `WON in 8 rounds, 99% HP left`.
- `squad-control.ron` at `reps: 1`: `report.warnings` is `[]`. The
  transcript shows five individually hex-prefixed bodies (`Rootkit 3`,
  `Virus 3`, ...) dying one at a time across separate rounds — no fold, as
  the scenario's own comment says four-of-a-kind is one short of
  `FORMATIONS[0].members`. `fought: rootkit x4, virus x1`, `WON in 8 rounds,
  100% HP left`.

Both `warnings: []` at `reps: 50` too — checked directly in the written
`.ron` reports below.

### The aggregate, 50 reps each

| | win rate | rounds (mean / median / sd / range) | player HP left (mean / sd) | companions downed |
|---|---|---|---|---|
| `squad.ron` (5 rootkits, folded) — **new** | 100% (50/50) | 8.04 / 8 / 1.22 / 6-13 | 92.9% / 10.9 | mean 0.02 (1 of 50 reps, seed 42, 13 rounds — the longest fight in the batch) |
| `squad-control.ron` (4 rootkit + 1 virus, loose) — **unchanged** | 100% (50/50) | 8.70 / 9 / 1.33 / 7-14 | 91.0% / 10.3 | 0.00 every rep |

Standard deviations are population sd over the 50 reps, matching how the
first run's numbers were computed (confirmed by the control figures
reproducing the old file's `1.33`/`10.3` exactly).

### How this moved against the contaminated run

| | old (`0b264583`, buggy squad) | new (`35db4357`, fixed squad) | moved by |
|---|---|---|---|
| squad rounds (mean) | 7.72 | 8.04 | **+0.32**, toward the control |
| squad player HP left | 94.7% | 92.9% | **-1.8 pts**, toward the control |
| squad companions downed | 0.00 every rep | mean 0.02 (1 of 50) | up, off the floor |
| squad rounds range | 6-11 | 6-13 | widened at the top |
| control rounds (mean) | 8.70 | 8.70 | unchanged |
| control player HP left | 91.0% | 91.0% | unchanged |
| gap (control − squad), rounds | 0.98 | 0.66 | **narrowed by ~33%** |
| gap (squad − control), HP left | +3.7 pts | +1.9 pts | **narrowed by ~49%** |

The direction did not flip: the folded squad was weaker than the loose pack
before the fix and is still weaker after it — fixing the movement and reach
bugs let the squad actually reach and swing from every anchor it is
offered, which closed roughly a third to a half of the gap, but did not
close it. **Correct reading, stated plainly**: fewer rounds and more player
HP left both favour the *player*, i.e. they mean the squad is the *weaker*
opponent of the pair. The old file's "matches the spec's prediction" framing
had this backwards.

## What it does not say

- **Both fights are still walkovers.** A mid-grade three-Scrapper party
  clears five zone-appropriate rootkits, folded or not, without being
  stressed either way. This measurement is a read on `swing_share`'s
  *direction*, not on whether it is survivable at the edge — that needs a
  pack sized to actually threaten this party.
- **The party still plays only `AllAttack`-equivalent tactical AI** — no
  companion Special ever fires (`dev-arenas/README.md`'s standing gap).
  That is a floor on the party's output in both scenarios equally, so it
  cannot bias the comparison between them, only the absolute numbers.
- **The one seed 42 rep that downed a companion also hit this batch's
  longest fight (13 rounds).** One rep out of 50 is not enough to say
  whether the fixed squad has a real tail risk the old buggy one couldn't
  reach (because it could barely act) or whether this is ordinary variance
  — flagged rather than treated as a finding.
- **One species pair, one seed pair, one build**, same as the first run.
  Nothing here sweeps `swing_share` itself, a different pack species, or a
  harder-scaled fight. A `tuning.rs` change reshuffles the `GameRng` stream
  along with any curve it moves, so this file is only valid against
  `35db4357` — a re-run after a `swing_share` retune must be re-measured
  fresh, not diffed against these numbers either.
- **No sweep of `swing_share` itself, still.** This file answers "which
  direction does the fixed squad's power sit relative to five loose bodies,
  and did the earlier bug distort that" for the shipped `0.5`. It does not
  say what value the constant should move to, and per this task's scope
  that retune stays a human decision, along with the `balance_sim` re-gate
  a `tuning.rs` edit would need. What it does add over the first run: the
  gap is real and in the same direction with the bug fixed, but smaller
  than the buggy numbers implied — so a retune informed only by the
  contaminated file would have overcorrected.
