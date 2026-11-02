//! Carrying a research project's material bill to its Research Station.
//!
//! The bill is fetched by the Station's own posted program through
//! `hauling::haul_step_system`'s research leg and consumed into the project
//! here, in `Game::absorb_research_deliveries`. **The sources are shelves**:
//! Depots and stockpile cells, the set a defence's supply is drawn from, and
//! never a machine's output buffer — that buffer may be feeding the machine
//! beside it, and a production line raided for research is a line that
//! stops. A machine's loose output reaches a shelf anyway, by the Tend and
//! idle clearing, both of which need a Depot or a stockpile cell to carry it
//! to; a base with neither is told so (`work_orders::no_shelf_for`).

use std::collections::BTreeMap;

use bevy_ecs::prelude::Entity;

use crate::base_grid::BaseGrid;
use crate::components::{Carrying, Position, Stock, Structure, Task, TaskKind};
use crate::game::base::{hauling, stock, work_orders};
use crate::items::{FloorThing, ItemId};
use crate::research::ResearchDb;
use crate::resources::ActiveResearch;
use crate::structures::StructureDb;
use crate::{Game, tuning};

/// One tick's view of the active bill for the haulers: what is still to be
/// loaded into any Station's hopper, and what is already on its way there.
///
/// Built once per `haul_step_system` pass and **updated as the pass loads and
/// lifts**, so two Stations' programs arriving on the same tick cannot both
/// fill the last units: the second finds no room and takes its load back to a
/// shelf.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ResearchLedger {
    /// Outstanding less every Station hopper, in bill order.
    left: Vec<(ItemId, u32)>,
    /// Carried by a Station's program toward its Station.
    in_flight: BTreeMap<ItemId, u32>,
}

impl ResearchLedger {
    pub(crate) fn new<'a>(
        outstanding: &[(ItemId, u32)],
        hoppers: impl IntoIterator<Item = &'a BTreeMap<ItemId, u32>>,
        carried: impl IntoIterator<Item = (ItemId, u32)>,
    ) -> Self {
        let mut left = outstanding.to_vec();
        for hopper in hoppers {
            for (item, short) in &mut left {
                *short = short.saturating_sub(hopper.get(item).copied().unwrap_or(0));
            }
        }
        let mut in_flight = BTreeMap::new();
        for (item, qty) in carried {
            *in_flight.entry(item).or_default() += qty;
        }
        Self { left, in_flight }
    }

    /// Whether any line still wants loading.
    pub(crate) fn wants_any(&self) -> bool {
        self.left.iter().any(|(_, n)| *n > 0)
    }

    /// How much of `item` a Station's hopper may take now — the overfill
    /// guard. Zero for an item the bill does not name.
    pub(crate) fn load_room(&self, item: &ItemId) -> u32 {
        self.left
            .iter()
            .find(|(id, _)| id == item)
            .map_or(0, |(_, n)| *n)
    }

    /// How much of `item` an empty-handed program should go and fetch: what
    /// is left once the loads already in flight land, a carry at most.
    pub(crate) fn fetch_want(&self, item: &ItemId) -> u32 {
        self.load_room(item)
            .saturating_sub(self.in_flight.get(item).copied().unwrap_or(0))
            .min(tuning::HAUL_CARRY_CAPACITY)
    }

    /// The bill's items, in bill order.
    pub(crate) fn items(&self) -> impl Iterator<Item = &ItemId> {
        self.left.iter().map(|(id, _)| id)
    }

    /// `qty` of `item` picked up by a Station's program.
    pub(crate) fn lifted(&mut self, item: &ItemId, qty: u32) {
        *self.in_flight.entry(item.clone()).or_default() += qty;
    }

    /// `qty` of `item` loaded into a Station's hopper.
    pub(crate) fn landed(&mut self, item: &ItemId, qty: u32) {
        if let Some((_, n)) = self.left.iter_mut().find(|(id, _)| id == item) {
            *n = n.saturating_sub(qty);
        }
        if let Some(n) = self.in_flight.get_mut(item) {
            *n = n.saturating_sub(qty);
        }
    }
}

impl Game {
    /// Every Research Station, in tile order — what makes the research
    /// currency is what receives the bill.
    pub(crate) fn research_stations(&self) -> Vec<Entity> {
        work_orders::producers_of(self, &self.research_currency())
    }

