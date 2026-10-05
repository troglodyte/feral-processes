# Base bench: the staff baseline on `bench-economy` and `chains`

Follows the [`bench-economy` baseline](2026-10-05-base-bench-economy-baseline.md).
P2 of the base bench adds a staff section to the report (morale, needs,
grievance rungs, tantrums, frays). This is what it says on the two templates
before any target range or knob is chosen. Nothing here is tuned.

## The claim

Over 3,000 ticks, seeds 1-5, with 15 staff on both templates:

- **`bench-economy` (a Sandbox and a Defrag Bay) barely moves.** Staff work
  94.7% of ticks (range 90.4-98.7%), mean morale +2.4, sulking 3.1% of
  staff-ticks. Coherence touches its critical line (min 19.7-19.9 against a
  critical of 20) for 0.04-0.09% of staff-ticks and recovers; slack never gets
  near it (min 54-66). One rung is reached, `sulking` (4.8% of staff-ticks,
  0.8-9.2% by seed). **No tantrum, no fray, no `downed_tools`, no
  `lashing_out`, on any seed.**
- **`chains` (no amenity) is the fray contrast.** Staff are on shift 97.8% of
  ticks (100% on four seeds, 89.0% on seed 1), mean morale -4.8, sulking 22.4%. Coherence is at 0 for someone on
  every seed and below critical for 5.2% of staff-ticks. 3-4 frays per run,
  first at tick 1922-2328. `downed_tools` is reached on 3 of 5 seeds (mean
  3.9%). No tantrum and no `lashing_out`.
- **The plan's risk, answered on `bench-economy`:** within 3,000 ticks a need
  does reach critical (coherence, barely, 31 staff-ticks of 45,000 on seed 1);
  one rung other than `none` is reached (`sulking`); no fray and no tantrum
  occurs. See "The risk" below.

## How to reproduce it

```sh
cargo build --release --bin bench
for t in bench-economy chains; do for s in 1 2 3 4 5; do
  cargo run --release --bin bench -- run --template $t --ticks 3000 --seed $s \
    --order patch_routine:9999 --order bytecode_block:9999 --order ice_breaker:9999 \
    --out $t-$s.ron
done; done
# diagnostic, not baseline
cargo run --release --bin bench -- run --template bench-economy --ticks 6000 --seed 1 \
  --order patch_routine:9999 --order bytecode_block:9999 --order ice_breaker:9999
```

Build: branch `staff-bench` at 44b646f4, release, 16-core machine. Full gate
before the runs: `cargo test --workspace`, 7655 passed, 0 failed, 2 ignored.
A 3000-tick `bench-economy` run takes 1.7 s wall (1.7 s user).

No run in the baseline stopped (`stopped_at: None` on all ten). Both templates
have 15 staff, so `staff_ticks` is 45,000 everywhere (15 x 3000).

## The numbers

All new: no earlier note measured staff. Shares are of `staff_ticks`.

### `bench-economy`, 3,000 ticks

