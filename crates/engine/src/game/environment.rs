//! The one reader of `environment::GroundCondition`, and the zone-1 gate it
//! holds.
//!
//! Kept apart from the catalogue itself so a later weather layer has an
//! obvious home beside it rather than inside the catalogue module.

use crate::Game;
use crate::environment::{EnvironmentEffect, GroundCondition};
use crate::resources::ZoneLevel;
use crate::world::{Biome, WorldMap};

/// What the ground at `(x, y)` is, and what it does to whoever stands on it.
pub struct Terrain {
    pub biome: Biome,
    pub condition: Option<GroundCondition>,
    /// Folded and clamped — the one number the turn hook needs, so it never
    /// has to know how many sources contributed to it.
    pub effect: EnvironmentEffect,
}

impl Game {
    /// Resolves `(x, y)` to its `Terrain`.
    ///
    /// The one door: both the zone-1 rule and the `Platform` refusal live
    /// here rather than at the call site, so neither can lapse when a
    /// second caller appears. Zone 1 is where a run learns the game, and
    /// ground that bites there would be a tax on the tutorial rather than
    /// an exception to it. The biome's *name* is deliberately outside the
    /// zone-1 gate — a zone-1 player must still learn the world's
    /// vocabulary, so this returns the real biome and a neutral effect
    /// rather than an `Option` that would hide both.
    ///
    /// `&mut self` because `WorldMap::tile` generates its chunk on demand;
    /// nothing here writes anything the caller can observe.
    pub fn terrain_at(&mut self, x: i32, y: i32) -> Terrain {
        let biome = self.world.resource_mut::<WorldMap>().tile(x, y).biome;
        if self.world.resource::<ZoneLevel>().0 <= 1 {
            return Terrain {
                biome,
                condition: None,
                effect: EnvironmentEffect::NONE,
            };
        }
        // The one safe floor in the game should not depend on a condition
        // never claiming it — a base can be stamped over any ground at all.
        if biome == Biome::Platform {
            return Terrain {
                biome,
                condition: None,
                effect: EnvironmentEffect::NONE,
            };
        }
        let condition = GroundCondition::for_biome(biome);
        let ground = condition
            .map(|c| c.def().effect)
            .unwrap_or(EnvironmentEffect::NONE);
        // Folded onto the identity rather than just clamped: there is only
        // one source this task, but the fold is the shape a later weather
        // layer stacks onto, not a case-split that has to change shape when
        // a second source arrives.
        let effect = EnvironmentEffect::NONE.fold(ground).clamped();
        Terrain {
            biome,
            condition,
            effect,
        }
    }
}
