//! The base's footprint on the field (ADR 0032): the floor founding lays,
//! the field content floor is refused on, and distance measured to the
//! nearest floor cell rather than to the anchor.

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::resources::Outposts;
use crate::tuning::FOUNDING_FLOOR_CELLS;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

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

/// The cells founding floors, in the order they join: `home` first, then
/// repeatedly the open cell beside the patch (four-way) nearest `home` in a
/// straight line, until `target` cells or nothing open is left.
///
/// Grown through the patch rather than picked by distance alone, so the
/// floor is one piece the "new floor touches floor" rule can extend, and
/// open ground behind a thin wall never joins it. Ties break on `(y, x)`,
/// so the same ground always gives the same patch.
pub(crate) fn founding_cells(
    home: (i32, i32),
    target: usize,
    mut is_open: impl FnMut(i32, i32) -> bool,
) -> Vec<(i32, i32)> {
    let dist2 = |(x, y): (i32, i32)| (x - home.0).pow(2) + (y - home.1).pow(2);
    let mut cells = Vec::with_capacity(target);
    let mut seen = HashSet::from([home]);
    let mut frontier = BinaryHeap::from([Reverse((0, home.1, home.0))]);
    while cells.len() < target {
        let Some(Reverse((_, y, x))) = frontier.pop() else {
            break;
        };
        cells.push((x, y));
        for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if seen.insert(next) && is_open(next.0, next.1) {
                frontier.push(Reverse((dist2(next), next.1, next.0)));
            }
        }
    }
    cells
}

impl Game {
    /// Lays founding's floor: `FOUNDING_FLOOR_CELLS` cells grown from the
    /// anchor around anything that cannot take floor (`founding_cells`).
    /// Founding has already refused an anchor tile that cannot.
    pub(crate) fn lay_founding_floor(&mut self, x: i32, y: i32) {
        let cells = founding_cells((x, y), FOUNDING_FLOOR_CELLS, |cx, cy| {
            self.ground().walkable(cx, cy) && self.floor_conflict(cx, cy).is_none()
        });
        let mut grid = self.world.resource_mut::<BaseGrid>();
        for (cx, cy) in cells {
            grid.lay_floor(cx, cy);
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

#[cfg(test)]
mod tests {
    use super::founding_cells;
    use std::collections::HashSet;

    #[test]
    fn open_ground_fills_the_five_by_five() {
        let cells: HashSet<_> = founding_cells((10, 10), 25, |_, _| true)
            .into_iter()
            .collect();
        let square: HashSet<_> = (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| (10 + dx, 10 + dy)))
            .collect();
        assert_eq!(cells, square);
    }

    #[test]
    fn the_home_tile_comes_first() {
        assert_eq!(founding_cells((3, -4), 25, |_, _| true)[0], (3, -4));
    }

    /// Rock along the whole column east of the Home: the patch keeps its 25
    /// cells by bulging west, and takes `(-3, 0)` before `(-3, 1)` — rounded,
    /// nearest in a straight line.
    #[test]
    fn rock_beside_the_home_pushes_the_fill_the_other_way() {
        let cells = founding_cells((0, 0), 25, |x, _| x <= 0);
        assert_eq!(cells.len(), 25);
        assert!(cells.iter().all(|&(x, _)| x <= 0), "never floors rock");
        assert!(cells.contains(&(-3, 0)));
        assert!(cells.contains(&(-2, 2)));
    }

    /// A pocket behind a thin wall is near in a straight line but not
    /// reachable through open ground, so it never joins the patch.
    #[test]
    fn the_fill_never_jumps_a_wall() {
        // A one-wide corridor running east from the Home, with open ground
        // just north of it behind a wall row at y = -1.
        let open = |x: i32, y: i32| y == 0 && x >= 0 || y <= -2;
        let cells = founding_cells((0, 0), 25, open);
        assert_eq!(cells.len(), 25);
        assert!(cells.iter().all(|&(_, y)| y == 0), "stays in the corridor");
        assert_eq!(cells.iter().map(|c| c.0).max(), Some(24));
    }

    #[test]
    fn a_closed_pocket_lays_what_fits() {
        let open = |x: i32, y: i32| (0..=1).contains(&x) && (0..=1).contains(&y);
        let cells = founding_cells((0, 0), 25, open);
        assert_eq!(cells.len(), 4);
    }

    #[test]
    fn the_patch_is_connected() {
        // A checker of rock: every reached cell must touch an earlier one.
        let open = |x: i32, y: i32| (x + y) % 3 != 0 || (x, y) == (0, 0);
        let cells = founding_cells((0, 0), 25, open);
        for (i, &(x, y)) in cells.iter().enumerate().skip(1) {
            assert!(
                cells[..i]
                    .iter()
                    .any(|&(px, py)| (px - x).abs() + (py - y).abs() == 1),
                "{:?} joined without touching the patch",
                (x, y)
            );
        }
    }
}
