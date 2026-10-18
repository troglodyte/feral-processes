//! Tidying: an idle program carries a loose floor item onto a free stockpile
//! cell.
//!
//! A base with no stockpile cell marked builds no board and pays nothing, so
//! it behaves exactly as it did before the zones existed. The errand rides
//! `drift_idle_staff`'s fall-through: below every errand that has a claim on
//! the body, above the wander.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{
    Carrying, CarryingProgram, Clearing, FloorItem, Position, Task, Tidying, UnderStudy,
};
use crate::game::base::floor::take_floor_item;
use crate::game::base::hauling::{at_station, crew_reach, reaches};
use crate::game::base::offshift::Amenities;

/// The beat's tidying work, both lists `(x, y)`-sorted. Shared by every
/// tidier in the pass. An entry leaves only when its item is picked up or its
/// cell is filled, so two bodies may head for the same one; the arrival
/// checks (a free cell, an item still lying there) are what keep that correct.
pub(crate) struct TidyBoard {
    /// Free stockpile cells.
    cells: Vec<Position>,
    /// Floor items whose tile is not a stockpile.
    loose: Vec<(Entity, Position)>,
}

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Game {
    /// The free stockpile cells, `(x, y)`-sorted; `None` when no cell is
    /// marked at all. One computation for the tidy board and the clearing
    /// board, so the two errands cannot disagree about what a free cell is.
    pub(crate) fn free_stockpile_cells(&mut self) -> Option<Vec<Position>> {
        let marked: Vec<(i32, i32)> = self
            .world
            .resource::<BaseGrid>()
            .stockpile_cells()
            .collect();
        if marked.is_empty() {
            return None;
        }
        let floor = self.free_floor();
        let grid = self.world.resource::<BaseGrid>();
        Some(
            marked
                .into_iter()
                .filter(|&c| floor.free(grid, c))
                .map(|(x, y)| Position { x, y })
                .collect(),
        )
    }

    /// `None` when no cell is marked.
    pub(crate) fn tidy_board(&mut self) -> Option<TidyBoard> {
        let cells = self.free_stockpile_cells()?;
        let mut loose: Vec<(Entity, Position)> = self
            .world
            .query::<(Entity, &Position, &FloorItem)>()
            .iter(&self.world)
            .map(|(e, p, _)| (e, *p))
            .collect();
        let grid = self.world.resource::<BaseGrid>();
        loose.retain(|(_, p)| !grid.is_stockpile(p.x, p.y));
        loose.sort_by_key(|(_, p)| (p.x, p.y));
        Some(TidyBoard { cells, loose })
    }

    /// The gates every idle errand shares: on laid floor, no `Task`, not a
    /// study subject, not carrying a program, and on shift by the scheduler's
    /// own test (`is_on_shift`), so a body the assignment is about to free
    /// and set down never picks up what that pass would drop. What each
    /// errand adds is whose load the body may already be holding.
    fn may_run_idle_errand(&self, worker: Entity, amenities: &Amenities) -> bool {
        let w = self.world.get_entity(worker).ok();
        let Some(w) = w else { return false };
        let on_floor = w
            .get::<Position>()
            .is_some_and(|p| self.world.resource::<BaseGrid>().is_floor(p.x, p.y));
        on_floor
            && w.get::<Task>().is_none()
            && w.get::<UnderStudy>().is_none()
            && w.get::<CarryingProgram>().is_none()
            && self.is_on_shift(worker, amenities)
    }

    /// Whether `worker` may spend this beat tidying. A body holding a
    /// hauler's or a clearer's load is mid-trip for somebody else.
    pub(crate) fn may_tidy(&self, worker: Entity, amenities: &Amenities) -> bool {
        self.may_run_idle_errand(worker, amenities) && self.world.get::<Carrying>(worker).is_none()
    }

    /// Whether `worker` may spend this beat clearing: a load it holds must be
    /// the clearing errand's own, and a tidied item is a tidier's trip.
    pub(crate) fn may_clear(&self, worker: Entity, amenities: &Amenities) -> bool {
        self.may_run_idle_errand(worker, amenities)
            && self.world.get::<Tidying>(worker).is_none()
            && (self.world.get::<Carrying>(worker).is_none()
                || self.world.get::<Clearing>(worker).is_some())
    }

    /// One beat of the errand. Returns whether the body spent it.
    ///
    /// **A body is never left holding something it cannot place**: no
    /// reachable free cell, or a route that fails, sets the item down where
    /// it stands, and an item nobody can reach is skipped rather than waited
    /// for.
    pub(crate) fn tidy_step(&mut self, worker: Entity, board: &mut TidyBoard) -> bool {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return false;
        };
        let holding = self.world.get::<Tidying>(worker).is_some();
        if !holding && board.loose.is_empty() {
            return false;
        }
        if holding && board.cells.is_empty() {
            self.drop_load(worker);
            return true;
        }
        if !holding && board.cells.is_empty() {
            return false;
        }
        let blocked = self.blocked_tiles().rigid();
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        let reach = crew_reach(grid, here, &blocked, radius);
        let nearest = |spots: &mut dyn Iterator<Item = (usize, Position)>| {
            spots
                .filter(|&(_, p)| reaches(grid, &reach, here, p, 1, &blocked, radius))
                .min_by_key(|&(_, p)| (chebyshev(here, p), p.x, p.y))
        };
        let dest = nearest(&mut board.cells.iter().copied().enumerate());
        if holding {
            let Some((index, dest)) = dest else {
                self.drop_load(worker);
                return true;
            };
            if at_station(here, dest, 1) {
                let cell = (dest.x, dest.y);
                let free = {
                    let floor = self.free_floor();
                    floor.free(self.world.resource::<BaseGrid>(), cell)
                };
                if free && let Some(Tidying(thing)) = self.world.entity_mut(worker).take() {
                    crate::game::base::floor::spawn_floor_item(&mut self.world, dest, thing);
                }
                board.cells.remove(index);
            } else if self.walk_idle_toward(worker, here, dest).is_err() {
                self.drop_load(worker);
            }
            return true;
        }
        if dest.is_none() {
            return false;
        }
        let loose = nearest(&mut board.loose.iter().map(|&(_, p)| p).enumerate());
        let Some((index, target)) = loose else {
            return false;
        };
        if at_station(here, target, 1) {
            let (item, _) = board.loose.remove(index);
            if self.world.get::<FloorItem>(item).is_some()
                && let Some(thing) = take_floor_item(&mut self.world, item)
            {
                self.world.entity_mut(worker).insert(Tidying(thing));
            }
        } else if self.walk_idle_toward(worker, here, target).is_err() {
            return false;
        }
        true
    }

    /// Sets down what a body holds once the scheduler has handed it a job: a
    /// tidier's item and a clearer's load alike.
    pub(crate) fn set_down_posted_tidiers(&mut self) {
        let mut posted: Vec<Entity> = self
            .world
            .query_filtered::<Entity, (Or<(With<Tidying>, With<Clearing>)>, With<Task>)>()
            .iter(&self.world)
            .collect();
        posted.sort();
        for who in posted {
            self.shelve_posted_clearing_load(who);
            self.drop_load(who);
        }
    }
}
