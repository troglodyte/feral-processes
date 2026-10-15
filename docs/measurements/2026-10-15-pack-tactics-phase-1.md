# Pack tactics, phase 1: Together and Hold against Rush

**The claim.** On the mixed scenario `dev-arenas/tactical-mixed-pack.ron`
(200 reps, `party_turns: AutoPlay`) `Hold` is within the bar against `Rush`
as shipped (win rate equal at 98.0%, party Integrity lost 18.0% against
17.6%). `Together` was not: at `TACTICAL_COHESION_SLACK = 2` it lost the party
only 9.3% Integrity against Rush's 17.6% (-47%) and cleared fights in 11.9
rounds against 14.5. At slack 4 it is inside the bar (win 99.0% vs 98.0%, HP
lost 16.7% vs 17.6%, 14.2 rounds). `tactical-full-group.ron` is a walkover
under all three tactics (100% wins, ~1% Integrity lost), so it can bound a
tactic only from above. Hold ends in a fight no longer than Rush's, so it does
not stall.

## How to reproduce it

Same build for every row (`unset FERAL_DEV_NO_SIEGES`). Each variant is the
committed scenario with `party_turns: AutoPlay`, `reps: 200` and one forced
`tactic: Some(Rush|Together|Hold)` line, seed 7 (reps run at seeds 7..206):

```sh
cargo run --bin arena -- /tmp/claude-1000/m1/tactical-mixed-pack-Together.ron --out out.ron
```

`tactical-full-group.ron` carries no `party_turns`, so the variants add it.
HP lost is `1 - mean(player_hp_fraction)` over the report's reps; rounds is
the mean of `rounds`. Slack 2 and slack 4 are the only difference between the
two Together rows.

## The numbers

New measurements, none replicating an earlier note.

`tactical-mixed-pack.ron` (4 sentinels, melee reach 1, and 4 rootkits, ranged
reach 2, against a level-14 player with three level-8 scrappers):

| Tactic | Reps | Win rate | HP lost | Mean rounds |
|---|---|---|---|---|
| Rush | 200 | 98.0% | 17.64% | 14.53 |
| Together, slack 2 | 200 | 99.5% | 9.29% | 11.89 |
| Together, slack 4 | 200 | 99.0% | 16.65% | 14.22 |
| Hold | 200 | 98.0% | 18.01% | 14.67 |

`tactical-full-group.ron` (4 rootkits, level-20 player, three level-12
scrappers):

| Tactic | Reps | Win rate | HP lost | Mean rounds |
|---|---|---|---|---|
| Rush | 200 | 100.0% | 1.27% | 5.03 |
| Together, slack 2 | 200 | 100.0% | 0.80% | 4.56 |
| Together, slack 4 | 200 | 100.0% | 1.45% | 5.00 |
| Hold | 200 | 100.0% | 1.35% | 5.04 |

The one weight change: `TACTICAL_COHESION_SLACK` 2 -> 4. Factor and every
`TACTICAL_HOLD_*` constant are untouched.

## What it does not say

- The party plays `AutoPlay`, not a person; no Special fires from a companion
  that the `[A]` logic would not fire.
- The full-group scenario is a walkover: a 20% bar on ~1% Integrity lost is
  noise, and 200 wins in 200 cannot show a win-rate gap. Only the mixed
  scenario carries signal, and it is one pack shape and one party.
- Hold broke cover in all 200 reps of both scenarios, on the round the party
  came inside its alert range; rounds match Rush's, so "Hold" here is a short
  delay and a cover-seeking approach, not a long ambush. Whether a longer wait
  matters against a party that also waits is unmeasured.
- Together's slack was moved to the first value inside the bar, not fitted;
  slack 3 was not run.
- The mixed scenario was sized by trial (2+2 and 3+3 at level 20/12 were
  100% wins losing under 10% Integrity, so insensitive). 200 reps puts a
  win-rate difference of one point inside the noise.
- A crash found on the way (a step that ends the fight, then a read of the
  vanished board in `run_tactical_beat`) was fixed in a separate commit; no
  completed rep's result depends on it.
