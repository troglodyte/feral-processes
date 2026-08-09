# Learned enemy battle policy — training report

**Date:** 2026-08-09
**Spec:** `docs/superpowers/specs/2026-08-09-enemy-battle-policy-design.md`
**Plan:** `docs/superpowers/plans/2026-08-09-enemy-battle-policy.md`

Three training runs were made. **The first two are not what shipped**, and
why they were rejected is the more useful half of this report.

The short version: the policy works, it beats the baseline by 29 percentage
points, and on move choice it found nothing a designer did not already know.
What it *did* find is that two of the game's mechanics — Defend, and status
effects on wild moves — were only ever working because the enemy chose at
random. That is a finding about the game, not about the optimiser.

## Setup

Cross-entropy method, 30 generations of 40 candidates, each candidate scored
over eight `dev-training/` scenarios at 200 reps: **1.92M arena fights per
run**, about seven minutes on 16 cores. Fitness is enemy win rate plus
`0.1 * (1 - mean player HP fraction)` as a tiebreak — three of the eight
scenarios are 100% player wins at baseline, where a pure win/loss signal has
no gradient at all.

`dev-arenas/`'s three scenarios were held back and never trained on.

Throughput, measured before committing to a budget: 730 fights/sec on
`opening-fight`, 551 on `full-group`, 368 on the heaviest training scenario.
The plan expected `arena::stage` re-parsing ~100 RON files per fight to
dominate. It does not.

## Run 1 — unconstrained, rejected

Enemy win rate **0.324 → 0.663**. It worked, and then it ate the game.

| Weight | Plain English |
|---|---|
| `target_is_player` **+7.42** | Kill the player. Nothing else matters. |
| `move_power_rel` **+6.45** | Always use your biggest move. |
| `target_hp_frac` **−5.39** | Finish the wounded one. |
| `would_kill` **+4.10** | Take a kill when it is there. |
| `move_effect_stun` **−3.95** | Never spend a swing on a stun. |
| `target_def_rel` **−3.87** | Avoid whoever is hard to hurt. |
| `target_bracing` **−3.34** | **Especially avoid whoever braced.** |

Four of those — biggest move, finish the wounded, take the kill, prefer the
higher-damage pairing — are exactly what a person would have hand-written.
Worth saying plainly: **on move choice the policy discovered nothing new.**

The two that matter are the ones a designer would not have shipped.
`target_is_player: +7.42` means companions stop being targets: measured, a
companion took **0.2%** of swings and **0.0%** while bracing. On
`geared-vs-boss`, companions-downed went 1.90 → **0.00** while the player's
remaining HP went 32% → 3%. The party stood untouched and the player was
executed. `target_bracing: −3.34` swamps the aggro prior's `ln(7/3) = +0.85`,
so bracing *reduced* incoming fire.

Three suite failures said so independently, including
`bracing_..._under_the_shipped_weights` — the Defend census the spec added
for exactly this, catching exactly this on its first real outing.

## Run 2 — `target_is_player` and `target_bracing` pinned

Enemy win rate **0.324 → 0.614**, so the constraint cost only five points.
Companion targeting came back. Defend did not:

> bracing drew **0.10** of the fire against **0.46** not bracing

The policy had simply routed around the pin. With `target_bracing` unable to
say "avoid the brace", `target_def_rel: −7.32` said it instead — because
what Defend grants is `+6 DEF`, and "avoid high-DEF targets" is the same
sentence in different words.

## Run 3 — `target_def_rel` pinned as well, **shipped**

Enemy win rate **0.324 → 0.611**. Pinning the third feature cost 0.3 points.

| Weight | Plain English |
|---|---|
| `target_hp_frac` **−10.86** | Finish the wounded one. |
| `est_damage_frac` **+10.10** | Hit whoever this move hurts proportionally most. |
| `effect_x_target_healthy` **−5.51** | Do not spend a condition on someone healthy. |
| `move_power_rel` **−5.00** | (see below) |
| `would_kill` **+4.23** | Take a kill when it is there. |
| `move_effect_stun` **−2.81** | Do not spend a swing on a stun. |
| `self_hp_frac` **−2.67** | Press harder when hurt. |
| `target_is_player`, `target_bracing`, `target_def_rel` | **0.00**, pinned. |

`move_power_rel: −5.00` alongside `est_damage_frac: +10.10` looks
contradictory and is not: the two are strongly correlated, and the model has
split them into "how much damage does this actually do to *this* target"
(kept) minus "how big is the move in the abstract" (discounted). A linear
model over correlated features will do this, and the pair should be read
together rather than one at a time.

### Per-scenario, training set

