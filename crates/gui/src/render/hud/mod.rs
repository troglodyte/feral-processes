//! The main HUD's frame: the five regions every non-battle screen draws the
//! world into, and the chrome mounted on their borders.
//!
//! The layout is a port of the "2a Paned Command" handoff in
//! `Rust Bevy Base Building UI/design_handoff_hud/` — its *arithmetic*, not
//! its character grid. That handoff assumes a strict 160x38 cell grid; this
//! renderer has never had one, because `Metrics` ramps UI text continuously
//! off window height while `map_cell` is an integer ladder off unscii's
//! native cell, and those two rules deliberately never mix. So `layout`
//! takes a measured character width as a *parameter* and stays free of
//! `Painter`, which is what makes the geometry unit-testable headlessly.
//!
//! See `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md`.

pub(super) mod bar;
pub(super) mod layout;
pub(super) mod log_frame;
pub(super) mod map_frame;
pub(super) mod palette;
pub(super) mod status_bar;
pub(super) mod strip;
