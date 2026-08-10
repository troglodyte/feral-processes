# Trained weights: which ones mean anything

**Date:** 2026-08-10
**Instrument:** `train` at three seeds on one build, weights compared directly
**Follows:** [the stun move levers](2026-08-10-stun-move-levers.md), whose
correction rested on reading one coefficient.

## The claim

**Individual weights in `assets/policies/enemy_battle.ron` are mostly
unidentifiable, and must not be read as statements about how the enemy
plays.** Trained three times at the same budget on the same assets, changing
only the CEM seed, **seven of the sixteen free features flip sign** and
`move_power_rel` spans 0.93 to 7.17 — while the thing being optimised moves
by 0.02.

Two features are stable in sign *and* rough magnitude across all three runs:
`target_hp_frac` (≈ −9.7) and `est_damage_frac` (≈ +8.8). Those two are the
policy. Everything else is the optimiser settling somewhere on a wide plateau.

This is not a bug in the trainer. It is what an over-parameterised linear
model does when several features carry overlapping information: many weight
vectors score nearly identically, so the search stops at whichever one it
reached first, and *which* one is a property of the seed.

## How to reproduce it

Three runs differing only in `--seed`. Rebuild first and run all three on
one binary — a rebuilt engine can shift the RNG stream, and weights from
different builds do not compare (see
[arena numbers compare within one build only](2026-08-10-enemy-policy-pin-sweep.md)).

```sh
cargo build --release --bin train
for N in 1 2 3; do
  nice -n 15 ./target/release/train --out seed$N.ron --scenarios dev-training \
      --iters 30 --pop 40 --reps 200 --seed $N \
      --pin target_is_player,target_bracing,target_def_rel \
      --report seed$N-report.ron &
done; wait
```

~23 min wall clock for all three together on 16 cores. `train` already
self-parallelises to 16 workers, so running them concurrently costs about
what running them in sequence would; it is not a speedup, just unattended.

No `--log-dir`: this question is answered from the weights and the report,
and logging the evaluation passes would add ~10 GB for nothing.

## The numbers

All three seeds, shipped assets, `pin3`. **Spread** is max − min, or a note
where the sign is not even consistent.

| feature | seed 1 | seed 2 | seed 3 | spread |
|---|---|---|---|---|
| `move_power_rel` | 2.14 | 0.93 | 7.17 | 6.24 |
| `move_ranged` | −2.29 | −0.68 | 0.26 | **sign flips** |
| `move_has_effect` | 0.87 | −0.44 | −0.73 | **sign flips** |
| `move_effect_stun` | 0.75 | 2.64 | 2.60 | 1.89 |
| `move_effect_bleed` | −0.71 | −0.84 | −3.18 | 2.47 |
| `move_effect_chance` | 2.05 | −0.24 | −3.14 | **sign flips** |
| `target_hp_frac` | −10.88 | −9.35 | −8.99 | 1.89 |
| `target_stunned` | 0.15 | −1.03 | 0.61 | **sign flips** |
| `target_bleeding` | 0.78 | 1.93 | 1.08 | 1.15 |
| `est_damage_frac` | 10.13 | 8.14 | 8.13 | 2.00 |
| `would_kill` | 5.84 | 1.24 | 0.94 | 4.90 |
| `self_hp_frac` | −2.80 | 0.05 | 2.16 | **sign flips** |
| `self_front_group` | −0.38 | −1.46 | −2.09 | 1.71 |
| `effect_x_target_healthy` | −6.87 | −5.07 | −1.64 | 5.23 |
| `stun_x_not_stunned` | −1.57 | −2.63 | 1.24 | **sign flips** |
| `bleed_x_not_bleeding` | −1.39 | 1.27 | −2.58 | **sign flips** |

`target_is_player`, `target_def_rel` and `target_bracing` are 0 in all three
by `--pin` and are omitted.

And what all that variation bought:

| seed | baseline win | trained win | player HP | best fitness |
|---|---|---|---|---|
| 1 | 0.2631 | 0.5650 | 0.3596 | 0.6290 |
| 2 | 0.2637 | 0.5450 | 0.3727 | 0.6077 |
| 3 | 0.2650 | 0.5481 | 0.3716 | 0.6110 |

**The outcome is reproducible; the explanation is not.** Enemy win rate
lands within 2 points across all three, and every run roughly doubles it
from baseline. Three quite different-looking policies play about equally
well — which is the finding, stated the other way round.

`self_hp_frac` is the cleanest illustration: −2.80, +0.05, +2.16. Read
literally those are "press when you're hurt", "ignore your own health" and
"press when you're healthy" — three opposite tactical doctrines, at
indistinguishable fitness.

### What this replicates and what it corrects

New. Nothing before this had trained the same configuration twice.

It **corrects** the reasoning behind
[the stun entry's correction](2026-08-10-stun-move-levers.md), which
attributed a roster-wide loss of move variety to `move_power_rel` flipping
−5.00 → +2.14 and treated 2.14 as a quantity. The magnitude is arbitrary. The
*direction* survives — all three seeds are positive, against −5.00 before the
retrain — so higher-power moves really are favoured now; but the sign is all
that entry is entitled to claim.

It is also the same failure as the `target_bracing` correction in
[a party that braces](2026-08-10-a-party-that-braces.md), one level up. That
one found a single feature's weight meaningless because the harness never
varied it. This finds most weights meaningless because the *fitness landscape*
cannot distinguish them. Both look identical from the outside: a plausible
number, confidently read.

## What it does not say

- **Nothing is wrong with the shipped policy.** It doubles the enemy win rate
  and does so as reliably as its siblings. "Arbitrary among near-equals" is
  not "bad" — it is only fatal to *explaining* the policy in terms of its
  weights.
- **Not measured: whether the three policies play the same in a fight.**
  Equal fitness over 3,200 fights is an aggregate. Two of these could differ
  in move choice or targeting in ways that matter to a player and cancel out
  in the win rate. Answering that needs `--log-dir` on the evaluation passes
  of seeds 2 and 3 and a `move_concentration` comparison — the honest way to
  settle whether the roster-wide concentration is real, and the way that does
  not involve reading a coefficient.
- **Three seeds is few.** Enough to prove instability, since one sign flip is
  a counterexample and there are seven. Not enough to estimate a mean for any
  feature — a stable-looking one here could still be a coincidence of three.
- **This says nothing about which features are worth having.** An
  unidentifiable weight is not an unnecessary feature; two collinear features
  can jointly matter while neither is separately pinned down. Deleting one to
  "clean up" is a change to the model, gated by fitness, not a tidy-up.
- **`balance_sim` is blind to all of it**, as ever: RNG-free, models no
  abilities.

## Open questions

- **Would fewer features be identifiable, and would they play as well?**
  Dropping to the two stable features plus the aggro prior is a testable
  policy. If it matches 0.55 enemy win, the other fourteen are decoration.
- **Does averaging weights across seeds beat any single seed?** Cheap to try
  and standard practice for a plateau; would also produce a policy whose
  coefficients mean something closer to what a reader expects.
- **Is `ENEMY_POLICY_TEMPERATURE` doing more work than the weights?** It
  divides the prior and the learned term alike, and with weights this loosely
  determined it may be the actual shipping control — which is what the stun
  entry already suspected for a different reason.
