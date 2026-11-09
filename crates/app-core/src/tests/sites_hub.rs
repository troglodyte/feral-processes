//! The Sites hub: where it opens, what its rail lists, how its tabs walk, and
//! the actions each tab gathers. Fixtures write sites straight into a save,
//! as `dispatch.rs`'s do, because the hub reads the engine's own records.

use super::dispatch::app_at_a_relay;
use super::support::*;
use crate::app::sites_hub::SITES_OPEN_REFUSAL;
use crate::*;
use feral_processes_engine::items::ItemId;

use feral_processes_engine::save;
use feral_processes_engine::settlements::SettlementKey;

pub(super) const TOWN_A: SettlementKey = SettlementKey { rx: 7, ry: 1 };
pub(super) const TOWN_B: SettlementKey = SettlementKey { rx: 7, ry: 2 };
/// Within reach of the party's spot in `app_at_a_relay` (it stands at
/// `(0, -1)`), and clear of the base's own structures.
pub(super) const NEAR_TOWN_TILE: (i32, i32) = (0, -3);
pub(super) const FAR_TILE: (i32, i32) = (500, 500);

pub(super) fn town_def(name: &str) -> feral_processes_engine::settlements::SettlementDef {
    feral_processes_engine::settlements::SettlementDef {
        id: "test_settlement".to_string(),
        name: name.to_string(),
        blurb: "A settlement placed for a test.".to_string(),
        kind: feral_processes_engine::settlements::SettlementKind::Server,
        specialty: feral_processes_engine::settlements::Specialty::Materials,
        temperament: feral_processes_engine::settlements::Temperament::Open,
    }
}

/// Rewrites the run's known sites: `towns` are visited and `Allied` (so a relay link and a
/// standing route is allowed), `outposts` are `(tile, stock)`.
type Tile = (i32, i32);

