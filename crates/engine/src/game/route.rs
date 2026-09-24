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
use crate::routes::{Route, RouteEnd, RouteLeg};
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
    /// `destination` names no settlement the run has discovered yet, or (for
    /// `Game::dispatch_outpost_route`) no outpost stands at the tile given.
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

/// Which destination a hub row or an in-flight report names — the
/// lightweight, `Copy` id these two picker-facing shapes key on, as opposed
/// to `RouteEnd`, which is the whole resolved endpoint a dispatched `Route`
/// stores. Widening this rather than `RouteEnd` itself is what let outposts
/// join the hub without touching the dispatch/predation machinery in this
/// module's lower half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteDestinationId {
    Settlement(SettlementKey),
    Outpost((i32, i32)),
}

/// One known destination — settlement or outpost — as a caravan target: the
/// whole of what `Mode::Dispatch`'s hub needs before a cargo basket is
/// built.
///
/// Not a `views::*` type: the row a manifest picker needs is that screen's
/// own to design (Task 5), and this engine-side shape carries only what a
/// destination itself can say before any cargo is chosen — a manifest's
/// worth is `Game::route_quote`'s, not this row's.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteDestination {
    pub destination: RouteDestinationId,
    pub name: String,
    /// `None` for an outpost — it carries no diplomatic standing to show.
    pub band: Option<Standing>,
    /// The duration `dispatch_route`/`dispatch_outpost_route` will actually
    /// run — `sortie_duration`'s rule that a quoted figure and a run figure
    /// are one call.
    pub ticks: u64,
}

