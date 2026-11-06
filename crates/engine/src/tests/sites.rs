//! The Sites stores: saved with the game, written at the doors, read back as
//! views. A RON round trip cannot see a skipped field, so persistence is
//! asserted through a real `Game::save` and `Game::load`.

use super::breeding::save_and_load;
use super::support::*;
use crate::RouteDestinationId;
use crate::settlements::SettlementKey;
use crate::sites::{SeenOffer, SeenShelf, SiteLogKind};
use crate::*;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn texts(game: &Game, site: RouteDestinationId) -> Vec<String> {
    game.sites
        .log
        .newest_first(site)
        .map(|e| e.text.clone())
        .collect()
}

#[test]
fn the_site_log_survives_save_and_load() {
    let mut game = game();
    let town = RouteDestinationId::Settlement(SettlementKey { rx: 2, ry: -1 });
    let outpost = RouteDestinationId::Outpost((7, 9));
    game.note_site(town, SiteLogKind::Visit, "Walked in.".into());
    game.note_site(town, SiteLogKind::Trade, "Sold wafers.".into());
    game.note_site(outpost, SiteLogKind::Founded, "Founded.".into());

    let loaded = save_and_load(&mut game, "site_log");

    assert_eq!(texts(&loaded, town), vec!["Sold wafers.", "Walked in."]);
    assert_eq!(texts(&loaded, outpost), vec!["Founded."]);
    assert_eq!(texts(&loaded, town), texts(&game, town));
}

#[test]
fn a_seen_shelf_survives_save_and_load() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 1 };
    let shelf = SeenShelf {
        tick: 77,
        offers: vec![SeenOffer {
            name: "Logic Wafer".into(),
            detail: "x".into(),
            unit_cost: 12,
            qty: 3,
        }],
    };
    game.sites.seen.record(key, shelf.clone());

    let loaded = save_and_load(&mut game, "seen_shelf");

    assert_eq!(loaded.sites.seen.get(key), Some(&shelf));
}

// ---- writers ----

/// `tests/settlements.rs`'s fixture: materialised well clear and fully
/// grown first, so the first tick's footprint growth cannot shove the party,
/// then the party stands one step west of its nearest cell.
fn town_east_of_player(game: &mut Game, rx: i32) -> SettlementKey {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let key = SettlementKey { rx, ry: 0 };
    let target = (pos.x + 5, pos.y);
    place_settlement(game, key, target.0, target.1);
    game.sync_settlement_footprint(key);
    let radius = game.settlement_radius(key).expect("materialised");
    let mut player_pos = game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap();
    player_pos.x = target.0 - radius - 1;
    player_pos.y = target.1;
    key
}

fn town_site(key: SettlementKey) -> RouteDestinationId {
    RouteDestinationId::Settlement(key)
}

fn kinds(game: &Game, site: RouteDestinationId) -> Vec<SiteLogKind> {
    game.sites.log.newest_first(site).map(|e| e.kind).collect()
}

fn set_standing(game: &mut Game, key: SettlementKey, standing: i32) {
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = standing;
}

#[test]
fn a_first_bump_logs_a_first_visit_and_the_next_a_visit() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    let name = game.settlement_name(key);

    game.move_player(1, 0);
    assert_eq!(kinds(&game, town_site(key)), vec![SiteLogKind::FirstVisit]);
    assert!(texts(&game, town_site(key))[0].contains(&name));

    game.move_player(1, 0);
    assert_eq!(
        kinds(&game, town_site(key)),
        vec![SiteLogKind::Visit, SiteLogKind::FirstVisit]
    );
}

#[test]
fn a_committed_trade_logs_its_net_credits() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    let scrap = crate::items::ItemId::from(crate::items::ids::CORE_FRAGMENT);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(scrap.clone(), 5);
    let copy = GearCopy::plain(scrap);

    game.commit_settlement_basket(key, vec![(copy, 5)], Vec::new())
        .expect("the basket commits");

    let last = game.sites.log.newest_first(town_site(key)).next().unwrap();
    assert_eq!(last.kind, SiteLogKind::Trade);
    assert!(last.text.starts_with("Trade netted "), "{}", last.text);
}

#[test]
fn only_a_band_crossing_logs_a_standing_change() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);

    game.adjust_standing(key, 1);
    assert!(kinds(&game, town_site(key)).is_empty(), "inside the band");

    game.adjust_standing(key, crate::tuning::SETTLEMENT_WARM_STANDING);
    assert_eq!(kinds(&game, town_site(key)), vec![SiteLogKind::Standing]);
    assert!(texts(&game, town_site(key))[0].contains("Warm"));
}

