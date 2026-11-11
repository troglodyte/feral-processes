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

// ---- shelf capture and the live shelf ----

fn set_tick(game: &mut Game, tick: u64) {
    game.world
        .resource_mut::<crate::resources::GameClock>()
        .tick = tick;
}

fn visit(game: &mut Game) {
    game.move_player(1, 0);
    game.take_visit();
}

#[test]
fn bumping_a_town_records_its_shelf_as_seen() {
    use crate::sites::{TownGoods, seen_offers};
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);

    visit(&mut game);

    let expected = seen_offers(&game.settlement_view(key).unwrap().offers);
    assert!(!expected.is_empty(), "the premise: the town has a shelf");
    assert_eq!(
        game.town_goods(key),
        TownGoods::Seen {
            offers: expected,
            age: "just now"
        }
    );
}

#[test]
fn a_seen_shelf_keeps_its_prices_after_the_shelf_turns() {
    use crate::sites::TownGoods;
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    visit(&mut game);
    let TownGoods::Seen { offers: before, .. } = game.town_goods(key) else {
        panic!("seen after a visit");
    };

    let now = game.current_tick();
    set_tick(
        &mut game,
        now + 4 * crate::tuning::SETTLEMENT_MARKET_ROTATION_TICKS + 5000,
    );

    let TownGoods::Seen { offers, age } = game.town_goods(key) else {
        panic!("still seen");
    };
    assert_eq!(offers, before);
    assert_eq!(age, "long ago");
}

#[test]
fn a_town_never_reached_is_not_seen() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    assert_eq!(game.town_goods(key), crate::sites::TownGoods::NotSeen);
}

#[test]
fn the_research_shows_every_known_towns_live_shelf() {
    use crate::sites::{TownGoods, seen_offers};
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    assert!(!game.live_shelf_known());
    assert_eq!(game.town_goods(key), TownGoods::NotSeen);
    // Before reading the shelf beside the town: the chain raises the zone,
    // and prices follow the zone.
    unlock_research_chain(&mut game, crate::tuning::LIVE_SHELF_RESEARCH);
    assert!(game.live_shelf_known());
    let beside = seen_offers(&game.settlement_view(key).unwrap().offers);
    // Far from the town, with no visit ever recorded.
    let mut pos = game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap();
    pos.x += 400;
    assert!(game.settlement_view(key).is_none(), "out of reach");
    assert_eq!(game.town_goods(key), TownGoods::Live { offers: beside });
}

#[test]
fn a_commit_rerecords_the_shelf() {
    use crate::sites::TownGoods;
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    visit(&mut game);
    let now = game.current_tick();
    set_tick(&mut game, now + 10_000);
    let scrap = crate::items::ItemId::from(crate::items::ids::CORE_FRAGMENT);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(scrap.clone(), 2);
    assert!(matches!(
        game.town_goods(key),
        TownGoods::Seen {
            age: "long ago",
            ..
        }
    ));

    game.commit_settlement_basket(key, vec![(GearCopy::plain(scrap), 2)], Vec::new())
        .expect("the basket commits");

    assert!(matches!(
        game.town_goods(key),
        TownGoods::Seen {
            age: "just now",
            ..
        }
    ));
}

// ---- views ----

fn name_town(game: &mut Game, key: SettlementKey, name: &str, visited: bool) {
    let mut towns = game.world.resource_mut::<crate::resources::Settlements>();
    let known = towns.0.get_mut(&key).unwrap();
    known.def.name = name.to_string();
    known.visited = visited;
}

fn add_town(game: &mut Game, key: SettlementKey, name: &str, visited: bool, at: (i32, i32)) {
    place_settlement(game, key, at.0, at.1);
    name_town(game, key, name, visited);
}

/// The fixture world materializes some towns of its own; the rail tests
/// want only the ones they add.
fn forget_towns(game: &mut Game) {
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .clear();
}

