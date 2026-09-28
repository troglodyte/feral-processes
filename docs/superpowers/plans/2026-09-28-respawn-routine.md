# Respawn routine — plan

Spec: `docs/superpowers/specs/2026-09-28-respawn-routine-design.md`.
Branch: `respawn-routine` (spec committed). TDD, a commit per green step.
No save-format change (`Perk` appended at the end; `TacticalBattle` unsaved).

Three phases, one sonnet dispatch each. Each ends green on
`cargo test -p <crate(s) touched>` + `cargo clippy --workspace --all-targets`
+ `cargo fmt`. After phase 3: `cargo test --workspace` once, then
`cargo test -p feral-processes-engine balance_sim`. Final review: opus, whole
branch. Dispatches must forbid push. Read `.claude/rules/seams-*.md` for
tactical/battle before editing.

## Anchors (verified 2026-09-28; paths under `crates/engine/src` unless noted)

| What | Where |
|---|---|
| `Fallen {cell, footprint}`; misplaced `TacticalBattle` doc at 79-91 | `tactical/mod.rs:93` |
| `TacticalBattle::fall` (push mark, then `remove`) | `tactical/mod.rs:677` |
| `fall` callers: structure destroyed; `reap_tactical_dead` (body still has all components) | `tactical/turn.rs:593`, `:1779` |
| `reap_tactical_dead`: `Hostile` branch, then base-staff `else if body != player && !Party.contains` | `tactical/turn.rs:1751`, `:1789` |
| `AbilityEffect`; exhaustive: `tactical_only` 708, `affinity_kind` 742, `breaks_cloak` 801, `effect_label` 1657 | `abilities.rs:433` |
| Group `use_ability` `unreachable!` arms (Teleport precedent) | `game/combat_round.rs:1818` |
| `routine_tree::gets_node` | `routine_tree.rs:95` |
| `tactical_use_routine` (aim validation, pre-cost refusals; Summon room check at 1156) | `tactical/turn.rs:1079` |
| `run_tactical_routine` (charges, then branches; Summon branch 1345) | `tactical/turn.rs:1283` |
| `RoutineRefusal` (+ Display); `ability_unavailable` → `BattleMapOnly` first | `game/combat.rs:18`, `:1488` |
| `reach::shape_cells`; `deploy::nearest_free`; `insert_after_cursor` | `tactical/reach.rs:373`; `tactical/deploy.rs:111`; `tactical/mod.rs:467` |
| `dissolve_summons` (sets hp 0 on every live `Summoned`) | `game/combat_round.rs:1272` |
| `fork_programs` (`depth_mult = level_mult * SUMMON_STAT_MULT`; strips `Hostile`/`WanderAi`, inserts `Summoned`+`PowerReserve`) | `game/spawning.rs:535` |
| `spawn_wild_creature_pinned` + `SpawnPins { rarity, routines }` | `game/spawning.rs:313`, `:88` |
| `SUMMON_STAT_MULT` | `tuning.rs:5533` |
| `summon_rarity_window`; `emulation_fidelity_level` (query-fn precedent) | `perks.rs:332`, `:356` |
| `Perk` / `Perk::all() -> [Perk; 20]` / `one_level_is_worth_something` | `perks.rs:36`, `:161`, `:821` |
| Perk index asserts | `tests/perks.rs:391` |
| `Summoned` sweeps in teardown; `mark_nemeses` | `game/combat_teardown.rs:433,498`; `:555` |
| Tactical Decompile (inline in `run_tactical_routine`) | `tactical/turn.rs:1326` |
| `tactical_ai_actor`, `aimable_this_turn`, `tactical_sides_from`, `best_aim` (decoy count ~1386) | `tactical/ai.rs:411`, `:878`, `:977`, `:1338` |
| `ready_from_candidates` (filters `tactical_only`) | `game/combat_enemy.rs:107` |
| `family_is_discoverable` (carrier = `wild_weight > 0` or species kit; tactical-only not consulted) | `game/routines.rs:262` |
| Hunt-only count `34` | `tests/assets.rs:718` |
| `TacticalBody`; built in `body_view` | `tactical/view.rs:39`, `:513` |
| `glyph_color(body.color)` → glyph ink + sprite tint | `crates/gui/src/render/tactical.rs:630` |
| `palette` | `crates/gui/src/render/hud/palette.rs` |
| Arena `CharacterSpec.routine` | `arena/scenario.rs:228` |
| README family classification | `assets/abilities/README.md:713` |

## Decisions the spec left to the plan

- **Discovery needs no classifier change.** `family_is_discoverable` checks
  only carriers, so `wild_weight > 0` makes Respawn discoverable. Test it;
  fix the README line only if it claims tactical-only wins.
- **The fork-death bug is likely real.** Forks strip `Hostile` and aren't in
  `Party`, so a tactical fork that falls takes the base-staff branch: it logs
  "falls in the siege" and calls `bench_or_dissolve`. Reproduce it, then fix it
  with a `Summoned` guard ahead of that branch: fall, despawn, no log. That
  guard also covers party-side raised bodies.
- **Multiplier fn:** `perks::spawn_priority_stat_bonus(perks: Option<&Perks>)
  -> f32` = level × `SPAWN_PRIORITY_STAT_PER_LEVEL`. Forks use
  `SUMMON_STAT_MULT + bonus`; Respawn uses `REANIMATE_STAT_MULT + bonus` when
  the caster is on the party side and `REANIMATE_STAT_MULT` alone when
  hostile. New constants go in `tuning.rs` next to `SUMMON_STAT_MULT`.