| Scenario | Enemy win, baseline → trained | Player HP left |
|---|---|---|
| `01-opening-solo` | 0.00 → 0.00 | 0.78 → 0.77 |
| `02-early-pair` | 0.00 → 0.00 | 0.98 → 0.98 |
| `03-midgame-group` | 0.05 → **0.85** | 0.82 → 0.11 |
| `04-back-rank` | 0.75 → **1.00** | 0.25 → 0.00 |
| `05-status-heavy` | 0.01 → **0.70** | 0.72 → 0.12 |
| `06-geared-lategame` | 0.52 → 0.76 | 0.43 → 0.22 |
| `07-rolled-field` | 0.48 → 0.67 | 0.44 → 0.29 |
| `08-rolled-stack` | 0.80 → 0.92 | 0.19 → 0.08 |

The two scenarios that do not move are the two with no target choice to
make. Everything the policy is worth, it is worth against a party.

### Held-back set — the number that actually counts

Never trained on. Note `run_rep` plays All-Attack, so nobody braces and the
`DEFEND_AGGRO_WEIGHT` change below cannot touch these figures: this is the
policy alone.

| Scenario | Player win, baseline → shipped | Player HP left | Companions downed |
|---|---|---|---|
| `opening-fight` | 100% → 100% | 44% → **51%** | 0.00 → 0.00 |
| `full-group` | 100% → 100% | 99% → 99% | 0.08 → **1.72** |
| `geared-vs-boss` | 50% → **40%** | 32% → 34% | 1.90 → 2.65 |

Read honestly: on the held-back set this is a **10-point** swing on the one
scenario that was not already decided, plus companions taking a great deal
more fire. `opening-fight` got *easier* — the player finishes with more HP
than against the random baseline. In a 1v1 with two moves there is no
targeting to exploit, and the `move_power_rel`/`est_damage_frac` pair is
locally worse than a coin flip there. That is a real limitation of a single
global linear policy and it is not worth hiding: the trained enemy is
sharply better in a group fight and marginally worse one-on-one.

## The two design findings

**1. Defend was calibrated against an enemy that does not think.**

The aggro weights enter the score as `ln(weight)`, so Defend's old `+4` on a
base of `3` bought `+0.85`. The `est_damage_frac` term is worth about `−1.0`
against a bracing target — because *reducing incoming damage is what bracing
is*. Even with all three targeting features pinned, bracing still drew 0.40
of the fire against 0.44 exposed.

This is not fixable by pinning: any damage-aware policy has a reason to walk
past the tank. Nor by `ENEMY_POLICY_TEMPERATURE`, which divides the prior and
the learned term alike and so cannot flip the sign. The prior has to be big
enough to win. **`DEFEND_AGGRO_WEIGHT` is therefore raised from 4 to 7** —
bisected: 7 clears the census with margin, 6 flips the sign back by only
0.02, inside the noise. Bracing is now a stronger taunt than it was, and the
intended reading is that its old value was an artifact.

**2. Effect-carrying moves are priced not to be worth taking.**

The shipped policy picks one on roughly 1 turn in 400. Every shipped species
prices its effect move *below* its damage-only sibling — Worm's Replicate is
power 5 with Bleed against Burrow Strike's 8; Cipher's Encrypt is 6 with Stun
against Cross-Reference's 9 — and `WILD_ABILITY_CHANCE` then gates the effect
down to roughly 6–10% of swings. Skipping them is correct expected-damage
play. The condition variety in wild fights was a product of the enemy not
thinking.

**This one is left unfixed**, deliberately: repricing the roster or raising
`WILD_ABILITY_CHANCE` is a balance change to shipped content rather than to
this feature, and it wants a playtest.
`a_trained_policy_rarely_picks_an_effect_carrying_move` records the current
state so that a future reprice fails a test rather than passing unnoticed.

## What shipped

- `assets/policies/enemy_battle.ron` — run 3's weights.
- `DEFEND_AGGRO_WEIGHT` 4 → 7.
- `ENEMY_POLICY_TEMPERATURE` at 1.0, the trained distribution as-is. Raise it
  to dial the enemy back without retraining; 0 is argmax.

Deleting the weights file is still a supported way to play, and returns the
game exactly to its pre-policy behaviour.

## Known limits

- **Trained against All-Attack.** `arena::run_rep` plays the party's side as
  `[A]` every round, with no Defend and no companion Specials. The claim is
  "beats All-Attack by 29 points on the training set", and a party that
  braces and spends Specials is not what was measured. The irony is not lost:
  the one mechanic this whole report is about is the one the harness cannot
  exercise, which is why the Defend census is a unit test rather than an
  arena number.
- **`balance_sim` is blind to this.** RNG-free, models no abilities. Its
  curves were checked and did not move — the expected result, and evidence of
  nothing about the policy.
- **A linear model over correlated features is locally weird**, per
  `move_power_rel` above and the `opening-fight` regression.
- **A green suite is not evidence of play.** How this *feels* — whether
  fights read as smarter or merely harsher, and whether Defend at 7 is now
  overtuned — is unmeasurable headlessly and still wants
  `FERAL_DEV_ARENA=1 cargo run`, `[R]`.
