//! The world map's stored state: the fog the party's own walking writes.
//! `resources::ExploredChunks` holds only where the party has been; the base,
//! outposts and route corridors are derived by the view and never stored.

use crate::components::Position;
use crate::resources::{ExploredChunks, Locale};
use crate::tuning::WORLD_MAP_REVEAL_RADIUS_CHUNKS;
use crate::world::CHUNK_SIZE;
use crate::*;

impl Game {
    /// Reveals the chunks around the party. Called once from `tick_inner`,
    /// the one place every writer of `Position` (walking, recall, the
    /// footprint eject) passes through.
    ///
    /// **Guarded on `Locale::Surface`**, unlike the neighbouring stocking
    /// calls: a Stack frame or base space has its own coordinates, and
    /// marking them would reveal surface chunks the party never stood near.
    pub(crate) fn mark_explored_chunks(&mut self) {
        if !matches!(*self.world.resource::<Locale>(), Locale::Surface) {
            return;
        }
        let pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (px, py) = (pos.x.div_euclid(CHUNK_SIZE), pos.y.div_euclid(CHUNK_SIZE));
        let r = WORLD_MAP_REVEAL_RADIUS_CHUNKS;
        let mut explored = self.world.resource_mut::<ExploredChunks>();
        for cy in (py - r)..=(py + r) {
            for cx in (px - r)..=(px + r) {
                explored.0.insert((cx, cy));
            }
        }
    }
}
