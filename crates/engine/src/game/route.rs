//! Caravan routes: the dispatch doors — reach, the board of known
//! destinations, what a manifest is worth, dispatching, severing and
//! reading a trip.
//!
//! `crate::routes` holds the record and the pure predation geometry; this
//! is `&mut Game` work — the reach check, the refusals, the spend, the
//! save-worthy record push. `game/sortie.rs` is the shape being followed
//! throughout, since both features dispatch from the same Relay.

use rand::RngExt;

use crate::Game;
use crate::components::GlyphColor;
use crate::game::sortie::DispatchReach;
use crate::items::ItemId;
use crate::resources::{self, MessageKind};
use crate::routes::{Route, RouteLeg};
use crate::settlements::relations::Standing;
use crate::settlements::{SettlementKey, Temperament};

/// Why a dispatch or a sever was refused.
///
/// Typed rather than a `String`, `SortieRefusal`'s reason: each of these
/// leaves the player a different errand, and app-core words them for the
/// screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteRefusal {
    NotAtRelay,
    /// `destination` names no settlement the run has discovered yet.
    UnknownDestination,
    /// The town is Hostile and refuses service outright — `refuses_service`.
    Refused,
    /// A standing route was asked for at a town below
    /// `Standing::allows_standing_route` — a one-off dispatch needs only
    /// `!refuses_service`, so this is a stricter gate than `Refused`.
    NoStandingRoutes,
    EmptyManifest,
    Understocked {
        item: ItemId,
        need: u32,
        held: u32,
    },
    /// A route to this destination is already in flight. Refused rather
    /// than queued behind it: `Route` is keyed on its destination, so a
    /// second trip to the same town has nowhere of its own to record
    /// progress that would not collide with the first.
    Duplicate,
    /// `tuning::ROUTE_MAX_ACTIVE` trips are already running, dispatched and
    /// standing combined.
    TooMany,
}

/// One known settlement as a caravan destination — the whole of what
/// `Mode::Dispatch`'s hub needs before a cargo basket is built.
///
/// Not a `views::*` type: the row a manifest picker needs is that screen's
/// own to design (Task 5), and this engine-side shape carries only what a
/// destination itself can say before any cargo is chosen — a manifest's
/// worth is `Game::route_quote`'s, not this row's.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteDestination {
    pub destination: SettlementKey,
    pub name: String,
    pub band: Standing,
    /// The duration `dispatch_route` will actually run — `sortie_duration`'s
    /// rule that a quoted figure and a run figure are one call.
    pub ticks: u64,
}

/// One route in flight, worded for a screen — `Game::sortie_reports`' shape.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteReport {
    pub destination: SettlementKey,
    pub destination_name: String,
    pub standing: bool,
    pub stalled: bool,
    pub leg: RouteLeg,
    pub cargo: Vec<(ItemId, u32)>,
    pub ticks_left: u64,
    pub proceeds: u32,
    pub losses: Vec<String>,
}

/// The glyph and tint a caravan's cargo cue draws with — cosmetic, and
/// its own rather than any creature's `Glyph` since cargo has none.
pub(crate) const ROUTE_CARGO_GLYPH: char = '$';
pub(crate) const ROUTE_CARGO_COLOR: GlyphColor = GlyphColor::Yellow;

impl Game {
    /// Chebyshev distance from the base anchor to `destination`, in ticks —
    /// `sortie_duration`'s counterpart for a route: **one computation**,
    /// quoted by `route_destinations` and run by `dispatch_route`, so a
    /// screen and the countdown it starts cannot disagree.
    pub(crate) fn route_duration(anchor: (i32, i32), destination: (i32, i32)) -> u64 {
        let d = (anchor.0 - destination.0)
            .abs()
            .max((anchor.1 - destination.1).abs()) as u64;
        crate::tuning::ROUTE_TICKS_BASE + crate::tuning::ROUTE_TICKS_PER_TILE * d
    }

