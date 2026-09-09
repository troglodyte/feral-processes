//! The tactical battle model: a disposable grid a surface fight is fought
//! on.
//!
//! Opt-in, off by default, and the second of the game's two combat models —
//! see
//! `docs/superpowers/specs/2026-09-09-tactical-surface-battles-design.md`.
//! Nothing outside this module's own tests calls into it yet; the router
//! that chooses between the two models is a later phase.
//!
//! **A battle coordinate lives here and nowhere else.** No world `Position`
//! is ever written for a body standing on a battle map, the same way the
//! Stack keeps its coordinates in `resources::Locale` and base space keeps
//! its own in `Locale::Base`. This module does not import `Position`.

pub mod map;