pub(super) fn set_sites(
    app: &mut App,
    towns: &[(SettlementKey, &str, (i32, i32))],
    outposts: &[(Tile, Vec<(ItemId, u32)>)],
) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("sites_hub", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.settlements.0.clear();
    data.outposts.clear();
    let near = |(x, y): (i32, i32), t: (i32, i32)| (x - t.0).abs().max((y - t.1).abs()) <= 2;
    for &(key, name, tile) in towns {
        data.creatures.retain(|c| !near(c.position, tile));
        data.nests.retain(|n| !near(n.position, tile));
        data.link_sites.retain(|&site| !near(site, tile));
        data.settlements.0.insert(
            key,
            feral_processes_engine::resources::KnownSettlement {
                tile,
                def: town_def(name),
                visited: true,
            },
        );
        data.standings.0.insert(
            key,
            feral_processes_engine::settlements::relations::Relation {
                standing: feral_processes_engine::tuning::SETTLEMENT_ALLIED_STANDING,
                trade_credits: 0,
                ..Default::default()
            },
        );
    }
    for (tile, stock) in outposts {
        data.creatures.retain(|c| c.position != *tile);
        data.nests.retain(|n| n.position != *tile);
        data.outposts.push(save::OutpostSave {
            name: String::new(),
            tile: *tile,
            biome: feral_processes_engine::world::Biome::Deadlock,
            growth: 0,
            integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
            stock: stock.clone(),
            stale_ticks: 0,
            cycle_progress: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// A base with a Relay and two far towns, "Birch" listed after "Alder"
/// though keyed first, and two far outposts.
pub(super) fn app_with_far_sites(seed: u32) -> App {
    let mut app = app_at_a_relay(seed, &ItemId::from("cache_grain"), 40);
    set_sites(
        &mut app,
        &[(TOWN_B, "Alder", FAR_TILE), (TOWN_A, "Birch", (510, 500))],
        &[((520, 500), Vec::new()), ((530, 500), Vec::new())],
    );
    app
}

pub(super) fn tick(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

pub(super) fn rail(app: &App) -> Vec<(String, Option<&'static str>, bool)> {
    Hub::Sites
        .kind()
        .pages(app)
        .into_iter()
        .map(|p| (p.title, p.section, p.marked))
        .collect()
}

#[test]
fn w_refuses_out_in_the_field_with_one_line() {
    let mut app = test_app(7100);
    let before = tick(&app);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.status_line.as_deref(), Some(SITES_OPEN_REFUSAL));
    assert_eq!(tick(&app), before);
    assert_eq!(app.hub_at(), None);
}

#[test]
fn w_at_base_opens_on_the_rail() {
    let mut app = app_with_far_sites(7101);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.hub_at(), Some((Hub::Sites, 0, 0)));
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::SiteStatus);
}

#[test]
fn w_beside_a_town_opens_on_that_town() {
    let mut app = app_at_a_relay(7102, &ItemId::from("cache_grain"), 40);
    set_sites(
        &mut app,
        &[
            (TOWN_B, "Alder", FAR_TILE),
            (TOWN_A, "Birch", NEAR_TOWN_TILE),
        ],
        &[],
    );
    assert!(app.game.as_ref().unwrap().site_reach().on_base);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(
        app.hub_page(),
        Some(&HubPageKey::Site(RouteDestinationId::Settlement(TOWN_A)))
    );
    assert_eq!(app.hub_at().map(|(_, page, _)| page), Some(1));
}

#[test]
fn w_beside_an_outpost_opens_on_its_own_tabs() {
    let mut app = app_at_a_relay(7103, &ItemId::from("cache_grain"), 40);
    set_sites(&mut app, &[], &[((0, -2), Vec::new())]);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(
        app.hub_page(),
        Some(&HubPageKey::Site(RouteDestinationId::Outpost((0, -2))))
    );
    assert_eq!(app.mode, Mode::OutpostStatus);
}

#[test]
fn the_rail_is_towns_then_outposts_then_caravans() {
    let app = app_with_far_sites(7104);
    let rail = rail(&app);
    let titles: Vec<&str> = rail.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(titles[0], "Alder");
    assert_eq!(titles[1], "Birch");
    assert!(titles[2].starts_with("Outpost"), "{titles:?}");
    assert!(titles[3].starts_with("Outpost"), "{titles:?}");
    assert_eq!(titles[4], "Caravans");
    let sections: Vec<_> = rail.iter().map(|(_, s, _)| *s).collect();
    assert_eq!(
        sections,
        [
            Some("Towns and cities"),
            None,
            Some("Outposts"),
            None,
            Some("Traffic")
        ]
    );
}

#[test]
fn every_site_page_walks_its_tabs_with_left_and_right() {
    let mut app = app_with_far_sites(7105);
    app.handle_key(GameKey::Char('w'));
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail);
    let mut seen = vec![app.mode];
    for _ in 0..6 {
        app.handle_key(GameKey::Right);
        if seen.last() != Some(&app.mode) {
            seen.push(app.mode);
        }
    }
    assert_eq!(seen, TOWN_TABS.iter().map(|t| t.mode).collect::<Vec<_>>());
    assert_eq!(app.mode, Mode::SiteLog);
    for _ in 0..TOWN_TABS.len() {
        app.handle_key(GameKey::Left);
    }
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::SiteStatus);
}

#[test]
fn turning_pages_keeps_the_site_when_tabs_share_a_mode() {
    let mut app = app_with_far_sites(7106);
    app.handle_key(GameKey::Char('w'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::SiteGoods);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::SiteStatus);
    assert_eq!(
        app.hub_page(),
        Some(&HubPageKey::Site(RouteDestinationId::Settlement(TOWN_A)))
    );
    // The held key, not the mode, says which of two towns this is.
    assert_eq!(
        app.held_site(),
        Some(RouteDestinationId::Settlement(TOWN_A))
    );
}

