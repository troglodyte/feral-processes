# 0009 Screen layout is separable from paint

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-16 (narrowed 2026-10-21)
- Pattern: Functional core, imperative shell

## Context

The UI font is monospace, yet screens learn the cell width by measuring `"M"` through the `Painter` at about seventy sites.
`text::Metrics` carries no cell width, so that measurement is the only thing keeping a screen's layout arithmetic from being pure.
Several helpers each compute "how many columns fit", and `ROW_WRAP_COLUMNS` is redeclared per screen.

Pure layout already exists where it was needed: `render::popup::popup_layout`, `render/manifest/layout.rs` and `render::hud::layout::regions` compute rectangles from a `Metrics` and are tested headlessly.
The giant draw functions (`render::base::draw_surface_map`, `render::draw_mode_overlay`) mix gathering, layout and painting in one body.

## Decision

Put `cell_w` on `Metrics` so every screen can lay out without a `Painter`.
A screen draws in three steps: gather (read `App` and `Game` into a view struct), lay out (a pure function of the view and `Metrics` returning rectangles and rows), paint (walk the layout through the `Painter`).
The view and the layout are what a test asserts on.
The four over-map card screens (notify, level-up, perk-bought, research graph) share one `render/card.rs` for scrim, panel rect and column fit.
When `render/mod.rs` is split, the style half becomes `render/style.rs` and the mode-to-painter routing becomes `render/screens.rs`; `render/dispatch.rs` is already the Relay hub's screen and keeps that name.

Apply it to a screen when that screen is being changed, as `CLAUDE.md` already says for splitting big files.

## Alternatives considered

- `Painter` as a trait, with a recording test painter. Rejected in [0011](0011-restructurings-considered-and-rejected.md); one backend.
- Keep measuring `"M"`. Every new screen copies the measurement, and layout stays testable only through an egui context.

## Consequences

Layout becomes testable without egui, and the never-clip rule can be asserted per screen in pure code.
The row types (`popup::Row`, `hud::panes::Row`, `SectionRow`) can converge once there is one layout vocabulary.
Small cleanups found by the same review are in [backlog.md](backlog.md).

## Evidence

- `crates/gui/src/text.rs` — `Metrics`.
- `crates/gui/src/render/popup.rs` — `popup_layout`.
- `crates/gui/src/render/hud/layout.rs` — `regions`.
- `crates/gui/src/render/dispatch.rs` — the existing name.
