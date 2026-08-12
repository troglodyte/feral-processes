# Policies

One file lives here: `enemy_battle.ron`, the weights a wild program uses to
pick which move it swings and who it swings at.

**The file is optional.** With it absent the game plays exactly as it did
before policies existed — a uniform roll over the moves that reach, crossed
with the slot-weighted target roll in `battle::slot_aggro_weight`. That is
not a degraded mode; it is the baseline the weights are measured against,
and deleting the file is a supported way to play.

Weights are trained offline, not learned during play. See
`crates/launcher/src/bin/train.rs`.

## File shape

```ron
(
    features: [
        ("target_hp_frac", -1.84),
        ("would_kill",      2.31),
    ],
)
```

`features` is a list of `(name, weight)` pairs. Rules:

- **Keyed by name, not position.** Order does not matter, and a file naming
  three features is complete — the other sixteen are zero.
- **An unknown name is ignored with a logged warning.** The rest of the file
  still loads. That is what lets a policy written against a later build with
  more features degrade instead of scoring garbage.
- **A non-finite weight (`NaN`, `inf`) rejects the whole file** with a
  warning, and the game falls back to the baseline. It is refused once at
  load rather than reaching `exp` every round of every fight.
- **Malformed RON is skipped with a warning**, never a panic — the same rule
  every other asset directory follows.
- **An all-zero file is exactly the baseline.** The score is
  `ln(slot_aggro_weight(..)) + w·features`, so zero weights leave the aggro
  prior alone and reproduce today's odds precisely. Weights *multiply*
  today's targeting rather than replacing it.

## Scoring

Every candidate `(move, target)` pair a program could act on is scored, and
one is sampled from a softmax over those scores at
`tuning::ENEMY_POLICY_TEMPERATURE`. A temperature of 0 is "always the
best-scoring pair"; a large temperature returns the uniform baseline.
Everything is drawn through the run's seeded RNG, so a fight replays
identically from the same seed.

## The features

Each is normalised to roughly `[0, 1]`. A positive weight makes candidates
scoring high on that feature *more* likely.

| Name | Meaning |
|---|---|
| `move_power_rel` | This move's power over the biggest power in this species' own moveset. Relative, so it means the same thing for a modded roster. |
| `move_ranged` | 1 if the move reaches past the front line. |
| `move_has_effect` | 1 if the move carries a status effect at all. |
| `move_effect_stun` | 1 if that effect is `Stun`. |
| `move_effect_bleed` | 1 if that effect is `Bleed`. |
| `move_effect_chance` | The effect's own `chance` from the species file, 0 if there is no effect. |
| `target_hp_frac` | The target's current HP over its maximum. Negative weight = finish the wounded. |
| `target_is_player` | 1 if the target is the player rather than a companion. |
| `target_def_rel` | The target's effective DEF against this attacker's ATK, squashed into `[0, 1]`. High = hard to hurt. |
| `target_stunned` | 1 if the target is already stunned. |
| `target_bleeding` | 1 if the target is already bleeding. |
| `target_bracing` | 1 if the target is holding Defend. |
| `est_damage_frac` | The damage this move would actually do, over the target's remaining HP, capped at 1. |
| `would_kill` | 1 if that damage would drop the target. |
| `self_hp_frac` | The *attacker's* own HP fraction, so a policy may act differently when hurt. |
| `self_front_group` | 1 if the attacker's group is close enough to swing rather than having to shoot. |
| `effect_x_target_healthy` | `move_has_effect × target_hp_frac` — is a condition worth spending on someone who will be around to suffer it. |
| `stun_x_not_stunned` | `move_effect_stun × (1 − target_stunned)` — a stun on an already-stunned target is wasted. |
| `bleed_x_not_bleeding` | `move_effect_bleed × (1 − target_bleeding)`. |

The three interaction terms are how a linear model buys the only
nonlinearity that matters here. There is no bias term: a constant added to
every candidate cancels under the softmax.

## The shipped file, read out loud

`enemy_battle.ron` was trained on 2026-08-09 — see
`docs/superpowers/reports/2026-08-09-enemy-policy-training.md`. Its largest
weights say:

| Weight | Meaning |
|---|---|
| `target_hp_frac` −10.86 | Finish the wounded one. |
| `est_damage_frac` +10.10 | Hit whoever this move hurts proportionally most. |
| `effect_x_target_healthy` −5.51 | Do not spend a condition on someone healthy. |
| `move_power_rel` −5.00 | Read together with `est_damage_frac` — the two are correlated and the model split them, so neither means much alone. |
| `would_kill` +4.23 | Take a kill when it is there. |

**Three features are deliberately zero**, and that is not the trainer
running out of things to say — it is a design boundary:

- `target_is_player`
- `target_bracing`
- `target_def_rel`

Left free, the policy learns to kill the player and ignore everyone else,
and to walk past whoever braced. That is optimal play, and it deletes soft
ranks, party positioning and Defend in one go. The trainer's `--pin` flag is
what holds them at zero; the aggro table in `tuning.rs` owns the question of
*who* gets hit, and the policy may not reopen it.

A retrained file that gives any of those three a non-zero weight will fail
`bracing_still_draws_more_fire_under_the_shipped_weights` rather than
shipping quietly. If you are writing weights by hand, that test is the one
to run.

## The weights survive a rebalance; the training data may not

Every scale-sensitive feature is a **ratio**, by construction —
`target_def_rel` is DEF over the attacker's ATK clamped to [0, 2],
`est_damage_frac` is the real `compute_damage` result over the target's
current HP, and the HP terms are fractions. So a change to how big stats get
does not invalidate a trained file: double every number in the game and
every feature value is unchanged. **A rebalance is not a reason to retrain.**

What a rebalance *can* move is the **distribution** over that feature space,
and v0.8.1 moved it in a way worth knowing about. Under the geometric zone
and depth curves a deep fight drove those ratios into their clamps —
`target_def_rel` pinned, `est_damage_frac` and `would_kill` pinned, every
candidate target scoring alike — so the policy had no discriminating signal
in exactly the fights it most needed one, and gradient there was flat. With
linear scaling those features sit in their informative middle range again.

Two practical consequences:

- The shipped `enemy_battle.ron` is still **valid** and still passes its
  census. It is simply fit to a sample that over-represents saturated
  states. A retrain would probably find better weights, and is worth doing
  before reading any conclusion about deep-fight behaviour off the current
  file.
- `dev-logs/policy-sweep/*.jsonl` was recorded before the change and is
  **off-distribution** as a comparison set. Treat those runs the way
  `CLAUDE.md` says to treat arena numbers generally: comparable within one
  build, not across a rebalance.

And the standing caveat, which no retrain changes: individual weights are
not identifiable. A three-seed run put `move_power_rel` at 2.14, 0.93 and
7.17 at equal fitness. Read the behaviour the file produces, never one
coefficient.