#[test]
fn no_sites_page_advances_the_game() {
    let mut app = app_with_far_sites(7107);
    let before = tick(&app);
    app.handle_key(GameKey::Char('w'));
    for _ in 0..12 {
        for key in [
            GameKey::Right,
            GameKey::Down,
            GameKey::Up,
            GameKey::Left,
            GameKey::Char(']'),
            GameKey::Right,
            GameKey::Right,
            GameKey::PageDown,
        ] {
            app.handle_key(key);
            assert_eq!(tick(&app), before, "{key:?} on {:?}", app.mode);
        }
    }
}

#[test]
fn caravans_has_its_own_two_tabs() {
    let mut app = app_with_far_sites(7108);
    app.handle_key(GameKey::Char('w'));
    for _ in 0..4 {
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("Caravans")));
    assert_eq!(app.mode, Mode::CaravanSchedule);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::RoutesInFlight);
}

#[test]
fn a_site_that_left_the_rail_reopens_on_the_first_page() {
    let mut app = app_with_far_sites(7109);
    app.handle_key(GameKey::Char('w'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Down);
    assert!(matches!(
        app.hub_page(),
        Some(HubPageKey::Site(RouteDestinationId::Outpost(_)))
    ));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    set_sites(&mut app, &[(TOWN_B, "Alder", FAR_TILE)], &[]);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.hub_at(), Some((Hub::Sites, 0, 0)));
}

#[test]
fn a_held_site_that_vanishes_does_not_panic() {
    for key in [GameKey::Esc, GameKey::Down, GameKey::Char('T')] {
        let mut app = app_with_far_sites(7110);
        app.handle_key(GameKey::Char('w'));
        app.handle_key(GameKey::Right);
        set_sites(&mut app, &[], &[]);
        assert_eq!(app.hub_at(), None);
        assert_eq!(app.held_site(), None);
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Playing, "{key:?} must not strand the tab");
    }
}

#[test]
fn row_counts_follow_each_tab() {
    let mut app = app_at_a_relay(7112, &ItemId::from("cache_grain"), 40);
    set_sites(
        &mut app,
        &[(TOWN_A, "Alder", FAR_TILE)],
        &[(
            (520, 500),
            vec![
                (ItemId::from("cache_grain"), 3),
                (ItemId::from("raw_trace"), 1),
            ],
        )],
    );
    app.handle_key(GameKey::Char('w'));
    app.handle_key(GameKey::Right);
    let town = RouteDestinationId::Settlement(TOWN_A);
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(town), Mode::SiteRoutes);
    assert_eq!(app.sites_row_count(), 1, "no routes: just New route");
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(town), Mode::SiteGoods);
    assert_eq!(app.sites_row_count(), 0, "never seen");
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(town), Mode::SiteJobs);
    assert_eq!(app.sites_row_count(), 0);
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(town), Mode::SiteLog);
    assert_eq!(app.sites_row_count(), 0);
    let post = RouteDestinationId::Outpost((520, 500));
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(post), Mode::OutpostCrew);
    assert_eq!(app.sites_row_count(), 1, "no crew: just Post a program");
    app.enter_hub_at(Hub::Sites, HubPageKey::Site(post), Mode::OutpostStock);
    assert_eq!(app.outpost_stock_split(), (2, 0));
    assert_eq!(app.sites_row_count(), 3);
    let routes = app
        .game
        .as_mut()
        .unwrap()
        .dispatch_outpost_route((520, 500), false);
    assert!(routes.is_ok(), "{routes:?}");
    assert_eq!(app.outpost_stock_split(), (2, 1));
    assert_eq!(app.sites_row_count(), 4);
}

// ----- actions and "came from" -----

fn site(key: SettlementKey) -> HubPageKey {
    HubPageKey::Site(RouteDestinationId::Settlement(key))
}

