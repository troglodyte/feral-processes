# A party that braces

**Date:** 2026-08-10
**Instrument:** `train --party-plan brace`, then `analysis/`
**Follows:** [the pin sweep](2026-08-10-enemy-policy-pin-sweep.md), which
could not touch Defend at all.

## The claim

The arena can now exercise Defend, and two things fell out of the first run.
**Bracing is a net loss against a stupid enemy and a net gain against a
smart one.** And **the run could not answer the question it was built for**,
because the brace rule was collinear with the policy's strongest feature.

The pins question — whether `target_is_player`, `target_bracing` and
`target_def_rel` still need pinning once Defend costs the enemy something —
remains **open**.

## How to reproduce it

```sh
./target/release/train --scenarios dev-training --iters 30 --pop 40 \
    --reps 200 --seed 1 --party-plan brace \
    --log-dir dev-logs/policy-sweep --label unpinned-brace
```

`nice -n 19 ionice -c3` in front of it if anything else is using the box;
it saturates every core for ~8 minutes otherwise.

## Defend is worth more the smarter the enemy is

Same build, same assets, same seed — only the party plan differs.

| party plan | baseline enemy win | pinned-config trained | policy's edge |
|---|---|---|---|
| All-Attack | 0.263 | 0.565 | **+0.302** |
| BraceWhenHurt | 0.318 | 0.538 | **+0.220** |

Read both columns. Against the **uniform** enemy a bracing party does
*worse* — 0.263 → 0.318 — because a turn spent Defending costs more damage
than +6 DEF saves, and the rule braces without judgement. But the trained
policy's advantage over its own baseline **shrinks by 8 points**, so Defend
blunts a thinking opponent while being a straight loss against a random one.

That is a real design observation and it came free. It also means the
scripted party is **bad play**, which is a limitation of the harness rather
than a finding about Defend: the enemy is being trained against a party that
handicaps itself.

## Why the pins question is still open

`target_bracing` in the unpinned weights went **−1.93** (All-Attack) to
**−0.11** (bracing party), which reads like "the policy stopped avoiding the
brace". Two reasons not to believe it.

First, what moved to compensate: `target_def_rel` **−2.78 → −3.45** and
`target_is_player` **+6.50 → +7.39**. Defend grants +6 DEF, so with a party
that actually braces, DEF becomes the better signal and the flag goes
redundant — the same routing-around the 2026-08-09 report saw when it pinned
two features and watched a third absorb the job.

Second, and fatally: **the brace rule was a threshold on `target_hp_frac`.**
A member Defended exactly when it dropped under half health, so *is bracing*
and *is wounded* are the same variable (measured r = −0.78 to −0.81), and
the policy's largest weight by far is on being wounded. Bracing targets drew
24% of swings against the baseline's 12% — fully explained by wounded
targets drawing fire, with nothing left over to attribute to Defend.

No reading of this data can separate the two. The instrument was built and
then blunted by the rule chosen to drive it.

## The fix, and the guard

The run needs a brace trigger that varies **independently of health** — a
designated member bracing every round regardless, say. Less like real play,
but it was never realistic; its job is to make one variable move on its own.

Rather than write that down and hope, `analysis/policy_report.py` now checks
every run for collinear observables and prints the warning **above the first
table**. It caught its own first version, which pooled the sweep and
reported all clear because five of seven configs never brace and their
constant rows swamped the two that do — so the guard is grouped per run, and
`test_pooling_runs_that_never_braced_hides_the_confound` holds it there.

**The generalisable lesson, which is the reason this file exists:** a
treatment variable must not be a function of the model's strongest feature.
If it is, every number comes out clean and means nothing.

## What it does not say

- **Nothing about the pins.** See above. Do not cite the −0.11.
- **Nothing about companion Specials**, which no party plan exercises.
- **Nothing about how bracing feels.** The rule is not how a person plays;
  it is a lever for moving one variable.
