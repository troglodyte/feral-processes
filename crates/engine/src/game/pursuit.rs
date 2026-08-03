//! A bounded cost field for chasing something across the map, routed around
//! obstacles rather than assumed open ground. `Game::nest_aggro_tick`
//! (`game/turn.rs`) is the one caller: it walks each provoked nest guardian
//! downhill along the field this module builds.

use std::collections::HashMap;

use pathfinding::directed::dijkstra::dijkstra_all;

use crate::world::{Biome, NEIGHBOURS, WorldMap};

/// Chebyshev step counts from `origin` to every tile reachable within
/// `radius`, routed around unwalkable terrain. `origin` itself is present
/// with cost 0. A tile absent from the map is unreachable, off-limits (the
/// base platform), or outside the box — callers must not try to tell those
/// apart, since none of them is a tile a pursuer should step onto.
pub(crate) fn pursuit_field(
    map: &mut WorldMap,
    origin: (i32, i32),
    radius: i32,
) -> HashMap<(i32, i32), u32> {
    // `WorldMap::tile` takes `&mut self` — it generates chunks lazily — so
    // the successor closure has to hold the map mutably. `dijkstra_all`
    // takes `FnMut`, which permits exactly this; nothing else in the call
    // borrows the map.
    let reached = dijkstra_all(&origin, |&(x, y)| {
        NEIGHBOURS
            .iter()
            .map(move |(dx, dy)| (x + dx, y + dy))
            .filter(|&(nx, ny)| {
                // Bounding the *successors*, not just the result, is what
                // bounds the search: an unbounded failed search on this
                // lazily-generated, effectively infinite map would keep
                // walking `WorldMap::tile` outward, generating chunks with
                // nothing to stop it.
                (nx - origin.0).abs() <= radius && (ny - origin.1).abs() <= radius && {
                    let tile = map.tile(nx, ny);
                    // The base slab stays the one safe ground, as
                    // `maybe_ambush` and `stamp_platform` already
                    // establish. A leash measured from the nest can't
                    // guarantee this alone — a nest can stand within
                    // leash range of the base.
                    tile.walkable && tile.biome != Biome::Platform
                }
            })
            // Movement is Chebyshev: all eight directions, diagonals
            // included, cost the same single step.
            .map(|n| (n, 1u32))
            .collect::<Vec<_>>()
    });

    let mut field: HashMap<(i32, i32), u32> = reached
        .into_iter()
        .map(|(node, (_, cost))| (node, cost))
        .collect();
    field.insert(origin, 0);
    field
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Tile;

    fn floor() -> Tile {
        Tile {
            biome: Biome::OpenGrid,
            walkable: true,
        }
    }

    fn wall() -> Tile {
        Tile {
            biome: Biome::DataVoid,
            walkable: false,
        }
    }

    /// A greedy chase — always step to the neighbour that most reduces raw
    /// Chebyshev distance to `origin` — picks (4, 0) here: it's the only
    /// neighbour of the pursuer's tile that lowers the raw distance (5 to
    /// 4). But (4, 0) is a pocket sealed against the cup's back wall with no
    /// through-route to the origin; the real shortest path has to back out
    /// through the mouth at x = 6 and go around the wall's open end, which
    /// *raises* raw distance before it can fall. The field has to prefer
    /// that detour over the dead end, which is exactly what a greedy chase
    /// cannot see.
    #[test]
    fn a_field_routes_around_a_concave_obstacle() {
        let mut map = WorldMap::new(1);
        // A generous hand-carved floor so the search never depends on
        // procedurally generated terrain outside the cup.
        for x in -8..=8 {
            for y in -8..=8 {
                map.set_override(x, y, floor());
            }
        }
        // Three walls of a cup: back against the origin (x = 3), sides
        // running out to the mouth at x = 6, open only there.
        for &(x, y) in &[(3, -1), (3, 0), (3, 1), (4, -1), (4, 1), (5, -1), (5, 1)] {
            map.set_override(x, y, wall());
        }

        let origin = (0, 0);
        let pursuer = (5, 0);
        let field = pursuit_field(&mut map, origin, 8);

        let dead_end = (4, 0);
        let mut neighbour_costs: Vec<((i32, i32), u32)> = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (pursuer.0 + dx, pursuer.1 + dy))
            .filter_map(|n| field.get(&n).map(|&c| (n, c)))
            .collect();
        neighbour_costs.sort_by_key(|&(_, c)| c);
        let (cheapest, cheapest_cost) = neighbour_costs[0];

        assert_ne!(
            cheapest, dead_end,
            "the dead-end pocket must lose to the detour around the wall"
        );
        assert!(
            cheapest_cost < field[&dead_end],
            "the detour ({cheapest_cost}) must cost less than the dead end \
             ({}), even though the dead end is raw-distance-closer to the origin",
            field[&dead_end]
        );
    }

    #[test]
    fn a_field_is_bounded_by_its_radius() {
        let mut map = WorldMap::new(2);
        let radius = 4;
        for x in -(radius + 2)..=(radius + 2) {
            for y in -(radius + 2)..=(radius + 2) {
                map.set_override(x, y, floor());
            }
        }
        let origin = (0, 0);
        let field = pursuit_field(&mut map, origin, radius);

        for &(x, y) in field.keys() {
            assert!(
                x.abs() <= radius && y.abs() <= radius,
                "({x}, {y}) lies outside the {radius}-tile box"
            );
        }
        // Walkable, but one step past the box edge — its absence has to
        // come from the radius bound, not from the tile being closed
        // terrain.
        assert!(!field.contains_key(&(radius + 1, 0)));
    }

    #[test]
    fn an_enclosed_origin_yields_a_field_of_just_itself() {
        let mut map = WorldMap::new(3);
        let origin = (0, 0);
        for (dx, dy) in NEIGHBOURS {
            map.set_override(origin.0 + dx, origin.1 + dy, wall());
        }

        let field = pursuit_field(&mut map, origin, 8);

        assert_eq!(field.len(), 1, "callers rely on absence meaning no route");
        assert_eq!(field[&origin], 0);
    }
}