/// At base with "Alder" (A) far away and "Birch" (B) beside the party.
fn app_beside_birch(seed: u32) -> App {
    let mut app = app_at_a_relay(seed, &ItemId::from("cache_grain"), 40);
    set_sites(
        &mut app,
        &[
            (TOWN_A, "Alder", FAR_TILE),
            (TOWN_B, "Birch", NEAR_TOWN_TILE),
        ],
        &[],
    );
    app
}

fn open_beside(app: &mut App, tab: Mode) {
    app.handle_key(GameKey::Char('w'));
    let page = app.hub_page().cloned().expect("the hub opened");
    app.enter_hub_at(Hub::Sites, page, tab);
    app.hub_on_rail = false;
}

#[test]
fn esc_from_the_market_returns_to_the_goods_tab_of_that_town() {
    let mut app = app_beside_birch(7120);
    open_beside(&mut app, Mode::SiteGoods);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SettlementMarket, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::SiteGoods);
    assert_eq!(app.hub_page(), Some(&site(TOWN_B)));
    assert!(!app.hub_on_rail);
}

#[test]
fn opening_the_market_from_the_hub_remembers_the_shelf() {
    let mut app = app_beside_birch(7121);
    open_beside(&mut app, Mode::SiteGoods);
    assert_eq!(app.sites_row_count(), 0, "never seen");
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Esc);
    assert!(app.sites_row_count() > 0, "the shelf was captured");
}

#[test]
fn esc_from_the_market_opened_on_the_visit_page_still_returns_there() {
    let mut app = app_beside_birch(7122);
    app.pending_settlement = Some(TOWN_B);
    app.mode = Mode::Settlement;
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.mode, Mode::SettlementMarket);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Settlement);
}

#[test]
fn the_board_returns_to_the_jobs_tab() {
    let mut app = app_beside_birch(7123);
    open_beside(&mut app, Mode::SiteJobs);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SettlementBoard);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::SiteJobs, Some(&site(TOWN_B)))
    );
}

#[test]
fn route_cargo_returns_to_its_routes_tab() {
    let mut app = app_beside_birch(7124);
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteRoutes);
    app.hub_on_rail = false;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::SiteRoutes, Some(&site(TOWN_A)))
    );
    assert!(!app.hub_on_rail);
}

#[test]
fn the_post_picker_and_the_take_return_to_their_tabs() {
    let mut app = app_at_a_relay(7125, &ItemId::from("cache_grain"), 40);
    let tile = (0, -2);
    set_sites(
        &mut app,
        &[],
        &[(tile, vec![(ItemId::from("cache_grain"), 4)])],
    );
    let post = HubPageKey::Site(RouteDestinationId::Outpost(tile));
    open_beside(&mut app, Mode::OutpostCrew);
    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.mode, Mode::OutpostPost, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!((app.mode, app.hub_page()), (Mode::OutpostCrew, Some(&post)));

    app.enter_hub_at(Hub::Sites, post.clone(), Mode::OutpostStock);
    app.handle_key(GameKey::Char('C'));
    assert_eq!(app.mode, Mode::Transfer, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::OutpostStock, Some(&post))
    );
}

#[test]
fn an_at_site_action_is_refused_on_another_sites_entry() {
    let mut app = app_beside_birch(7126);
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteGoods);
    app.hub_on_rail = false;
    let before = tick(&app);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SiteGoods);
    assert_eq!(
        app.status_line.as_deref(),
        Some("Walk over to Alder to trade.")
    );
    assert_eq!(tick(&app), before);
}

#[test]
fn base_actions_work_beside_a_site_next_to_the_floor() {
    let mut app = app_beside_birch(7127);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(
        app.held_site(),
        Some(RouteDestinationId::Settlement(TOWN_B))
    );
    // Another site's route still dispatches from the base floor.
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteRoutes);
    app.hub_on_rail = false;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo);
}

