//! The base's footprint on the field (ADR 0032): the floor founding lays,
//! the field content floor is refused on, and distance measured to the
//! nearest floor cell rather than to the anchor.

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::resources::Outposts;

/// Chebyshev distance from `tile` to the nearest floor cell, or `None` with
/// no floor laid.
///
/// The one measure of "how far from the base" now that the base is a shape
/// rather than a point: the outpost minimum distance and the raiding and
/// garrison radii all read it, so a base grown toward a town is nearer that
/// town.
pub(crate) fn distance_to_floor(floor: &BaseGrid, tile: (i32, i32)) -> Option<i32> {
    floor
        .floor_cells()
        .map(|(x, y)| (x - tile.0).abs().max((y - tile.1).abs()))
        .min()
}

impl Game {
    /// Lays founding's floor: the anchor's own cell and the ring around it.
    /// Founding has already refused a ring with a blocked or conflicting
    /// cell, so every cell of it takes floor.
    pub(crate) fn lay_founding_floor(&mut self, x: i32, y: i32) {
        let mut grid = self.world.resource_mut::<BaseGrid>();
        for dy in -1..=1 {
            for dx in -1..=1 {
                grid.lay_floor(x + dx, y + dy);
            }
        }
    }

    /// Why `(x, y)` cannot take floor because of what already stands on the
    /// field there, or `None`: a Stack link (walked onto to descend, so a
    /// floor cell there could never be stood on), a settlement's footprint,
    /// or an outpost. Each is a fixture of the field the base must grow
    /// around, not over.
    pub(crate) fn floor_conflict(&mut self, x: i32, y: i32) -> Option<&'static str> {
        if self.find_surface_link_at(x, y).is_some() {
            return Some("A Stack link opens there — floor can't go over it.");
        }
        if self.find_settlement_at(x, y).is_some() {
            return Some("That ground belongs to a settlement.");
        }
        if self.world.resource::<Outposts>().0.contains_key(&(x, y)) {
            return Some("An outpost stands there.");
        }
        None
    }
}
