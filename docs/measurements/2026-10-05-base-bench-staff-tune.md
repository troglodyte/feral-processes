# Base bench: a staff tune on `bench-economy`

Follows the [staff baseline](2026-10-05-base-bench-staff-baseline.md), whose
"6,000 ticks, sieges off" section holds the shipped values used here. This is
what `bench tune dev-tuning/staff.ron` proposed. Nothing was applied;
`assets/` is untouched and the decision is the user's.

## The claim

The search found no setting that puts the shipped base into "occasional
trouble". Of the five targets, three (slack mean, frays, `downed_tools`) were
already met by the shipped assets and stay met. The two that were not,
`sulking_share` (want 0.05-0.15, shipped 0.031) and
`need_critical_share.coherence` (want 0.01-0.05, shipped 0.0016), moved
little: sulking reaches 0.052 on the hold-out seeds but stays 0.031 on the
search seeds, and coherence-critical does not move (0.0024). **The proposal is
worse than shipped on the search seeds (error 0.0856 against 0.0788) and
better on the hold-out seeds (0.0362 against 0.1284).** That is a mixed
result, not evidence the proposal improves the base. Read it as: the bounds
given cannot reach the coherence-critical target on this base in 6,000 ticks.

## How to reproduce it

```sh
cargo build --release --bin bench
bench tune dev-tuning/staff.ron --out /tmp/staff-out
```

Branch `staff-bench`, release, 16-core machine. `search_seed` 1337, search
seeds 1-3, hold-out 4-6, 6,000 ticks, sieges off, orders as in
`economy-bench.ron`. The tune is deterministic (two runs with different
`iterations` print identical generations as far as they overlap).

| budget | wall time | note |
|---|---|---|
| 30 iterations x 24 candidates | 18 m 40 s (191 CPU-min) | over the 15-minute cap; first run, discarded |
| **22 iterations x 24 candidates** | **13 m 53 s (140 CPU-min)** | the committed budget; the numbers below |

About 5.3 CPU-seconds per 6,000-tick run (about 1.5 times the 2 x 1.7 s
estimated), three runs per candidate. The 30-iteration run proposed a more
extreme setting (coherence drain 0.03, working multiplier 3, Defrag Bay 0.9,
machine-running thought 4: all at their bounds) and scored sulking 0.035 and
coherence-critical 0.003 on the hold-out; it was not the committed result.

Per-seed numbers below come from running `bench run` for seeds 1-6 against the
shipped assets and against a copy of the repo with the proposed files in
`assets/` (a scratch copy; the real `assets/` was never edited).

## The numbers

New. Fitness is the objective's error (squared miss outside each range over
the range width; lower is better).

| fitness (error) | shipped | proposed |
|---|---|---|
| search seeds 1-3 | 0.0788 | 0.0856 |
| hold-out seeds 4-6 | 0.1284 | 0.0362 |

Targets, mean over seeds (shipped / proposed). "Want" is the range.

| measure | want | train shipped | train proposed | hold-out shipped | hold-out proposed |
|---|---|---|---|---|---|
| sulking_share | 0.05-0.15 | 0.0317 | 0.0313 | 0.0239 | 0.0522 |
| need_critical_share.coherence | 0.01-0.05 | 0.0016 | 0.0024 | 0.0016 | 0.0025 |
| need_mean.slack | 60-85 | 83.30 | 82.33 | 81.46 | 81.74 |
| frays_per_1000 | 0-0.2 | 0 | 0 | 0 | 0 |
| rung_share.downed_tools | 0-0.02 | 0 | 0 | 0 | 0 |

Per seed, proposed (shipped in brackets):

| seed | sulking_share | coherence critical_share | on_shift_share | morale mean / min | tantrums, frays |
|---|---|---|---|---|---|
| 1 | 0.0156 (0.0282) | 0.0025 (0.0024) | 0.960 (0.941) | 5.46 / -19.99 (4.89 / -15.36) | 0, 0 |
| 2 | 0.0333 (0.0338) | 0.0025 (0.0011) | 0.928 (0.926) | 4.90 / -14.71 (4.33 / -15.38) | 0, 0 |
| 3 | 0.0452 (0.0330) | 0.0023 (0.0012) | 0.921 (0.939) | 6.21 / -17.48 (3.48 / -16.40) | 0, 0 |
| 4 | 0.0489 (0.0403) | 0.0023 (0.0020) | 0.913 (0.926) | 5.23 / -14.18 (5.40 / -14.32) | 0, 0 |
| 5 | 0.0618 (0.0220) | 0.0023 (0.0012) | 0.905 (0.963) | 5.15 / -19.49 (4.80 / -14.42) | 0, 0 |
| 6 | 0.0457 (0.0095) | 0.0028 (0.0015) | 0.916 (0.974) | 5.85 / -14.01 (4.78 / -13.06) | 0, 0 |

