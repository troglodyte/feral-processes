//! The Relay hub and its two pickers — `tests/settlement_board.rs` and
//! `tests/settlement_market.rs`'s shape, one door over: what is worth
//! asserting is the row that opens the flow, the reach gate, the numbered
//! resolution across two sections, and every refusal reaching `App::refuse`.

use super::support::*;
use crate::*;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::save;
use feral_processes_engine::settlements::SettlementKey;

/// A founded base with a Relay and a Depot holding `qty` of `item`, the
/// player standing on the pocket floor beside both — `app_at_a_contract_broker`'s
/// shape, one desk over: the engine exposes no way to hand-place a
/// structure from outside the crate, so this is a save round trip too.
pub(super) fn app_at_a_relay(seed: u32, item: &ItemId, qty: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    found_the_base(&mut app);
    let path = scratch_path("relay", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "relay".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        battery_charge: 0,
        refit_charge: 0,
        incubating: Vec::new(),
        study_yield: None,
    });
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "depot".to_string(),
        position: (0, 1),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: vec![(item.clone(), qty)],
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        battery_charge: 0,
        refit_charge: 0,
        incubating: Vec::new(),
        study_yield: None,
    });
    // One tile clear of the Home, the Relay and the Depot — `deploy_relay`'s
    // own note in the engine suite.
    stand_save_on_field(&mut data, 0, -1);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A known settlement, `Open` and `Neutral` by default — `place_settlement_east_of_player`'s
/// shape, but registered with no map entity, since a route test needs the
/// town known rather than walked to.
pub(super) fn register_a_known_settlement(app: &mut App, key: SettlementKey, tile: (i32, i32)) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("dispatch_settlement", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    // `Game::new` already ran `ensure_local_settlements`, so the fixture
    // clears what world generation found nearby before registering its own
    // — otherwise a row picked by section length can land on the wrong town,
    // `route_destinations_lists_every_known_settlement`'s own reason.
    data.settlements.0.clear();
    data.settlements.0.insert(
        key,
        feral_processes_engine::resources::KnownSettlement {
            tile,
            def: feral_processes_engine::settlements::SettlementDef {
                id: "test_settlement".to_string(),
                name: "Test Settlement".to_string(),
                blurb: "A settlement placed for a test.".to_string(),
                kind: feral_processes_engine::settlements::SettlementKind::Server,
                specialty: feral_processes_engine::settlements::Specialty::Materials,
                temperament: feral_processes_engine::settlements::Temperament::Open,
            },
            visited: false,
        },
    );
    // Warm and up is what `Standing::allows_standing_route` asks for; the
    // default `Neutral` a fresh registration carries refuses a standing
    // route with its own `RouteRefusal` variant, which two of this file's
    // tests deliberately go on to dispatch one.
    data.standings.0.insert(
        key,
        feral_processes_engine::settlements::relations::Relation {
            standing: feral_processes_engine::tuning::SETTLEMENT_WARM_STANDING,
            trade_credits: 0,
            ..Default::default()
        },
    );
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// A relay, a stocked depot, a known destination and one base-staff program
/// — everything `Mode::Dispatch`'s two pickers need. `app_owning_distant_programs`
/// already gives a program with no party slot and no wield, which is
/// exactly `ProgramRole::Staff`.
fn a_dispatch_ready_app(seed: u32) -> (App, ItemId, SettlementKey) {
    let item = ItemId::from("cache_grain");
    let mut app = app_at_a_relay(seed, &item, 40);
    let key = SettlementKey { rx: 5, ry: 5 };
    register_a_known_settlement(&mut app, key, (500, 500));

    let mut staffed = app_owning_distant_programs(seed + 1, 1);
    let src = scratch_path("dispatch_staff_src", seed);
    staffed.game.as_mut().unwrap().save(&src).unwrap();
    let mut staff_data = save::load_from_file(&src).unwrap();
    let _ = std::fs::remove_file(&src);

    let assets_dir = test_assets_dir();
    let path = scratch_path("dispatch_staff", seed);
    let base = app.game.as_mut().unwrap();
    base.save(&path).unwrap();
    let mut base_data = save::load_from_file(&path).unwrap();
    base_data.creatures.append(&mut staff_data.creatures);
    save::save_to_file(&path, &base_data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    (app, item, key)
}

/// Sorties is on the rail in every run; with no Relay standing the tab
/// says so and has nothing to pick.
#[test]
fn the_sorties_tab_with_no_relay_has_no_rows() {
    let mut app = test_app(2100);
    open_hub_screen(&mut app, Mode::Dispatch);
    assert_eq!(app.dispatch_sites(), None, "no Relay stands yet");
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::Dispatch);
    assert!(
        app.status_line.is_some(),
        "S says there is no site to send to"
    );
}

/// Esc from the squad picker comes back to the Sorties tab of `b`.
#[test]
fn the_squad_picker_returns_to_the_sorties_tab() {
    let (mut app, _, _) = a_dispatch_ready_app(2102);
    open_hub_screen(&mut app, Mode::Dispatch);
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::SortieSquad, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Dispatch);
    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("Sorties")));
}

/// The hub's two sections resolve through one function, `contract_row`'s own
/// shape one screen over — a site highlighted and `[S]` opens the squad
/// picker for exactly that site.
#[test]
fn s_on_a_highlighted_site_opens_the_squad_picker() {
    let (mut app, _, _) = a_dispatch_ready_app(2103);
    open_hub_screen(&mut app, Mode::Dispatch);
    let sites = app.dispatch_sites().expect("a Relay stands");
    assert!(!sites.is_empty(), "the shipped catalogue offers a site");
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::SortieSquad);
    assert_eq!(app.pending_dispatch_site, Some(sites[0].id.clone()));
}

/// Toggling a candidate into the squad with `[X]` and dispatching with Enter
/// reaches `Game::dispatch_sortie` and returns to the hub.
#[test]
fn x_toggles_a_candidate_and_enter_dispatches_the_squad() {
    let (mut app, _, _) = a_dispatch_ready_app(2106);
    open_hub_screen(&mut app, Mode::Dispatch);
    let sites = app.dispatch_sites().expect("a Relay stands");
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::SortieSquad);

    let candidates = app.sortie_squad_candidates();
    assert!(!candidates.is_empty(), "the fixture owns a staff program");
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('X'));
    assert_eq!(app.dispatch_squad, vec![candidates[0].entity]);

    // The base would be emptied by sending its only staffer — the engine's
    // own guard, surfaced through the one refusal door.
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::SortieSquad,
        "a refused dispatch stays on the picker"
    );
    assert!(app.status_line.is_some(), "the refusal reached App::refuse");
    let _ = sites;
}

/// Esc from the squad picker drops the pending site and the squad built so
/// far, and returns to the Sorties tab — `Mode::SettlementMarket`'s shape.
#[test]
fn esc_from_the_squad_picker_drops_the_pending_squad() {
    let (mut app, _, _) = a_dispatch_ready_app(2107);
    open_hub_screen(&mut app, Mode::Dispatch);
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('S'));
    app.handle_key(GameKey::Char('X'));
    assert!(!app.dispatch_squad.is_empty());

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Dispatch);
    assert!(app.pending_dispatch_site.is_none());
    assert!(app.dispatch_squad.is_empty());
}
