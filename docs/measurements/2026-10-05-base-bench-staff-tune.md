# Base bench: a staff tune on `bench-economy`

Follows the [staff baseline](2026-10-05-base-bench-staff-baseline.md), whose
"6,000 ticks, sieges off" section holds the shipped values used here. This is
what `bench tune dev-tuning/staff.ron` proposes. Nothing was applied;
`assets/` is untouched and the decision is the user's.

## The claim

With the coherence-critical target dropped, the search finds a setting that
meets every remaining target on the three search seeds (error 0.0000 against
0.0342 shipped) and improves the hold-out seeds (0.0391 against 0.0839). All
of the shipped error is `sulking_share` being too low; the other three targets
were met by the shipped assets and stay met. The proposal gets sulking up
by turning the knobs hard: four knobs end at a bound, morale weights go to
their strongest allowed, and staff spend 3-6 points less of their time on shift
(about 0.88-0.90 against 0.94-0.95). One of the three search seeds now has a
fray. Whether that is "occasional trouble" is a design call (see
"Suspicious").

## First run, superseded

The first run (same budget, with a `need_critical_share.coherence` target at
0.01-0.05) proposed a setting that scored 0.0856 on the search seeds against
0.0788 shipped (and 0.0362 against 0.1284 on hold-out). It was superseded for
two reasons: `tune` proposed the CEM's final mean, which the search never
evaluated, so the proposal could score worse than shipped and than every
generation's best candidate (fixed: `tune` now re-scores the final mean and the
best candidate seen and proposes the lowest, or the shipped values if neither
beats them); and the user dropped the coherence-critical target, which no bound
reached (shipped 0.0016, proposed 0.0024 against a want of 0.01).

## How to reproduce it

```sh
cargo build --release --bin bench
bench tune dev-tuning/staff.ron --out /tmp/staff-out
```

Branch `staff-bench`, release, 16-core machine. `search_seed` 1337, search
seeds 1-3, hold-out 4-6, 6,000 ticks, sieges off, orders as in
`economy-bench.ron`, 22 iterations x 24 candidates: **13 m 09 s** wall (131
CPU-min). The tune is deterministic. The log ended `proposing the best
candidate seen`: the best candidate seen beat the final mean on the search
seeds (the mean's re-score is not logged, so how far behind it was is not
recorded). Generation bests ran 0.0339 down to 0.0000 (generation 19); the
generation means stayed at 0.04-0.13.

Per-seed numbers below come from `bench run` for seeds 1-6 against the shipped
assets and against a scratch copy of the repo with the proposed files in
`assets/` (the real `assets/` was never edited). They reproduce the tune's own
errors exactly (0.0342, 0.0000, 0.0839, 0.0391).

## The numbers

Fitness is the objective's error (squared miss outside each range over the
range width, summed over targets, mean over seeds; lower is better).

| fitness (error) | shipped | proposed |
|---|---|---|
| search seeds 1-3 | 0.0342 | 0.0000 |
| hold-out seeds 4-6 | 0.0839 | 0.0391 |

Targets, mean over seeds (shipped / proposed). "Want" is the range.

| measure | want | search shipped | search proposed | hold-out shipped | hold-out proposed |
|---|---|---|---|---|---|
| staff.sulking_share | 0.05-0.15 | 0.0317 | 0.0718 | 0.0239 | 0.0481 |
| staff.need_mean.slack | 60-85 | 83.30 | 83.90 | 81.46 | 82.89 |
| staff.frays_per_1000 | 0-0.2 | 0 | 0.056 | 0 | 0 |
| staff.rung_share.downed_tools | 0-0.02 | 0 | 0 | 0 | 0 |

Per seed, proposed (shipped in brackets). Coherence critical share is not a
target any more; it is shown because the proposal moves it.

| seed | sulking_share | slack mean | coherence critical | frays | on shift | morale min |
|---|---|---|---|---|---|---|
| 1 | 0.0643 (0.0282) | 84.1 (82.9) | 0.0373 (0.0024) | 1 (0) | 0.882 (0.941) | -15.9 (-15.4) |
| 2 | 0.0586 (0.0338) | 84.0 (84.2) | 0.0031 (0.0011) | 0 (0) | 0.896 (0.926) | -17.0 (-15.4) |
| 3 | 0.0925 (0.0330) | 83.6 (82.9) | 0.0039 (0.0012) | 0 (0) | 0.860 (0.939) | -18.8 (-16.4) |
| 4 | 0.0666 (0.0403) | 85.1 (81.9) | 0.0029 (0.0020) | 0 (0) | 0.902 (0.926) | -17.5 (-14.3) |
| 5 | 0.0620 (0.0220) | 84.0 (79.5) | 0.0035 (0.0012) | 0 (0) | 0.869 (0.963) | -16.4 (-14.4) |
| 6 | 0.0158 (0.0095) | 79.5 (83.0) | 0.0032 (0.0015) | 0 (0) | 0.942 (0.974) | -14.2 (-13.1) |

