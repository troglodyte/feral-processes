//! Bounty contracts: a named elite program the Broker sends the party after.
//!
//! The contract owns the quarry's whole definition (ADR 0044); the map holds
//! only a projection of it. Everything here is plain data and pure functions,
//! so the roll is testable without a `Game`.

mod names;
mod roll;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::components::Rarity;

pub use roll::{BountyInput, RolledBounty, roll};

/// The individual a bounty names, fixed at roll time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BountyQuarry {
    /// "Vesk the Long Wire".
    pub name: String,
    pub species: String,
    /// Stat multiplier on top of the boss path, in percent (150 is x1.5).
    /// Whole percent rather than an `f32` because `Objective` is `Eq`.
    pub grade_pct: u32,
    pub rarity: Rarity,
    /// Species and count, sorted by species.
    pub escort: Vec<(String, u32)>,
    pub anchor: (i32, i32),
    pub tether: i32,
}

impl BountyQuarry {
    /// The stat multiplier the quarry is scaled by at spawn.
    pub fn grade(&self) -> f32 {
        self.grade_pct as f32 / 100.0
    }

    /// How many programs stand with the quarry.
    pub fn escort_size(&self) -> u32 {
        self.escort.iter().map(|(_, n)| n).sum()
    }
}