| measure | seed 1 | 2 | 3 | 4 | 5 | mean | range |
|---|---|---|---|---|---|---|---|
| on_shift_share | 0.9517 | 0.9180 | 0.9718 | 0.9043 | 0.9868 | 0.9465 | 0.9043-0.9868 |
| morale_mean | 2.854 | 2.612 | 2.844 | 1.584 | 2.306 | 2.440 | 1.584-2.854 |
| morale_min | -11.44 | -15.03 | -10.84 | -16.65 | -10.21 | -12.84 | -16.65 to -10.21 |
| sulking_share | 0.0246 | 0.0468 | 0.0115 | 0.0677 | 0.0045 | 0.0310 | 0.0045-0.0677 |
| need_strain_mean | -0.1949 | -0.1719 | -0.2261 | -0.1693 | -0.1776 | -0.1880 | -0.2261 to -0.1693 |
| coherence mean | 77.18 | 78.70 | 78.49 | 79.84 | 78.74 | 78.59 | 77.18-79.84 |
| coherence min | 19.70 | 19.77 | 19.80 | 19.86 | 19.75 | 19.78 | 19.70-19.86 |
| coherence critical_share | 0.00069 | 0.00050 | 0.00090 | 0.00040 | 0.00070 | 0.00064 | 0.0004-0.0009 |
| slack mean | 90.60 | 91.01 | 89.48 | 92.05 | 90.05 | 90.64 | 89.48-92.05 |
| slack min | 60.39 | 63.62 | 54.47 | 62.16 | 66.05 | 61.34 | 54.47-66.05 |
| slack critical_share | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| rung none | 0.9571 | 0.9228 | 0.9791 | 0.9077 | 0.9921 | 0.9518 | 0.9077-0.9921 |
| rung sulking | 0.0429 | 0.0772 | 0.0209 | 0.0923 | 0.0079 | 0.0482 | 0.0079-0.0923 |
| rung downed_tools | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| rung lashing_out | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| tantrums | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| frays | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| first_fray | none | none | none | none | none | none | |
| stopped_at | none | none | none | none | none | none | |

### `chains`, 3,000 ticks

| measure | seed 1 | 2 | 3 | 4 | 5 | mean | range |
|---|---|---|---|---|---|---|---|
| on_shift_share | 0.8904 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.9781 | 0.8904-1.0000 |
| morale_mean | -7.792 | -4.207 | -4.480 | -3.638 | -4.032 | -4.830 | -7.792 to -3.638 |
| morale_min | -54.75 | -57.17 | -32.54 | -40.95 | -50.61 | -47.20 | -57.17 to -32.54 |
| sulking_share | 0.2422 | 0.1942 | 0.2559 | 0.2283 | 0.1969 | 0.2235 | 0.1942-0.2559 |
| need_strain_mean | -0.6259 | -0.6829 | -0.6254 | -0.5911 | -0.6301 | -0.6311 | -0.6829 to -0.5911 |
| coherence mean | 62.91 | 62.43 | 62.94 | 63.47 | 63.04 | 62.96 | 62.43-63.47 |
| coherence min | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| coherence critical_share | 0.0405 | 0.0767 | 0.0449 | 0.0422 | 0.0567 | 0.0522 | 0.0405-0.0767 |
| slack mean | 79.86 | 79.64 | 79.86 | 79.95 | 79.85 | 79.83 | 79.64-79.95 |
| slack min | 52.74 | 39.26 | 51.95 | 49.60 | 49.60 | 48.63 | 39.26-52.74 |
| slack critical_share | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| rung none | 0.7507 | 0.7998 | 0.7442 | 0.7605 | 0.7920 | 0.7694 | 0.7442-0.7998 |
| rung sulking | 0.1397 | 0.1644 | 0.2558 | 0.2395 | 0.1579 | 0.1915 | 0.1397-0.2558 |
| rung downed_tools | 0.1096 | 0.0358 | 0 | 0 | 0.0500 | 0.0391 | 0-0.1096 |
| rung lashing_out | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| tantrums | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| frays | 4 | 4 | 4 | 3 | 3 | 3.6 | 3-4 |
| first_fray (tick) | 2328 | 1922 | 2246 | 2000 | 2000 | 2099 | 1922-2328 |
| stopped_at | none | none | none | none | none | none | |

### The risk

The plan feared that, with coherence draining 0.02/tick x2.0 at work, a full
reserve reaches critical only after about 2,000 working ticks and a siege
stops `bench-economy` near tick 3,800, so 3,000 ticks might show no staff
movement at all. What 3,000 ticks of `bench-economy` showed:

| question | answer |
|---|---|
| any need critical? | yes, coherence only: min 19.70-19.86 on all five seeds, critical for 0.04-0.09% of staff-ticks (18-41 staff-ticks of 45,000). It dips just under 20 and comes back; slack min is 54-66. |
| any rung other than `none`? | yes, `sulking` on every seed (0.8-9.2% of staff-ticks). `downed_tools` and `lashing_out`: no. |
| any fray? | no, 0 on all seeds, `first_fray` none |
| any tantrum? | no, 0 on all seeds |

So the staff section is not flat on `bench-economy`, but everything past
`sulking` (the fray, the two heavier rungs, the tantrum) is out of reach in
3,000 ticks with these orders. The run length was not changed.

### Diagnostic, not baseline: `bench-economy` seed 1, 6,000 ticks requested

This is the same siege that stops the clock in the economy baseline (4902 for
seed 1); it is a one-off to see whether more time brings the fray, not a
baseline row and not comparable to the table above.

`stopped_at` 4902, `ticks` 4902, `staff_ticks` 73,530.

| measure | 3000 ticks (baseline row) | 4902 ticks (diagnostic) |
|---|---|---|
| on_shift_share | 0.9517 | 0.9402 |
| morale_mean / min | 2.854 / -11.44 | 3.728 / -11.44 |
| sulking_share | 0.0246 | 0.0257 |
| need_strain_mean | -0.1949 | -0.3863 |
| coherence mean / min / critical_share | 77.18 / 19.70 / 0.0007 | 70.63 / 19.70 / 0.0014 |
| slack mean / min / critical_share | 90.60 / 60.39 / 0 | 85.72 / 31.19 / 0 |
| rung none / sulking | 0.9571 / 0.0429 | 0.9518 / 0.0482 |
| downed_tools / lashing_out | 0 / 0 | 0 / 0 |
| tantrums / frays / first_fray | 0 / 0 / none | 0 / 0 / none |

Coherence mean falls and slack's minimum falls to 31 over the extra 1,900
ticks, but no one reaches a fray or a heavier rung before the siege.

## What moved and what did not

- **Moved, `bench-economy`:** coherence just reaches critical; `sulking` is
  entered on every seed; morale has a long negative tail (min -10 to -17 on
  one program) while the mean is positive.
- **Did not move, `bench-economy`:** frays, tantrums, `downed_tools`,
  `lashing_out`, slack criticality, `stopped_at`.
- **The amenity is the difference between the templates:** the same 15
  programs and the same orders give 3.1% against 22.4% sulking, mean morale
  +2.4 against -4.8, coherence critical for 0.06% against 5.2% of
  staff-ticks, 0 frays against 3.6. `chains` has no seeds without a fray.
- **Slack is never critical on either template** (min 39-66); every need
  result above is coherence. A knob on `slack` would have nothing to move in
  3,000 ticks.
- **No tantrum anywhere.** Neither template produced one, so
  `staff.tantrums_per_1000` is zero on every row.
- **`on_shift_share` on `chains`:** the first version of this note read 1.0 on
  every seed because it asked the examine line's errand label, and `chains`
  has no amenity, so nobody ever had an errand. It now asks `Game::on_shift`,
  the scheduler's own test, and the numbers above are from that. Seed 1 reads
  0.8904, which is exactly one minus its `downed_tools` share (0.1096): a
  program at downed tools is off the line. Seeds 2 and 5 also reach
  `downed_tools` (3.6% and 5.0%) and still read 1.0; the sim keeps a program
  that is carrying a load on shift whatever its mood, which is the likely
  reason, but that was not checked here. On `bench-economy` the re-measure
  moved the figures by at most 0.0003 (it has an amenity, so the label and
  the test agree).

## What it does not say

- **Two templates, five seeds each, one set of orders.** `chains` is the
  stalled fixture from the earlier notes and not a base a player builds; its
  frays follow from having no amenity.
- **3,000 ticks, chosen to sit under the siege** (3789-4902 on `bench-economy`).
  Anything slower than the first fray at about tick 1900-2300 on `chains`
  cannot be seen on `bench-economy` here.
