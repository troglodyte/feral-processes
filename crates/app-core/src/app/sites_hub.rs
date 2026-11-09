//! The site pages of `b`: TOWNS and OUTPOSTS, one entry per known town,
//! city and outpost, then TRAFFIC's Caravans and Sorties. A site's tabs are
//! shared by every site of its kind, so the page is held by key (ADR 0043)
//! and every entry here names it through `turn_hub_to` or `enter_hub_at`.
//! The pages read anywhere; their actions call the doors the rules already
//! allow and refuse in those doors' own words.

use crate::*;
use feral_processes_engine::outposts::{POST_REACH_REFUSAL, TAKE_REACH_REFUSAL};
use feral_processes_engine::sites::{SiteSection, TownGoods};

pub const TOWN_TABS: &[HubTab] = &[
    HubTab::new("Status", Mode::SiteStatus),
    HubTab::new("Goods", Mode::SiteGoods),
    HubTab::new("Routes", Mode::SiteRoutes),
    HubTab::new("Jobs", Mode::SiteJobs),
    HubTab::new("Log", Mode::SiteLog),
];
pub const OUTPOST_TABS: &[HubTab] = &[
    HubTab::new("Status", Mode::OutpostStatus),
    HubTab::new("Crew", Mode::OutpostCrew),
    HubTab::new("Stock", Mode::OutpostStock),
    HubTab::new("Log", Mode::SiteLog),
];
/// Trader is the wagon at the base's counter, which the old Trade page's
/// Caravan row opened.
pub const CARAVAN_TABS: &[HubTab] = &[
    HubTab::new("Schedule", Mode::CaravanSchedule),
    HubTab::new("In flight", Mode::RoutesInFlight),
    HubTab::new("Trader", Mode::Caravan),
];
/// The Relay's sortie board, which the old Trade page's Dispatch row opened.
pub const SORTIE_TABS: &[HubTab] = &[HubTab::new("Sorties", Mode::Dispatch)];

const CARAVANS_TITLE: &str = "Caravans";
const SORTIES_TITLE: &str = "Sorties";

/// The visit page's refusals, shared with the Goods and Jobs tabs.
pub(crate) const WALK_TO_TRADE: &str = "You'd have to walk over there to trade.";
pub(crate) const WALK_TO_BOARD: &str = "You'd have to walk over there to read their board.";
pub(crate) const NO_STANDING_ROUTE: &str = "There's no standing route there to cut.";
/// Recalling crew and stopping a route are run from the base, or standing
/// at the site itself — where `w` used to open before `b` held the sites.
pub(crate) const RECALL_REACH_REFUSAL: &str =
    "Recall crew from the base, or standing at the outpost.";
pub(crate) const SEVER_REACH_REFUSAL: &str =
    "Stop a route from the base, or standing at the place it runs to.";

fn section_title(section: SiteSection) -> &'static str {
    match section {
        SiteSection::Settlements => "TOWNS",
        SiteSection::Outposts => "OUTPOSTS",
    }
}

