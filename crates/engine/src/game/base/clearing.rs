//! Clearing: an idle program carries finished output out of a machine to a
//! Depot with room, or failing that onto free stockpile tiles.
//!
//! The errand rides `drift_idle_staff`'s fall-through above tidying and
//! below every errand with a claim on the body: product sitting in a
//! machine blocks production, and a loose floor item blocks nothing. A base
//! with no Depot and no stockpile cell builds no board and pays nothing,
//! tidy's own rule.
//!
//! **Destination before pickup.** Nothing leaves a machine until a Depot
//! with room that accepts it, or a free stockpile cell, is in reach; with
//! nowhere to go the product stays where it is and the dot on the map says
//! so. Where a held load goes is derived each beat from where the body
//! stands, as a hauler's is, so a destination that fills or falls mid-walk
//! costs a re-derivation rather than a stored target going stale.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{
    Carrying, Clearing, MachineRecipe, Position, StandingJob, Stock, Structure,
};
use crate::game::base::floor::spawn_floor_item;
use crate::game::base::hauling::{
    Consumer, at_station, consumer_beside, crew_reach, deposit, depot_beside_accepting,
    nearest_depot, reaches, take_from, wanted_consumer,
};
use crate::game::base::work_orders;
use crate::items::{FloorThing, ItemId};
use crate::items_db::ItemDb;
use crate::resources::WorkOrders;
use crate::structures::{Placed, StructureDb};
use crate::tuning;

/// The beat's clearing work, shared by every clearer in the pass. A source
/// leaves when a body has taken from it; a Depot leaves when a deposit fills
/// it; a cell leaves when a unit lands on it. Two bodies may still head for
/// the same one — the arrival checks keep that correct, as the tidy board's
/// do.
pub(crate) struct ClearBoard {
    /// Machines holding finished output nobody beside them will take — no
    /// wanted consumer and no accepting Depot with room touching them, unless
    /// the buffer is full — with the items held, `(x, y)`-sorted.
    sources: Vec<(Entity, Position, Vec<ItemId>)>,
    /// Depots with room, `(x, y)`-sorted.
    depots: Vec<(Entity, Position)>,
    /// Free stockpile cells, from `free_stockpile_cells`.
    cells: Vec<Position>,
}

