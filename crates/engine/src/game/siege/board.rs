//! The siege board: a flood fill from the base's one door through
//! `BaseGrid::walkable`, seated in the bounding box of the fill.
//!
//! **The fill's frontier is the wall.** `BaseGrid` solid rock is never
//! breached (§6 of the design), so nothing here does a separate wall pass —
//! whatever the fill does not reach reads `Blocked` because `Board::solid`
//! started every cell that way and only the fill's own cells are opened.
//! That is also what makes a sealed-off pocket excluded from the board *by
//! construction*: a cell with no walkable path to the door is simply never
//! visited, never opened, and so never fought over — a player who walled
//! off their far works has genuinely protected them, with nothing here
//! having to check for the case.

use std::collections::{BTreeSet, HashSet, VecDeque};

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::Position;
use crate::game::base_space::BASE_EXIT_CELL;
use crate::tactical::TacticalBattle;
use crate::tactical::map::{BattleCell, Board};

/// Four-way — rock is never breached, so a diagonal fill would let the
/// board slip past a wall no corridor was ever cut through.
const NEIGHBOURS4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// A battle map built from the base rather than generated — `map::generate`
/// untouched, and this is the one place a siege departs from every other
/// tactical fight.
pub(crate) struct SiegeBoard {
    pub board: Board,
    /// The base-space cell that maps to the board's `(0, 0)` — the flood
    /// fill's bounding box's own minimum corner.
    pub origin: (i32, i32),
    /// `BASE_EXIT_CELL`, in board coordinates.
    pub door: (i32, i32),
}

/// Flood-fills base space from `BASE_EXIT_CELL` and seats the result in the
/// bounding box of what it found. `None` when the door cell itself is not
/// walkable — a base that does not exist yet.
pub(crate) fn build(game: &mut Game) -> Option<SiegeBoard> {
    let grid = game.world.resource::<BaseGrid>();
    if !grid.walkable(BASE_EXIT_CELL.0, BASE_EXIT_CELL.1) {
        return None;
    }

    let mut filled = BTreeSet::new();
    filled.insert(BASE_EXIT_CELL);
    let mut queue = VecDeque::from([BASE_EXIT_CELL]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in NEIGHBOURS4 {
            let next = (x + dx, y + dy);
            if grid.walkable(next.0, next.1) && filled.insert(next) {
                queue.push_back(next);
            }
        }
    }

    // `filled` always holds at least `BASE_EXIT_CELL`, so every one of
    // these has something to answer.
    let min_x = filled.iter().map(|&(x, _)| x).min().unwrap();
    let max_x = filled.iter().map(|&(x, _)| x).max().unwrap();
    let min_y = filled.iter().map(|&(_, y)| y).min().unwrap();
    let max_y = filled.iter().map(|&(_, y)| y).max().unwrap();
    let origin = (min_x, min_y);
    // The longer edge of the box — a non-square fill is seated inside the
    // bounding box it needs and the remainder is left `Blocked`, exactly as
    // `Board::solid` started it.
    let side = (max_x - min_x + 1).max(max_y - min_y + 1);

    let mut board = Board::solid(side);
    for &(x, y) in &filled {
        board.set(x - origin.0, y - origin.1, BattleCell::Open);
    }

    let door = (BASE_EXIT_CELL.0 - origin.0, BASE_EXIT_CELL.1 - origin.1);
    Some(SiegeBoard {
        board,
        origin,
        door,
    })
}

impl SiegeBoard {
    /// A base-space cell in board coordinates, or `None` outside the box —
    /// which includes a cell the fill never reached, since that box is
    /// exactly what the fill found.
    pub(crate) fn to_board(&self, base: (i32, i32)) -> Option<(i32, i32)> {
        let cell = (base.0 - self.origin.0, base.1 - self.origin.1);
        self.board.in_bounds(cell.0, cell.1).then_some(cell)
    }

    /// A board cell in base-space coordinates — the inverse of
    /// [`to_board`](Self::to_board), and total: every cell of a built board
    /// has a base-space address, whether or not anything stands there.
    ///
    /// `#[allow(dead_code)]` until Task 16's `persist::restore` becomes its
    /// first production caller, rebuilding a saved body's board cell back
    /// into base-space `Position` — `study_pen`'s own precedent for a task
    /// landing a door before the task that walks through it.
    #[allow(dead_code)]
    pub(crate) fn to_base(&self, cell: (i32, i32)) -> (i32, i32) {
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
/// Returns which structures actually seated, so a caller building
/// initiative from every other body on the board can exclude them.
pub(crate) fn seat_structures(
    battle: &mut TacticalBattle,
    siege_board: &SiegeBoard,
    structures: Vec<(Entity, Position, u8)>,
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
            seated.insert(entity);
        }
    }
    seated
}
