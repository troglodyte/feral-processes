# Phase 4 — Screens (app-core, gui, help)

Read first: plan index; spec "The Workbench screens", "Research" (screen
bullet); `.claude/rules/seams-screens.md`, `drawing-seam.md`;
`app/routine_craft.rs` + its gui renderer as the model; memories
`all-modes-length-is-a-semantic-merge-conflict`, `gui-text-never-clips-or-wraps`.

## Files

- New `crates/app-core/src/app/gear_craft.rs`: Modes for Forge (designs list,
  delete with typed reason), New design (category rows → part picker →
  preview → name 1–24 chars → create), Weapon parts (craft), Salvage entry.
  Reads `views/gear_craft.rs` only. Nothing new in `lib.rs` beyond `Mode`
  variants and dispatch.
- `app/structure_hub.rs`: Workbench pages beside the routine ones.
- Research screen: Weapons tree tab — new Mode, gui title, empty state.
- New `crates/gui/src/render/gear_craft.rs`, drawing only through `Painter`.
- `assets/help/`: one page for the Forge.

## Tasks

- [ ] 1. App-core modes + key flow, tests per page (highlight + Enter,
  type-to-filter on part picker, read-only row counts owned by app-core).
- [ ] 2. Research Weapons tab.
- [ ] 3. Gui renderers; `ALL_MODES` updated.
- [ ] 4. Screenshot each page via `--template` + `--keys` (pick a dev-save
  with `weapon_bench` researched, or `capture` one) and Read the PNGs.
- [ ] 5. Help page.

## Gate

`cargo test --workspace`, clippy. Handback: the screenshot paths and the
dev-save used.