#[test]
fn stop_repeating_clears_standing_and_keeps_the_trip() {
    let mut app = app_beside_birch(7128);
    let sent = app.game.as_mut().unwrap().dispatch_route(
        TOWN_A,
        vec![(ItemId::from("cache_grain"), 3)],
        true,
    );
    assert!(sent.is_ok(), "{sent:?}");
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteRoutes);
    app.hub_on_rail = false;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.status_line, None);
    let reports = app.game.as_ref().unwrap().route_reports();
    assert_eq!(reports.len(), 1);
    assert!(!reports[0].standing);
    app.handle_key(GameKey::Char('S'));
    assert_eq!(
        app.status_line.as_deref(),
        Some("There's no standing route there to cut.")
    );
}

#[test]
fn abandon_from_the_hub_drops_the_contract() {
    let mut app = app_beside_birch(7129);
    let game = app.game.as_mut().unwrap();
    let rows = game.settlement_board(TOWN_B).expect("beside the counter");
    let row = rows.iter().find(|r| !r.tutorial).expect("a giveable job");
    let accepted = game.accept_contract(&row.id, Some(TOWN_B));
    assert!(accepted.is_ok(), "{accepted:?}");
    open_beside(&mut app, Mode::SiteJobs);
    assert_eq!(app.sites_row_count(), 1);
    app.handle_key(GameKey::Char('A'));
    assert_eq!(app.status_line, None);
    assert_eq!(app.sites_row_count(), 0);
}

#[test]
fn travel_from_the_hub_leaves_it_and_drains_the_visit() {
    let mut app = app_beside_birch(7130);
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteStatus);
    app.hub_on_rail = false;
    let before = tick(&app);
    app.handle_key(GameKey::Char('T'));
    assert!(app.hub_at().is_none(), "{:?}", app.status_line);
    assert!(tick(&app) > before, "{:?}", app.status_line);
    assert!(app.game.as_mut().unwrap().take_visit().is_none());
}

#[test]
fn every_refusal_lands_on_the_status_line_and_spends_nothing() {
    let mut app = app_with_far_sites(7131);
    let post = HubPageKey::Site(RouteDestinationId::Outpost((520, 500)));
    let cases = [
        (site(TOWN_A), Mode::SiteStatus, GameKey::Char('G')),
        (site(TOWN_A), Mode::SiteGoods, GameKey::Enter),
        (site(TOWN_A), Mode::SiteJobs, GameKey::Enter),
        (site(TOWN_A), Mode::SiteJobs, GameKey::Char('A')),
        (site(TOWN_A), Mode::SiteRoutes, GameKey::Char('S')),
        (post.clone(), Mode::OutpostStatus, GameKey::Char('R')),
        (post.clone(), Mode::OutpostCrew, GameKey::Char('U')),
        (post.clone(), Mode::OutpostCrew, GameKey::Char('P')),
        (post, Mode::OutpostStock, GameKey::Char('C')),
    ];
    for (page, tab, key) in cases {
        app.handle_key(GameKey::Char('w'));
        app.enter_hub_at(Hub::Sites, page, tab);
        app.hub_on_rail = false;
        app.status_line = None;
        let before = tick(&app);
        app.handle_key(key);
        assert!(app.status_line.is_some(), "{tab:?} {key:?} said nothing");
        assert_eq!(app.mode, tab, "{tab:?} {key:?} left the tab");
        assert_eq!(tick(&app), before, "{tab:?} {key:?} spent a tick");
    }
}

#[test]
fn a_new_route_off_base_is_refused() {
    let mut app = test_app(7132);
    set_sites(&mut app, &[(TOWN_A, "Alder", FAR_TILE)], &[]);
    app.mode = Mode::SiteRoutes;
    app.enter_hub_at(Hub::Sites, site(TOWN_A), Mode::SiteRoutes);
    app.hub_on_rail = false;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SiteRoutes);
    assert!(app.status_line.is_some());
}

// ----- what Dispatch's destination rows used to prove -----

