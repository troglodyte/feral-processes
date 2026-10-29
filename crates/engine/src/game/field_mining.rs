//! Mining a blocker on the field: the gate that says which blockers the base
//! may dig, and the strike that breaks one into `Biome::Excavated`.

use std::collections::BTreeSet;

use crate::base_grid::Ground;
use crate::components::DigSite;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

/// Whether the player or crew may mine `cell`: it is a blocker, and an
/// orthogonal neighbour is laid floor or pending floor, so the base digs
/// outward from itself rather than anywhere on the map.
///
/// `pending` is the cells with an open floor request — a dig mark with no
/// finish order (`Game::pending_floor`) — so a line of marks queued into the
/// rock in one go is dug in order (plan, resolved question 5).
pub(crate) fn minable(cell: (i32, i32), ground: Ground, pending: &BTreeSet<(i32, i32)>) -> bool {
    let (x, y) = cell;
    !ground.walkable(x, y)
        && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
            let n = (x + dx, y + dy);
            ground.is_floor(n.0, n.1) || pending.contains(&n)
        })
}

impl Game {
    /// One swing at the field blocker `(x, y)`. Breaking it overrides the
    /// tile to `Biome::Excavated`, walkable, which the save already carries
    /// in `tile_overrides`. Walkable ground is left alone.
    ///
    /// The gate is `minable`, asked by the caller (the bump in
    /// `Game::move_player`, the crew in `Game::run_dig_crew`): this is the
    /// strike alone.
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
        let marked = self.world.get::<DigSite>(site).is_some_and(|d| d.marked);
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

    /// The cells with an open floor request: every marked `DigSite` with no
    /// finish order. What `minable` counts as pending adjacency.
    pub(crate) fn pending_floor(&mut self) -> BTreeSet<(i32, i32)> {
        let mut query = self.world.query::<(&DigSite, &Position)>();
        query
            .iter(&self.world)
            .filter(|(d, _)| d.marked && d.finish.is_none())
            .map(|(_, p)| (p.x, p.y))
            .collect()
    }
}
