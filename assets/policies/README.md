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
