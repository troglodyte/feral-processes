# Tactical enemy personalities — decision core

Hostiles on the tactical battle map play one fixed way: fire the first
ready routine, otherwise swing at the lowest-HP body in reach, and walk to
a cell scored by global weights. Every species fights the same. This
change gives hostiles a scored decision core driven by per-species
personality profiles (data in `assets/behaviours/`), with weights tuned
offline so each personality lands the same overall difficulty as today.

This is sub-project 1 of 4 of "smarter enemies in tactical combat":

1. **Decision core + personality profiles + training harness** (this spec).
2. Use their kit, and read the player: species abilities installed at
   fight start; scoring stops wasting effects (stun on the stunned, heal
   on the healthy).
3. Teamwork: heal/buff wounded packmates, guard the weak, focus fire.
4. Self-preservation: Defend, fall back to cover, keep distance. Bodies
   never leave the map.

Each later sub-project gets its own spec. Group (non-tactical) battles and
their learned policy (`assets/policies/enemy_battle.ron`) are untouched.

## Decisions

| Question | Decision |
|---|---|
| Mode | Tactical map only. |
| Difficulty | More interesting, same overall difficulty; personalities differ per species. |
| Decision model | Utility scoring: hand-designed features x profile weights, deterministic argmax. |
| Tuning | Profile pins the character; CEM training tunes the free weights to hit a difficulty target (not to maximise wins). |
| Training opponent | A new arena `AutoPlay` party driven by the existing `[A]` logic. `SwingOnly` stays the arena default. |
| Rejected | Priority-rule lists (brittle as behaviours grow); a fully learned policy (opaque, fights hand-authored character); training to maximise wins then pulling difficulty back with a global knob (two tuning steps). |
| Fleeing (sub-project 4) | Never leaves the map — no XP/loot question. |
| Kit (sub-project 2) | Species abilities installed at tactical fight start, deterministically, same unlock rule companions use. |
| Save format | Unchanged. The profile is looked up via the body's species at decision time. |

## Engine — `crates/engine`

### Candidates and scoring (`tactical/policy.rs`, new)

On a hostile's turn the hostile branch of `tactical_intent` (ai.rs) asks
the policy module for an `Intent`. Candidates:

- one `Swing` per target in reach (props included, as `best_swing` does);
- each ready routine (`wild_routine_ready` / `ready_routines`), aimed by
  `best_aim` as today. A routine with no aim is not a candidate except
  where today's `hostile_routine_worth_choosing` admits it, so the pinned
  "hostile wastes its turn without an aim" behaviour is preserved under
  `default`.

`score = Σ weight[f] · feature[f]`. The chosen candidate is
`policy::argmax_scored` over candidates in a sorted order (swings in board
reading order, as `best_swing` ties today; routines in `Routines` order).
No RNG is drawn: `tactical_forecast`, which calls `tactical_intent`, keeps
predicting the real turn, and the "one draw per AI turn" seam is
unchanged — the only draw is still the cell softmax.

Swing candidates are the bodies in reach *after* the walk, exactly the
set `best_swing` sees today; the profile replaces only `best_swing`'s
lowest-HP rule. With nothing in reach the intent is today's
`Swing{range}` and the walk is scored by the profile's cell weights.
Choosing a far target to close on is deliberately not a candidate here:
it would change today's walk and break the `default` gate. Decide-then-walk
order (seams-tactical) is unchanged.

Sub-project 1 features (`TacticalFeature` enum, named for the data file):

| Feature | Meaning |
|---|---|
| `target_hp_frac` | target Integrity / max |
| `est_damage_frac` | `battle::expected_damage` / target Integrity (swings) |
| `would_kill` | 1 if expected damage ≥ target Integrity |
| `target_is_player` | 1 for the player body |
| `target_stunned` | `is_stunned(target)` |
| `target_distance` | Chebyshev distance to target, normalised by board size |
| `is_routine` | 1 for a routine candidate |
| `routine_net_recipients` | `best_aim`'s net recipient count |
| `self_hp_frac` | actor Integrity / max |
| `self_x_target_distance` | `(1 - self_hp_frac) · target_distance` — lets a wounded body prefer far targets |

Later sub-projects append features; a profile file that does not name one
reads it as 0.

### Cell weights per profile

`TACTICAL_AI_REACH_SCORE`, `_CLOSING_WEIGHT`, `_COVER_WEIGHT`,
`_COVER_TIEBREAK`, `_CROWDING_WEIGHT`, `_REACTION_WEIGHT` become the
defaults of a `CellWeights` struct (constants stay in `tuning.rs`).
`cell_merit`, `cell_score`, `scored_cells` and `walk_risk` take
`&CellWeights`; the hostile path passes its profile's, the party path and
existing unit tests pass `CellWeights::default()`.
`TACTICAL_AI_TEMPERATURE` is not in the profile — `decision_temperature`
stays the one door.

### Profiles (`behaviours.rs`, new `BehaviourDb`)

Loaded from `assets/behaviours/*.ron` via the `*Db::load_dir` pattern
into the same place other dbs live on `Game`. A malformed file is skipped
with a logged warning. `default` must exist (startup error otherwise, the
`priority_boost` rule). An unknown feature name logs a warning and is
ignored.

```ron
(
    id: "skirmisher",
    name: "Skirmisher",
    description: "Keeps its distance, hugs cover, picks at the weakest.",
    pinned: { "target_distance": 1.5 },
    signs: { "target_hp_frac": Negative, "would_kill": Positive },
    weights: { "target_hp_frac": -3.2, "would_kill": 4.1 },
    cells: (reach: 40.0, closing: 0.6, cover: 9.0, crowding: 1.5, reaction: 2.0),
)
```