- **The diagnostic is one seed.** It says nothing about spread.
- **The report is per staff-tick over `base_staff()`;** an individual program
  is not tracked, so `morale_min` and `min` are minima over any program at any
  tick, and `end_morale` (in the raw reports) is the only per-program view.
- **The 31 critical staff-ticks on seed 1 are from the share (0.00069 x
  45,000);** the count is derived, not printed.
- **Rung shares, `sulking_share` and `need_strain_mean` are not target
  ranges.** No range is proposed here; that is the next decision.

## Open questions

- Whether a base that never frays in the siege-free window is the question the
  tune should ask, or whether the party-outside-base-space siege fix has to
  come first.
- Why `chains` reaches `downed_tools` on only three of five seeds while every
  seed frays.

## 6,000 ticks, sieges off (the staff objective's baseline)

Re-baseline for `dev-tuning/staff.ron`, taken before any knob moved. New
measurement; replaces the 4,902-tick diagnostic above as the long-run view.

```sh
cargo build --release --bin bench
for s in 1 2 3 4 5; do
  ./target/release/bench run --template bench-economy --ticks 6000 --seed $s --no-sieges \
    --order patch_routine:9999 --order bytecode_block:9999 --order ice_breaker:9999 --out s$s.ron
done
```

Branch `staff-bench` at 54eb6049, release. `stopped_at` is None on all five,
so every run played the full 6,000 ticks (`staff_ticks` 90,000). The first
five rows are the staff objective's targets; `frays_per_1000` is `frays` / 6.

| measure | seed 1 | 2 | 3 | 4 | 5 | mean |
|---|---|---|---|---|---|---|
| sulking_share | 0.0282 | 0.0338 | 0.0330 | 0.0403 | 0.0220 | 0.0315 |
| need_critical_share.coherence | 0.00244 | 0.00109 | 0.00119 | 0.00204 | 0.00116 | 0.00158 |
| need_mean.slack | 82.88 | 84.15 | 82.86 | 81.90 | 79.49 | 82.26 |
| frays_per_1000 | 0 | 0 | 0 | 0 | 0 | 0 |
| rung_share.downed_tools | 0 | 0 | 0 | 0 | 0 | 0 |
| on_shift_share | 0.9415 | 0.9257 | 0.9391 | 0.9257 | 0.9629 | 0.9390 |
| morale_mean | 4.888 | 4.325 | 3.485 | 5.395 | 4.801 | 4.579 |
| morale_min | -15.36 | -15.38 | -16.40 | -14.32 | -14.42 | -15.18 |
| tantrums | 0 | 0 | 0 | 0 | 0 | 0 |
| first_fray | none | none | none | none | none | |
| stopped_at | none | none | none | none | none | |
| coherence mean / min | 67.64 / 17.70 | 68.99 / 19.68 | 71.82 / 19.71 | 68.33 / 18.59 | 71.12 / 19.51 | 69.58 / 19.04 |
| slack min | 24.87 | 27.03 | 24.43 | 24.83 | 24.91 | 25.21 |
| slack critical_share | 0.0001 | 0 | 0.00054 | 0.00012 | 0.00008 | 0.00017 |
| labour mean_unworked_total | 0 | 0 | 0 | 0 | 0 | 0 |

The economy orders still keep staff on shift for 6,000 ticks: 92.6-96.3% of
staff-ticks on shift (mean 93.9%, a little under the 94.7% at 3,000 ticks),
and no labour goes unworked on any seed. Against the plan's targets, shipped
values already meet the slack, fray and `downed_tools` ranges and sit below
the `sulking_share` (0.05-0.15) and coherence-critical (0.01-0.05) ranges:
the shipped base is quieter than "occasional trouble". Slack now reaches
critical (min 24-27 against 25) for a handful of staff-ticks, which the 3,000
tick baseline did not show.
