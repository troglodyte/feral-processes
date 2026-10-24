# Phase 3 — Workbench screens (app-core, gui)

Read first: plan index; spec "The Workbench screens"; mockup
<https://claude.ai/artifact/9CaSbxtTHm77yxx9sSNgfv> (layout only — use
`Artifact` read, not WebFetch); `.claude/rules/seams-screens.md`,
`drawing-seam.md`, `seams-help.md`. Engine API is phase 2's; add to it only
through `Game` methods + views.

## Files

- `crates/app-core/src/app/structure_hub.rs`: three rail pages gated on
  `extracts_routines`.
- New `crates/app-core/src/app/routine_craft.rs` (one module per mode; nothing
  new in `lib.rs` beyond `Mode` variants and dispatch).
- `app/routines.rs`: Install lists unslotted built routines; Extract yields
  parts. (The Etch row is removed in phase 4 with its engine API.)
- `crates/gui/src/render/routine_craft.rs` (new), `render/routines.rs`,
  `render/mod.rs`. Draw only through `Painter`.
- `assets/help/` page for building routines.
- Tests: `crates/app-core/src/tests/routine_craft.rs`.

## Tasks

- [ ] 1. Build page: part rows, Enter opens highlight+Enter list of fitting
  parts with held counts; non-fitting rows show the reason; live preview;
  name field (1–24 chars); Compile. Lists: no row letters/digits.
- [ ] 2. Subroutines page: type-to-filter (`list_filter.rs`), Enter crafts;
  locked rows say why.
- [ ] 3. Built routines page.
- [ ] 4. Install / Extract on `m` use the new API, player + companion.
- [ ] 5. Help page; `all_modes` list updated (watch the semantic merge
  conflict on its length).
- [ ] 6. Screenshot each page: `cargo run -- --template <workbench template>
  --keys "…" --screenshot out.png`, Read the PNGs; text never clips.

## Gate

`cargo test --workspace`, clippy, fmt. Handback: screenshot paths, keys used.