No run stopped (`stopped_at` None on all twelve). Labour unworked is 0 on
every run, so staff stay on shift (90-96% of staff-ticks) with the proposed
numbers too. No tantrum and no fray on any seed; `first_fray` none.

### Knobs

Proposed against shipped, with the bound range given. "At bound" means within
1% of the range width of a limit.

| file | field | shipped | proposed | range | at bound |
|---|---|---|---|---|---|
| needs/coherence.ron | drain_per_tick | 0.02 | 0.02848 | 0.01-0.03 | near max |
| needs/coherence.ron | working_multiplier | 2 | 2.607 | 1-3 | |
| needs/coherence.ron | critical | 20 | 16.23 | 10-30 | |
| needs/coherence.ron | content | 60 | 69.87 | 50-80 | |
| needs/coherence.ron | morale_weight | -4 | -4.088 | -6 to -2 | |
| needs/slack.ron | drain_per_tick | 0.012 | 0.012902 | 0.006-0.018 | |
| needs/slack.ron | working_multiplier | 1.4 | 1.018 | 0.7-2.1 | |
| needs/slack.ron | critical | 25 | 21.54 | 10-30 | |
| needs/slack.ron | content | 70 | 77.54 | 50-80 | near max |
| needs/slack.ron | morale_weight | -3 | -2.329 | -4.5 to -1.5 | |
| structures/defrag_bay.ron | services.coherence.per_tick | 0.6 | 0.9 | 0.3-0.9 | **max** |
| structures/sandbox.ron | services.slack.per_tick | 0.5 | 0.398 | 0.25-0.75 | |
| thoughts/beside_friend.ron | intensity | 2 | 1.690 | 0.5-4 | |
| thoughts/beside_rival.ron | intensity | -3 | -2.264 | -4 to -0.5 | |
| thoughts/machine_running.ron | intensity | 1 | 2.690 | 0.5-4 | |
| thoughts/no_amenity.ron | intensity | -2 | -3.466 | -4 to -0.5 | |
| thoughts/unpowered.ron | intensity | -2 | -2.924 | -4 to -0.5 | |

Only the Defrag Bay restore rate sits exactly on a bound; coherence drain
(0.0285 of 0.03) and slack `content` (77.5 of 80) are close to theirs. The
30-iteration run pinned more: coherence drain 0.03, working multiplier 3,
Defrag Bay 0.9 and machine-running thought 4, all at their maxima.

## What it does not say

Flagged plainly, because several of these make the headline numbers weaker
than they look.

- **The proposal is not better on the search seeds.** Error 0.0856 after,
  0.0788 before. The per-generation log shows a best candidate at 0.02-0.05
  every generation, but the proposal written is not that candidate: tune
  re-scores the search's final answer, and that re-score is higher than any
  generation's best. A search whose answer scores worse than its own best
  candidates and than the shipped assets is not converged. The hold-out gain
  (0.1284 to 0.0362) is mostly `sulking_share` on seeds 4-6, where the shipped
  base happens to be low (0.0239), and a three-seed hold-out is thin.
- **The coherence-critical target is not reached by any knob here.**
  Shipped 0.0016, proposed 0.0024, want 0.01-0.05. The search raised
  coherence drain and the working multiplier, but also raised the Defrag Bay
  to its maximum rate and **lowered `critical` from 20 to 16.2**, which makes
  being "critical" harder. Critical share compared across the two columns is
  against different lines (20 against 16.2), so the 0.0016-to-0.0024 rise is
  not like for like.
- **Three targets are guards the shipped base already meets** (slack mean,
  frays, `downed_tools`). Meeting them is not evidence of anything.
- **Sulking moves by the thoughts, not by the needs.** `no_amenity` and
  `unpowered` are both pushed toward -3 and `machine_running` up to 2.7, so
  the search is widening the morale spread (morale min goes to about -17 to
  -20 on some seeds, from about -15) rather than starving staff. Whether that
  is "occasional trouble" or just a wider swing is a design call.
- **Slack never meaningfully reaches critical** (min about 21-39 against a
  critical that the search lowered to 21.5), so the five slack knobs are
  observed only through `need_mean.slack` and morale, and slack
  `critical`/`content` mostly have nothing to act on.
- **Three seeds, one search seed (1337), a deterministic search.** A different
  `search_seed` may land elsewhere; it was not tried.

## Open questions

- Whether the coherence-critical range is reachable at all within these
  bounds on `bench-economy`, or whether the Defrag Bay's reach (radius 1)
  means most staff never need it. A wider `critical` or a lower Defrag Bay
  minimum (below 0.3) would test that.
- Why the final answer re-scores worse than the best candidate in every
  generation (see above); the tune's output is the CEM mean, which may be
  the cause, but this was not checked in code.