/// `b`'s rail after JOBS: every known site under its section, then
/// TRAFFIC, which is there in every run.
pub(crate) fn site_pages(app: &App) -> Vec<HubPage> {
    let Some(game) = app.game.as_ref() else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    let mut last = None;
    for entry in game.site_rail() {
        let tabs = match entry.site {
            RouteDestinationId::Settlement(_) => TOWN_TABS,
            RouteDestinationId::Outpost(_) => OUTPOST_TABS,
        };
        pages.push(HubPage {
            key: HubPageKey::Site(entry.site),
            title: entry.name,
            glyph: Some(entry.glyph),
            section: (last != Some(entry.section)).then(|| section_title(entry.section)),
            tabs,
            marked: entry.attention,
            name_cut_at_end: true,
        });
        last = Some(entry.section);
    }
    pages.push(HubPage {
        key: HubPageKey::Named(CARAVANS_TITLE),
        title: CARAVANS_TITLE.to_string(),
        glyph: Some('~'),
        section: Some("TRAFFIC"),
        tabs: CARAVAN_TABS,
        marked: game.route_reports().iter().any(|r| r.stalled),
        name_cut_at_end: false,
    });
    pages.push(HubPage {
        key: HubPageKey::Named(SORTIES_TITLE),
        title: SORTIES_TITLE.to_string(),
        glyph: None,
        section: None,
        tabs: SORTIE_TABS,
        marked: false,
        name_cut_at_end: false,
    });
    pages
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
        Mode::OutpostStatus => &[("R", "repair"), ("N", "rename")],
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

impl App {
    /// `w`: opens `b` on the site beside the party, else on the first
    /// TOWNS entry, else on the Overview — never `b`'s remembered page,
    /// which would make `w` a second `b` key.
    pub(crate) fn open_sites_hub(&mut self) {
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let beside = game.site_reach().beside;
        let first_town = game
            .site_rail()
            .into_iter()
            .find(|e| e.section == SiteSection::Settlements)
            .map(|e| e.site);
        self.open_hub(Hub::Base);
        let page = match beside.or(first_town) {
            Some(site) => HubPageKey::Site(site),
            None => HubPageKey::Named("Overview"),
        };
        self.turn_hub_to(Hub::Base, &page);
    }

    /// The site the held page names, `None` on any other page or outside
    /// the hub.
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

    /// Selectable rows on the current site or traffic tab.
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

    /// Opens a screen a site tab launches, remembering the tab to come back
    /// to.
    pub(crate) fn launch_from_site(&mut self, to: Mode) {
        self.remember_site_tab();
        self.status_line = None;
        self.mode = to;
    }

    fn remember_site_tab(&mut self) {
        if let Some(key) = self.hub_page().cloned() {
            self.site_return = Some((key, self.mode));
        }
    }

    /// Leaves a screen a site tab can launch: back to the tab it was
    /// launched from, else `fallback` (the screen's own exit before this).
    pub(crate) fn leave_site_screen(&mut self, fallback: Mode) {
        let back = self.site_return.take().filter(|(key, mode)| {
            Hub::Base
                .kind()
                .pages(self)
                .iter()
                .any(|p| &p.key == key && p.offers(*mode))
        });
        match back {
            Some((key, mode)) => {
                self.pending_settlement = None;
                self.pending_outpost = None;
                self.enter_hub_at(Hub::Base, key, mode);
                self.hub_on_rail = false;
            }
            None => self.mode = fallback,
        }
    }

    /// Whether the party stands beside the held site.
    fn beside_held_site(&self) -> bool {
        let (Some(site), Some(game)) = (self.held_site(), self.game.as_ref()) else {
            return false;
        };
        game.beside_site(site)
    }

    /// At base, or beside `site` itself.
    fn reaches_site(&self, site: RouteDestinationId) -> bool {
        self.game
            .as_ref()
            .is_some_and(|g| g.site_reach().on_base || g.beside_site(site))
    }

    pub(crate) fn handle_sites_key(&mut self, key: GameKey) {
        if self.hub_at().is_none() {
            // The held site left the rail under the player; nothing on this
            // tab is drawn any more, so leave rather than strand it.
            self.close_screen();
            return;
        }
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
                    self.refuse(WALK_TO_TRADE);
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
                    self.refuse(WALK_TO_BOARD);
                    return;
                }
                self.pending_settlement = Some(k);
                self.launch_from_site(Mode::SettlementBoard);
            }
            (Mode::SiteJobs, Some(RouteDestinationId::Settlement(k)), GameKey::Char('A')) => {
                let contract = self
                    .game
                    .as_mut()
                    .and_then(|g| g.town_contracts(k).into_iter().nth(row));
                match contract {
                    Some(contract) => self.give_back(&contract),
                    None => self.refuse("Highlight a contract you are holding to give it back."),
                }
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
            (Mode::OutpostStatus, Some(RouteDestinationId::Outpost(t)), GameKey::Char('N')) => {
                self.open_outpost_name(t, true)
            }
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(t)), GameKey::Char('U')) => {
                if !self.reaches_site(RouteDestinationId::Outpost(t)) {
                    self.refuse(RECALL_REACH_REFUSAL);
                    return;
                }
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
            (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(t)), GameKey::Enter)
                if row + 1 == rows =>
            {
                self.post_from_hub(t)
            }
            _ => {}
        }
    }

    fn traffic_tab_key(&mut self, site: Option<RouteDestinationId>, key: GameKey, row: usize) {
        match (self.mode, site, key) {
            (Mode::OutpostStock, Some(RouteDestinationId::Outpost(t)), GameKey::Char('C')) => {
                if !self.beside_held_site() {
                    self.refuse(TAKE_REACH_REFUSAL);
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
        if !self.reaches_site(site) {
            self.refuse(SEVER_REACH_REFUSAL);
            return;
        }
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
            self.refuse(POST_REACH_REFUSAL);
            return;
        }
        self.pending_outpost = Some(tile);
        self.launch_from_site(Mode::OutpostPost);
    }
}
