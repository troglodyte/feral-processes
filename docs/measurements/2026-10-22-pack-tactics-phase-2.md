# Pack tactics, phase 2: Formation and Flank against Rush

**The claim.** On `dev-arenas/tactical-mixed-pack.ron` (200 reps,
`party_turns: AutoPlay`) `Flank` is within the bar against `Rush` (win rate
100.0% against 99.0%, party Integrity lost 15.8% against 15.4%). `Formation`
is not, and no setting of its three knobs brings it there: the party loses
11.1% as shipped, and every variant measured lost 6.7–11.1%, against a bar
of at least 12.3%. Holding the ranged bodies behind the melee ones is what
costs the pack: with every Formation term zeroed it matches Rush exactly,
and the back-rank charge alone accounts for nearly all of the gap. The user
chose to ship Formation as an easier tactic (2026-10-22).

## How to reproduce it

Same build for every row (`unset FERAL_DEV_NO_SIEGES`). Each variant is the
committed scenario with one forced `tactic: Some(Rush|Formation|Flank)`
line; `tactical-full-group.ron` variants also add `party_turns: AutoPlay`
and `reps: 200`. Seed 7, so reps run at seeds 7..206:

```sh
cargo run --bin arena -- tactical-mixed-pack-Formation.ron --out out.ron
```

HP lost is `1 - mean(player_hp_fraction)` over the report's reps; rounds is
the mean of `rounds`. The knob sweeps edited `tuning/tactical.rs` in place
and restored it after each run.

## The numbers

New measurements. The Rush rows replicate phase 1's on a later build
(15.4% against 17.6% then); compare only within this note
(`arena-numbers-compare-within-one-build-only`).

`tactical-mixed-pack.ron`, as shipped (depth 2, back closing 0.25, weight
1.0, back rank charged one-sided):

| Tactic | Reps | Win rate | HP lost | Mean rounds |
|---|---|---|---|---|
| Rush | 200 | 99.0% | 15.38% | 13.71 |
| Formation | 200 | 100.0% | 11.09% | 10.11 |
| Flank | 200 | 100.0% | 15.83% | 12.34 |

`tactical-full-group.ron` (a walkover, so an upper bound only):

| Tactic | Reps | Win rate | HP lost | Mean rounds |
|---|---|---|---|---|
| Rush | 200 | 100.0% | 1.27% | 4.89 |
| Flank | 200 | 100.0% | 1.75% | 4.92 |

Formation variants on the mixed pack, 200 reps each. "Two-sided" is the
first build, where the back rank was charged its whole distance to the slot;
"one-sided" charges only for standing ahead of it.

| Back-rank charge | Depth | Back closing | Weight | HP lost | Mean rounds |
|---|---|---|---|---|---|
| two-sided | 2 | 0.25 | 1.0 | 8.58% | 9.64 |
| two-sided | 1 | 0.25 | 1.0 | 6.88% | 9.22 |
| two-sided | 1 | 0.5 | 1.0 | 6.73% | 9.22 |
| two-sided | 2 | 0.25 | 0.5 | 7.91% | 9.47 |
| two-sided | 2 | 0.6 | 0.5 | 7.59% | 9.60 |
| two-sided | 2 | 1.0 | 1.0 | 7.95% | 9.61 |
| two-sided | 3 | 0.25 | 1.0 | 8.92% | 9.44 |
| two-sided | 2 | 0.6 | 0.25 | 7.33% | 9.55 |
| one-sided | 2 | 0.25 | 1.0 | 11.09% | 10.11 |
| one-sided | 1 | 0.25 | 1.0 | 10.19% | 10.49 |
| one-sided | 1 | 0.5 | 1.0 | 10.47% | 10.45 |
| one-sided | 2 | 0.5 | 1.0 | 10.74% | 10.16 |
| one-sided | 1 | 1.0 | 1.0 | 10.58% | 10.39 |
| one-sided | 0 | 1.0 | 1.0 | 10.45% | 10.93 |

Isolation runs, two-sided build, 40 reps (Rush at 40 reps: 14% lost, 14.4
rounds): every Formation term zeroed, 14% and 14.4 (identical to Rush); the
back-rank term alone, 6% and 10.1; the front rank's along-the-line term
alone, 17% and 13.3.

## What it does not say

- One pack shape (four sentinels, four rootkits) and one party. In this pack
  the rootkits are the damage, so anything that keeps them back weakens it;
  a pack whose melee bodies hit harder may not lose by lining up.
- The party plays `AutoPlay`, not a person.
- Fights under Formation are shorter (10 rounds against 14) because the
  party reaches the front rank as a block; whether a player notices that as
  "easier" or as "different" is unmeasured.
- The 40-rep isolation runs are directional only; a 3-point difference at
  40 reps is inside the noise.