/// One route in flight, worded for a screen — `Game::sortie_reports`' shape.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteReport {
    pub destination: RouteDestinationId,
    pub destination_name: String,
    pub standing: bool,
    pub stalled: bool,
    pub leg: RouteLeg,
    pub cargo: Vec<(ItemId, u32)>,
    pub ticks_left: u64,
    pub proceeds: u32,
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

    /// `route_quote` with `destination`'s own `Temperament` resolved
    /// internally — the cargo picker's live preview crosses the app-core/
    /// engine seam with no `Temperament` of its own to carry, so this is the
    /// one door that resolves it rather than leaking the type out. `None`
    /// for a destination the run has not discovered.
    pub fn route_manifest_quote(
        &self,
        destination: SettlementKey,
        cargo: &[(ItemId, u32)],
    ) -> Option<u32> {
        let temperament = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&destination)?
            .def
            .temperament;
        Some(self.route_quote(cargo, temperament))
    }

    /// Every settlement the run has discovered, plus every founded outpost,
    /// as a caravan destination — three-state exactly as `board_defs`: `None`
    /// for no Relay, `Some(vec![])` for a Relay with no destination reachable
    /// yet, `Some(rows)` otherwise. Settlements first, then outposts in
    /// `resources::Outposts`' own key order — the traps precedent.
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
        let mut destinations: Vec<RouteDestination> = known
            .into_iter()
            .map(|(destination, tile, name)| RouteDestination {
                destination: RouteDestinationId::Settlement(destination),
                name,
                band: Some(self.standing_band(destination)),
                ticks: Self::route_duration(anchor, tile),
            })
            .collect();
        let outpost_tiles: Vec<(i32, i32)> = self
            .world
            .resource::<resources::Outposts>()
            .0
            .keys()
            .copied()
            .collect();
        destinations.extend(outpost_tiles.into_iter().map(|tile| RouteDestination {
            destination: RouteDestinationId::Outpost(tile),
            name: self.outpost_destination_name(tile),
            band: None,
            ticks: Self::route_duration(anchor, tile),
        }));
        Some(destinations)
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
            .any(|r| r.destination.tile() == known.tile)
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
                destination: RouteEnd::Settlement {
                    key: destination,
                    def: known.def,
                    tile: known.tile,
                },
                cargo,
                standing,
                stalled: false,
                leg: RouteLeg::Outbound,
                ticks_total: ticks,
                ticks_elapsed: 0,
                proceeds: 0,
            });
        self.log_base(format!("A caravan departs for {name}."));
        Ok(())
    }

    /// The one-line sentence `views::OutpostReport::route` shows for the
    /// caravan route bound to `tile`, if one exists — `Trend::reason`'s own
    /// convention that the screen builds no prose of its own. `None` when
    /// no route runs there. `Game::route_reports` also carries this trip for
    /// the hub's own "in flight" list — the two surfaces read the same live
    /// record and cannot disagree.
    pub(crate) fn outpost_route_line(&self, tile: (i32, i32)) -> Option<String> {
        let route = self
            .world
            .resource::<resources::Routes>()
            .0
            .iter()
            .find(|r| matches!(&r.destination, RouteEnd::Outpost(t) if *t == tile))?;
        if route.stalled {
            return Some("A caravan can't reach it right now — nothing stands there.".to_string());
        }
        let ticks_left = route.ticks_total.saturating_sub(route.ticks_elapsed);
        Some(match route.leg {
            RouteLeg::Outbound => {
                format!("A caravan is {ticks_left} ticks out, coming to collect its stock.")
            }
            RouteLeg::Inbound => {
                let units: u32 = route.cargo.iter().map(|(_, qty)| *qty).sum();
                format!("A caravan is hauling {units} units home, {ticks_left} ticks out.")
            }
        })
    }

    /// `dispatch_route`'s twin for an outpost endpoint — design spec §7.
    ///
    /// Carries no manifest and spends no base stock up front: the outbound
    /// leg sells nothing, so there is nothing to refuse for want of stock —
    /// `EmptyManifest`/`Understocked` simply do not apply to this door.
    pub fn dispatch_outpost_route(
        &mut self,
        tile: (i32, i32),
        standing: bool,
    ) -> Result<(), RouteRefusal> {
        if self.dispatch_reach() != DispatchReach::AtRelay {
            return Err(RouteRefusal::NotAtRelay);
        }
        if !self
            .world
            .resource::<resources::Outposts>()
            .0
            .contains_key(&tile)
        {
            return Err(RouteRefusal::UnknownDestination);
        }
        if self
            .world
            .resource::<resources::Routes>()
            .0
            .iter()
            .any(|r| r.destination.tile() == tile)
        {
            return Err(RouteRefusal::Duplicate);
        }
        if self.world.resource::<resources::Routes>().0.len() >= crate::tuning::ROUTE_MAX_ACTIVE {
            return Err(RouteRefusal::TooMany);
        }
        let anchor = self.anchor_position().unwrap_or((0, 0));
        let ticks = Self::route_duration(anchor, tile);
        self.queue_cargo_walk(true);
        self.world
            .resource_mut::<resources::Routes>()
            .0
            .push(Route {
                destination: RouteEnd::Outpost(tile),
                cargo: Vec::new(),
                standing,
                stalled: false,
                leg: RouteLeg::Outbound,
                ticks_total: ticks,
                ticks_elapsed: 0,
                proceeds: 0,
            });
        self.log_base("A caravan departs for the outpost.".to_string());
        Ok(())
    }

    /// Clears `standing` on the route running to `destination` — settlement
    /// or outpost — if one is both in flight and still standing. Returns
    /// whether anything was cleared.
    ///
    /// **Clears `standing` and nothing else** — the trip already in flight
    /// still completes and still pays, through the ordinary tick. No
    /// refund path, no cargo teleport.
    pub fn sever_route(&mut self, destination: RouteDestinationId) -> bool {
        let cleared = {
            let mut routes = self.world.resource_mut::<resources::Routes>();
            let found = routes.0.iter_mut().find(|r| match destination {
                RouteDestinationId::Settlement(key) => r.destination.settlement_key() == Some(key),
                RouteDestinationId::Outpost(tile) => {
                    matches!(r.destination, RouteEnd::Outpost(t) if t == tile)
                }
            });
            match found {
                Some(route) if route.standing => {
                    route.standing = false;
                    true
                }
                _ => false,
            }
        };
        if cleared {
            let name = match destination {
                RouteDestinationId::Settlement(key) => self.settlement_name(key),
                RouteDestinationId::Outpost(tile) => self.outpost_destination_name(tile),
            };
            self.log_base(format!("You cut the standing arrangement with {name}."));
        }
        cleared
    }

    /// Every trip currently in flight, settlement or outpost, worded for a
    /// screen — `Game::sortie_reports`' shape: `&self`, and derives nothing
    /// back into the world, so a screen that draws it twice cannot move a
    /// trip.
    ///
    /// An outpost route's status is *also* read off `Game::outpost_report
    /// (tile).route` for the outpost's own screen — the two surfaces read
    /// the same live record and cannot disagree.
    pub fn route_reports(&self) -> Vec<RouteReport> {
        self.world
            .resource::<resources::Routes>()
            .0
            .iter()
            .map(|r| {
                let (destination, destination_name) = match &r.destination {
                    RouteEnd::Settlement { key, def, .. } => {
                        (RouteDestinationId::Settlement(*key), def.name.clone())
                    }
                    RouteEnd::Outpost(tile) => (
                        RouteDestinationId::Outpost(*tile),
                        self.outpost_destination_name(*tile),
                    ),
                };
                RouteReport {
                    destination,
                    destination_name,
                    standing: r.standing,
                    stalled: r.stalled,
                    leg: r.leg,
                    cargo: r.cargo.clone(),
                    ticks_left: r.ticks_total.saturating_sub(r.ticks_elapsed),
                    proceeds: r.proceeds,
                }
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

    /// The base's own Depot standing lowest in tile order — a deterministic
    /// pick and nothing more: the cue is cosmetic, and any consistent choice
    /// is as good as any other.
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
            // the inbound-complete (or, for an outpost, the arrival) point,
            // and every tick just retries rather than re-running predation
            // and the deposit a second time.
            return self.retry_stalled_route(index);
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

    /// Dispatches a stalled route's retry to the right endpoint —
    /// `RouteEnd`'s own extension point: a settlement retries its reload
    /// (dry stock), an outpost retries its pickup (the record went missing).
    fn retry_stalled_route(&mut self, index: usize) -> bool {
        match self.world.resource::<resources::Routes>().0[index].destination {
            RouteEnd::Settlement { .. } => self.try_reload_route(index),
            RouteEnd::Outpost(tile) => self.try_outpost_pickup(index, tile),
        }
    }

    /// The outbound leg lands — dispatched by endpoint kind, `retry_stalled_route`'s
    /// own split. A settlement sells what survives predation; an outpost
    /// carries nothing outbound and loads its stock instead (design spec §7).
    fn complete_outbound_leg(&mut self, index: usize) {
        match self.world.resource::<resources::Routes>().0[index].destination {
            RouteEnd::Settlement { .. } => self.complete_settlement_outbound_leg(index),
            RouteEnd::Outpost(tile) => {
                self.try_outpost_pickup(index, tile);
            }
        }
    }

    /// The inbound leg lands — dispatched by endpoint kind.
    fn complete_inbound_leg(&mut self, index: usize) -> bool {
        match self.world.resource::<resources::Routes>().0[index].destination {
            RouteEnd::Settlement { .. } => self.complete_settlement_inbound_leg(index),
            RouteEnd::Outpost(tile) => self.complete_outpost_inbound_leg(index, tile),
        }
    }

    /// A settlement's outbound leg lands: predation against the cargo, the
    /// sale of what survives at the destination's own price, and standing
    /// paid on the turnover — then the trip turns around.
    ///
    /// **`Route::cargo` is never mutated by predation** — only a local copy
    /// is, which is what lets a standing route's next departure keep asking
    /// for the manifest it was given rather than one that shrinks a little
    /// on every trip a predator catches.
    fn complete_settlement_outbound_leg(&mut self, index: usize) {
        let (anchor, destination_tile, destination, temperament, mut surviving) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            let RouteEnd::Settlement { key, def, tile } = &route.destination else {
                unreachable!("complete_settlement_outbound_leg is the Settlement arm")
            };
            (
                self.anchor_position().unwrap_or((0, 0)),
                *tile,
                *key,
                def.temperament,
                route.cargo.clone(),
            )
        };
        let name = self.settlement_name(destination);
        self.roll_cargo_predation(anchor, destination_tile, &name, &mut surviving);
        let proceeds = self.route_quote(&surviving, temperament);
        self.credit_trade_volume(destination, proceeds);
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
    }

    /// A settlement's inbound leg lands: predation against the proceeds, the
    /// deposit of whatever survives into base stock, then a reload
    /// (standing, stock allowing), a stall, or dropping the record for
    /// good. Returns whether the record was dropped.
    fn complete_settlement_inbound_leg(&mut self, index: usize) -> bool {
        let (anchor, destination_tile, destination, mut proceeds) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            let RouteEnd::Settlement { key, tile, .. } = &route.destination else {
                unreachable!("complete_settlement_inbound_leg is the Settlement arm")
            };
            (
                self.anchor_position().unwrap_or((0, 0)),
                *tile,
                *key,
                route.proceeds,
            )
        };
        self.roll_proceeds_predation(anchor, destination_tile, destination, &mut proceeds);
        let currency = self.trade_currency();
        let name = self.settlement_name(destination);
        let currency_name = self.item_name(&currency).to_string();
        self.log_base_kind(
            MessageKind::Loot,
            format!("The caravan returns from {name} with {proceeds} {currency_name}."),
        );
        // Depot first, the player's pack second, a log line for anything
        // that fits nowhere — `Game::return_material`'s own rule. Calling
        // `stock::return_to_depots` alone here silently destroyed whatever a
        // Depot-less base could not catch, Finding 1 of the 2026-09-05
        // whole-branch review.
        self.return_material(&currency, proceeds);
        self.queue_cargo_walk(false);

        self.try_reload_route(index)
    }

    /// Attempts to reload a settlement route's own manifest from base stock
    /// and send it out again — the initial attempt at inbound completion,
    /// and every stalled tick's retry. Returns whether the record was
    /// dropped for good.
    ///
    /// **Reads `standing` first**, `complete_settlement_inbound_leg`'s own
    /// check moved in here: a stalled route that gets severed is parked at
    /// home with its proceeds already deposited, not in flight, so severing
    /// it must drop the record rather than leave it consuming a
    /// `ROUTE_MAX_ACTIVE` slot forever (stock never returns) or send it out
    /// on one more round trip the player refused (stock does return) —
    /// Finding 2 of the 2026-09-05 whole-branch review.
    ///
    /// Marks `stalled` on a standing route's failed reload rather than
    /// dropping or severing the record — a stalled work order's rule,
    /// retried every tick rather than given up on.
    fn try_reload_route(&mut self, index: usize) -> bool {
        let (cargo, destination, standing) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            let RouteEnd::Settlement { key, .. } = &route.destination else {
                unreachable!("try_reload_route is the Settlement arm")
            };
            (route.cargo.clone(), *key, route.standing)
        };
        if !standing {
            self.world
                .resource_mut::<resources::Routes>()
                .0
                .remove(index);
            return true;
        }
        let ok = cargo
            .iter()
            .all(|(item, qty)| crate::game::base::work_orders::base_holding(self, item) >= *qty);
        if !ok {
            self.world.resource_mut::<resources::Routes>().0[index].stalled = true;
            return false;
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
        false
    }

    /// Loads up to `ROUTE_OUTPOST_CARRY` units off the outpost standing at
    /// `tile` into a fresh cargo manifest, in `ItemId` order (`Outpost::
    /// stock`'s own `BTreeMap` order) — design spec §7. Draws down the
    /// outpost's stock as it goes, `Game::take_from_outpost`'s own clamp
    /// shape one level over.
    fn load_outpost_cargo(&mut self, tile: (i32, i32)) -> Vec<(ItemId, u32)> {
        let available: Vec<(ItemId, u32)> = self
            .world
            .resource::<resources::Outposts>()
            .0
            .get(&tile)
            .map(|o| o.stock.iter().map(|(id, &qty)| (id.clone(), qty)).collect())
            .unwrap_or_default();
        let mut remaining = crate::tuning::ROUTE_OUTPOST_CARRY;
        let mut cargo = Vec::new();
        for (item, have) in available {
            if remaining == 0 {
                break;
            }
            let take = have.min(remaining);
            if take == 0 {
                continue;
            }
            remaining -= take;
            cargo.push((item.clone(), take));
            let mut outposts = self.world.resource_mut::<resources::Outposts>();
            if let Some(outpost) = outposts.0.get_mut(&tile) {
                let stock_qty = outpost
                    .stock
                    .get_mut(&item)
                    .expect("just read from this map");
                *stock_qty -= take;
                if *stock_qty == 0 {
                    outpost.stock.remove(&item);
                }
            }
        }
        cargo
    }

    /// The outbound leg's arrival at an outpost — `complete_outbound_leg`'s
    /// Outpost arm, and `retry_stalled_route`'s once the outpost went
    /// missing at a previous attempt. `try_reload_route`'s twin in shape
    /// only: unlike a settlement reload, this never fires from the inbound
    /// side — `complete_outpost_inbound_leg`'s "departs again" is a fresh
    /// outbound journey, and this is reached again only once that journey's
    /// `ticks_total` has actually elapsed. Returns whether the record was
    /// dropped (always `false`: a missing outpost stalls rather than drops
    /// the route, design spec §7).
    fn try_outpost_pickup(&mut self, index: usize, tile: (i32, i32)) -> bool {
        if !self
            .world
            .resource::<resources::Outposts>()
            .0
            .contains_key(&tile)
        {
            let was_stalled = self.world.resource::<resources::Routes>().0[index].stalled;
            self.world.resource_mut::<resources::Routes>().0[index].stalled = true;
            if !was_stalled {
                self.log_base(
                    "The caravan finds nothing at the outpost — it isn't there anymore."
                        .to_string(),
                );
            }
            return false;
        }
        let cargo = self.load_outpost_cargo(tile);
        let route = &mut self.world.resource_mut::<resources::Routes>().0[index];
        route.cargo = cargo;
        route.leg = RouteLeg::Inbound;
        route.ticks_elapsed = 0;
        route.stalled = false;
        false
    }

    /// An outpost's inbound leg lands: predation against the goods, the
    /// deposit of whatever survives through `return_material`, then
    /// departing again empty-handed (standing) or dropping the record for
    /// good (one-off) — design spec §7. Returns whether the record was
    /// dropped.
    ///
    /// **A standing route departs, it does not instantly reload.** Unlike a
    /// settlement leg — where loading happens at the base and costs no
    /// ticks — an outpost's cargo lives at the *far* end, so "again" means a
    /// fresh `RouteLeg::Outbound` empty-handed journey back to the tile, not
    /// a same-tick pickup. `try_outpost_pickup` is reached only once that
    /// journey's `ticks_total` has actually elapsed, through the ordinary
    /// `complete_outbound_leg` path — never from here.
    fn complete_outpost_inbound_leg(&mut self, index: usize, tile: (i32, i32)) -> bool {
        let (mut cargo, standing) = {
            let route = &self.world.resource::<resources::Routes>().0[index];
            (route.cargo.clone(), route.standing)
        };
        let anchor = self.anchor_position().unwrap_or((0, 0));
        self.roll_cargo_predation(anchor, tile, "the outpost", &mut cargo);
        let total: u32 = cargo.iter().map(|(_, qty)| *qty).sum();
        if total > 0 {
            for (item, qty) in &cargo {
                self.return_material(item, *qty);
            }
            self.log_base_kind(
                MessageKind::Loot,
                format!("The caravan returns from the outpost with {total} units of cargo."),
            );
        } else {
            self.log_base("The caravan returns from the outpost empty-handed.".to_string());
        }
        self.queue_cargo_walk(false);
        if !standing {
            self.world
                .resource_mut::<resources::Routes>()
                .0
                .remove(index);
            return true;
        }
        self.queue_cargo_walk(true);
        let route = &mut self.world.resource_mut::<resources::Routes>().0[index];
        route.leg = RouteLeg::Outbound;
        route.ticks_elapsed = 0;
        route.cargo = Vec::new();
        self.log_base("The caravan departs again for the outpost.".to_string());
        false
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
    /// reducing each line by `ROUTE_PREDATION_LOSS` on a hit — narrated to
    /// the log as it happens, which is this feature's only record of a hit
    /// (`Route` carries no loss log of its own; see the module doc).
    ///
    /// **The only place this feature draws `resources::GameRng`**, and only
    /// once nothing has filtered a predator out — an empty `predators` rolls
    /// nothing at all.
    ///
    /// `dest_name` is narration only — a settlement's own name, or a fixed
    /// string for an outpost, which has none of its own worth resolving —
    /// so this reads the same for either endpoint kind, `RouteEnd`'s own
    /// extension point.
    fn roll_cargo_predation(
        &mut self,
        base: (i32, i32),
        destination: (i32, i32),
        dest_name: &str,
        cargo: &mut [(ItemId, u32)],
    ) {
        let predators = self.route_predators(base, destination);
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
            let line = format!(
                "{predator_name} raids the caravan bound for {dest_name}, seizing {taken_units} units of cargo."
            );
            self.log_base_kind(MessageKind::Outcome, line);
        }
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
    ) {
        let predators = self.route_predators(base, destination);
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
            self.log_base_kind(MessageKind::Outcome, line);
        }
    }
}