- `pinned`: authored character, never written by training.
- `signs`: directions training may not reverse.
- `weights`: free weights, written by the trainer.
- `cells`: `#[serde(default)]`, defaults to `CellWeights::default()`.
  Cell weights are pinned (authored), not trained, in this sub-project.

A feature present in both `pinned` and `weights` is a load warning;
`pinned` wins.

### Species field

`SpeciesDef.behaviour: Option<String>`, `#[serde(default)]`, next to
`taunts`. Absent → `default`. A name with no profile → logged warning,
`default`. Chosen per species, never derived from class (seams-species:
class has one derivation).

### `default` reproduces today

`default.ron` weights make a ready routine outscore any swing
(`is_routine` large) and the lowest-HP target win (`target_hp_frac`
negative, nothing else); cells equal today's constants. Gate: every
existing tactical test passes **unedited** under `default`; the only edit
allowed in existing tests is passing `CellWeights::default()` to the free
functions.

### Personalities and mapping

| Profile | Character | Species |
|---|---|---|
| `default` | today's behaviour | crawler, proxy, virus, sub_process, sentinel, overseer, wintermute |
| `brute` | closes fast, ignores cover, hits the nearest | rootkit, botnet, worm, scrapper |
| `hunter` | focuses the wounded, takes kills, prefers soft targets over the player | cipher, zero_day, trojan, polymorph |
| `skirmisher` | keeps distance, hugs cover, avoids reaction swings | glitch, sprite |
| `cautious` | weighs its own HP; when hurt, prefers far targets, cover, no crowding | drone, construct |

Support species (crawler, proxy, virus, sub_process) and the guardian
(sentinel) stay `default` until sub-projects 2–3 give them kit-aware
`support`/`guardian` profiles. Bosses stay `default` (a boss profile is
out of scope).

## Launcher — `crates/launcher`

### Arena `AutoPlay` party

Tactical scenarios gain `party: AutoPlay | SwingOnly` (`#[serde(default)]`
= `SwingOnly`). `AutoPlay` drives the party through `tactical_drive_turn`
with `PartyTurns::Invoke` (routines, Power cells, own-side blast veto).
The seams-tactical rule "a headless rep drives both sides through
`tactical_drive_turn`, and the party swings without invoking" is updated
in all three places (graph, `seams` skill, rules file) to name the mode.

Tactical scenarios also gain `behaviour: Option<String>`, overriding every
hostile's profile so a profile is evaluated on fixed fights.

### Benchmark set

`dev-training/tactical/*.ron`, 6–8 fixed fights spanning level, group
size, board and approach, all `party: AutoPlay`, with routines installed
via the scenario's routines field where a fight needs them. Documented in
`dev-training/README.md`.

### Baseline and training

1. Baseline: `default` on the set, ≥200 reps per scenario, recording enemy
   win rate, mean player HP lost (fraction), mean rounds. Written to
   `docs/measurements/<run date>-tactical-personality-baseline.md`.
2. `train --tactical --profile <id> --scenarios dev-training/tactical`:
   CEM over the profile's free weights only; pinned fixed; signed weights
   clamped to their side. Objective (minimised), summed over scenarios:
   `(hp_lost - base)² · 1.0 + (enemy_win - base)² · 0.5 + (rounds_rel - 1)² · 0.1`.
   Writes the `weights:` block back into the profile file (the file is
   re-serialised whole; comments in profile files are not preserved, said
   in the README).
3. Each trained profile gets a measurement note showing player HP lost and
   enemy win rate within ±10% (relative) of baseline per the whole set.
   Baseline and trained runs from one build (arena numbers compare within
   one build only).

`train`'s group-model path is unchanged; the CEM loop is shared, not
copied.

## Assets

- `assets/behaviours/` with `README.md` (schema, features table, the
  pinned/signs/weights split, the training command) and five profiles.
- `behaviour:` added to the mapped species files; `assets/species/README.md`
  documents the field.

## Testing

- Refactor gate: existing tactical tests unedited under `default` (see
  above).
- One behaviour test per personality on a hand-built board, each
  mutation-checked (zero the profile's distinguishing weight → fails):
  - brute walks past cover to the nearest body;
  - skirmisher picks the covered cell at range;
  - hunter swings at the wounded companion over the healthy player;
  - cautious at low HP picks a cell with no adjacent threat.
- Purity: for each profile, `tactical_forecast`'s action equals the turn
  taken; same seed + profile replays the same fight.
- Data: malformed profile skipped with a warning; unknown feature ignored
  with a warning; unknown species `behaviour` falls back with a warning;
  census in `tests/assets.rs`: `default.ron` exists, every shipped species
  names an existing profile, every profile's feature names are known.
- Arena: `AutoPlay` party invokes a ready routine (a rep with a party
  routine fires it); `SwingOnly` default unchanged.

Gates: `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
`balance_sim` unchanged (it models no tactical choice — a moved curve
means something leaked), measurement notes inside the band.

## Release

Engine, launcher and assets; schema change (`SpeciesDef.behaviour`, new
asset kind); no save-format change → compatible, so a patch bump on `0.x` (`CHANGELOG.md` preamble). CHANGELOG entry and
schema READMEs in the same change.

## Out of scope

Choosing an out-of-reach target to close on, kit installation, effect-redundancy features, ally features, Defend /
fall-back intents (sub-projects 2–4); training cell weights; boss
profiles; group-battle AI; party AI changes.