fn open_routes_tab(app: &mut App, page: HubPageKey, tab: Mode) {
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, page, tab);
    app.hub_on_rail = false;
}

/// Editing the cargo basket and dispatching with Enter reaches
/// `Game::dispatch_route`, and the quoted figure the basket carries never
/// disagrees with what was asked for.
#[test]
fn right_builds_a_manifest_and_enter_dispatches_the_route() {
    let mut app = app_beside_birch(7140);
    let item = ItemId::from("cache_grain");
    open_routes_tab(&mut app, site(TOWN_A), Mode::SiteRoutes);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo);

    let basket = app.route_cargo_basket().expect("the site is pending");
    let row = basket
        .stock
        .iter()
        .position(|r| r.item == item)
        .expect("the depot holds cargo of this item");
    app.menu_selected = row;
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    let basket = app.route_cargo_basket().unwrap();
    assert_eq!(basket.cells[row].0, 2);
    assert!(basket.quote > 0, "two units must quote for something");

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SiteRoutes, "back on the tab it came from");
    assert!(app.pending_dispatch_destination.is_none());
    let reports = app.game.as_ref().unwrap().route_reports();
    assert!(
        reports
            .iter()
            .any(|r| r.destination == RouteDestinationId::Settlement(TOWN_A)),
        "the trip must be recorded in flight"
    );
    assert_eq!(app.sites_row_count(), 2, "the trip and New route");
}

/// `T` toggles the standing flag, and an empty basket's Enter is refused
/// through `App::refuse` rather than silently doing nothing.
#[test]
fn t_toggles_standing_and_an_empty_basket_is_refused() {
    let mut app = app_beside_birch(7141);
    open_routes_tab(&mut app, site(TOWN_A), Mode::SiteRoutes);
    app.handle_key(GameKey::Enter);
    assert!(!app.route_standing);
    app.handle_key(GameKey::Char('T'));
    assert!(app.route_standing);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo);
    assert!(app.status_line.is_some());
}

#[test]
fn an_outpost_route_has_no_manifest_and_dispatches_from_its_stock_tab() {
    let mut app = app_at_a_relay(7142, &ItemId::from("cache_grain"), 40);
    let tile = (520, 500);
    set_sites(&mut app, &[], &[(tile, Vec::new())]);
    let post = HubPageKey::Site(RouteDestinationId::Outpost(tile));
    open_routes_tab(&mut app, post.clone(), Mode::OutpostStock);
    assert_eq!(app.outpost_stock_split(), (0, 0));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo);
    let basket = app.route_cargo_basket().expect("the site is pending");
    assert!(basket.stock.is_empty(), "an outpost leg carries no cargo");
    assert_eq!(
        basket.outpost_carry_cap,
        Some(feral_processes_engine::tuning::ROUTE_OUTPOST_CARRY)
    );
    app.handle_key(GameKey::Enter);
    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::OutpostStock, Some(&post))
    );
    assert_eq!(app.outpost_stock_split(), (0, 1));
}

#[test]
fn stop_repeating_reaches_an_outpost_route_from_its_stock_tab() {
    let mut app = app_at_a_relay(7143, &ItemId::from("cache_grain"), 40);
    let tile = (520, 500);
    set_sites(&mut app, &[], &[(tile, Vec::new())]);
    let sent = app
        .game
        .as_mut()
        .unwrap()
        .dispatch_outpost_route(tile, true);
    assert!(sent.is_ok(), "{sent:?}");
    let post = HubPageKey::Site(RouteDestinationId::Outpost(tile));
    open_routes_tab(&mut app, post, Mode::OutpostStock);
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.status_line, None);
    let reports = app.game.as_ref().unwrap().route_reports();
    assert!(!reports[0].standing);
}