    /// What `cargo` is worth, sold at `temperament` — the one derivation a
    /// preview and a sale share, `Game::sortie_duration`'s rule again: a
    /// quoted figure and a granted figure may not differ, which is the
    /// point of a shared function rather than a comment claiming they
    /// match.
    pub fn route_quote(&self, cargo: &[(ItemId, u32)], temperament: Temperament) -> u32 {
        cargo
            .iter()
            .map(|(item, qty)| self.settlement_sell_price(item, temperament) * qty)
            .sum()
    }

    /// Every settlement the run has discovered, as a caravan destination —
    /// three-state exactly as `board_defs`: `None` for no Relay,
    /// `Some(vec![])` for a Relay with no known settlement reachable yet,
    /// `Some(rows)` otherwise.
    pub fn route_destinations(&mut self) -> Option<Vec<RouteDestination>> {
        if self.dispatch_reach() == DispatchReach::NoRelay {
            return None;
        }
        let anchor = self.anchor_position().unwrap_or((0, 0));
        let known: Vec<(SettlementKey, (i32, i32), String)> = self
            .world
            .resource::<resources::Settlements>()
            .0
            .iter()
            .map(|(key, settlement)| (*key, settlement.tile, settlement.def.name.clone()))
            .collect();
        Some(
            known
                .into_iter()
                .map(|(destination, tile, name)| RouteDestination {
                    destination,
                    name,
                    band: self.standing_band(destination),
                    ticks: Self::route_duration(anchor, tile),
                })
                .collect(),
        )
    }

    /// Sends a caravan out to `destination` carrying `cargo`, standing or
    /// one-off.
    ///
    /// Every refusal lands **before anything is spent**,
    /// `dispatch_sortie`'s rule, asserted per refusal in
    /// `tests::routes::every_refusal_leaves_stock_and_routes_exactly_as_they_were`.
    /// The record stores the **whole resolved destination**, never the key
    /// alone — a town's def edited between sessions must not be able to
    /// rewrite a trip already in flight, `SortieSave::site`'s reason.
    pub fn dispatch_route(
        &mut self,
        destination: SettlementKey,
        cargo: Vec<(ItemId, u32)>,
        standing: bool,
    ) -> Result<(), RouteRefusal> {
        if self.dispatch_reach() != DispatchReach::AtRelay {
            return Err(RouteRefusal::NotAtRelay);
        }
        let Some(known) = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&destination)
            .cloned()
        else {
            return Err(RouteRefusal::UnknownDestination);
        };
        let band = self.standing_band(destination);
        if band.refuses_service() {
            return Err(RouteRefusal::Refused);
        }
        if standing && !band.allows_standing_route() {
            return Err(RouteRefusal::NoStandingRoutes);
        }
        if self
            .world
            .resource::<resources::Routes>()
            .0
            .iter()
            .any(|r| r.destination == destination)
        {
            return Err(RouteRefusal::Duplicate);
        }
        if self.world.resource::<resources::Routes>().0.len() >= crate::tuning::ROUTE_MAX_ACTIVE {
            return Err(RouteRefusal::TooMany);
        }
        if cargo.is_empty() {
            return Err(RouteRefusal::EmptyManifest);
        }
        for (item, qty) in &cargo {
            let held = crate::game::base::work_orders::base_holding(self, item);
            if held < *qty {
                return Err(RouteRefusal::Understocked {
                    item: item.clone(),
                    need: *qty,
                    held,
                });
            }
        }