No run stopped (`stopped_at` None on all twelve). No tantrum on any seed; the
one fray is seed 1, first at tick 3406. Labour unworked is 0 except 0.0002 on
seed 1.

### Knobs

Proposed against shipped, with the bound range given. "At bound" means within
1% of the range width of a limit.

| file | field | shipped | proposed | range | at bound |
|---|---|---|---|---|---|
| needs/coherence.ron | drain_per_tick | 0.02 | 0.03 | 0.01-0.03 | **max** |
| needs/coherence.ron | working_multiplier | 2 | 2.890 | 1-3 | |
| needs/coherence.ron | critical | 20 | 15.03 | 10-30 | |
| needs/coherence.ron | content | 60 | 61.43 | 50-80 | |
| needs/coherence.ron | morale_weight | -4 | -6 | -6 to -2 | **strongest** |
| needs/slack.ron | drain_per_tick | 0.012 | 0.013478 | 0.006-0.018 | |
| needs/slack.ron | working_multiplier | 1.4 | 1.695 | 0.7-2.1 | |
| needs/slack.ron | critical | 25 | 23.82 | 10-30 | |
| needs/slack.ron | content | 70 | 63.17 | 50-80 | |
| needs/slack.ron | morale_weight | -3 | -4.5 | -4.5 to -1.5 | **strongest** |
| structures/defrag_bay.ron | services.coherence.per_tick | 0.6 | 0.800 | 0.3-0.9 | |
| structures/sandbox.ron | services.slack.per_tick | 0.5 | 0.75 | 0.25-0.75 | **max** |
| thoughts/beside_friend.ron | intensity | 2 | 3.684 | 0.5-4 | |
| thoughts/beside_rival.ron | intensity | -3 | -1.632 | -4 to -0.5 | |
| thoughts/machine_running.ron | intensity | 1 | 1.566 | 0.5-4 | |
| thoughts/no_amenity.ron | intensity | -2 | -0.5 | -4 to -0.5 | **weakest** |
| thoughts/unpowered.ron | intensity | -2 | -2.359 | -4 to -0.5 | |

Five of 17 knobs sit on a bound: coherence drain (max), both needs' morale
weights (strongest pull), Sandbox restore rate (max), `no_amenity` thought
(weakest).

## Suspicious

- **The search-seed zero is thin.** Error 0.0000 needs every seed's sulking in
  0.05-0.15 and the fray rate at or under 0.2; seed 1 has one fray in 6,000
  ticks, which is 0.167 per 1,000, just under the cap. A second fray on any
  seed would break it. Hold-out is 0.0391, not zero: seed 6 sulks 0.0158, still
  below range, so the gain does not carry to every seed.
- **Sulking comes from amplifying need strain, not from the thoughts.**
  Coherence drains at its fastest and both morale weights are at their
  strongest; the Sandbox is at its fastest restore to hold slack up. Staff
  spend less time on shift (0.88-0.90 against 0.94-0.95) and morale minima
  fall (-14 to -19). That meets "occasional trouble" by the metric; whether it
  looks like it in play is untested here.
- **Seed 1 is an outlier.** Coherence critical share is 0.037 there against
  about 0.003 on the others (shipped 0.001-0.002), and it is the seed with the
  fray. The other five seeds put coherence critical at 0.003-0.004, so the
  setting only touches critical on one seed in six.
- **`no_amenity` at -0.5 is the weakest allowed.** The missing-amenity thought
  is nearly switched off, so what an absent Defrag Bay or Sandbox costs staff
  is mostly the need itself. The search chose that; the model has no other cost
  for it.
- **Three targets are guards the shipped base already meets** (slack mean,
  frays, `downed_tools`). Meeting them is not evidence of anything.
- **Three seeds, one search seed (1337).** A different `search_seed` may land
  elsewhere; it was not tried.

## Open questions

- Whether sulking at about 0.05-0.07 reached by maximum drain and morale
  weights reads as occasional trouble in play, or as a base that is simply
  worse; the on-shift drop is the cost to weigh.
- Whether a wider hold-out (more than three seeds) keeps seed 6's miss as a
  one-off.