fn add_outpost(game: &mut Game, tile: (i32, i32), name: &str) {
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .insert(
            tile,
            crate::outposts::Outpost {
                name: name.to_string(),
                ..crate::outposts::Outpost::new(
                    crate::world::Biome::Deadlock,
                    crate::tuning::OUTPOST_MAX_INTEGRITY,
                )
            },
        );
}

#[test]
fn the_rail_lists_towns_by_name_then_outposts_by_name() {
    use crate::sites::SiteSection;
    let mut game = game();
    forget_towns(&mut game);
    add_town(
        &mut game,
        SettlementKey { rx: 1, ry: 0 },
        "Zeta",
        true,
        (600, 0),
    );
    add_town(
        &mut game,
        SettlementKey { rx: 2, ry: 0 },
        "Alpha",
        true,
        (700, 0),
    );
    add_outpost(&mut game, (900, 900), "Outpost south");
    add_outpost(&mut game, (800, 800), "Outpost east");

    let rail = game.site_rail();

    let names: Vec<&str> = rail.iter().map(|e| e.name.as_str()).collect();
    let short: Vec<&str> = rail.iter().map(|e| e.rail_name.as_str()).collect();
    assert_eq!(
        &short[..2],
        ["Alpha E", "Zeta E"],
        "towns take compass letters"
    );
    assert_eq!(&short[2..], ["Outpost east", "Outpost south"]);
    assert_eq!(names[0], "Alpha east");
    assert_eq!(names[1], "Zeta east");
    assert_eq!(&names[2..], ["Outpost east", "Outpost south"]);
    assert_eq!(rail[0].section, SiteSection::Settlements);
    assert_eq!(rail[0].glyph, 's');
    assert_eq!(rail[2].section, SiteSection::Outposts);
}

#[test]
fn two_towns_with_one_name_stay_in_key_order() {
    let mut game = game();
    forget_towns(&mut game);
    let (a, b) = (
        SettlementKey { rx: 5, ry: 0 },
        SettlementKey { rx: 3, ry: 0 },
    );
    add_town(&mut game, a, "Twin", true, (600, 0));
    add_town(&mut game, b, "Twin", true, (700, 0));

    let sites: Vec<RouteDestinationId> = game.site_rail().iter().map(|e| e.site).collect();

    assert_eq!(sites, vec![town_site(b), town_site(a)]);
}

#[test]
fn an_unvisited_town_is_on_the_rail_by_name() {
    let mut game = game();
    forget_towns(&mut game);
    let key = SettlementKey { rx: 1, ry: 0 };
    add_town(&mut game, key, "Hidden", false, (600, 0));
    add_town(
        &mut game,
        SettlementKey { rx: 2, ry: 0 },
        "Seen",
        true,
        (700, 0),
    );

    let rail = game.site_rail();

    let listed: Vec<&str> = rail.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(listed, vec!["Hidden east", "Seen east"]);
    assert_eq!(rail[0].site, town_site(key));
}

#[test]
fn a_hostile_town_and_a_damaged_outpost_are_marked() {
    let mut game = game();
    forget_towns(&mut game);
    let calm = SettlementKey { rx: 1, ry: 0 };
    let angry = SettlementKey { rx: 2, ry: 0 };
    add_town(&mut game, calm, "Calm", true, (600, 0));
    add_town(&mut game, angry, "Angry", true, (700, 0));
    set_standing(&mut game, angry, crate::tuning::SETTLEMENT_HOSTILE_STANDING);
    add_outpost(&mut game, (800, 800), "Camp");
    add_outpost(&mut game, (900, 900), "Camp");
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .get_mut(&(900, 900))
        .unwrap()
        .integrity = 3;

    let marks: Vec<bool> = game.site_rail().iter().map(|e| e.attention).collect();

    assert_eq!(
        marks,
        vec![true, false, false, true],
        "Angry, Calm, then the damaged outpost"
    );
}

