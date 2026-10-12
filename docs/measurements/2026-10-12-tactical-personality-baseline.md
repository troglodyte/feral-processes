# Tactical personality baseline: what `default` does on the benchmark set

**The claim.** On the eight fixed tactical fights in `dev-training/tactical/`
(party on `AutoPlay`, 200 reps each, every hostile forced to the `default`
profile) the player loses a mean 47.0% of Integrity and the hostiles win
31.4% of fights. These are the targets each personality is trained toward.
The run is deterministic: repeated invocations on the same build reproduce
these figures to the digit. Re-taken on f147dde3 after the final-review fixes
(`default` now picks the lowest *absolute* Integrity, as the rule it replaced
did, and only its first ready routine is a candidate); the earlier figures
(41.9% / 30.3%, build 86b219b7) measured a `default` that was not the
pre-branch behaviour and are withdrawn.

## How to reproduce it

```sh
git rev-parse HEAD      # build: f147dde3
cargo build --release --bin train
./target/release/train --tactical --profile brute --eval --reps 200
```

`--eval` prints the baseline (`default`) beside the named profile as it is
on disk. Seed offset 1, scenario seeds in the files.

## The numbers

Replaces the 86b219b7 table.

| Scenario | HP lost | Enemy win | Mean rounds |
|---|---|---|---|
| 01-solo-early | 0.959 | 0.865 | 7.8 |
| 02-early-pair | 0.503 | 0.135 | 6.5 |
| 03-midgame-group | 0.631 | 0.315 | 8.7 |
| 04-squad | 0.020 | 0.000 | 7.6 |
| 05-geared-late | 0.115 | 0.000 | 7.7 |
| 06-covered-board | 0.438 | 0.340 | 31.1 |
| 07-ranged-heavy | 0.953 | 0.850 | 9.2 |
| 08-opponent-routines | 0.139 | 0.010 | 5.6 |
| **Set mean** | 0.4699 | 0.3144 | |

## What it does not say

- Two scenarios (04, 05) are walkovers: the player wins every rep and only
  the HP-lost term moves. 01 and 07 are near-certain hostile wins. The set
  spans fights but not the middle of the win-rate range.
- 06 (rolled Mainframe field) has a mean of 31.1 rounds against a median far
  lower: a few reps stall. Its rounds term is noisy.
- Every hostile in a run plays one profile, so this measures a profile
  against the `default` one, not the species mix the game fields.
- Arena figures compare within one build only; do not set these beside a
  number from another commit.
