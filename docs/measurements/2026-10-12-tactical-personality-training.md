# Tactical personalities trained toward the `default` baseline

**The claim.** CEM over each profile's free weights, scored against
`2026-10-12-tactical-personality-baseline.md`, put every profile inside
+/-10% (relative) of baseline on both set-wide HP lost and enemy win rate.
Two of the trained results (brute, hunter) failed the profile behaviour
tests in `crates/engine/src/tests/tactical_policy.rs` — the training erased
the character — so those two files keep their hand-set weights. Shipped:
skirmisher and cautious trained; brute and hunter hand-set.

## How to reproduce it

Build 86b219b7 release, baseline and trained in the same process:

```sh
./target/release/train --tactical --profile <id> --reps 200 --iters 15 --pop 24 --search-reps 50
./target/release/train --tactical --profile <id> --eval --reps 200   # a profile as on disk
```

The search moves a delta from the hand-set weights (generation 0 is the
authored profile), 50 reps per scenario per candidate; the comparison below
is 200 reps on the baseline's seeds.

## The numbers

Set means, baseline HP lost 0.4193, enemy win 0.3031.

| Profile | Hand-set (HP lost / enemy win) | Trained (HP lost / enemy win) | In band | Behaviour tests |
|---|---|---|---|---|
| brute | -3.2% / -5.4% | -3.5% / -4.1% | yes | trained FAILS; hand-set kept |
| hunter | -11.7% / -13.0% | -8.7% / -8.5% | yes | trained FAILS; hand-set kept (out of band) |
| skirmisher | -0.7% / -0.6% | +3.4% / -6.0% | yes | pass; trained shipped |
| cautious | -0.9% / -0.8% | -1.0% / -1.0% | yes | pass; trained shipped |

Trained free weights (hand -> trained): brute `target_hp_frac` -0.5 ->
-1.64; hunter `target_hp_frac` -3.0 -> -9.34, `est_damage_frac` 1.0 ->
3.62; skirmisher `target_hp_frac` -3.2 -> 0.0 (clamped at its sign),
`would_kill` 4.1 -> 3.07; cautious `target_hp_frac` -1.5 -> -1.14,
`would_kill` 1.0 -> 0.0.

Brute and cautious hand-set weights were already in band; hunter's was not.
The measured hunter band is therefore not achievable with the weights that
keep its character, and is left open for a decision.

## What it does not say

- Skirmisher's trained `target_hp_frac` sits on its sign boundary (0) and
  cautious's `would_kill` too: the search found the free weights worth
  nothing for difficulty there. They pass the behaviour tests but their
  `description` text ("picks at the weakest") no longer describes the
  weights.
- The hand-set column is one 200-rep run on a shared seed; differences of a
  few percent are within run-to-run noise from a different seed.
- Same blind spots as the baseline note: two walkover scenarios, one noisy
  rounds term (06), every hostile on one profile.