#[test]
fn reach_is_base_beside_a_site_or_neither() {
    use crate::sites::SiteReach;
    let mut game = game();
    let anchor = game.anchor_position().unwrap();
    game.dev_lay_floor([anchor]);

    assert_eq!(
        game.site_reach(),
        SiteReach {
            on_base: true,
            beside: None
        }
    );

    // Beside a town that is also next to the base floor: both hold.
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(&mut game, key, anchor.0 + 1, anchor.1);
    name_town(&mut game, key, "Close", true);
    let reach = game.site_reach();
    assert!(reach.on_base);
    assert_eq!(reach.beside, Some(town_site(key)));

    // Far from everything: beside nothing, and off the base.
    let mut pos = game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap();
    pos.x += 400;
    let reach = game.site_reach();
    assert!(!reach.on_base && reach.beside.is_none());
}

#[test]
fn an_unvisited_town_in_reach_is_not_beside() {
    let mut game = game();
    let anchor = game.anchor_position().unwrap();
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(&mut game, key, anchor.0 + 1, anchor.1);
    name_town(&mut game, key, "Unbumped", false);
    assert!(game.settlement_reach(key));
    assert_eq!(game.site_reach().beside, None);
}

#[test]
fn town_status_reads_the_same_from_base_and_beside() {
    let mut game = game();
    let anchor = game.anchor_position().unwrap();
    game.dev_lay_floor([anchor]);
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(&mut game, key, anchor.0 + 1, anchor.1);
    name_town(&mut game, key, "Close", true);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_ALLIED_STANDING);
    assert_eq!(game.site_reach().beside, Some(town_site(key)));
    let beside = game.town_status(key).unwrap();

    let mut pos = game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap();
    pos.x += 400;
    assert_eq!(game.site_reach().beside, None);

    assert_eq!(game.town_status(key).unwrap(), beside);
    assert_eq!(beside.standing, "Allied");
    assert!(!beside.hostile);
    assert!(
        beside.condition.is_some(),
        "a town reads its growth outlook"
    );
    assert!(!beside.meaning.is_empty());
}

#[test]
fn a_route_line_carries_no_tick_figure() {
    let (mut game, item, key) = super::routes::a_dispatch_ready_base(7000, 300);
    game.dispatch_route(key, vec![(item, 300)], false)
        .expect("a legal dispatch");

    for lines in [game.site_routes(town_site(key)), game.routes_in_flight()] {
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.leg, "Outbound");
        assert!(line.cargo.starts_with("300 "), "{}", line.cargo);
        assert!(
            !line.due.chars().any(|c| c.is_ascii_digit()),
            "{}",
            line.due
        );
    }
    assert!(
        game.site_routes(RouteDestinationId::Outpost((1, 1)))
            .is_empty()
    );
}

fn market_base(game: &mut Game) {
    game.lay_test_pocket();
    for (kind, x) in [("home", 0), ("market", 2)] {
        game.world.spawn((
            crate::components::Structure {
                kind: kind.to_string(),
            },
            Position { x, y: 0 },
            crate::components::Glyph {
                ch: '!',
                color: crate::components::GlyphColor::Yellow,
            },
        ));
    }
    stand_in_base_at(game, 1, 1);
}

#[test]
fn the_next_trader_is_named_and_banded() {
    let mut game = game();
    skip_tutorial(&mut game);
    assert_eq!(game.caravan_schedule().next, None, "no market, no trader");
    market_base(&mut game);
    let visit = game.visit_at(game.visit_index()).unwrap();
    set_tick(&mut game, visit.arrival_tick.saturating_sub(10));
    if visit.arrival_tick < 10 {
        return;
    }

    let schedule = game.caravan_schedule();

    assert_eq!(schedule.docked, None);
    let (name, when) = schedule.next.expect("a visit is coming");
    assert!(!name.is_empty());
    assert_eq!(when, "any moment");
}

#[test]
fn a_trader_still_walking_in_is_not_docked() {
    let mut game = game();
    skip_tutorial(&mut game);
    market_base(&mut game);
    let visit = game.visit_at(game.visit_index()).unwrap();
    set_tick(&mut game, visit.arrival_tick);
    game.caravan_tick();
    assert_eq!(game.caravan_schedule().docked, None, "it has not arrived");
}

