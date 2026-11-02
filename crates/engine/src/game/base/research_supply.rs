//! Carrying a research project's material bill to its Research Station.
//!
//! The bill is fetched by the Station's own posted program through
//! `hauling::haul_step_system`'s research leg and consumed into the project
//! here, in `Game::absorb_research_deliveries`. **The sources are what the
//! base holds that no active line is using**: a Depot's shelf and a stockpile
//! cell first, then a machine's buffer whose consuming neighbour stands idle
//! (`buffer_is_free`). Shelves alone stalled a bill whose standing order was
//! satisfied by units sitting in a feed buffer — the order slept on them, and
//! neither the Tend nor idle clearing moves a feed buffer. A buffer an active
//! line is pulling from right now is never raided. And only what a Station's
//! program can **walk to** counts — for fetching, for staffing and for the
//! flag alike (`research_reached`).

use std::collections::{BTreeMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::base_grid::BaseGrid;
use crate::components::{Carrying, MachineRecipe, Position, Stock, Structure, Task, TaskKind};
use crate::game::base::hauling::Consumer;
use crate::game::base::{hauling, stock, work_orders};
use crate::items::{FloorThing, ItemId};
use crate::items_db::ItemDb;
use crate::research::ResearchDb;
use crate::resources::ActiveResearch;
use crate::structures::{Placed, StructureDb};
use crate::systems::intake_recipe;
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

/// Whether a machine at `machine` may give up its buffered `item` to the
/// research bill: no consumer beside it that takes `item` is being worked
/// right now (`staffed`). A buffer an active line is pulling from is the
/// line's; one whose neighbour stands idle is just units the base holds —
/// and since the order that made them reads them as held, nothing else will
/// ever move them. Shared by the haulers and the `Game` side, so the program
/// fetches exactly what the flag and the staffing count.
pub(crate) fn buffer_is_free(machine: Position, item: &ItemId, staffed: &[Consumer]) -> bool {
    !hauling::consumer_beside(machine, item, staffed)
}

/// A unit of the bill a Station's program could pick up.
struct Source {
    pos: Position,
    side: u8,
    item: ItemId,
    qty: u32,
}

impl Game {
    /// Every Research Station, in tile order — what makes the research
    /// currency is what receives the bill.
    pub(crate) fn research_stations(&self) -> Vec<Entity> {
        work_orders::producers_of(self, &self.research_currency())
    }

    /// Every consumer being worked right now: a structure with an intake
    /// recipe and a program posted to it.
    fn staffed_consumers(&self) -> Vec<Consumer> {
        let posted: HashSet<Entity> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let task = e.get::<Task>()?;
                (task.kind == TaskKind::GatherResource).then_some(task.target)
            })
            .collect();
        let db = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        self.world
            .iter_entities()
            .filter(|e| posted.contains(&e.id()))
            .filter_map(|e| {
                let def = db.get(&e.get::<Structure>()?.kind)?;
                let recipe = intake_recipe(Placed::of(def, e.get::<MachineRecipe>()), items)?;
                Some((*e.get::<Position>()?, recipe))
            })
            .collect()
    }

    /// Every unit of `items` a Station's program may take: a Depot's shelf, a
    /// free machine buffer (`buffer_is_free`), or a stockpiled unit. Never a
    /// Station's own hopper.
    fn research_sources(&self, items: &[ItemId]) -> Vec<Source> {
        let stations = self.research_stations();
        let staffed = self.staffed_consumers();
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<Source> = Vec::new();
        for e in self.world.iter_entities() {
            if stations.contains(&e.id()) {
                continue;
            }
            let (Some(s), Some(pos), Some(stock)) =
                (e.get::<Structure>(), e.get::<Position>(), e.get::<Stock>())
            else {
                continue;
            };
            let Some(def) = db.get(&s.kind) else { continue };
            for item in items {
                let qty = stock.output.get(item).copied().unwrap_or(0);
                if qty > 0 && (def.stores || buffer_is_free(*pos, item, &staffed)) {
                    found.push(Source {
                        pos: *pos,
                        side: def.footprint,
                        item: item.clone(),
                        qty,
                    });
                }
            }
        }
        found.extend(
            stock::stockpiled(self)
                .into_iter()
                .filter(|(_, _, held)| items.contains(held))
                .map(|(pos, _, item)| Source {
                    pos,
                    side: 1,
                    item,
                    qty: 1,
                }),
        );
        found
    }

    /// Which of `sources` a program standing at any free face of any of
    /// `stations` could walk to. Structures are walls and bodies are not —
    /// a body is squeezed past (`Occupancy`), and a readout that flickered
    /// with who is standing where would say nothing — so this is `&self`
    /// and the flag, the stall and the staffing all ask it. A face's field
    /// is walked only while some source is still unreached, so the common
    /// base pays for one.
    fn research_reached(&self, stations: &[Entity], sources: &[Source]) -> Vec<bool> {
        let mut reached = vec![false; sources.len()];
        if sources.is_empty() {
            return reached;
        }
        let walls = hauling::blocked_tiles(
            self.structure_footprints()
                .into_iter()
                .filter(|(e, ..)| !self.is_door(*e))
                .map(|(_, p, side)| (p, side)),
            std::iter::empty(),
        );
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        for &station in stations {
            let Some(at) = self.world.get::<Position>(station).copied() else {
                continue;
            };
            let side = self.structure_footprint_of(station);
            for face in hauling::station_candidates(grid, at, side, |c| walls.taken(c)) {
                if reached.iter().all(|r| *r) {
                    return reached;
                }
                let field = hauling::crew_reach(grid, face, &walls, radius);
                for (hit, source) in reached.iter_mut().zip(sources) {
                    *hit = *hit
                        || hauling::reaches(
                            grid,
                            &field,
                            face,
                            source.pos,
                            source.side,
                            &walls,
                            radius,
                        );
                }
            }
        }
        reached
    }

    /// Recomputes the running project's `ActiveResearch::reach` snapshot:
    /// for every outstanding line, what the bill can still be fed from and
    /// what lies out of reach. **The one place the reach is walked for the
    /// views**, once a tick from `settle_research` and again after a select,
    /// an abandon and a load, so the flag is right before the next tick. The
    /// flag, the stall and the readout read the snapshot; the staffing and the
    /// haulers ask `research_reached` live, over the same sources and walls,
    /// so all four agree.
    pub(crate) fn refresh_research_reach(&mut self) {
        let owed: Vec<ItemId> = {
            let research = self.world.resource::<ActiveResearch>();
            research
                .id
                .as_ref()
                .and_then(|id| {
                    self.world
                        .resource::<ResearchDb>()
                        .get(id)
                        .map(|def| research.outstanding(id, &def.materials))
                })
                .unwrap_or_default()
                .into_iter()
                .map(|(item, _)| item)
                .collect()
        };
        let mut reach = crate::research::ResearchReach::default();
        if !owed.is_empty() {
            let stations = self.research_stations();
            let sources = self.research_sources(&owed);
            let reached = self.research_reached(&stations, &sources);
            for item in &owed {
                let on_way = self.research_in_flight(&stations, item)
                    + stations
                        .iter()
                        .filter_map(|&s| self.world.get::<Stock>(s))
                        .map(|s| s.input.get(item).copied().unwrap_or(0))
                        .sum::<u32>();
                reach.supply.insert(item.clone(), on_way);
            }
            for (source, hit) in sources.iter().zip(reached) {
                let into = if hit {
                    &mut reach.supply
                } else {
                    &mut reach.unreachable
                };
                *into.entry(source.item.clone()).or_default() += source.qty;
            }
        }
        self.world.resource_mut::<ActiveResearch>().reach = reach;
    }

    /// How much of `item` the running project's bill can still be fed from:
    /// every unit a Station's program can reach (`research_sources`), plus
    /// what is already in a Station's hopper or in a Station program's hands
    /// — read off the tick's snapshot (`refresh_research_reach`). The flag
    /// and the stall read this rather than `base_holding`, so the last load
    /// on its way is not read as none, and units nobody can walk to — or that
    /// an active line is pulling — are not read as supply.
    pub(crate) fn research_supply(&self, item: &ItemId) -> u32 {
        self.world
            .resource::<ActiveResearch>()
            .reach
            .supply_of(item)
    }

    /// Units of `item` the bill could take but no Station's program can walk
    /// to — what `work_orders::no_reach_for` is about. The snapshot's.
    pub(crate) fn research_unreachable(&self, item: &ItemId) -> u32 {
        self.world
            .resource::<ActiveResearch>()
            .reach
            .unreachable_of(item)
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

    /// Whether a program at `station` could walk to a unit of any of `items`
    /// it may take — the staffing half of the research leg, from every one of
    /// the Station's faces. A Station posted with nothing it can reach would
    /// hold the body that should be making the material.
    pub(crate) fn research_bill_reachable(&self, station: Entity, items: &[ItemId]) -> bool {
        let sources = self.research_sources(items);
        self.research_reached(&[station], &sources)
            .into_iter()
            .any(|hit| hit)
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

    /// Whatever is left in `station`'s hopper once the bill has taken its
    /// share goes back to a Depot that accepts it (`Game::depot_accepts`), or
    /// onto the floor beside the Station — never into the void. What the
    /// Station's own recipe consumes stays: that is its program's fetch, and
    /// spilling it would teleport every load straight back out.
    fn spill_station_hopper(&mut self, station: Entity) {
        let keeps: Vec<ItemId> = {
            let db = self.world.resource::<StructureDb>();
            let items = self.world.resource::<ItemDb>();
            self.world
                .get::<Structure>(station)
                .and_then(|s| db.get(&s.kind))
                .and_then(|def| {
                    intake_recipe(
                        Placed::of(def, self.world.get::<MachineRecipe>(station)),
                        items,
                    )
                })
                .map(|recipe| recipe.into_iter().map(|(item, _)| item).collect())
                .unwrap_or_default()
        };
        let Some(spilled) = self.world.get_mut::<Stock>(station).map(|mut s| {
            let all = std::mem::take(&mut s.input);
            let (kept, spilled): (BTreeMap<_, _>, BTreeMap<_, _>) =
                all.into_iter().partition(|(item, _)| keeps.contains(item));
            s.input = kept;
            spilled
        }) else {
            return;
        };
        let at = self
            .world
            .get::<Position>(station)
            .copied()
            .unwrap_or(Position { x: 0, y: 0 });
        for (item, qty) in spilled {
            let landed = stock::shelve_on_depots(self, &item, qty, |game, depot| {
                game.depot_accepts(depot, &item)
            });
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
