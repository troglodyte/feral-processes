# Tactical personality baseline: what `default` does on the benchmark set

**The claim.** On the eight fixed tactical fights in `dev-training/tactical/`
(party on `AutoPlay`, 200 reps each, every hostile forced to the `default`
profile) the player loses a mean 41.9% of Integrity and the hostiles win
30.3% of fights. These are the targets each personality is trained toward.
The run is deterministic: three further invocations on the same build
reproduced these figures to the digit.

## How to reproduce it

```sh
git rev-parse HEAD      # build: 86b219b7 (engine unchanged since 5cc16239)
cargo build --release --bin train
./target/release/train --tactical --profile brute --eval --reps 200
```

`--eval` prints the baseline (`default`) beside the named profile as it is
on disk. Seed offset 1, scenario seeds in the files.

## The numbers

New measurement, nothing to replicate.

| Scenario | HP lost | Enemy win | Mean rounds |
|---|---|---|---|
| 01-solo-early | 0.959 | 0.865 | 7.8 |
| 02-early-pair | 0.390 | 0.130 | 6.8 |
| 03-midgame-group | 0.456 | 0.220 | 9.3 |
| 04-squad | 0.013 | 0.000 | 7.6 |
| 05-geared-late | 0.052 | 0.000 | 7.8 |
| 06-covered-board | 0.413 | 0.330 | 31.7 |
| 07-ranged-heavy | 0.946 | 0.870 | 10.7 |
| 08-opponent-routines | 0.124 | 0.010 | 5.6 |
| **Set mean** | 0.4193 | 0.3031 | |

## What it does not say

- Two scenarios (04, 05) are walkovers: the player wins every rep and only
  the HP-lost term moves. 01 and 07 are near-certain hostile wins. The set
  spans fights but not the middle of the win-rate range.
- 06 (rolled Mainframe field) has a mean of 31.7 rounds against a median far
  lower: a few reps stall. Its rounds term is noisy.
- Every hostile in a run plays one profile, so this measures a profile
  against the `default` one, not the species mix the game fields.
- Arena figures compare within one build only; do not set these beside a
  number from another commit.