fn dock_the_trader(game: &mut Game) {
    let visit = game.visit_at(game.visit_index()).unwrap();
    set_tick(game, visit.arrival_tick);
    game.caravan_tick();
    for _ in 0..600 {
        let mut query = game.world.query::<&crate::components::Caravan>();
        if query
            .iter(&game.world)
            .any(|c| matches!(c.stage, crate::components::CaravanStage::Docked))
        {
            return;
        }
        game.caravan_tick();
        game.world
            .resource_mut::<crate::resources::GameClock>()
            .tick += 1;
    }
    panic!("the trader never docked");
}

#[test]
fn a_docked_trader_says_how_long_it_stays() {
    let mut game = game();
    skip_tutorial(&mut game);
    market_base(&mut game);
    dock_the_trader(&mut game);
    let visit = game.visit_at(game.visit_index()).unwrap();
    assert_eq!(game.caravan_schedule().docked.unwrap().1, "a while yet");
    set_tick(&mut game, visit.depart_tick - 100);

    let schedule = game.caravan_schedule();

    let (_, leaves) = schedule.docked.expect("the trader is in");
    assert_eq!(leaves, "soon");
    let next = schedule.next.expect("and another is coming");
    assert!(!next.1.chars().any(|c| c.is_ascii_digit()));
}

#[test]
fn town_contracts_hold_only_that_towns_jobs() {
    let mut game = game();
    let key = town_east_of_player(&mut game, 0);
    let id = game.settlement_board(key).unwrap()[0].id.clone();
    assert!(game.town_contracts(key).is_empty());

    game.accept_contract(&id, Some(key)).unwrap();

    let rows = game.town_contracts(key);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert!(
        game.town_contracts(SettlementKey { rx: 9, ry: 9 })
            .is_empty()
    );
}

#[test]
fn the_log_reads_newest_first_with_banded_ages() {
    let mut game = game();
    let site = town_site(SettlementKey { rx: 1, ry: 1 });
    game.note_site(site, SiteLogKind::Visit, "First.".into());
    set_tick(&mut game, 10_000);
    game.note_site(site, SiteLogKind::Trade, "Second.".into());

    let log = game.site_log(site);

    assert_eq!(log[0].text, "Second.");
    assert_eq!(log[0].age, "just now");
    assert_eq!(log[1].text, "First.");
    assert_eq!(log[1].age, "long ago");
}

#[test]
fn town_rail_label_appends_the_heading_from_the_base() {
    use crate::game::sites::town_rail_label;
    assert_eq!(
        town_rail_label("Tally Yard", (10, -10), Some((0, 0))),
        (
            "Tally Yard north-east".to_string(),
            "Tally Yard NE".to_string()
        )
    );
}

#[test]
fn town_rail_label_is_bare_at_the_base_or_without_one() {
    use crate::game::sites::town_rail_label;
    let bare = ("Tally Yard".to_string(), "Tally Yard".to_string());
    assert_eq!(town_rail_label("Tally Yard", (0, 0), Some((0, 0))), bare);
    assert_eq!(town_rail_label("Tally Yard", (50, 50), None), bare);
}

#[test]
fn same_named_towns_stay_separate_pages() {
    let mut game = game();
    forget_towns(&mut game);
    let anchor = game.anchor_position().expect("a base");
    let (a, b) = (
        SettlementKey { rx: 1, ry: 0 },
        SettlementKey { rx: 2, ry: 0 },
    );
    add_town(&mut game, a, "Twin", true, (anchor.0 + 600, anchor.1));
    add_town(&mut game, b, "Twin", true, (anchor.0 - 600, anchor.1));

    let rail = game.site_rail();

    assert_eq!(rail.len(), 2);
    assert_ne!(rail[0].site, rail[1].site);
    assert_ne!(rail[0].name, rail[1].name);
}
