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

A second run with a fixed rule then **answered** the pins question: they are
justified. See "The answer" below — and note that the reason recorded in the
2026-08-09 report for pinning `target_bracing` turns out to have been
measuring nothing at all.

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

## Why the first run could not answer it

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

The run needs a brace trigger that varies **independently of health**. Note
that the obvious candidate — a *designated* member bracing every round —
trades one confound for another: bracing would then track slot position,
which carries its own weight in `slot_aggro_weight`. Rotating by round
decorrelates from both.

Rather than write that down and hope, `analysis/policy_report.py` now checks
every run for collinear observables and prints the warning **above the first
table**. It caught its own first version, which pooled the sweep and
reported all clear because five of seven configs never brace and their
constant rows swamped the two that do — so the guard is grouped per run, and
`test_pooling_runs_that_never_braced_hides_the_confound` holds it there.

**The generalisable lesson, which is the reason this file exists:** a
treatment variable must not be a function of the model's strongest feature.
If it is, every number comes out clean and means nothing.

## The answer

`PartyPlan::BraceInRotation` braces one slot per round by round number,
whatever anyone's health — an instrument, not a model of play. The guard
reports it as the **only unconfounded run** in the sweep, which is what makes
the rest of this section readable at all.

```sh
./target/release/train --scenarios dev-training --iters 30 --pop 40 \
    --reps 200 --seed 1 --party-plan rotate \
    --log-dir dev-logs/policy-sweep --label unpinned-rotate
```

Unpinned, the three targeting features across all three harnesses:

| feature | All-Attack | brace-when-hurt | **brace-in-rotation** |
|---|---|---|---|
| `target_bracing` | −1.93 | −0.11 | **−1.64** |
| `target_def_rel` | −2.78 | −3.45 | −2.88 |
| `target_is_player` | +6.50 | +7.39 | +6.29 |

And the behaviour, which does not depend on reading coefficients: the
trained policy puts **52.3%** of its swings on bracing targets against the
uniform baseline's **64.7%**. It is pushing *against* `DEFEND_AGGRO_WEIGHT`'s
taunt, not merely ignoring it.

**So the pins are justified.** Given a party that braces for reasons the
policy cannot otherwise read, a free search learns to avoid the brace, which
would delete Defend. `target_def_rel`'s −3.45 in the confounded run was
mostly that confound; at −2.88 it sits where All-Attack had it.

### The 2026-08-09 justification was unidentifiable

Worth stating plainly, because it changes *why* the pin is right.

Under `PartyPlan::AllAttack` **no party member ever braces**, so
`target_bracing` is zero for every candidate in every fight. It contributes
nothing to any score, so fitness is completely indifferent to its weight and
CEM leaves it wherever the Gaussian happened to drift. The 2026-08-09 report
read run 1's `target_bracing: −3.34` as "training learned to dodge the
brace". Training **could not have learned that** — it never saw a brace.

The pin was still the right call, and arguably more urgent than the report
knew: an *unconstrained* weight is worse than a badly-learned one, because
nothing bounds it, and it is then applied in a live game where players do
brace. What is new here is that the behaviour is now confirmed rather than
inferred — at −1.64, measured where the feature is identifiable, a free
policy really does avoid the brace.

No shipped change follows. The pins stay, `DEFEND_AGGRO_WEIGHT` stays at 7.

## What it does not say

- **The −0.11 is not a result.** It came from the confounded run; the
  identifiable figure is −1.64. Do not cite the former.
- **One seed.** The sign is large and matches All-Attack's drift direction
  and the behavioural read, so it is unlikely to be noise — but a second
  seed is what would settle it, and none was run.
- **Nothing about companion Specials**, which no party plan exercises.
- **Nothing about how bracing feels.** The rule is not how a person plays;
  it is a lever for moving one variable.