#[test]
fn a_town_contract_logs_accept_and_abandon_on_its_issuer() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    let id = game.settlement_board(key).unwrap()[0].id.clone();

    game.accept_contract(&id, Some(key)).unwrap();
    assert_eq!(
        kinds(&game, town_site(key)),
        vec![SiteLogKind::ContractAccepted]
    );
    assert!(game.abandon_contract(&id));
    let after = kinds(&game, town_site(key));
    assert_eq!(after[0], SiteLogKind::ContractAbandoned);

    // Finishing a second job logs the delivery on its issuer too.
    game.accept_contract(&id, Some(key)).unwrap();
    let idx = game
        .world
        .resource::<crate::resources::ActiveContracts>()
        .active
        .iter()
        .position(|c| c.def.id == id)
        .unwrap();
    game.complete_contract(idx);
    assert!(kinds(&game, town_site(key)).contains(&SiteLogKind::ContractDelivered));

    // The Broker's own contracts have no issuer and log nowhere.
    let total: usize = game
        .sites
        .log
        .to_save()
        .iter()
        .map(|s| s.entries.len())
        .sum();
    let broker = game
        .world
        .resource::<crate::resources::ActiveContracts>()
        .active
        .iter()
        .find(|c| c.issuer.is_none())
        .map(|c| c.def.id.clone());
    if let Some(broker) = broker {
        let idx = game
            .world
            .resource::<crate::resources::ActiveContracts>()
            .active
            .iter()
            .position(|c| c.def.id == broker)
            .unwrap();
        game.complete_contract(idx);
        let after: usize = game
            .sites
            .log
            .to_save()
            .iter()
            .map(|s| s.entries.len())
            .sum();
        assert_eq!(after, total, "a Broker contract logs on no site");
    }
}

#[test]
fn a_gift_logs_on_the_giving_town() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_ALLIED_STANDING);

    game.request_program_gift(key)
        .expect("an allied town gives");

    assert_eq!(kinds(&game, town_site(key))[0], SiteLogKind::Gift);
}

#[test]
fn a_returning_route_logs_its_proceeds() {
    let (mut game, item, key) = super::routes::a_dispatch_ready_base(7000, 300);
    game.dispatch_route(key, vec![(item, 300)], false)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    assert!(kinds(&game, town_site(key)).contains(&SiteLogKind::RouteDelivered));
}

#[test]
fn an_outpost_route_logs_the_stock_it_hauled_home() {
    let tile = (500, 500);
    let mut game = super::routes::an_outpost_ready_base(8200, tile);
    let item = crate::items::ItemId::from("cache_grain");
    super::routes::stock_the_outpost(&mut game, tile, &item, 5);
    game.dispatch_outpost_route(tile, false).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    let site = RouteDestinationId::Outpost(tile);
    assert_eq!(kinds(&game, site), vec![SiteLogKind::RouteHauledHome]);
    assert!(texts(&game, site)[0].contains('5'));
}

#[test]
fn an_outpost_raid_logs_raided_then_dark() {
    use crate::outposts::Outpost;
    let tile = (500, 500);
    let mut game = game();
    let mut outpost = Outpost::new(
        crate::world::Biome::Deadlock,
        crate::tuning::OUTPOST_MAX_INTEGRITY,
    );
    outpost.integrity = 1;
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .insert(tile, outpost);

    // The raid roll is chance; sweep until one lands on a fresh game state.
    for _ in 0..200 {
        game.raid_one_outpost(tile);
        if kinds(&game, RouteDestinationId::Outpost(tile)).contains(&SiteLogKind::Dark) {
            break;
        }
    }
    let kinds = kinds(&game, RouteDestinationId::Outpost(tile));
    assert_eq!(kinds[0], SiteLogKind::Dark);
    assert_eq!(kinds[1], SiteLogKind::Raided);
}

#[test]
fn a_town_raid_on_the_base_logs_on_the_raiding_town() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);

    game.dev_force_town_raid(key);

    assert_eq!(kinds(&game, town_site(key)), vec![SiteLogKind::RaidedBase]);
}

#[test]
fn no_log_write_draws_rng() {
    use rand::RngExt;
    let draw = |game: &mut Game| -> u64 {
        game.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    };
    let mut control = game();
    let expected = draw(&mut control);

    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    let id = game.settlement_board(key).unwrap()[0].id.clone();
    game.adjust_standing(key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.accept_contract(&id, Some(key)).unwrap();
    game.abandon_contract(&id);
    game.dev_force_town_raid(key);
    game.note_site(town_site(key), SiteLogKind::Visit, "x".into());
    assert!(!kinds(&game, town_site(key)).is_empty());

    assert_eq!(draw(&mut game), expected, "a log write moved the stream");
}
