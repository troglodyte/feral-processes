//! The Sites hub (`w`): one rail entry per known town, city and outpost, then
//! Caravans. A site's tabs are shared by every site, so the page is held by
//! key (ADR 0043) and every entry here names it through `turn_hub_to` or
//! `enter_hub_at`. The pages are read-only views; their actions call the
//! doors the rules already allow and refuse in those doors' own words.

use crate::*;
use feral_processes_engine::sites::{SiteSection, TownGoods};

pub const TOWN_TABS: &[Mode] = &[
    Mode::SiteStatus,
    Mode::SiteGoods,
    Mode::SiteRoutes,
    Mode::SiteJobs,
    Mode::SiteLog,
];
pub const OUTPOST_TABS: &[Mode] = &[
    Mode::OutpostStatus,
    Mode::OutpostCrew,
    Mode::OutpostStock,
    Mode::SiteLog,
];
pub const CARAVAN_TABS: &[Mode] = &[Mode::CaravanSchedule, Mode::RoutesInFlight];

const CARAVANS_TITLE: &str = "Caravans";

/// What `w` says when the party is neither at base nor beside a site.
pub const SITES_OPEN_REFUSAL: &str = "The sites list opens at base, or beside a town or outpost.";
/// The visit page's refusals, shared with the hub's Goods and Jobs tabs.
pub(crate) const WALK_TO_TRADE: &str = "You'd have to walk over there to trade.";
pub(crate) const WALK_TO_BOARD: &str = "You'd have to walk over there to read their board.";
pub(crate) const NO_STANDING_ROUTE: &str = "There's no standing route there to cut.";

pub(crate) struct SitesHub;

fn section_title(section: SiteSection) -> &'static str {
    match section {
        SiteSection::Settlements => "Towns and cities",
        SiteSection::Outposts => "Outposts",
    }
}

impl HubKind for SitesHub {
    fn title(&self) -> &'static str {
        "SITES"
    }

    fn pages(&self, app: &App) -> Vec<HubPage> {
        let Some(game) = app.game.as_ref() else {
            return Vec::new();
        };
        let mut pages = Vec::new();
        let mut last = None;
        for entry in game.site_rail() {
            let screens = match entry.site {
                RouteDestinationId::Settlement(_) => TOWN_TABS,
                RouteDestinationId::Outpost(_) => OUTPOST_TABS,
            };
            pages.push(HubPage {
                key: HubPageKey::Site(entry.site),
                title: entry.name,
                glyph: Some(entry.glyph),
                section: (last != Some(entry.section)).then(|| section_title(entry.section)),
                screens,
                marked: entry.attention,
            });
            last = Some(entry.section);
        }
        pages.push(HubPage {
            key: HubPageKey::Named(CARAVANS_TITLE),
            title: CARAVANS_TITLE.to_string(),
            glyph: Some('~'),
            section: Some("Traffic"),
            screens: CARAVAN_TABS,
            marked: game.route_reports().iter().any(|r| r.stalled),
        });
        pages
    }
}

/// The keys a Sites tab binds, as `(key, verb)` for the keybar, in the
/// handler's own case.
pub fn sites_screen_keys(mode: Mode) -> &'static [(&'static str, &'static str)] {
    match mode {
        Mode::SiteStatus => &[("T", "travel"), ("G", "gift"), ("H", "relay home")],
        Mode::SiteGoods => &[("↑/↓", "select"), ("Enter", "trade")],
        Mode::SiteRoutes => &[
            ("↑/↓", "select"),
            ("S", "stop repeating"),
            ("Enter", "new route"),
        ],
        Mode::SiteJobs => &[("↑/↓", "select"), ("A", "abandon"), ("Enter", "board")],
        Mode::SiteLog => &[("↑/↓", "scroll")],
        Mode::OutpostStatus => &[("R", "repair")],
        Mode::OutpostCrew => &[("↑/↓", "select"), ("U", "recall"), ("P", "post")],
        Mode::OutpostStock => &[
            ("↑/↓", "select"),
            ("C", "take"),
            ("S", "stop repeating"),
            ("Enter", "new route"),
        ],
        Mode::RoutesInFlight => &[("↑/↓", "select"), ("S", "stop repeating")],
        _ => &[],
    }
}

