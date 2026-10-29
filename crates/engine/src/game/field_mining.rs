//! Mining a blocker on the field: the gate that says which blockers the base
//! may dig, and the strike that breaks one into `Biome::Excavated`.

use crate::base_grid::BaseGrid;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

/// Whether the player or crew may mine `cell`: it is a blocker, and it
/// touches laid floor orthogonally, so the base digs outward from itself
/// rather than anywhere on the map.
///
/// Pending floor also counts as adjacency (plan, resolved question 5); it
/// is a dig mark and not a `BaseGrid` cell, so the caller that owns marks
/// answers that half.
// Wired to the bump and the crew in the next phase; until then only tests call it.
#[cfg_attr(not(test), expect(dead_code))]
pub(crate) fn minable(cell: (i32, i32), floor: &BaseGrid, map: &mut WorldMap) -> bool {
    let (x, y) = cell;
    !map.tile(x, y).walkable
        && [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(dx, dy)| floor.is_floor(x + dx, y + dy))
}

impl Game {
    /// One swing at the field blocker `(x, y)`. Breaking it overrides the
    /// tile to `Biome::Excavated`, walkable, which the save already carries
    /// in `tile_overrides`. Walkable ground is left alone.
    ///
    /// The gate is `minable`, asked by the caller: this is the strike, as
    /// `strike_rock` is for base space.
    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn strike_field_rock(&mut self, swinger: Entity, x: i32, y: i32) {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if tile.walkable {
            return;
        }
        let by_player = swinger == self.player_entity();
        let wall = self
            .world
            .resource::<crate::rock::RockDb>()
            .wall_at(tile.biome);
        let Some(site) = self.swing_at_rock(swinger, x, y, wall) else {
            return;
        };
        let marked = self
            .world
            .get::<crate::components::DigSite>(site)
            .is_some_and(|d| d.marked);
        if !marked {
            self.world.despawn(site);
        }
        self.world.resource_mut::<WorldMap>().set_override(
            x,
            y,
            Tile {
                biome: Biome::Excavated,
                walkable: true,
                rock_shade: None,
            },
        );
        if by_player {
            self.log("The rock gives way, and the cell opens.");
        } else {
            self.log_base("Your crew cuts through, and the cell opens.");
        }
        self.roll_rock_fragment(by_player);
    }
}