/// Just past the base's floor, so the landing search answers within reach of
/// the town and the visit page must open.
#[test]
fn arriving_within_reach_opens_the_town_page() {
    let mut app = app_at_a_relay(7144, &ItemId::from("cache_grain"), 10);
    let anchor = app
        .game
        .as_mut()
        .unwrap()
        .anchor_position()
        .expect("a base");
    let tile = (anchor.0, anchor.1 - TEST_POCKET_HALF_HEIGHT - 1);
    set_sites(&mut app, &[(TOWN_A, "Alder", tile)], &[]);
    // The party stands in reach of this one, so name the far page directly.
    open_routes_tab(&mut app, site(TOWN_A), Mode::SiteStatus);
    app.handle_key(GameKey::Char('T'));
    assert_eq!(app.status_line, None, "the trip was refused");
    assert_eq!(
        app.mode,
        Mode::Settlement,
        "arriving in reach opens the page"
    );
    assert_eq!(app.pending_settlement, Some(TOWN_A));
    assert!(app.game.as_mut().unwrap().take_visit().is_none());
}

#[test]
fn travel_below_allied_refuses_and_stays_on_the_tab() {
    let mut app = app_beside_birch(7145);
    super::settlement_aid::set_standing(&mut app, TOWN_A, 0);
    open_routes_tab(&mut app, site(TOWN_A), Mode::SiteStatus);
    let before = app.game.as_ref().unwrap().player_status().position;
    app.handle_key(GameKey::Char('T'));
    assert_eq!(app.game.as_ref().unwrap().player_status().position, before);
    assert_eq!(app.mode, Mode::SiteStatus);
    assert!(app.status_line.is_some());
}

/// What `app_at_a_hub_with_an_ally` proved on Dispatch: a town the party has
/// only heard of, and has since made Allied, still takes a cargo route and a
/// relay trip from the base.
fn app_at_base_with_an_unvisited_ally(seed: u32) -> (App, SettlementKey) {
    let mut app = app_at_a_relay(seed, &ItemId::from("cache_grain"), 10);
    let key = SettlementKey { rx: 6, ry: 6 };
    super::dispatch::register_a_known_settlement(&mut app, key, (60, 60));
    super::settlement_aid::set_standing(
        &mut app,
        key,
        feral_processes_engine::tuning::SETTLEMENT_ALLIED_STANDING,
    );
    (app, key)
}

#[test]
fn an_unvisited_allied_town_is_on_the_rail_by_name() {
    let (app, key) = app_at_base_with_an_unvisited_ally(7150);
    let titles: Vec<String> = rail(&app).into_iter().map(|(t, _, _)| t).collect();
    assert_eq!(titles, ["Test Settlement", "Caravans"]);
    assert_eq!(
        Hub::Sites.kind().pages(&app)[0].key,
        HubPageKey::Site(RouteDestinationId::Settlement(key))
    );
}

#[test]
fn an_unvisited_allied_town_takes_a_route_from_the_base() {
    let (mut app, key) = app_at_base_with_an_unvisited_ally(7151);
    open_routes_tab(&mut app, site(key), Mode::SiteRoutes);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RouteCargo, "{:?}", app.status_line);
    let basket = app.route_cargo_basket().expect("the site is pending");
    let row = basket
        .stock
        .iter()
        .position(|r| r.item == ItemId::from("cache_grain"))
        .expect("the depot holds cargo");
    app.menu_selected = row;
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SiteRoutes, "{:?}", app.status_line);
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .route_reports()
            .iter()
            .any(|r| r.destination == RouteDestinationId::Settlement(key)),
        "the trip is in flight"
    );
}

#[test]
fn an_unvisited_allied_town_takes_a_relay_trip_and_leaves_no_cue_dangling() {
    let (mut app, key) = app_at_base_with_an_unvisited_ally(7152);
    open_routes_tab(&mut app, site(key), Mode::SiteStatus);
    let before = tick(&app);
    app.handle_key(GameKey::Char('T'));
    assert!(app.hub_at().is_none(), "{:?}", app.status_line);
    assert!(tick(&app) > before, "{:?}", app.status_line);
    assert_ne!(
        app.game.as_ref().unwrap().player_status().position,
        (60, 60),
        "the party landed on the settlement tile"
    );
    assert!(app.game.as_mut().unwrap().take_visit().is_none());
}