fn walk_over(name: &str, to: &str) -> String {
    format!("Walk over to {name} {to}.")
}

impl App {
    /// `w`: opens the hub at base or beside a site, on that site when beside
    /// one. Anywhere else it says why and changes nothing.
    pub(crate) fn open_sites_hub(&mut self) {
        let Some(reach) = self.game.as_ref().map(|g| g.site_reach()) else {
            return;
        };
        if !reach.opens() {
            self.refuse(SITES_OPEN_REFUSAL);
            return;
        }
        self.open_hub(Hub::Sites);
        if let Some(site) = reach.beside {
            self.turn_hub_to(Hub::Sites, &HubPageKey::Site(site));
        }
    }

    /// The site the held page names, `None` on Caravans or outside the hub.
    pub fn held_site(&self) -> Option<RouteDestinationId> {
        match self.hub_page()? {
            HubPageKey::Site(site) => Some(*site),
            HubPageKey::Named(_) => None,
        }
    }

    /// `(stock lines, routes)` on an outpost's Stock tab. The tab's rows are
    /// the stock lines, then the routes, then "New outpost route…", and the
    /// renderer calls this for the same split.
    pub fn outpost_stock_split(&self) -> (usize, usize) {
        let (Some(RouteDestinationId::Outpost(tile)), Some(game)) =
            (self.held_site(), self.game.as_ref())
        else {
            return (0, 0);
        };
        (
            game.outpost_transfer_offer(tile).len(),
            game.site_routes(RouteDestinationId::Outpost(tile)).len(),
        )
    }

