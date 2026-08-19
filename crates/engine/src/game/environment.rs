//! The one reader of `EnvironmentDb`, and the zone-1 gate it holds.
//!
//! Kept apart from the db itself so a later weather layer has an obvious
//! home beside it rather than inside the loader.

use crate::Game;
use crate::environment::{EnvironmentDb, EnvironmentDef};
use crate::resources::ZoneLevel;
use crate::world::{Biome, WorldMap};

impl Game {
    /// What the ground at `(x, y)` does to whoever stands on it, or `None`
    /// for neutral ground.
    ///
    /// The one door: the zone-1 rule lives here rather than at the call
    /// site so it cannot lapse when a second caller appears. Zone 1 is
    /// where a run learns the game, and ground that bites there would be a
    /// tax on the tutorial rather than an exception to it. The biome's
    /// *name* is deliberately outside this gate — see `Biome::name`.
    ///
    /// `&mut self` because `WorldMap::tile` generates its chunk on demand;
    /// nothing here writes anything the caller can observe.
    pub fn ground_effect(&mut self, x: i32, y: i32) -> Option<&EnvironmentDef> {
        if self.world.resource::<ZoneLevel>().0 <= 1 {
            return None;
        }
        let biome = self.world.resource_mut::<WorldMap>().tile(x, y).biome;
        // The slab is refused at load too, so this is belt and braces —
        // but a base can be stamped over any ground at all, and the one
        // safe floor in the game should not depend on a file's good
        // behaviour.
        if biome == Biome::Platform {
            return None;
        }
        self.world.resource::<EnvironmentDb>().for_biome(biome)
    }
}
