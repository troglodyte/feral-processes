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
    Carrying, CarryingProgram, Downed, FloorItem, OffShift, Position, Task, Tidying, UnderStudy,
};
use crate::game::base::floor::take_floor_item;
use crate::game::base::hauling::{at_station, crew_reach, reaches};
use crate::game::base::offshift::Amenities;

/// The beat's tidying work, both lists `(x, y)`-sorted. Shared by every
/// tidier in the pass and shrunk as bodies claim from it, so two of them are
/// not sent for the same item or the same cell in one beat.
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
    /// `None` when no cell is marked.
    pub(crate) fn tidy_board(&mut self) -> Option<TidyBoard> {
        let marked: Vec<(i32, i32)> = self
            .world
            .resource::<BaseGrid>()
            .stockpile_cells()
            .collect();
        if marked.is_empty() {
            return None;
        }
        let floor = self.free_floor();
        let cells = {
            let grid = self.world.resource::<BaseGrid>();
            marked
                .into_iter()
                .filter(|&c| floor.free(grid, c))
                .map(|(x, y)| Position { x, y })
                .collect()
        };
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

    /// Whether `worker` may spend this beat on the errand. A body holding a
    /// hauler's load or a carried program is mid-trip for somebody else.
    pub(crate) fn may_tidy(&self, worker: Entity, amenities: &Amenities) -> bool {
        let w = self.world.get_entity(worker).ok();
        let Some(w) = w else { return false };
        let on_floor = w
            .get::<Position>()
            .is_some_and(|p| self.world.resource::<BaseGrid>().is_floor(p.x, p.y));
        on_floor
            && w.get::<Task>().is_none()
            && w.get::<Downed>().is_none()
            && w.get::<OffShift>().is_none()
            && w.get::<UnderStudy>().is_none()
            && w.get::<Carrying>().is_none()
            && w.get::<CarryingProgram>().is_none()
            && !self.on_respite(worker, amenities)
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

    /// Sets down what a body holds once the scheduler has handed it a job.
    pub(crate) fn set_down_posted_tidiers(&mut self) {
        let mut posted: Vec<Entity> = self
            .world
            .query_filtered::<Entity, (With<Tidying>, With<Task>)>()
            .iter(&self.world)
            .collect();
        posted.sort();
        for who in posted {
            self.drop_load(who);
        }
    }
}
