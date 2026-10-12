# Behaviours

One `.ron` file per tactical personality. A species names one with its
`behaviour` field (`assets/species/README.md`); the profile decides how that
species' hostiles weigh their choices on a tactical battle map. Group
battles are untouched.

**`default.ron` must exist.** Every species that names no profile plays as
it, and a missing one is a startup error. A malformed file is skipped with a
logged warning. A species naming a profile that does not exist plays as
`default` and logs a warning.

```ron
(
    id: "skirmisher",
    name: "Skirmisher",
    description: "Keeps its distance, hugs cover, picks at the weakest.",
    pinned: { "target_distance": 1.5 },
    signs: { "target_hp_frac": Negative },
    weights: { "target_hp_frac": -3.2 },
    cells: (reach: 40.0, closing: 0.6, cover: 9.0, crowding: 1.5, reaction: 2.0),
)
```

- `id`, `name`, `description`: required.
- `pinned`: feature name to weight. The authored character; the trainer
  never writes these. A feature in both `pinned` and `weights` logs a
  warning and `pinned` wins.
- `signs`: feature name to `Positive` or `Negative`. A direction the trainer
  may not reverse.
- `weights`: feature name to weight. The free weights, rewritten by the
  trainer. A feature a profile does not name reads as 0.
- `cells`: how the profile walks. Any of `reach`, `closing`, `cover`,
  `cover_tiebreak`, `crowding`, `reaction`; an omitted one takes the global
  tuning value (`tuning.rs`, `TACTICAL_AI_*`). Authored, not trained.

All four maps are optional (`#[serde(default)]`).

The trainer re-serialises the whole file, so **comments in a profile are
not preserved** by training.

## Features

A hostile scores each candidate (a swing at each body it fights, each ready
routine) as the sum of weight times feature, and takes the highest; ties go
to the first in reading order, and a swing candidate precedes a routine. An
unknown feature name logs a warning at load and is ignored.

| Feature | Meaning |
|---|---|
| `target_hp_frac` | target Integrity over its max |
| `est_damage_frac` | expected damage of a swing over the target's Integrity (cover not read) |
| `would_kill` | 1 if expected damage reaches the target's Integrity |
| `target_is_player` | 1 for the player's body |
| `target_stunned` | 1 if the target skips its turn |
| `target_distance` | gap to the target over the board's side |
| `is_routine` | 1 for a routine candidate |
| `routine_net_recipients` | bodies the routine's best aim helps or harms, net |
| `self_hp_frac` | the actor's Integrity over its max |
| `self_x_target_distance` | `(1 - self_hp_frac) * target_distance` |
| `target_hp` | target's raw Integrity (not a fraction; weigh it with a small magnitude, or sign it Negative and read it as an argmax only — `default` does) |

`default` weights `is_routine` high enough to beat any swing and
`target_hp` negative: its first ready routine, otherwise the body with the
fewest Integrity points (not the lowest fraction, which is what the
personality profiles use).

## Shipped profiles

| Profile | Character | Species |
|---|---|---|
| `default` | Its first ready routine, otherwise the body with the fewest Integrity in reach. | crawler, proxy, virus, sub_process, sentinel, overseer, wintermute |
| `brute` | Closes fast, ignores cover and reactions, hits the nearest. | rootkit, botnet, worm, scrapper |
| `hunter` | Focuses the wounded, takes kills, prefers soft targets over the player. | cipher, zero_day, trojan, polymorph |
| `skirmisher` | Keeps its distance, hugs cover, avoids reaction swings. | glitch, sprite |
| `cautious` | Weighs its own Integrity: when hurt, prefers far targets, cover and room. | drone, construct |