    /// How much of `item` the bill could still be fed from: every Depot's
    /// shelf and stockpile cell, plus what is already in a Station's hopper
    /// or in a Station program's hands. The blocker and the stall read this
    /// rather than `base_holding`, so the last load on its way is not read as
    /// the base having none, and a machine's own buffer — which the haulers
    /// do not raid — is not read as supply.
    pub(crate) fn research_supply(&self, item: &ItemId) -> u32 {
        let stations = self.research_stations();
        let db = self.world.resource::<StructureDb>();
        let shelved: u32 = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let stock = e.get::<Stock>()?;
                let held = if db.get(kind).is_some_and(|d| d.stores) {
                    stock.output.get(item).copied().unwrap_or(0)
                } else {
                    0
                };
                let hopper = if stations.contains(&e.id()) {
                    stock.input.get(item).copied().unwrap_or(0)
                } else {
                    0
                };
                Some(held + hopper)
            })
            .sum();
        let piled = stock::stockpiled(self)
            .iter()
            .filter(|(_, _, held)| held == item)
            .count() as u32;
        shelved + piled + self.research_in_flight(&stations, item)
    }

    fn research_in_flight(&self, stations: &[Entity], item: &ItemId) -> u32 {
        self.world
            .iter_entities()
            .filter_map(|e| {
                let task = e.get::<Task>()?;
                let load = e.get::<Carrying>()?;
                (task.kind == TaskKind::GatherResource
                    && stations.contains(&task.target)
                    && load.item == *item)
                    .then_some(load.qty)
            })
            .sum()
    }

    /// Whether the base has anywhere to carry a bill from — a Depot or a
    /// stockpile cell.
    pub(crate) fn research_has_shelves(&self) -> bool {
        let db = self.world.resource::<StructureDb>();
        self.world
            .resource::<BaseGrid>()
            .stockpile_cells()
            .next()
            .is_some()
            || self.world.iter_entities().any(|e| {
                e.get::<Structure>()
                    .is_some_and(|s| db.get(&s.kind).is_some_and(|d| d.stores))
            })
    }

    /// Whether a program at `station` could walk to a unit of any of `items`
    /// on a shelf — the staffing half of the research leg. One walk field,
    /// from the Station's first free face, `crew_reach`'s trade: the
    /// scheduler asks it once a tick, and a Station posted with nothing it can
    /// reach would hold the body that should be making the material.
    pub(crate) fn research_bill_reachable(&mut self, station: Entity, items: &[ItemId]) -> bool {
        let Some(at) = self.world.get::<Position>(station).copied() else {
            return false;
        };
        let side = self.structure_footprint_of(station);
        let sources = self.research_sources(items);
        if sources.is_empty() {
            return false;
        }
        let blocked = self.blocked_tiles();
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        let Some(face) = hauling::station_candidates(grid, at, side, |c| blocked.taken(c))
            .first()
            .copied()
        else {
            return false;
        };
        let field = hauling::crew_reach(grid, face, &blocked, radius);
        sources
            .into_iter()
            .any(|(pos, side)| hauling::reaches(grid, &field, face, pos, side, &blocked, radius))
    }

    /// Every shelf holding any of `items`: a Depot's tile and footprint, or a
    /// stockpiled unit's cell.
    fn research_sources(&self, items: &[ItemId]) -> Vec<(Position, u8)> {
        let mut found: Vec<(Position, u8)> = {
            let db = self.world.resource::<StructureDb>();
            self.world
                .iter_entities()
                .filter_map(|e| {
                    let (s, p, stock) = (
                        e.get::<Structure>()?,
                        e.get::<Position>()?,
                        e.get::<Stock>()?,
                    );
                    let def = db.get(&s.kind).filter(|d| d.stores)?;
                    items
                        .iter()
                        .any(|i| stock.output.get(i).copied().unwrap_or(0) > 0)
                        .then_some((*p, def.footprint))
                })
                .collect()
        };
        found.extend(
            stock::stockpiled(self)
                .into_iter()
                .filter(|(_, _, held)| items.contains(held))
                .map(|(p, ..)| (p, 1)),
        );
        found
    }

    /// Moves what haulers have loaded into every Research Station's hopper
    /// into the active project's delivered tally — the tick the units
    /// actually leave the run, so it is where the ledger hears of them — and
    /// hands back anything a hopper holds that the bill does not want.
    ///
    /// Gather (the bill and each hopper, Stations in tile order), decide
    /// (`research::absorb_from_input`, per Station against what is still
    /// outstanding after the ones before it), apply. The haulers' ledger
    /// keeps a surplus from landing; the spill is the guarantee that one
    /// that does — a project finished or abandoned between a load and this
    /// pass — goes back to a shelf rather than sitting where nothing counts
    /// it.
    pub(crate) fn absorb_research_deliveries(&mut self) {
        let active = self.world.resource::<ActiveResearch>().id.clone();
        let materials = active
            .as_ref()
            .and_then(|id| self.world.resource::<ResearchDb>().get(id))
            .map(|d| d.materials.clone())
            .unwrap_or_default();
        for station in self.research_stations() {
            let Some(input) = self.world.get::<Stock>(station).map(|s| s.input.clone()) else {
                continue;
            };
            if input.is_empty() {
                continue;
            }
            let taken = match &active {
                Some(id) => crate::research::absorb_from_input(
                    &self
                        .world
                        .resource::<ActiveResearch>()
                        .outstanding(id, &materials),
                    &input,
                ),
                None => Vec::new(),
            };
            for (item, qty) in &taken {
                self.consume_into_project(
                    station,
                    active.as_deref().unwrap_or_default(),
                    item,
                    *qty,
                );
            }
            self.spill_station_hopper(station);
        }
    }

    fn consume_into_project(&mut self, station: Entity, id: &str, item: &ItemId, qty: u32) {
        if let Some(mut stock) = self.world.get_mut::<Stock>(station) {
            hauling::take_from_input(&mut stock, item, qty);
        }
        *self
            .world
            .resource_mut::<ActiveResearch>()
            .delivered
            .entry(id.to_string())
            .or_default()
            .entry(item.clone())
            .or_default() += qty;
        let name = item.0.clone();
        let qty_logged = qty;
        self.report_base(
            crate::base_ledger::Event::Consume {
                item: item.clone(),
                qty,
            },
            move |tick, zone, _| crate::telemetry::Record::Consume {
                tick,
                zone,
                item: name,
                qty: qty_logged,
                source: crate::base_ledger::ConsumeSource::Base.as_str().to_string(),
            },
        );
    }

    /// Whatever is left in `station`'s hopper goes back to a Depot, or onto
    /// the floor beside the Station — never into the void.
    fn spill_station_hopper(&mut self, station: Entity) {
        let Some(left) = self
            .world
            .get_mut::<Stock>(station)
            .map(|mut s| std::mem::take(&mut s.input))
        else {
            return;
        };
        let at = self
            .world
            .get::<Position>(station)
            .copied()
            .unwrap_or(Position { x: 0, y: 0 });
        for (item, qty) in left {
            let landed = stock::return_to_depots(self, &item, qty);
            let rest = (0..qty - landed)
                .map(|_| FloorThing::Item(item.clone()))
                .collect();
            self.set_down(at, rest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag() -> ItemId {
        ItemId::from("core_fragment")
    }

    #[test]
    fn the_ledger_nets_every_hopper_and_counts_loads_in_flight() {
        let hopper: BTreeMap<ItemId, u32> = [(frag(), 3)].into_iter().collect();
        let other: BTreeMap<ItemId, u32> = [(frag(), 2)].into_iter().collect();
        let ledger = ResearchLedger::new(&[(frag(), 12)], [&hopper, &other], [(frag(), 4)]);
        assert_eq!(ledger.load_room(&frag()), 7);
        assert_eq!(ledger.fetch_want(&frag()), 3);
        assert_eq!(ledger.load_room(&ItemId::from("power_cell")), 0);
    }

    #[test]
    fn a_load_landing_shrinks_the_room_the_next_one_sees() {
        let mut ledger = ResearchLedger::new(&[(frag(), 5)], [], [(frag(), 10)]);
        assert_eq!(ledger.load_room(&frag()), 5);
        ledger.landed(&frag(), 5);
        assert_eq!(ledger.load_room(&frag()), 0);
        assert!(!ledger.wants_any());
        ledger.lifted(&frag(), 1);
        assert_eq!(ledger.fetch_want(&frag()), 0);
    }
}