impl ClearBoard {
    /// Whether `machine` is one this beat's clearers may take from.
    #[cfg(test)]
    pub(crate) fn has_source(&self, machine: Entity) -> bool {
        self.sources.iter().any(|&(e, ..)| e == machine)
    }
}

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Game {
    /// `None` when there is no Depot with room and no stockpile cell: nothing
    /// to clear *to*, so no body is asked.
    pub(crate) fn clear_board(&mut self) -> Option<ClearBoard> {
        let cells = self.free_stockpile_cells().unwrap_or_default();
        let standing: Vec<(Entity, Position, String, Vec<ItemId>, u32)> = self
            .world
            .query::<(Entity, &Position, &Structure, &Stock)>()
            .iter(&self.world)
            .map(|(e, p, s, stock)| {
                let held = stock
                    .output
                    .iter()
                    .filter(|(_, n)| **n > 0)
                    .map(|(i, _)| i.clone())
                    .collect();
                (e, *p, s.kind.clone(), held, stock.output_room())
            })
            .collect();
        let mut depots = Vec::new();
        let mut machines = Vec::new();
        let mut consumers: Vec<Consumer> = Vec::new();
        {
            let db = self.world.resource::<StructureDb>();
            let items = self.world.resource::<ItemDb>();
            let needed = work_orders::queue_needs(&self.world.resource::<WorkOrders>().0, items);
            for (e, p, kind, held, room) in standing {
                let Some(def) = db.get(&kind) else { continue };
                if def.stores {
                    if room > 0 {
                        depots.push((e, p));
                    }
                    continue;
                }
                let placed = Placed::of(def, self.world.get::<MachineRecipe>(e));
                let standing_work = self.world.get::<StandingJob>(e).is_some_and(|j| j.work);
                if let Some(recipe) = wanted_consumer(placed, items, &needed, standing_work) {
                    consumers.push((p, recipe));
                }
                if !held.is_empty() {
                    machines.push((e, p, held, room == 0));
                }
            }
        }
        if depots.is_empty() && cells.is_empty() {
            return None;
        }
        depots.sort_by_key(|(_, p)| (p.x, p.y));
        // The Tend's own rule, shared piece by piece: a machine whose product
        // a wanted consumer beside it pulls, or an accepting Depot touching
        // it has room for, is a feed buffer and not a pile — `assembler_system`
        // or `depot_intake_system` is already taking from it, and a body
        // walking it across the base starves the machine next door. A full
        // buffer is the one exception, as it is for the Tend: the neighbour
        // is not keeping up, and the overflow may go.
        let accepts = |depot: Entity, item: &ItemId| self.depot_accepts(depot, item);
        let mut sources: Vec<(Entity, Position, Vec<ItemId>)> = machines
            .into_iter()
            .filter_map(|(e, p, held, clogged)| {
                let items: Vec<ItemId> = held
                    .into_iter()
                    .filter(|item| {
                        clogged
                            || !(consumer_beside(p, item, &consumers)
                                || depot_beside_accepting(p, item, &depots, accepts))
                    })
                    .collect();
                (!items.is_empty()).then_some((e, p, items))
            })
            .collect();
        sources.sort_by_key(|(_, p, _)| (p.x, p.y));
        Some(ClearBoard {
            sources,
            depots,
            cells,
        })
    }

    /// One beat of the errand. Returns whether the body spent it.
    ///
    /// **A body is never left holding something it cannot place**: a load
    /// whose Depot has filled goes to the stockpile, one whose cells have
    /// gone goes to a Depot, and one with neither is set down where the body
    /// stands. Nothing is picked up without a destination in reach.
    pub(crate) fn clear_step(&mut self, worker: Entity, board: &mut ClearBoard) -> bool {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return false;
        };
        let holding = self.world.get::<Clearing>(worker).is_some();
        if holding {
            let Some(load) = self.world.get::<Carrying>(worker).cloned() else {
                // A marker with no load behind it is a finished trip.
                self.world.entity_mut(worker).remove::<Clearing>();
                return false;
            };
            return self.clear_deliver(worker, here, load, board);
        }
        if board.sources.is_empty() {
            return false;
        }
        let blocked = self.blocked_tiles().rigid();
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        let reach = crew_reach(grid, here, &blocked, radius);
        let reachable = |p: Position| reaches(grid, &reach, here, p, 1, &blocked, radius);
        let mut ranked: Vec<usize> = (0..board.sources.len()).collect();
        ranked.sort_by_key(|&i| {
            let p = board.sources[i].1;
            (chebyshev(here, p), p.x, p.y)
        });
        // Destination before pickup: the first source, nearest first, with
        // an item some Depot in reach has room for, else any item at all
        // when a stockpile cell is in reach. A source with neither is left
        // alone and the next is tried.
        let depots_with_room: Vec<(Entity, Position)> = board.depots.clone();
        let cell_room = board.cells.iter().filter(|&&c| reachable(c)).count() as u32;
        let mut pick: Option<(usize, Position, ItemId, u32)> = None;
        'sources: for i in ranked {
            let (_, pos, ref items) = board.sources[i];
            if !reachable(pos) {
                continue;
            }
            for item in items {
                let depot_room = nearest_depot(
                    &depots_with_room
                        .iter()
                        .copied()
                        .filter(|&(d, _)| self.depot_accepts(d, item))
                        .collect::<Vec<_>>(),
                    pos,
                    |_, p| reachable(p),
                )
                .filter(|&(_, p)| reachable(p))
                .and_then(|(d, _)| self.world.get::<Stock>(d))
                .map_or(0, |s| s.output_room());
                let room = depot_room.max(cell_room);
                if room > 0 {
                    pick = Some((i, pos, item.clone(), room));
                    break 'sources;
                }
            }
        }
        let Some((index, target, item, room)) = pick else {
            return false;
        };
        if at_station(here, target, 1) {
            let (machine, ..) = board.sources.remove(index);
            let Some(mut stock) = self.world.get_mut::<Stock>(machine) else {
                return true;
            };
            let qty = take_from(&mut stock, &item, tuning::HAUL_CARRY_CAPACITY.min(room));
            if qty > 0 {
                self.world
                    .entity_mut(worker)
                    .insert((Carrying { item, qty }, Clearing));
            }
            return true;
        }
        self.walk_idle_toward(worker, here, target).is_ok()
    }

    /// The held half of `clear_step`: the nearest Depot in reach with room
    /// that accepts the load, else the nearest free stockpile cell in reach,
    /// else the floor where the body stands.
    fn clear_deliver(
        &mut self,
        worker: Entity,
        here: Position,
        load: Carrying,
        board: &mut ClearBoard,
    ) -> bool {
        let blocked = self.blocked_tiles().rigid();
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        let reach = crew_reach(grid, here, &blocked, radius);
        let reachable = |p: Position| reaches(grid, &reach, here, p, 1, &blocked, radius);
        let candidates: Vec<(Entity, Position)> = board
            .depots
            .iter()
            .copied()
            .filter(|&(d, _)| {
                self.depot_accepts(d, &load.item)
                    && self
                        .world
                        .get::<Stock>(d)
                        .is_some_and(|s| s.output_room() > 0)
            })
            .collect();
        let depot =
            nearest_depot(&candidates, here, |_, p| reachable(p)).filter(|&(_, p)| reachable(p));
        let cell = board
            .cells
            .iter()
            .copied()
            .enumerate()
            .filter(|&(_, c)| reachable(c))
            .min_by_key(|&(_, c)| (chebyshev(here, c), c.x, c.y));
        if let Some((depot, pos)) = depot {
            if at_station(here, pos, 1) {
                let Some(mut stock) = self.world.get_mut::<Stock>(depot) else {
                    return true;
                };
                let moved = deposit(&mut stock, &load);
                if stock.output_room() == 0 {
                    board.depots.retain(|&(d, _)| d != depot);
                }
                self.lighten(worker, &load, moved);
            } else if self.walk_idle_toward(worker, here, pos).is_err() {
                self.drop_load(worker);
            }
            return true;
        }
        if let Some((index, cell_pos)) = cell {
            if at_station(here, cell_pos, 1) {
                let free = {
                    let floor = self.free_floor();
                    floor.free(self.world.resource::<BaseGrid>(), (cell_pos.x, cell_pos.y))
                };
                if free {
                    spawn_floor_item(
                        &mut self.world,
                        cell_pos,
                        FloorThing::Item(load.item.clone()),
                    );
                    self.lighten(worker, &load, 1);
                }
                board.cells.remove(index);
            } else if self.walk_idle_toward(worker, here, cell_pos).is_err() {
                self.drop_load(worker);
            }
            return true;
        }
        self.drop_load(worker);
        true
    }

    /// A clearer the scheduler has posted mid-trip puts its load onto a shelf
    /// with room first — `machine_recipe::empty_machine`'s precedent, through
    /// the same loop (`stock::shelve_on_depots`, here honouring the Depot's
    /// filter because this is a put) — and only what no shelf takes is set
    /// down. Product that was safely in a machine must not become floor
    /// litter nobody counts because a want appeared four beats into a walk;
    /// a trip bound for the stockpile (no shelf had room) still ends on the
    /// floor, where tidy finds it.
    pub(crate) fn shelve_posted_clearing_load(&mut self, who: Entity) {
        if self.world.get::<Clearing>(who).is_none() {
            return;
        }
        let Some(load) = self.world.get::<Carrying>(who).cloned() else {
            return;
        };
        let landed = crate::game::base::stock::shelve_on_depots(
            self,
            &load.item,
            load.qty,
            |game, depot| game.depot_accepts(depot, &load.item),
        );
        self.lighten(who, &load, landed);
    }

    /// Takes `moved` units off the body's load, ending the trip at zero.
    fn lighten(&mut self, worker: Entity, load: &Carrying, moved: u32) {
        let mut body = self.world.entity_mut(worker);
        if moved >= load.qty {
            body.remove::<(Carrying, Clearing)>();
        } else if moved > 0 {
            body.insert(Carrying {
                item: load.item.clone(),
                qty: load.qty - moved,
            });
        }
    }
}