#[test]
fn an_unvisited_town_refuses_its_beside_only_actions_from_the_base() {
    let (mut app, key) = app_at_base_with_an_unvisited_ally(7153);
    open_routes_tab(&mut app, site(key), Mode::SiteGoods);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SiteGoods);
    assert_eq!(
        app.status_line.as_deref(),
        Some("Walk over to Test Settlement to trade.")
    );
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(GameKey::Char(c));
    }
}

#[test]
fn n_opens_the_field_holding_the_current_name() {
    let mut app = app_with_far_sites(7140);
    let tile = (520, 500);
    app.game
        .as_mut()
        .unwrap()
        .rename_outpost(tile, "Quarry")
        .unwrap();
    let page = HubPageKey::Site(RouteDestinationId::Outpost(tile));
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, page, Mode::OutpostStatus);
    app.hub_on_rail = false;

    app.handle_key(GameKey::Char('N'));

    assert_eq!(app.mode, Mode::OutpostName);
    assert_eq!(app.outpost_name.as_str(), "Quarry");
    app.handle_key(GameKey::Backspace);
    assert_eq!(
        app.outpost_name.as_str(),
        "Quarr",
        "cursor starts at the end"
    );
}

#[test]
fn renaming_from_sites_returns_to_that_outposts_status_tab() {
    let mut app = app_with_far_sites(7141);
    let (first, second) = ((520, 500), (530, 500));
    let page = HubPageKey::Site(RouteDestinationId::Outpost(second));
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, page.clone(), Mode::OutpostStatus);
    app.hub_on_rail = false;
    let before = tick(&app);

    app.handle_key(GameKey::Char('N'));
    for _ in 0..40 {
        app.handle_key(GameKey::Backspace);
    }
    type_text(&mut app, "Ridge");
    app.handle_key(GameKey::Enter);

    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::OutpostStatus, Some(&page))
    );
    let game = app.game.as_ref().unwrap();
    assert_eq!(game.outpost_destination_name(second), "Ridge");
    assert_ne!(game.outpost_destination_name(first), "Ridge");
    assert_eq!(tick(&app), before, "naming never advances the game");
}

#[test]
fn esc_in_the_sites_name_field_keeps_the_name_and_returns() {
    let mut app = app_with_far_sites(7142);
    let tile = (520, 500);
    let page = HubPageKey::Site(RouteDestinationId::Outpost(tile));
    app.handle_key(GameKey::Char('w'));
    app.enter_hub_at(Hub::Sites, page.clone(), Mode::OutpostStatus);
    app.hub_on_rail = false;
    let name = app.game.as_ref().unwrap().outpost_destination_name(tile);

    app.handle_key(GameKey::Char('N'));
    type_text(&mut app, "zzz");
    app.handle_key(GameKey::Esc);

    assert_eq!(
        (app.mode, app.hub_page()),
        (Mode::OutpostStatus, Some(&page))
    );
    assert_eq!(
        app.game.as_ref().unwrap().outpost_destination_name(tile),
        name
    );
}

#[test]
fn the_rail_shows_the_new_name() {
    let mut app = app_with_far_sites(7143);
    let tile = (520, 500);
    app.game
        .as_mut()
        .unwrap()
        .rename_outpost(tile, "Quarry")
        .unwrap();
    let titles: Vec<String> = rail(&app).into_iter().map(|(t, _, _)| t).collect();
    assert!(titles.iter().any(|t| t == "Quarry"), "{titles:?}");
}

#[test]
fn the_status_tab_lists_rename_in_its_keybar() {
    assert!(sites_screen_keys(Mode::OutpostStatus).contains(&("N", "rename")));
}
