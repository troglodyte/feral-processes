# Tactical personalities against the `default` baseline

**The claim.** Against the corrected baseline
(`2026-10-12-tactical-personality-baseline.md`, build f147dde3: HP lost
0.4699, enemy win 0.3144), **no profile is inside the +/-10% band on both
set-wide means**, and training did not find one that is. All four profiles
ship hand-set; the band was not loosened. The earlier 86b219b7 training
(which put skirmisher and cautious in band) was measured against a `default`
that ranked targets by Integrity fraction rather than the pre-branch absolute
Integrity, and is withdrawn.

## How to reproduce it

Build f147dde3 release:

```sh
./target/release/train --tactical --profile <id> --eval --reps 200   # a profile as on disk
./target/release/train --tactical --profile <id> --reps 200 --iters 15 --pop 24 --search-reps 50
```

The search moves a delta from the hand-set weights (generation 0 is the
authored profile), 50 reps per scenario per candidate; the comparison is 200
reps on the baseline's seeds. A trained profile is written only if in band.

## The numbers

Relative to baseline, set means.

| Profile | Hand-set HP lost / enemy win | Trained HP lost / enemy win | In band | Shipped |
|---|---|---|---|---|
| brute | -14.0% / -9.7% | not retrained | no (HP lost) | hand-set |
| hunter | -21.2% / -16.7% | not retrained (`target_hp_frac` pinned, one free weight) | no | hand-set |
| skirmisher | -11.4% / -3.8% | -11.4% / -3.8% (`would_kill` 5.72 -> 7.34, `target_hp_frac` -3.2 pinned) | no | hand-set |
| cautious | -11.4% / -4.4% | -11.1% / -4.2% (`target_hp_frac` -1.14 -> -0.29) | no | hand-set |

Every profile is easier on the player than `default` on HP lost, by 11% to
21%. Each personality walks to cover or closes faster and so trades fewer
blows with the player than `default`; the free target weights cannot buy that
back, which is why the search ends where it started (skirmisher's trained
numbers equal its hand-set ones to three places).

## What it does not say

- The trained candidates of skirmisher and cautious bounded the search at the
  free weights alone; the pinned character (distance, cover, `self_x_target_
  distance`) and the cell weights were not moved. A band for these profiles
  would need a decision about pinning, not more iterations.
- Scenario 06 inflates under both trained candidates (rounds 31 -> 52..62):
  the fights stall, and the rounds term of the objective is noisy there.
- Same blind spots as the baseline note: two walkover scenarios, one noisy
  rounds term (06), every hostile on one profile.
