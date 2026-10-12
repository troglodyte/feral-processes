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