- **Spawn:** `spawn_wild_creature_pinned(species, sentinel, 1.0, false,
  SpawnPins { rarity: Some(snapshot.rarity), routines: Some(vec![]) })`, then
  overwrite `Stats` from the snapshot (`max_hp`/`atk`/`mitigation` ×
  mult, `hp = max_hp`). A raised body rolls no wild routine.
- **Per-side dissolve:** `dissolve_summons` becomes `With<Summoned>,
  Without<Respawned>`. A new `dissolve_respawned(hostile: bool)` zeroes hp
  on the living `Respawned` bodies whose `Hostile` presence matches.
  The next reap removes them as usual.
- **Refusal:** `RoutineRefusal::NothingToRespawn`, Display "nothing to
  respawn". It covers two cases: no raisable mark in the shape, or no free
  cell for any of them. Check that app-core shows refusals through Display
  and add an arm there if it matches on the variants.

## Phase 1 — engine: snapshot, perk, party-side cast

Files: `tactical/mod.rs`, `tactical/turn.rs`, `abilities.rs`,
`game/combat_round.rs`, `game/combat.rs`, `game/spawning.rs`, `perks.rs`,
`tuning.rs`, `components.rs`, `routine_tree.rs` (check only),
`assets/abilities/respawn.ron`, `assets/perks/spawn_priority.ron`,
`assets/perks/groups.ron`, tests in `tests/{tactical,summons,perks,assets}.rs`.

1. **Fork-death reproducer** (tests/summons.rs): a tactical fork killed
   mid-fight produces no siege log and is not benched. Fix as decided above.
2. **Doc comment:** move the doc comment at `tactical/mod.rs:79-89` onto
   `TacticalBattle` (no test).
3. **Snapshot:** `FallenBody { species, rarity, stats }` and
   `Fallen.raise: Option<FallenBody>`. `fall` fills it for a `Creature` that
   has no `Boss` and no `Summoned`. Tests: an ordinary kill is raisable; a
   boss, a fork or a structure is not.
4. **Perk:** append `Perk::SpawnPriority` and update every item the spec
   lists: `all()` → 21, `one_level_is_worth_something`, the index asserts,
   the asset, `groups.ron`, `spawn_priority_level`,
   `spawn_priority_stat_bonus`. Forks read the bonus. Test: with the perk,
   fork stats are higher and can pass 1.0× the base.
5. **Effect:** add `Reanimate { count }` with an arm in every exhaustive
   match (Teleport's `unreachable!` in group `use_ability`). Add `respawn.ron`
   (`count: 3`, `wild_weight` > 0, same shape/cost style as `bit_rot.ron`).
   Test: the group model refuses with `BattleMapOnly` and spends no Power.
6. **Party cast:** add the `Respawned` component, the `NothingToRespawn`
   refusal, the `run_tactical_routine` branch and the per-side dissolve. It
   raises nearest the aim first, up to `count`. Tests (spec §Testing): raise
   at ×0.75, marks removed, count cap, refusal with no Power spent, a
   raised body leaves a plain `x`, recast replaces only its own side's set
   while forks survive, all gone after `finish_fight`.

## Phase 2 — engine: hostile side, AI, Decompile, discovery

Files: `tactical/turn.rs`, `tactical/ai.rs`, `game/combat_enemy.rs`,
`game/combat_teardown.rs` (verify only), `game/routines.rs` (verify only),
`tests/{tactical,routines,assets}.rs`.

1. **Hostile reap:** add a `Summoned` guard inside `reap_tactical_dead`'s
   `Hostile` branch: fall, despawn, pay nothing. Test: killing a hostile
   raised body gives no XP, loot, `DownedProgram` or nest/patrol change.
2. **Decompile** refuses any `Summoned` target. Test on a raised body.
3. **Wild casting:** `ready_from_candidates` admits `Reanimate` when
   `in_tactical_battle()`. `best_aim` scores it by the raisable marks the
   shape covers and rejects an aim that covers none. `aimable_this_turn`
   uses the same rule. Tests: a hostile carrier casts it and its raised
   bodies are `Hostile` and AI-driven; a companion carrier casts it too; with
   no marks in reach, the routine isn't chosen.
4. **Discovery + census:** Respawn stays hidden until extracted; the
   hunt-only count goes 34 → 35.

## Phase 3 — view, renderer, arena, docs

Files: `tactical/view.rs`, `crates/gui/src/render/tactical.rs`,
`crates/gui/src/render/hud/palette.rs`, `dev-arenas/respawn.ron` (+ README
row), `assets/abilities/README.md`, `assets/perks/README.md`, `CHANGELOG.md`
(unreleased section only; no version bump on the branch).

1. `TacticalBody.respawned`. Test: set for a raised body, unset for a fork.
2. `palette::RESPAWNED ≈ (0.42, 0.24, 0.24)` replaces `glyph_color(body.color)`
   as the glyph ink and sprite tint when `respawned` is set. Draw only
   through `Painter`.
3. Arena scenario: a Respawn-equipped player against a carrier. Run it and
   check it finishes without a panic.
4. Screenshot: `--template` into a battle map, cast, `--screenshot`, Read
   the PNG, tune the colour. Needs `DISPLAY`. If there's no display, say so
   and leave the colour at the spec's starting value.
5. Docs as listed.
