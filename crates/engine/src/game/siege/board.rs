//! The siege board: the base's laid floor plus the field cells besiegers
//! come in from, seated in the bounding box of the two (ADR 0032, decision 6).
//!
//! **The floor's edge is where a siege comes from.** Besiegers spawn on
//! walkable field cells orthogonally next to an edge floor tile
//! (`siege_spawn_cells`), and path inward. Everything not on the board reads
//! `Blocked` because `Board::solid` started every cell that way.

use std::collections::{BTreeSet, HashSet};

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::Position;
use crate::tactical::TacticalBattle;
use crate::tactical::map::{BattleCell, Board};
use crate::world::WorldMap;

const NEIGHBOURS4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A built board and where it sits in field coordinates.
pub(crate) struct SiegeBoard {
    pub board: Board,
    /// The field cell board `(0, 0)` stands on.
    pub origin: (i32, i32),
    /// Every spawn cell, in board coordinates, in `siege_spawn_cells`'
    /// coordinate order.
    pub spawns: Vec<(i32, i32)>,
}

/// The field cells a siege's besiegers spawn on, sorted by coordinate.
///
/// Walkable non-floor cells orthogonally next to the floor's edge. A sealed
/// base — no walkable cell touches its floor — falls back to the nearest
/// walkable cells outside it: a breadth-first walk out through the blockers,
/// ring by ring, returning the first ring that holds any, up to
/// `SIEGE_SPAWN_SEARCH_TILES`. Empty with no floor, or with nothing walkable
/// in reach. Pure, so the order never depends on query iteration.
pub(crate) fn siege_spawn_cells(floor: &BaseGrid, map: &WorldMap) -> Vec<(i32, i32)> {
    let open = |c: (i32, i32)| !floor.is_floor(c.0, c.1) && map.peek(c.0, c.1).walkable;
    let mut seen: BTreeSet<(i32, i32)> = floor.floor_cells().collect();
    let mut frontier: Vec<(i32, i32)> = seen.iter().copied().collect();
    for _ in 0..crate::tuning::SIEGE_SPAWN_SEARCH_TILES {
        let mut ring = BTreeSet::new();
        for &(x, y) in &frontier {
            for (dx, dy) in NEIGHBOURS4 {
                let next = (x + dx, y + dy);
                if seen.insert(next) {
                    ring.insert(next);
                }
            }
        }
        let found: Vec<(i32, i32)> = ring.iter().copied().filter(|&c| open(c)).collect();
        if !found.is_empty() || ring.is_empty() {
            return found;
        }
        frontier = ring.into_iter().collect();
    }
    Vec::new()
}

/// Builds the board from the floor and its spawn cells, `None` with no floor
/// or nowhere for a siege to come from. The caller picks the siege's own
/// entry from `spawns`.
pub(crate) fn build(game: &mut Game) -> Option<SiegeBoard> {
    let floor = game.world.resource::<BaseGrid>();
    let map = game.world.resource::<WorldMap>();
    let spawn_cells = siege_spawn_cells(floor, map);
    if spawn_cells.is_empty() {
        return None;
    }
    let cells: BTreeSet<(i32, i32)> = floor
        .floor_cells()
        .chain(spawn_cells.iter().copied())
        .collect();

    let min_x = cells.iter().map(|&(x, _)| x).min()?;
    let max_x = cells.iter().map(|&(x, _)| x).max()?;
    let min_y = cells.iter().map(|&(_, y)| y).min()?;
    let max_y = cells.iter().map(|&(_, y)| y).max()?;
    let origin = (min_x, min_y);
    let side = (max_x - min_x + 1).max(max_y - min_y + 1);

    let mut board = Board::solid(side);
    for &(x, y) in &cells {
        board.set(x - origin.0, y - origin.1, BattleCell::Open);
    }
    let spawns: Vec<(i32, i32)> = spawn_cells
        .iter()
        .map(|&(x, y)| (x - origin.0, y - origin.1))
        .collect();
    Some(SiegeBoard {
        board,
        origin,
        spawns,
    })
}

impl SiegeBoard {
    /// A field cell in board coordinates, or `None` outside the box.
    pub(crate) fn to_board(&self, field: (i32, i32)) -> Option<(i32, i32)> {
        let cell = (field.0 - self.origin.0, field.1 - self.origin.1);
        self.board.in_bounds(cell.0, cell.1).then_some(cell)
    }

    /// A board cell in field coordinates — the inverse of
    /// [`to_board`](Self::to_board).
    #[cfg(test)]
    pub(crate) fn to_field(&self, cell: (i32, i32)) -> (i32, i32) {
        (cell.0 + self.origin.0, cell.1 + self.origin.1)
    }
}

/// Seats every structure of `structures` (`Game::structure_footprints`'s own
/// shape) onto `battle` as a body — `Game::open_siege`'s placement loop,
/// lifted out so `game::siege::persist::restore` can seat the same
/// structures again once a save/load round trip has rebuilt them fresh.
/// Left out of initiative, `open_siege`'s own reason: a structure is an
/// obstacle a swing can be aimed at (Task 10), not a combatant.
///
/// A structure `is_barrier` answers yes for screens its cells from sight
/// once seated (`StructureDef::barrier`). Derived here on every seating
/// rather than saved, which is what lets `persist::restore` get it back.
///
/// Returns which structures actually seated, so a caller building
/// initiative from every other body on the board can exclude them.
pub(crate) fn seat_structures(
    battle: &mut TacticalBattle,
    siege_board: &SiegeBoard,
    structures: Vec<(Entity, Position, u8)>,
    is_barrier: impl Fn(Entity) -> bool,
) -> HashSet<Entity> {
    let mut seated = HashSet::new();
    for (entity, pos, side) in structures {
        let Some(cell) = siege_board.to_board((pos.x, pos.y)) else {
            continue;
        };
        if side > 1 {
            battle.set_shape(entity, side, 0);
        }
        if battle.place(entity, cell) {
            if is_barrier(entity) {
                battle.screen(entity);
            }
            seated.insert(entity);
        }
    }
    seated
}