        for (item, qty) in &cargo {
            crate::game::base::stock::spend_from_base(
                self,
                item,
                *qty,
                crate::base_ledger::ConsumeSource::Base,
            );
        }
        let anchor = self.anchor_position().unwrap_or((0, 0));
        let ticks = Self::route_duration(anchor, known.tile);
        self.queue_cargo_walk(true);
        let name = known.def.name.clone();
        self.world
            .resource_mut::<resources::Routes>()
            .0
            .push(Route {
                destination,
                destination_def: known.def,
                destination_tile: known.tile,
                cargo,
                standing,
                stalled: false,
                leg: RouteLeg::Outbound,
                ticks_total: ticks,
                ticks_elapsed: 0,
                proceeds: 0,
                losses: Vec::new(),
            });
        self.log_base(format!("A caravan departs for {name}."));
        Ok(())
    }

    /// Clears `standing` on the route running to `destination`, if one is
    /// both in flight and still standing. Returns whether anything was
    /// cleared.
    ///
    /// **Clears `standing` and nothing else** — the trip already in flight
    /// still completes and still pays, through the ordinary tick. No
    /// refund path, no cargo teleport.
    pub fn sever_route(&mut self, destination: SettlementKey) -> bool {
        let cleared = {
            let mut routes = self.world.resource_mut::<resources::Routes>();
            match routes.0.iter_mut().find(|r| r.destination == destination) {
                Some(route) if route.standing => {
                    route.standing = false;
                    true
                }
                _ => false,
            }
        };
        if cleared {
            let name = self.settlement_name(destination);
            self.log_base(format!("You cut the standing arrangement with {name}."));
        }
        cleared
    }

    /// Every trip currently in flight, worded for a screen —
    /// `Game::sortie_reports`' shape: `&self`, and derives nothing back
    /// into the world, so a screen that draws it twice cannot move a trip.
    pub fn route_reports(&self) -> Vec<RouteReport> {
        self.world
            .resource::<resources::Routes>()
            .0
            .iter()
            .map(|r| RouteReport {
                destination: r.destination,
                destination_name: r.destination_def.name.clone(),
                standing: r.standing,
                stalled: r.stalled,
                leg: r.leg,
                cargo: r.cargo.clone(),
                ticks_left: r.ticks_total.saturating_sub(r.ticks_elapsed),
                proceeds: r.proceeds,
                losses: r.losses.clone(),
            })
            .collect()
    }

    /// A caravan's own dispatch/arrival cue — `queue_squad_walk`'s per-member
    /// walk with no entity to read a tile from, so it walks between the door
    /// and whichever Depot the cargo would actually leave from or land at,
    /// the same set `game::base::stock::return_to_depots` fills on arrival.
    /// No Depot standing draws no cue at all — `transit_path`'s "a walk that
    /// does not exist is nothing" rule, one level up.
    pub(crate) fn queue_cargo_walk(&mut self, outbound: bool) {
        let Some(depot) = self.nearest_depot_tile() else {
            return;
        };
        let door = crate::game::base_space::BASE_EXIT_CELL;
        let (from, to) = if outbound {
            (depot, door)
        } else {
            (door, depot)
        };
        self.queue_transit_walk(ROUTE_CARGO_GLYPH, ROUTE_CARGO_COLOR, from, to);
    }

    /// The base's own Depot standing lowest in tile order —
    /// `game::base::stock::spend_from_base`'s own draw order, reused here
    /// purely for a deterministic pick: the cue is cosmetic, and any
    /// consistent choice is as good as any other.
    fn nearest_depot_tile(&self) -> Option<(i32, i32)> {
        let structures = self.world.resource::<crate::structures::StructureDb>();
        let mut depots: Vec<(i32, i32)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let structure = e.get::<crate::components::Structure>()?;
                let def = structures.get(&structure.kind)?;
                if !def.stores {
                    return None;
                }
                let pos = e.get::<crate::components::Position>()?;
                Some((pos.x, pos.y))
            })
            .collect();
        depots.sort_unstable();
        depots.into_iter().next()
    }
}

impl Game {
    /// One tick of every trip currently in flight — `run_sorties`' shape and
    /// its guard: a route's completion (a sale, a deposit) is exactly the
    /// "the world may change here" case that guard exists for.
    pub(crate) fn run_routes(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if self.world.resource::<resources::Routes>().0.is_empty() {
            return;
        }
        let mut index = 0;
        while index < self.world.resource::<resources::Routes>().0.len() {
            // A trip that came home removed its own record, so the record
            // behind it has slid into this index and must not be skipped —
            // `step_sortie`'s reason for reading the step rather than the
            // length.
            if !self.step_route(index) {
                index += 1;
            }
        }
    }

