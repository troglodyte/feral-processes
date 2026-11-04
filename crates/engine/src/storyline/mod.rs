//! Storylines: run-long arcs that live in plain Rust owned by `Game`
//! (ADR 0042). Each beat is a pure decide function; `game/storyline.rs` is
//! the shell that gathers its input and applies its outcome.

use serde::{Deserialize, Serialize};

mod base_ai;
#[cfg(test)]
mod tests;

pub use base_ai::*;

/// One field per arc. Not a `Resource`: a new resource shifts bevy's
/// iteration order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Storyline {
    pub base_ai: BaseAi,
}
