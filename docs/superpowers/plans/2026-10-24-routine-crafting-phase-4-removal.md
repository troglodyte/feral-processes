# Phase 4 — Removal

Read first: plan index; spec "What goes away"; `.claude/rules/content-schema.md`,
`seams-items.md`, `seams-research.md`. Delete, don't shim: no `// removed`,
no unused renames.

## Remove (grep every reference before each deletion)

- Etching: `Game::etch_disk`, `ItemDb::synthesise_etched_disks`, `etched_*`
  items, `Mode::RoutineEtch` (app-core `lib.rs`, `app/routines.rs`,
  `group_menu.rs`, `input.rs`, gui `render/mod.rs`) and the Etch row.
- Creation Routine step: `CharacterChoice.routine`, `starter_rows`, the
  `starter:` field on ability files (`abilities.rs`, `game/creation.rs`,
  `app/creation.rs`, `render/creation.rs`).
- `KnownRoutines`, `DiscoveredRoutines`, `family_is_discoverable`; their save
  fields; `routine/<id>` research ids. Callers in `game/unlocks.rs`,
  `views/market.rs`, `views/routines.rs`, `arena/setup.rs`, `tuning/stack.rs`,
  gui `render/base.rs`, `render/tactical.rs`.
- `install_disk` if phase 2 superseded every caller.
- The 31 player-only files in `assets/abilities/`: list them by computing the
  set referenced by nothing but the player research tree (script it; paste
  the list in the commit message). Arena scenarios → nearest wild-pool id;
  `dev-saves/` templates rewritten (dump/pack via savetool).
- `assets/abilities/README.md`: research/etching/creation-pool sections
  rewritten to "player routines are built"; fix the stale "sixteen" effects
  count and slot numbers.

## Tasks

- [ ] 1. App-core + gui first, with the engine API each removal takes, in one
  commit per removal so the workspace compiles at every commit.
- [ ] 2. Tests that used etching/known routines switch to `compile`-built
  routines — keep the assertion, change the setup. Don't delete a test whose
  behaviour still exists.
- [ ] 3. Asset deletions + arena + dev-saves; `cargo run --bin arena` on each
  touched scenario loads.
- [ ] 4. Census: no grant site gives the player an authored routine
  (research, extraction, boss drop, market, creation, items). New game:
  slots exactly Decompile + Patch, no Routine step.

## Gate

`cargo test --workspace`, `balance_sim`, clippy, fmt; `rg` for each removed
symbol returns nothing outside CHANGELOG/specs. Handback: the 31-file list,
scenario substitutions.
