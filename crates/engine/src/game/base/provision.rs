//! Provisioning: once the base AI owns research nobody is posted to a
//! Research Station, so an idle program carries the running project's
//! outstanding material bill from the base's stores to the AI's station.
//!
//! One more idle errand beside clearing and tidying, riding
//! `drift_idle_staff`'s fall-through above clearing: a bill nobody carries
//! stalls the project, where a pile of product blocks one machine. It credits
//! no research — the load lands in the station's input and
//! `Game::absorb_research_deliveries` is still the one place it is spent.
//! Dormant, ordinary posting fetches the bill and this builds no board.
//!
//! **The bill, the sources and the cap are calls, not copies**: the
//! `ResearchLedger` the posted haulers share, `research_sources` (what the
//! flag and the staffing count), and the same `take_from` and `at_station`.
//! The trip rides the clearing errand's hold, `Carrying` plus `Clearing`, so
//! every path that sets a load down covers it unchanged; a body holding a
//! load the bill no longer wants falls through to `clear_step`, which finds
//! it a home.

use crate::game::base::hauling::Touch;
use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{Carrying, Clearing, Position, Stock, Task, TaskKind};
use crate::game::base::floor::take_floor_item;
use crate::game::base::hauling::{at_station, crew_reach, reaches, take_from};
use crate::game::base::research_supply::{ResearchLedger, Source};
use crate::items::FloorThing;
use crate::research::ResearchDb;
use crate::resources::ActiveResearch;

/// The beat's provisioning work, shared by every provisioner in the pass.
pub(crate) struct ProvisionBoard {
    station: Entity,
    at: Position,
    side: u8,
    /// What the bill still wants, updated as bodies lift and land.
    ledger: ResearchLedger,
    /// Units a body may take, `(x, y)`-sorted.
    sources: Vec<Source>,
}

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Game {
    /// `None` unless the arc owns research, its station stands, and the
    /// active project's bill still wants units.
    pub(crate) fn provision_board(&mut self) -> Option<ProvisionBoard> {
        if !self.storyline_owns_research() {
            return None;
        }
        let station = self.base_ai_station()?;
        let at = self.world.get::<Position>(station).copied()?;
        let side = self.structure_footprint_of(station);
        let outstanding = {
            let research = self.world.resource::<ActiveResearch>();
            let id = research.id.as_ref()?;
            let def = self.world.resource::<ResearchDb>().get(id)?;
            research.outstanding(id, &def.materials)
        };
        let stations = self.research_stations();
        let hoppers: Vec<_> = stations
            .iter()
            .filter_map(|&s| self.world.get::<Stock>(s).map(|st| st.input.clone()))
            .collect();
        // Loads already on their way: a posted station program's, and an
        // idle body's own provisioning trip.
        let carried: Vec<_> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let load = e.get::<Carrying>()?;
                let bound = match e.get::<Task>() {
                    Some(task) => {
                        task.kind == TaskKind::GatherResource && stations.contains(&task.target)
                    }
                    None => e.get::<Clearing>().is_some(),
                };
                bound.then(|| (load.item.clone(), load.qty))
            })
            .collect();
        let ledger = ResearchLedger::new(&outstanding, hoppers.iter(), carried);
        if !ledger.wants_any() {
            return None;
        }
        let items: Vec<_> = ledger.items().cloned().collect();
        let mut sources = self.research_sources(&items);
        sources.sort_by_key(|s| (s.pos.x, s.pos.y, s.entity));
        Some(ProvisionBoard {
            station,
            at,
            side,
            ledger,
            sources,
        })
    }

    /// Whether the station, and each of `board.sources`, can be walked to
    /// from `here`: one walk field for all of them.
    fn provision_reach(&mut self, here: Position, board: &ProvisionBoard) -> (bool, Vec<bool>) {
        let blocked = self.blocked_tiles().rigid();
        let grid = self.world.resource::<BaseGrid>();
        let reach = crew_reach(grid, here, &blocked, grid.radius());
        let reachable =
            |p: Position, side: u8| reaches(grid, &reach, here, p, side, &blocked, Touch::Sides);
        (
            reachable(board.at, board.side),
            board
                .sources
                .iter()
                .map(|s| reachable(s.pos, s.side))
                .collect(),
        )
    }

    /// One beat of the errand. Returns whether the body spent it.
    pub(crate) fn provision_step(&mut self, worker: Entity, board: &mut ProvisionBoard) -> bool {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return false;
        };
        let (station_reached, source_reached) = self.provision_reach(here, board);
        if !station_reached {
            return false;
        }
        match self.world.get::<Carrying>(worker).cloned() {
            Some(load) => self.provision_deliver(worker, here, load, board),
            None => self.provision_fetch(worker, here, board, &source_reached),
        }
    }

    /// The held half: walk the load to the station and land what the bill
    /// still has room for. A load the bill does not want is not this
    /// errand's; `clear_step` finds it a home.
    fn provision_deliver(
        &mut self,
        worker: Entity,
        here: Position,
        load: Carrying,
        board: &mut ProvisionBoard,
    ) -> bool {
        let room = board.ledger.load_room(&load.item);
        if room == 0 {
            return false;
        }
        if at_station(here, board.at, board.side, Touch::Sides) {
            let moved = load.qty.min(room);
            if let Some(mut stock) = self.world.get_mut::<Stock>(board.station) {
                *stock.input.entry(load.item.clone()).or_default() += moved;
            }
            board.ledger.landed(&load.item, moved);
            self.lighten(worker, &load, moved);
        } else if self
            .walk_idle_toward_sized(worker, here, board.at, board.side)
            .is_err()
        {
            self.drop_load(worker);
        }
        true
    }

    /// The empty-handed half: the nearest reachable source of a line the
    /// bill still wants, walked to and lifted from.
    fn provision_fetch(
        &mut self,
        worker: Entity,
        here: Position,
        board: &mut ProvisionBoard,
        reached: &[bool],
    ) -> bool {
        let pick = board
            .sources
            .iter()
            .enumerate()
            .filter(|&(i, s)| reached[i] && board.ledger.fetch_want(&s.item) > 0)
            .min_by_key(|(_, s)| (chebyshev(here, s.pos), s.pos.x, s.pos.y, s.entity))
            .map(|(i, _)| i);
        let Some(index) = pick else {
            return false;
        };
        let (pos, side, entity, item, on_floor) = {
            let s = &board.sources[index];
            (s.pos, s.side, s.entity, s.item.clone(), s.on_floor)
        };
        if !at_station(here, pos, side, Touch::Sides) {
            return self.walk_idle_toward_sized(worker, here, pos, side).is_ok();
        }
        let want = board.ledger.fetch_want(&item);
        let qty = if on_floor {
            // The pile may have been lifted since the board was built.
            match take_floor_item(&mut self.world, entity) {
                Some(FloorThing::Item(_)) => 1,
                _ => 0,
            }
        } else {
            self.world
                .get_mut::<Stock>(entity)
                .map_or(0, |mut stock| take_from(&mut stock, &item, want))
        };
        let source = &mut board.sources[index];
        source.qty = source.qty.saturating_sub(qty.max(1));
        if source.qty == 0 {
            board.sources.remove(index);
        }
        if qty > 0 {
            board.ledger.lifted(&item, qty);
            self.world
                .entity_mut(worker)
                .insert((Carrying { item, qty }, Clearing));
        }
        true
    }
}