    /// Advances one trip by a tick. Returns whether the record at `index`
    /// was dropped for good.
    fn step_route(&mut self, index: usize) -> bool {
        if self.world.resource::<resources::Routes>().0[index].stalled {
            // The countdown does not move while stalled — it is parked at
            // the inbound-complete point, and every tick just retries the
            // reload rather than re-running predation and the deposit a
            // second time.
            self.try_reload_route(index);
            return false;
        }
        let (elapsed, total, leg) = {
            let route = &mut self.world.resource_mut::<resources::Routes>().0[index];
            route.ticks_elapsed += 1;
            (route.ticks_elapsed, route.ticks_total, route.leg)
        };
        if elapsed < total {
            return false;
        }
        match leg {
            RouteLeg::Outbound => {
                self.complete_outbound_leg(index);
                false
            }
            RouteLeg::Inbound => self.complete_inbound_leg(index),
        }
    }

    /// The outbound leg lands: predation against the cargo, the sale of
    /// what survives at the destination's own price, and standing paid on
    /// the turnover — then the trip turns around.
    ///
    /// **`Route::cargo` is never mutated by predation** — only a local copy
    /// is, which is what lets a standing route's next departure keep asking
    /// for the manifest it was given rather than one that shrinks a little
    /// on every trip a predator catches.
    fn complete_outbound_leg(&mut self, index: usize) {
        let (anchor, destination_tile, destination, temperament, mut surviving) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            (
                self.anchor_position().unwrap_or((0, 0)),
                route.destination_tile,
                route.destination,
                route.destination_def.temperament,
                route.cargo.clone(),
            )
        };
        let losses =
            self.roll_cargo_predation(anchor, destination_tile, destination, &mut surviving);
        let proceeds = self.route_quote(&surviving, temperament);
        self.credit_trade_volume(destination, proceeds);
        let name = self.settlement_name(destination);
        let currency = self.trade_currency();
        let currency_name = self.item_name(&currency).to_string();
        self.log_base_kind(
            MessageKind::Loot,
            format!("The caravan sells its cargo at {name} for {proceeds} {currency_name}."),
        );
        let route = &mut self.world.resource_mut::<resources::Routes>().0[index];
        route.proceeds = proceeds;
        route.leg = RouteLeg::Inbound;
        route.ticks_elapsed = 0;
        route.losses = losses;
    }

    /// The inbound leg lands: predation against the proceeds, the deposit
    /// of whatever survives into base stock, then a reload (standing, stock
    /// allowing), a stall, or dropping the record for good. Returns whether
    /// the record was dropped.
    fn complete_inbound_leg(&mut self, index: usize) -> bool {
        let (anchor, destination_tile, destination, mut proceeds) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            (
                self.anchor_position().unwrap_or((0, 0)),
                route.destination_tile,
                route.destination,
                route.proceeds,
            )
        };
        let losses =
            self.roll_proceeds_predation(anchor, destination_tile, destination, &mut proceeds);
        let currency = self.trade_currency();
        let landed = crate::game::base::stock::return_to_depots(self, &currency, proceeds);
        let name = self.settlement_name(destination);
        let currency_name = self.item_name(&currency).to_string();
        self.log_base_kind(
            MessageKind::Loot,
            format!("The caravan returns from {name} with {landed} {currency_name}."),
        );
        self.queue_cargo_walk(false);
        self.world.resource_mut::<resources::Routes>().0[index].losses = losses;

        let standing = self.world.resource::<resources::Routes>().0[index].standing;
        if !standing {
            self.world
                .resource_mut::<resources::Routes>()
                .0
                .remove(index);
            return true;
        }
        self.try_reload_route(index);
        false
    }

    /// Attempts to reload a route's own manifest from base stock and send
    /// it out again — the initial attempt at inbound completion, and every
    /// stalled tick's retry.
    ///
    /// Marks `stalled` on failure rather than dropping or severing the
    /// record — a stalled work order's rule, retried every tick rather than
    /// given up on.
    fn try_reload_route(&mut self, index: usize) {
        let (cargo, destination) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            (route.cargo.clone(), route.destination)
        };
        let ok = cargo
            .iter()
            .all(|(item, qty)| crate::game::base::work_orders::base_holding(self, item) >= *qty);
        if !ok {
            self.world.resource_mut::<resources::Routes>().0[index].stalled = true;
            return;
        }
        for (item, qty) in &cargo {
            crate::game::base::stock::spend_from_base(
                self,
                item,
                *qty,
                crate::base_ledger::ConsumeSource::Base,
            );
        }
        self.queue_cargo_walk(true);
        let route = &mut self.world.resource_mut::<resources::Routes>().0[index];
        route.leg = RouteLeg::Outbound;
        route.ticks_elapsed = 0;
        route.proceeds = 0;
        route.stalled = false;
        let name = self.settlement_name(destination);
        self.log_base(format!("The caravan reloads and departs again for {name}."));
    }

    /// Every known settlement close enough to this trip's segment, and
    /// Hostile enough, to try preying on it — `routes::settlements_near_route`
    /// filtered to `Standing::preys_on_routes`, the module doc's own
    /// requirement of the caller.
    fn route_predators(&self, base: (i32, i32), destination: (i32, i32)) -> Vec<SettlementKey> {
        let candidates: Vec<(SettlementKey, (i32, i32))> = self
            .world
            .resource::<resources::Settlements>()
            .0
            .iter()
            .filter(|(key, _)| self.standing_band(**key).preys_on_routes())
            .map(|(key, settlement)| (*key, settlement.tile))
            .collect();
        crate::routes::settlements_near_route(&candidates, base, destination)
    }

    /// Rolls every predator near this trip against `cargo` in place,
    /// reducing each line by `ROUTE_PREDATION_LOSS` on a hit, and returns
    /// one line of narration per hit — also logged as it happens.
    ///
    /// **The only place this feature draws `resources::GameRng`**, and only
    /// once nothing has filtered a predator out — an empty `predators` rolls
    /// nothing at all.
    fn roll_cargo_predation(
        &mut self,
        base: (i32, i32),
        destination: (i32, i32),
        destination_key: SettlementKey,
        cargo: &mut [(ItemId, u32)],
    ) -> Vec<String> {
        let predators = self.route_predators(base, destination);
        let mut losses = Vec::new();
        for predator in predators {
            let hit = self
                .world
                .resource_mut::<resources::GameRng>()
                .0
                .random_bool(crate::tuning::ROUTE_PREDATION_CHANCE as f64);
            if !hit {
                continue;
            }
            let mut taken_units = 0u32;
            for (_, qty) in cargo.iter_mut() {
                let take = (*qty as f32 * crate::tuning::ROUTE_PREDATION_LOSS) as u32;
                *qty -= take;
                taken_units += take;
            }
            let predator_name = self.settlement_name(predator);
            let dest_name = self.settlement_name(destination_key);
            let line = format!(
                "{predator_name} raids the caravan bound for {dest_name}, seizing {taken_units} units of cargo."
            );
            self.log_base_kind(MessageKind::Outcome, line.clone());
            losses.push(line);
        }
        losses
    }

    /// The same roll against `proceeds` — `ROUTE_PREDATION_LOSS` of the
    /// figure taken per hit, in place. See `roll_cargo_predation` for the
    /// draw itself.
    fn roll_proceeds_predation(
        &mut self,
        base: (i32, i32),
        destination: (i32, i32),
        destination_key: SettlementKey,
        proceeds: &mut u32,
    ) -> Vec<String> {
        let predators = self.route_predators(base, destination);
        let mut losses = Vec::new();
        let currency = self.trade_currency();
        let currency_name = self.item_name(&currency).to_string();
        for predator in predators {
            let hit = self
                .world
                .resource_mut::<resources::GameRng>()
                .0
                .random_bool(crate::tuning::ROUTE_PREDATION_CHANCE as f64);
            if !hit {
                continue;
            }
            let take = (*proceeds as f32 * crate::tuning::ROUTE_PREDATION_LOSS) as u32;
            *proceeds -= take;
            let predator_name = self.settlement_name(predator);
            let dest_name = self.settlement_name(destination_key);
            let line = format!(
                "{predator_name} tolls the caravan home from {dest_name}, taking {take} {currency_name}."
            );
            self.log_base_kind(MessageKind::Outcome, line.clone());
            losses.push(line);
        }
        losses
    }
}
