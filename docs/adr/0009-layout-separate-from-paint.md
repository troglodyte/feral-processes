# 0009 Screen layout is separable from paint

Status: Proposed (2026-10-16)

## Context

The UI font is monospace, yet screens learn the cell width by measuring `"M"` through the `Painter` at about 69 sites, which is the only thing keeping their layout arithmetic from being pure.
Four helpers each compute "how many columns fit" (`render/notify.rs:45`, `perk_bought.rs:55`, `research_graph.rs:239`, `popup.rs:1092` with `ROW_WRAP_COLUMNS = 100`).
There are three unrelated row types — `popup::Row`, `hud::panes::Row` and `SectionRow` — and the `SCRIM` colour is written out in `notify.rs:32`, `level_up.rs:24` and `perk_bought.rs:20`.
`settlement_market.rs:67-78` copies caravan's `cell` closure under a comment saying it is unchanged, which is the "mirrors must be a call" rule broken.

The size problem in `render/` is single functions, not files: `draw_surface_map` is 1,159 lines with eleven parameters (`render/base.rs:669-1827`), `draw_mode_overlay` about 700, `draw_playing_base` 424.
`render/base.rs` is 2,082 production lines plus 5,259 of tests.

Pure layout already exists where it was needed: `popup_layout`, `manifest/layout.rs` and `hud::layout::regions` compute rectangles from a `Metrics` and are tested headlessly.

## Decision

Put `cell_w` on `Metrics` so every screen can lay out without a `Painter`.
Add `render/card.rs` for the four over-map card screens (notify, level-up, perk-bought, research graph) with one scrim, one panel rect and one column-fit.
Split the three giant draw functions into a gather phase that returns a view struct and layer passes that paint it; the view struct is what a test asserts on.
Replace the copied `cell` closure with a call.
Split `render/mod.rs` into a style module and a dispatch module and replace the 73 `use super::*` imports with explicit ones.

`Painter` stays a struct; a trait with one implementation is not wanted.

## Consequences

Layout becomes testable without an egui context, which is cheaper than the current `with_painter` path and lets the never-clip rule be asserted per screen in pure code.
The three row types can converge once there is one layout vocabulary to converge on.
`.claude/rules/drawing-seam.md` is updated to the real size of `render/` (about 60k lines, twenty-odd painter operations, not 3k and fifteen).

## Evidence

- `crates/gui/src/render/popup.rs:1092` — `ROW_WRAP_COLUMNS`.
- `crates/gui/src/render/settlement_market.rs:67-78` — copied closure.
- `crates/gui/src/render/base.rs:669-1827` — `draw_surface_map`.