    /// Selectable rows on the current Sites tab.
    pub fn sites_row_count(&mut self) -> usize {
        let site = self.held_site();
        match (self.mode, site) {
            (Mode::SiteGoods, Some(RouteDestinationId::Settlement(key))) => {
                let Some(game) = self.game.as_mut() else {
                    return 0;
                };
                match game.town_goods(key) {
                    TownGoods::NotSeen => 0,
                    TownGoods::Seen { offers, .. } | TownGoods::Live { offers } => offers.len(),
                }
            }
            (Mode::SiteRoutes, Some(site)) => {
                self.game.as_ref().map_or(0, |g| g.site_routes(site).len()) + 1
            }
            (Mode::SiteJobs, Some(RouteDestinationId::Settlement(key))) => self
                .game
                .as_ref()
                .map_or(0, |g| g.town_contracts(key).len()),
            (Mode::SiteLog, Some(site)) => self.game.as_ref().map_or(0, |g| g.site_log(site).len()),
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(tile))) => {
                self.game
                    .as_mut()
                    .and_then(|g| g.outpost_report(tile))
                    .map_or(0, |r| r.crew.len())
                    + 1
            }
            (Mode::OutpostStock, _) => {
                let (stock, routes) = self.outpost_stock_split();
                stock + routes + 1
            }
            (Mode::RoutesInFlight, _) => {
                self.game.as_ref().map_or(0, |g| g.routes_in_flight().len())
            }
            _ => 0,
        }
    }

    /// Opens a screen the hub launches, remembering the tab to come back to.
    fn launch_from_site(&mut self, to: Mode) {
        self.remember_site_tab();
        self.status_line = None;
        self.mode = to;
    }

    fn remember_site_tab(&mut self) {
        if let Some(key) = self.hub_page().cloned() {
            self.site_return = Some((key, self.mode));
        }
    }

    /// Leaves a screen the Sites hub can launch: back to the tab it was
    /// launched from, else `fallback` (the screen's own exit before this).
    pub(crate) fn leave_site_screen(&mut self, fallback: Mode) {
        let back = self.site_return.take().filter(|(key, mode)| {
            Hub::Sites
                .kind()
                .pages(self)
                .iter()
                .any(|p| &p.key == key && p.screens.contains(mode))
        });
        match back {
            Some((key, mode)) => {
                self.pending_settlement = None;
                self.pending_outpost = None;
                self.enter_hub_at(Hub::Sites, key, mode);
                self.hub_on_rail = false;
            }
            None => self.mode = fallback,
        }
    }

    /// Whether the party stands beside the held site.
    fn beside_held_site(&self) -> bool {
        let reach = self.game.as_ref().map(|g| g.site_reach());
        reach.is_some_and(|r| r.beside.is_some() && r.beside == self.held_site())
    }

    fn held_site_name(&self) -> String {
        let (Some(site), Some(game)) = (self.held_site(), self.game.as_ref()) else {
            return String::new();
        };
        match site {
            RouteDestinationId::Settlement(key) => game.settlement_name(key),
            RouteDestinationId::Outpost(tile) => game.outpost_destination_name(tile),
        }
    }

    pub(crate) fn handle_sites_key(&mut self, key: GameKey) {
        let rows = self.sites_row_count();
        if matches!(
            key,
            GameKey::Up | GameKey::Down | GameKey::PageUp | GameKey::PageDown
        ) {
            self.scroll(key, rows);
            return;
        }
        self.menu_selected = self.menu_selected.min(rows.saturating_sub(1));
        let row = self.menu_selected;
        let site = self.held_site();
        match self.mode {
            Mode::SiteStatus | Mode::SiteGoods | Mode::SiteJobs => {
                self.town_tab_key(site, key, row)
            }
            Mode::OutpostStatus | Mode::OutpostCrew => self.outpost_tab_key(site, key, row, rows),
            _ => self.traffic_tab_key(site, key, row),
        }
    }

    fn town_tab_key(&mut self, site: Option<RouteDestinationId>, key: GameKey, row: usize) {
        match (self.mode, site, key) {
            (Mode::SiteStatus, Some(RouteDestinationId::Settlement(k)), GameKey::Char('T')) => {
                self.travel_from_hub(k, Game::travel_to_settlement)
            }
            (Mode::SiteStatus, Some(RouteDestinationId::Settlement(k)), GameKey::Char('H')) => {
                self.travel_from_hub(k, Game::travel_to_anchor)
            }
            (Mode::SiteStatus, Some(RouteDestinationId::Settlement(k)), GameKey::Char('G')) => {
                let outcome = self.game.as_mut().map(|g| g.request_program_gift(k));
                if let Some(outcome) = outcome {
                    self.report(outcome);
                }
            }
            (Mode::SiteGoods, Some(RouteDestinationId::Settlement(k)), GameKey::Enter) => {
                if !self.beside_held_site() {
                    let line = walk_over(&self.held_site_name(), "to trade");
                    self.refuse(line);
                    return;
                }
                self.pending_settlement = Some(k);
                if let Some(game) = &mut self.game {
                    game.record_shelf_seen(k);
                }
                self.settlement_amounts.clear();
                self.launch_from_site(Mode::SettlementMarket);
            }
            (Mode::SiteJobs, Some(RouteDestinationId::Settlement(k)), GameKey::Enter) => {
                if !self.beside_held_site() {
                    let line = walk_over(&self.held_site_name(), "to read their board");
                    self.refuse(line);
                    return;
                }
                self.pending_settlement = Some(k);
                self.launch_from_site(Mode::SettlementBoard);
            }
            (Mode::SiteJobs, Some(RouteDestinationId::Settlement(k)), GameKey::Char('A')) => {
                let Some(game) = &mut self.game else { return };
                let Some(contract) = game.town_contracts(k).into_iter().nth(row) else {
                    self.refuse("Highlight a contract you are holding to give it back.");
                    return;
                };
                if contract.tutorial {
                    self.refuse(crate::app::contracts::TUTORIAL_REFUSAL);
                    return;
                }
                game.abandon_contract(&contract.id);
                self.status_line = None;
            }
            _ => {}
        }
    }

    fn outpost_tab_key(
        &mut self,
        site: Option<RouteDestinationId>,
        key: GameKey,
        row: usize,
        rows: usize,
    ) {
        match (self.mode, site, key) {
            (Mode::OutpostStatus, Some(RouteDestinationId::Outpost(t)), GameKey::Char('R')) => {
                let outcome = self.game.as_mut().map(|g| g.repair_outpost(t));
                if let Some(outcome) = outcome {
                    self.report(outcome);
                }
            }
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(t)), GameKey::Char('U')) => {
                let Some(game) = &mut self.game else { return };
                let Some(&entity) = game.outpost_crew(t).get(row) else {
                    self.refuse("Select a crew row to recall.");
                    return;
                };
                let outcome = game.recall_from_outpost(entity);
                self.report(outcome);
            }
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(t)), GameKey::Char('P')) => {
                self.post_from_hub(t)
            }
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(t)), GameKey::Enter) => {
                if row + 1 == rows {
                    self.post_from_hub(t);
                }
            }
            _ => {}
        }
    }

    fn traffic_tab_key(&mut self, site: Option<RouteDestinationId>, key: GameKey, row: usize) {
        match (self.mode, site, key) {
            (Mode::OutpostStock, Some(RouteDestinationId::Outpost(t)), GameKey::Char('C')) => {
                if !self.beside_held_site() {
                    let line = walk_over(&self.held_site_name(), "to take stock");
                    self.refuse(line);
                    return;
                }
                let Some(game) = &mut self.game else { return };
                let offer = game.outpost_transfer_offer(t);
                if offer.is_empty() {
                    self.refuse("There's nothing standing at this outpost to take.");
                    return;
                }
                self.pending_outpost = Some(t);
                self.remember_site_tab();
                self.open_transfer(offer, Vec::new(), None, 0, TransferSource::Outpost(t));
            }
            (Mode::SiteRoutes | Mode::OutpostStock, Some(s), GameKey::Char('S')) => {
                self.sever_from_hub(s)
            }
            (Mode::RoutesInFlight, _, GameKey::Char('S')) => {
                let line = self
                    .game
                    .as_ref()
                    .and_then(|g| g.routes_in_flight().into_iter().nth(row));
                match line {
                    Some(line) => self.sever_from_hub(line.site),
                    None => self.refuse("Highlight a route to stop repeating."),
                }
            }
            (Mode::SiteRoutes, Some(s), GameKey::Enter) => {
                let routes = self.game.as_ref().map_or(0, |g| g.site_routes(s).len());
                if row >= routes {
                    self.new_route_from_hub(s);
                }
            }
            (Mode::OutpostStock, Some(s), GameKey::Enter) => {
                let (stock, routes) = self.outpost_stock_split();
                if row >= stock + routes {
                    self.new_route_from_hub(s);
                }
            }
            _ => {}
        }
    }

    /// `T` and `H`: a relay trip, which on success closes the hub to the map
    /// and spends the ordinary world-action tail (it is also what drains the
    /// arrival cue).
    fn travel_from_hub(
        &mut self,
        key: SettlementKey,
        door: fn(&mut Game, SettlementKey) -> Result<(), String>,
    ) {
        let Some(game) = &mut self.game else { return };
        let outcome = door(game, key);
        let went = outcome.is_ok();
        self.report(outcome);
        if went {
            self.pending_settlement = None;
            self.mode = Mode::Playing;
            self.after_world_action(true, false, 0);
        }
    }

    fn sever_from_hub(&mut self, site: RouteDestinationId) {
        let Some(game) = &mut self.game else { return };
        if game.sever_route(site) {
            self.status_line = None;
        } else {
            self.refuse(NO_STANDING_ROUTE);
        }
    }

    fn new_route_from_hub(&mut self, site: RouteDestinationId) {
        let Some(game) = &mut self.game else { return };
        if game.dispatch_reach() != DispatchReach::AtRelay {
            let line = crate::app::dispatch::route_refusal_line(game, RouteRefusal::NotAtRelay);
            self.refuse(line);
            return;
        }
        self.pending_dispatch_destination = Some(site);
        self.route_cargo_amounts.clear();
        self.route_standing = false;
        self.launch_from_site(Mode::RouteCargo);
    }

    fn post_from_hub(&mut self, tile: (i32, i32)) {
        if !self.beside_held_site() {
            let line = walk_over(&self.held_site_name(), "to post a program");
            self.refuse(line);
            return;
        }
        self.pending_outpost = Some(tile);
        self.launch_from_site(Mode::OutpostPost);
    }
}
