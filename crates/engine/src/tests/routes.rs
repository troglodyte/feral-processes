//! Caravan routes: the record, its save form, the dispatch doors and the
//! tick that runs them — Tasks 1 through 4, per
//! `docs/superpowers/plans/2026-09-05-settlements-phase-6-routes.md`.

use bevy_ecs::prelude::Entity;

use super::support::{scratch_assets_dir, test_assets_dir};
use crate::Game;
use crate::components::{Glyph, GlyphColor, Inventory, Position, Stock, Structure};
use crate::game::route::{RouteDestinationId, RouteRefusal};
use crate::items::ItemId;
use crate::resources::DifficultyMode;
use crate::routes::{Route, RouteEnd, RouteLeg};
use crate::settlements::relations::{Relation, Standing};
use crate::settlements::{SettlementDef, SettlementKey, SettlementKind, Specialty, Temperament};

/// A destination town, resolved, the way a route's record stores it.
fn a_destination() -> SettlementDef {
    SettlementDef {
        id: "test_town".to_string(),
        name: "Test Town".to_string(),
        blurb: "A town for tests.".to_string(),
        kind: SettlementKind::Server,
        specialty: Specialty::Materials,
        temperament: Temperament::Open,
    }
}

/// A route in flight, half a leg in, carrying one line of cargo.
fn an_in_flight_route() -> Route {
    Route {
        destination: RouteEnd::Settlement {
            key: SettlementKey { rx: 3, ry: -2 },
            def: a_destination(),
            tile: (300, -200),
        },
        cargo: vec![(ItemId("cache_grain".to_string()), 12)],
        standing: true,
        stalled: false,
        leg: RouteLeg::Outbound,
        ticks_total: 400,
        ticks_elapsed: 150,
        proceeds: 0,
    }
}

/// A route in flight survives a **real save round trip** — packed back into
/// a save file and loaded, not a bare RON round trip, which would leave a
/// `#[serde(skip)]` green. `an_in_flight_sortie_survives_a_save_and_load`'s
/// shape.
#[test]
fn a_route_in_flight_survives_a_real_save_round_trip() {
    let scratch = scratch_assets_dir("route_inflight_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(6000, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let before = an_in_flight_route();
    game.world
        .resource_mut::<crate::resources::Routes>()
        .0
        .push(before.clone());

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let routes = &loaded.world.resource::<crate::resources::Routes>().0;
    assert_eq!(routes.len(), 1, "the route in flight must survive the load");
    let after = &routes[0];
    assert_eq!(after.destination, before.destination);
    assert_eq!(after.cargo, before.cargo);
    assert_eq!(after.standing, before.standing);
    assert_eq!(after.stalled, before.stalled);
    assert_eq!(after.leg, before.leg);
    assert_eq!(after.ticks_total, before.ticks_total);
    assert_eq!(after.ticks_elapsed, before.ticks_elapsed);
    assert_eq!(after.proceeds, before.proceeds);
}

/// The whole feature is additive behind `#[serde(default)]` — no
/// `SAVE_FORMAT_VERSION` bump. `sorties::a_pre_sortie_save_loads_with_no_sorties`'
/// shape.
#[test]
fn save_format_version_is_unchanged_by_routes() {
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        32,
        "adding a route field is additive under field-named RON and must not \
         cost a version bump — see the doc comment on SAVE_FORMAT_VERSION"
    );
}

/// A save written before routes existed carries no `routes` key at all, and
/// must load with none rather than refusing or panicking.
#[test]
fn a_pre_routes_save_loads_with_no_routes() {
    let scratch = scratch_assets_dir("route_pre_save");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(6001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::Routes>()
        .0
        .push(an_in_flight_route());
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();

    // Stripped to what a save written before this branch looked like — the
    // real save's own `routes` key, removed, rather than a hand-built RON
    // fixture that only proves the parser accepts an absent field.
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.player.routes.clear();
    let text = crate::save::to_ron(&data).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("routes:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the key must have been there to strip, or this proves nothing"
    );
    let old_path = scratch.join("old.bin");
    let stripped_data = crate::save::from_ron(&stripped).expect("a pre-routes save still parses");
    crate::save::save_to_file(&old_path, &stripped_data).unwrap();

    let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
    assert!(
        loaded
            .world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a pre-routes save has no routes in flight"
    );
}

// ---------------------------------------------------------------- Task 3
// the dispatch doors

/// Stands a Home and a Relay up in base space and puts the party on the
/// laid floor beside them — `tests::sorties::deploy_relay`'s shape,
/// repeated here rather than shared: each fixture file owns its own.
pub(super) fn deploy_relay(game: &mut Game) {
    game.lay_starting_pocket();
    deploy_structure(game, "home", 0, 0);
    deploy_structure(game, "relay", 1, 0);
    super::support::stand_in_base_at(game, 1, 1);
}

/// A structure of `kind` standing at `(x, y)` in base space.
fn deploy_structure(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Glyph {
                ch: 'K',
                color: GlyphColor::Magenta,
            },
        ))
        .id()
}

/// A Depot at `(x, y)` holding `qty` of `item` on its output shelf.
fn deploy_depot(game: &mut Game, x: i32, y: i32, item: &ItemId, qty: u32) {
    let depot = deploy_structure(game, "depot", x, y);
    game.world.entity_mut(depot).insert(Stock {
        output: [(item.clone(), qty)].into_iter().collect(),
        capacity: 9_999,
        ..Default::default()
    });
}

/// Registers a known settlement at `key`/`tile` with no map entity —
/// `tests::settlement_market::register_settlement`'s shape: a route test
/// needs the town *known*, never walked to.
fn register_settlement(game: &mut Game, key: SettlementKey, def: SettlementDef, tile: (i32, i32)) {
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .insert(
            key,
            crate::resources::KnownSettlement {
                tile,
                def,
                visited: false,
            },
        );
}

/// Sets a town's standing directly, skipping every trade and contract mover
/// that would ordinarily earn it.
fn set_standing(game: &mut Game, key: SettlementKey, standing: i32) {
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .insert(
            key,
            Relation {
                standing,
                trade_credits: 0,
                ..Default::default()
            },
        );
}

/// A base with a Home, a Relay and a Depot holding `qty` of a Material item,
/// plus one known settlement — `Neutral` by default — reachable from it.
/// Returns the game, the cargo item and the destination's key.
fn a_dispatch_ready_base(seed: u32, qty: u32) -> (Game, ItemId, SettlementKey) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let item = ItemId::from("cache_grain");
    deploy_depot(&mut game, 0, 1, &item, qty);
    let key = SettlementKey { rx: 5, ry: 5 };
    register_settlement(&mut game, key, a_destination(), (500, 500));
    (game, item, key)
}

/// Sums what the base's shelves hold, so a refusal can be shown to have
/// spent nothing.
fn stock_total(game: &Game) -> u32 {
    game.base_stock().iter().map(|r| r.qty).sum()
}

/// No board without a Relay, and no panic either — `no_relay_means_no_board`'s
/// shape.
#[test]
fn no_relay_means_no_route_destinations() {
    let mut game = Game::new(6000, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.route_destinations().is_none());
}

/// A Relay with nothing known yet is an empty board, not the absence of
/// one — `board_defs`' three-state rule, and once a town is known it lists
/// its band and the duration `dispatch_route` will actually run.
#[test]
fn route_destinations_lists_every_known_settlement() {
    let mut game = Game::new(6001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    // `Game::new` already runs `ensure_local_settlements`, so the fixture
    // clears what world generation found nearby before asserting the empty
    // state — otherwise this proves nothing about the three-state rule.
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .clear();
    assert_eq!(
        game.route_destinations(),
        Some(Vec::new()),
        "a Relay with no known settlement yet is an empty board"
    );

    let key = SettlementKey { rx: 2, ry: -1 };
    register_settlement(&mut game, key, a_destination(), (200, -100));
    let rows = game.route_destinations().expect("a Relay stands");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.destination, RouteDestinationId::Settlement(key));
    assert_eq!(row.name, a_destination().name);
    assert_eq!(row.band, Some(Standing::Neutral));
    let (ax, ay) = game.anchor_position().unwrap();
    let d = (ax - 200).abs().max((ay - (-100)).abs()) as u64;
    assert_eq!(
        row.ticks,
        crate::tuning::ROUTE_TICKS_BASE + crate::tuning::ROUTE_TICKS_PER_TILE * d,
        "the row must quote the same duration the trip will actually run"
    );
}

/// `route_quote` is the sum of `settlement_sell_price` over the manifest —
/// the one derivation a preview and a sale both call, so they cannot quote
/// different numbers.
#[test]
fn route_quote_sums_settlement_sell_price_per_line() {
    let (game, item, key) = a_dispatch_ready_base(6002, 40);
    let temperament = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .get(&key)
        .unwrap()
        .def
        .temperament;
    let cargo = vec![(item.clone(), 7)];
    let expected = game.settlement_sell_price(&item, temperament) * 7;
    assert_eq!(game.route_quote(&cargo, temperament), expected);
}

/// `route_manifest_quote` is `route_quote` with the destination's own
/// `Temperament` resolved internally — the cargo picker's live preview has
/// no `Temperament` of its own to carry across the app-core/engine seam, so
/// this is the one door that resolves it rather than leaking the type out.
#[test]
fn route_manifest_quote_resolves_the_destinations_own_temperament() {
    let (game, item, key) = a_dispatch_ready_base(6003, 40);
    let temperament = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .get(&key)
        .unwrap()
        .def
        .temperament;
    let cargo = vec![(item.clone(), 7)];
    let expected = game.route_quote(&cargo, temperament);
    assert_eq!(game.route_manifest_quote(key, &cargo), Some(expected));
}

/// A destination the run has not discovered has no `Temperament` to
/// resolve, so the quote answers `None` rather than pricing off a default.
#[test]
fn route_manifest_quote_is_none_for_an_unknown_destination() {
    let (game, item, _) = a_dispatch_ready_base(6004, 40);
    let cargo = vec![(item, 7)];
    let unknown = SettlementKey { rx: 999, ry: 999 };
    assert_eq!(game.route_manifest_quote(unknown, &cargo), None);
}

/// Every refusal lands before anything is spent, asserted **per refusal** —
/// a single test over one of them passes against every path that never
/// spends anyway. `every_refusal_spends_nothing`'s shape, one door over.
#[test]
fn every_refusal_leaves_stock_and_routes_exactly_as_they_were() {
    #[allow(clippy::type_complexity)]
    let cases: Vec<(
        &str,
        Box<dyn Fn() -> (Game, SettlementKey, Vec<(ItemId, u32)>, bool)>,
    )> = vec![
        (
            "not at the Relay",
            Box::new(|| {
                let (mut game, item, key) = a_dispatch_ready_base(6100, 20);
                game.world
                    .insert_resource(crate::resources::Locale::Surface);
                (game, key, vec![(item, 5)], false)
            }),
        ),
        (
            "an unknown destination",
            Box::new(|| {
                let (game, item, _key) = a_dispatch_ready_base(6101, 20);
                (
                    game,
                    SettlementKey { rx: 99, ry: 99 },
                    vec![(item, 5)],
                    false,
                )
            }),
        ),
        (
            "a Hostile town",
            Box::new(|| {
                let (mut game, item, key) = a_dispatch_ready_base(6102, 20);
                set_standing(&mut game, key, crate::tuning::SETTLEMENT_HOSTILE_STANDING);
                (game, key, vec![(item, 5)], false)
            }),
        ),
        (
            "standing asked for below Warm",
            Box::new(|| {
                let (game, item, key) = a_dispatch_ready_base(6103, 20);
                (game, key, vec![(item, 5)], true)
            }),
        ),
        (
            "an empty manifest",
            Box::new(|| {
                let (game, _item, key) = a_dispatch_ready_base(6104, 20);
                (game, key, Vec::new(), false)
            }),
        ),
        (
            "understocked cargo",
            Box::new(|| {
                let (game, item, key) = a_dispatch_ready_base(6105, 3);
                (game, key, vec![(item, 20)], false)
            }),
        ),
        (
            "a duplicate destination",
            Box::new(|| {
                let (mut game, item, key) = a_dispatch_ready_base(6106, 40);
                game.dispatch_route(key, vec![(item.clone(), 5)], false)
                    .expect("the first dispatch must succeed");
                (game, key, vec![(item, 5)], false)
            }),
        ),
        (
            "too many routes",
            Box::new(|| {
                let (mut game, item, _key) = a_dispatch_ready_base(6107, 400);
                for n in 0..crate::tuning::ROUTE_MAX_ACTIVE {
                    let k = SettlementKey {
                        rx: 10 + n as i32,
                        ry: 10,
                    };
                    register_settlement(&mut game, k, a_destination(), (1000 + n as i32, 1000));
                    game.dispatch_route(k, vec![(item.clone(), 5)], false)
                        .expect("filling to the cap must succeed");
                }
                let extra = SettlementKey { rx: 999, ry: 999 };
                register_settlement(&mut game, extra, a_destination(), (2000, 2000));
                (game, extra, vec![(item, 5)], false)
            }),
        ),
    ];

    for (name, build) in cases {
        let (mut game, key, cargo, standing) = build();
        let before_stock = stock_total(&game);
        let before_routes = game.world.resource::<crate::resources::Routes>().0.len();
        assert!(
            game.dispatch_route(key, cargo, standing).is_err(),
            "{name} should have been refused"
        );
        assert_eq!(stock_total(&game), before_stock, "{name} spent something");
        assert_eq!(
            game.world.resource::<crate::resources::Routes>().0.len(),
            before_routes,
            "{name} filed a record anyway"
        );
    }
}

/// A refusal names its own reason where the reason is worth naming, rather
/// than a bare `Err(())` a screen could not word.
#[test]
fn a_hostile_town_is_refused_with_its_own_variant() {
    let (mut game, item, key) = a_dispatch_ready_base(6200, 20);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_HOSTILE_STANDING);
    assert_eq!(
        game.dispatch_route(key, vec![(item, 5)], false),
        Err(RouteRefusal::Refused)
    );
}

#[test]
fn a_standing_route_below_warm_is_refused_with_its_own_variant() {
    let (mut game, item, key) = a_dispatch_ready_base(6201, 20);
    assert_eq!(
        game.dispatch_route(key, vec![(item, 5)], true).unwrap_err(),
        RouteRefusal::NoStandingRoutes
    );
}

/// The stricter gate is standing's alone — a one-off dispatch needs only
/// `!refuses_service`, so the same Neutral town takes it.
#[test]
fn a_one_off_below_warm_still_dispatches() {
    let (mut game, item, key) = a_dispatch_ready_base(6202, 20);
    assert!(game.dispatch_route(key, vec![(item, 5)], false).is_ok());
}

#[test]
fn understocked_cargo_names_what_is_short() {
    let (mut game, item, key) = a_dispatch_ready_base(6203, 3);
    let err = game
        .dispatch_route(key, vec![(item.clone(), 20)], false)
        .unwrap_err();
    assert_eq!(
        err,
        RouteRefusal::Understocked {
            item,
            need: 20,
            held: 3,
        }
    );
}

/// A successful dispatch spends the manifest and files the whole resolved
/// record — `dispatch_sortie`'s shape.
#[test]
fn a_legal_dispatch_spends_cargo_and_records_a_route() {
    let (mut game, item, key) = a_dispatch_ready_base(6300, 40);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    let before = stock_total(&game);
    let (ax, ay) = game.anchor_position().unwrap();
    let tile = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .get(&key)
        .unwrap()
        .tile;
    let d = (ax - tile.0).abs().max((ay - tile.1).abs()) as u64;
    let expected_ticks = crate::tuning::ROUTE_TICKS_BASE + crate::tuning::ROUTE_TICKS_PER_TILE * d;

    game.dispatch_route(key, vec![(item.clone(), 12)], true)
        .expect("a legal dispatch");

    assert_eq!(stock_total(&game), before - 12);
    let routes = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.destination.settlement_key(), Some(key));
    assert_eq!(route.destination.tile(), tile);
    assert_eq!(route.cargo, vec![(item, 12)]);
    assert!(route.standing);
    assert!(!route.stalled);
    assert_eq!(route.leg, RouteLeg::Outbound);
    assert_eq!(route.ticks_total, expected_ticks);
    assert_eq!(route.ticks_elapsed, 0);
    assert_eq!(route.proceeds, 0);
}

/// A dispatch is *seen* to leave, the same as a sortie's squad —
/// `a_dispatch_queues_one_walk_out_per_member`'s shape, one cue rather than
/// one per member since cargo has no bodies.
#[test]
fn a_dispatch_queues_one_cargo_walk() {
    let (mut game, item, key) = a_dispatch_ready_base(6301, 40);
    game.dispatch_route(key, vec![(item, 5)], false)
        .expect("a legal dispatch");

    let walks = game.take_transits();
    assert_eq!(walks.len(), 1, "one cargo cue on departure");
    assert_eq!(
        walks[0].path.last(),
        Some(&crate::game::base_space::BASE_EXIT_CELL),
        "base space has one door and the walk ends at it"
    );
}

/// `Game::sever_route` clears `standing` and nothing else — the trip in
/// flight still completes and still pays.
#[test]
fn severing_clears_standing_and_nothing_else() {
    let (mut game, item, key) = a_dispatch_ready_base(6400, 40);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item.clone(), 5)], true)
        .unwrap();

    assert!(game.sever_route(key));
    let routes = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(routes.len(), 1, "severing does not drop the trip in flight");
    let route = &routes[0];
    assert!(!route.standing);
    assert_eq!(route.leg, RouteLeg::Outbound);
    assert_eq!(route.cargo, vec![(item, 5)]);

    assert!(
        !game.sever_route(key),
        "severing an already-severed route clears nothing further"
    );
}

#[test]
fn severing_an_absent_route_does_nothing() {
    let mut game = Game::new(6401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.sever_route(SettlementKey { rx: 1, ry: 1 }));
}

/// The report reads the record without changing it — `sortie_reports`'
/// rule, so a screen that draws it twice cannot move the trip.
#[test]
fn route_reports_reads_the_record_without_changing_it() {
    let (mut game, item, key) = a_dispatch_ready_base(6500, 40);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item, 5)], true).unwrap();

    let reports = game.route_reports();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.destination, RouteDestinationId::Settlement(key));
    assert!(report.standing);
    assert!(!report.stalled);
    assert_eq!(report.leg, RouteLeg::Outbound);

    let after = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(after.len(), 1);
    assert_eq!(
        after[0].ticks_elapsed, 0,
        "reading the report moved nothing"
    );
}

// ---------------------------------------------------------------- Task 4
// the tick

/// A second settlement definition, distinctly named from `a_destination`'s
/// "Test Town" — a predator and the town being raided must read apart in
/// the log, or a test asserting on a name proves nothing.
fn a_predator_def() -> SettlementDef {
    SettlementDef {
        id: "test_predator".to_string(),
        name: "Highwaymen's Watch".to_string(),
        blurb: "A place raised for a test.".to_string(),
        kind: SettlementKind::Server,
        specialty: Specialty::Materials,
        temperament: Temperament::Guarded,
    }
}

/// A full out-and-back pays exactly what `route_quote` quoted at dispatch,
/// and raises standing on the way — `credit_trade_volume`'s door, called
/// once the outbound leg sells.
#[test]
fn a_full_out_and_back_pays_the_quoted_proceeds_and_raises_standing() {
    let (mut game, item, key) = a_dispatch_ready_base(7000, 300);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    let temperament = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .get(&key)
        .unwrap()
        .def
        .temperament;
    let quote = game.route_quote(&[(item.clone(), 300)], temperament);
    assert!(
        quote >= crate::tuning::SETTLEMENT_TRADE_CREDITS_PER_POINT,
        "the manifest must be worth enough to move standing at all, or the test proves nothing"
    );

    let standing_before = game.standing(key);
    let currency = game.trade_currency();
    let credits_before: u32 = game
        .base_stock()
        .iter()
        .find(|r| r.item == currency)
        .map(|r| r.qty)
        .unwrap_or(0);

    game.dispatch_route(key, vec![(item, 300)], false)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a one-off route comes home for good"
    );
    let credits_after: u32 = game
        .base_stock()
        .iter()
        .find(|r| r.item == currency)
        .map(|r| r.qty)
        .unwrap_or(0);
    assert_eq!(
        credits_after - credits_before,
        quote,
        "the base must land exactly what was quoted, with nothing preying on this trip"
    );
    assert!(
        game.standing(key) > standing_before,
        "a paying trip must raise standing"
    );
}

/// A Hostile town beside the line takes its cut and says so in the log —
/// swept across seeds, `a_sortie_kill_leaves_a_downed_program_on_the_player`'s
/// reason: whether the one roll a leg completion makes lands is chance, and
/// a single seed proves only its own outcome.
#[test]
fn a_hostile_town_beside_the_route_taxes_it_and_says_so_in_the_log() {
    let found = (7100..7160).any(|seed| {
        let (mut game, item, key) = a_dispatch_ready_base(seed, 300);
        set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);

        let (ax, ay) = game.anchor_position().unwrap();
        let tile = game
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .unwrap()
            .tile;
        let midpoint = ((ax + tile.0) / 2, (ay + tile.1) / 2);
        let predator = SettlementKey { rx: 50, ry: 50 };
        register_settlement(&mut game, predator, a_predator_def(), midpoint);
        set_standing(
            &mut game,
            predator,
            crate::tuning::SETTLEMENT_HOSTILE_STANDING,
        );

        let temperament = game
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .unwrap()
            .def
            .temperament;
        let quote = game.route_quote(&[(item.clone(), 300)], temperament);

        game.dispatch_route(key, vec![(item, 300)], false)
            .expect("a legal dispatch");
        let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
        for _ in 0..total {
            game.run_routes();
        }

        // `run_routes` has just landed the outbound leg (predation against
        // the cargo, then the sale) — `route.proceeds` is what survived,
        // so a hit reads directly as a shortfall against the untaxed quote.
        // Predation is narrated only through the log as it happens, per
        // Finding 3 of the 2026-09-05 whole-branch review — `Route` keeps
        // no loss record of its own to read back.
        let route = &game.world.resource::<crate::resources::Routes>().0[0];
        let taxed = route.proceeds < quote;
        let logged = game
            .message_log(200)
            .iter()
            .any(|line| line.text.contains(&a_predator_def().name));
        taxed && logged
    });
    assert!(
        found,
        "no seed in the sweep saw the Hostile town take its cut and say so"
    );
}

/// A standing route reloads the same manifest and departs again on its own
/// arrival home, stock allowing.
#[test]
fn a_standing_route_departs_again_on_arrival() {
    let (mut game, item, key) = a_dispatch_ready_base(7200, 600);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item.clone(), 200)], true)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    let _ = game.take_transits();

    for _ in 0..(2 * total) {
        game.run_routes();
    }

    let routes = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(
        routes.len(),
        1,
        "a standing route does not come home for good"
    );
    let route = &routes[0];
    assert!(route.standing);
    assert!(!route.stalled);
    assert_eq!(route.leg, RouteLeg::Outbound, "it has departed again");
    assert_eq!(route.ticks_elapsed, 0, "the new leg has only just begun");
    assert_eq!(route.cargo, vec![(item, 200)], "the same manifest reloads");

    let walks = game.take_transits();
    assert!(
        !walks.is_empty(),
        "the reload's own departure must have queued a cue"
    );
}

/// Short stock stalls a standing route rather than severing it, and it is
/// retried every tick — restocking releases it on the very next one.
#[test]
fn short_stock_stalls_it_and_restocking_releases_it() {
    let (mut game, item, key) = a_dispatch_ready_base(7300, 200);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item.clone(), 200)], true)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;

    for _ in 0..(2 * total) {
        game.run_routes();
    }

    let route = game.world.resource::<crate::resources::Routes>().0[0].clone();
    assert!(
        route.stalled,
        "there is nothing left to reload, so it must park rather than depart or drop"
    );
    assert_eq!(
        route.leg,
        RouteLeg::Inbound,
        "it stays parked at the inbound-complete point"
    );

    // Restock, and the very next tick releases it.
    deploy_depot(&mut game, 0, 2, &item, 200);
    game.run_routes();

    let route = game.world.resource::<crate::resources::Routes>().0[0].clone();
    assert!(!route.stalled, "restocking must release the stall");
    assert_eq!(route.leg, RouteLeg::Outbound);
    assert_eq!(route.ticks_elapsed, 0);
}

/// A severed route completes its trip and pays, and does not go again.
#[test]
fn a_severed_route_completes_its_trip_and_pays_but_does_not_go_again() {
    let (mut game, item, key) = a_dispatch_ready_base(7400, 300);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item, 300)], true)
        .expect("a legal dispatch");
    assert!(game.sever_route(key));

    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    let before_stock = stock_total(&game);
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a severed route does not go again"
    );
    assert!(
        stock_total(&game) > before_stock,
        "it still pays on its way home"
    );
}

/// Predation is the only thing the tick may draw `GameRng` for, and it must
/// not draw at all when nothing is near enough to prey.
#[test]
fn the_tick_draws_no_rng_when_nothing_preys() {
    let (mut game, item, key) = a_dispatch_ready_base(7500, 300);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item, 300)], false)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(total - 1) {
        game.run_routes();
    }

    fn peek(g: &mut Game) -> u64 {
        use rand::RngExt;
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    super::support::reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    super::support::reseed_rng(&mut game, 55);
    game.run_routes();
    let with = peek(&mut game);

    assert_eq!(
        without, with,
        "no predator stands near this trip, so completing the leg must not touch GameRng"
    );
}

// ------------------------------------------------------------ Review findings
// 2026-09-05 whole-branch review, `docs/superpowers/plans/
// 2026-09-05-settlements-phase-6-routes.md`'s branch.

/// A structure that holds cargo in its own output buffer but is **not** a
/// Depot (`stores: false` on `mining_node`, the only field `return_to_depots`
/// reads) — `spend_from_base` draws from every `Structure + Stock` entity
/// regardless, so a dispatch can spend from here while nothing exists that
/// could ever receive a deposit back.
fn deploy_non_storing_buffer(game: &mut Game, x: i32, y: i32, item: &ItemId, qty: u32) {
    let node = deploy_structure(game, "mining_node", x, y);
    game.world.entity_mut(node).insert(Stock {
        output: [(item.clone(), qty)].into_iter().collect(),
        capacity: 9_999,
        ..Default::default()
    });
}

/// A base with a Relay and cargo sitting in a non-storing machine's buffer,
/// but **no Depot standing at all** — Finding 1's reproduction. Dispatch
/// still succeeds (`spend_from_base` draws from any buffer), but the inbound
/// leg's proceeds have nowhere built to land.
fn a_depot_less_dispatch_ready_base(seed: u32, qty: u32) -> (Game, ItemId, SettlementKey) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let item = ItemId::from("cache_grain");
    deploy_non_storing_buffer(&mut game, 0, 1, &item, qty);
    let key = SettlementKey { rx: 5, ry: 5 };
    register_settlement(&mut game, key, a_destination(), (500, 500));
    (game, item, key)
}

/// FINDING 1 (HIGH): `complete_inbound_leg` must route proceeds through
/// `Game::return_material` — Depot first, the player's pack second — rather
/// than `return_to_depots` alone with the remainder discarded. A base with no
/// Depot standing must not simply destroy the sale's proceeds.
#[test]
fn proceeds_land_on_the_player_when_the_base_has_no_depot() {
    let (mut game, item, key) = a_depot_less_dispatch_ready_base(7600, 300);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    let currency = game.trade_currency();

    game.dispatch_route(key, vec![(item, 300)], false)
        .expect("a legal dispatch — spend_from_base draws from any buffer");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a one-off route comes home for good"
    );
    let player = game.player_entity();
    let carried = game
        .world
        .get::<Inventory>(player)
        .map(|inv| inv.count(&currency))
        .unwrap_or(0);
    assert!(
        carried > 0,
        "with no Depot standing, the sale's proceeds must land in the player's \
         pack rather than being destroyed — see Game::return_material"
    );
}

/// FINDING 2 (HIGH), first half: a severed **stalled** route with no stock to
/// reload must be dropped, not left parked forever. `try_reload_route`
/// ignoring `standing` meant it kept retrying a reload every tick, finding
/// none, and staying stalled — consuming a `ROUTE_MAX_ACTIVE` slot and
/// blocking a fresh dispatch to that town for good.
#[test]
fn a_severed_stalled_route_with_no_stock_is_dropped_rather_than_stranded() {
    let (mut game, item, key) = a_dispatch_ready_base(7700, 200);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item, 200)], true)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }
    assert!(
        game.world.resource::<crate::resources::Routes>().0[0].stalled,
        "the base has no more of the item to reload with, so it must stall \
         first, or this proves nothing"
    );

    assert!(game.sever_route(key));
    game.run_routes();

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a severed stalled route must be dropped, not left parked forever"
    );
}

/// FINDING 2 (HIGH), second half: a severed stalled route must not depart
/// again just because stock came back — severing is a refusal of the next
/// trip, and a stalled route is parked at home with its proceeds already
/// deposited, not in flight.
#[test]
fn a_severed_stalled_route_does_not_depart_again_when_stock_returns() {
    let (mut game, item, key) = a_dispatch_ready_base(7701, 200);
    set_standing(&mut game, key, crate::tuning::SETTLEMENT_WARM_STANDING);
    game.dispatch_route(key, vec![(item.clone(), 200)], true)
        .expect("a legal dispatch");
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(2 * total) {
        game.run_routes();
    }
    assert!(
        game.world.resource::<crate::resources::Routes>().0[0].stalled,
        "must stall first, or this proves nothing"
    );

    assert!(game.sever_route(key));
    // Exactly the shape that lets a still-standing stalled route reload and
    // depart again — a severed one must not take it.
    deploy_depot(&mut game, 0, 2, &item, 200);
    game.run_routes();

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a severed route must not depart again just because stock returned"
    );
}

// ---------------------------------------------------------------- Phase 4
// the outpost endpoint — `routes::RouteEnd::Outpost`

/// A base with a Home and a Relay, and an outpost record standing at `tile`
/// — inserted directly into `resources::Outposts` rather than through
/// `Game::found_outpost`, since these tests are about the route mechanism
/// and not the placement ladder (already covered in `game::outposts::tests`).
fn an_outpost_ready_base(seed: u32, tile: (i32, i32)) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .insert(
            tile,
            crate::outposts::Outpost::new(
                crate::world::Biome::Deadlock,
                crate::tuning::OUTPOST_MAX_INTEGRITY,
            ),
        );
    game
}

/// Puts `qty` of `item` into the outpost's own stock at `tile`.
fn stock_the_outpost(game: &mut Game, tile: (i32, i32), item: &ItemId, qty: u32) {
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .get_mut(&tile)
        .unwrap()
        .stock
        .insert(item.clone(), qty);
}

fn outpost_stock_total(game: &Game, tile: (i32, i32)) -> u32 {
    game.world
        .resource::<crate::resources::Outposts>()
        .0
        .get(&tile)
        .map(|o| o.stock.values().sum())
        .unwrap_or(0)
}

/// Every refusal lands before anything changes, asserted per refusal —
/// `every_refusal_leaves_stock_and_routes_exactly_as_they_were`'s shape,
/// with no cargo to spend up front (`EmptyManifest`/`Understocked` do not
/// apply to this door).
#[test]
fn outpost_dispatch_every_refusal_leaves_routes_exactly_as_they_were() {
    let tile = (500, 500);
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Box<dyn Fn() -> (Game, (i32, i32))>)> = vec![
        (
            "not at the Relay",
            Box::new(move || {
                let mut game = an_outpost_ready_base(8000, tile);
                game.world
                    .insert_resource(crate::resources::Locale::Surface);
                (game, tile)
            }),
        ),
        (
            "no outpost at the tile",
            Box::new(move || {
                let mut game =
                    Game::new(8001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
                deploy_relay(&mut game);
                (game, tile)
            }),
        ),
        (
            "a duplicate destination",
            Box::new(move || {
                let mut game = an_outpost_ready_base(8002, tile);
                game.dispatch_outpost_route(tile, false)
                    .expect("the first dispatch must succeed");
                (game, tile)
            }),
        ),
        (
            "too many routes",
            Box::new(move || {
                let mut game = an_outpost_ready_base(8003, tile);
                for n in 0..crate::tuning::ROUTE_MAX_ACTIVE {
                    let k = SettlementKey {
                        rx: 10 + n as i32,
                        ry: 10,
                    };
                    register_settlement(&mut game, k, a_destination(), (1000 + n as i32, 1000));
                    let item = ItemId::from("cache_grain");
                    deploy_depot(&mut game, 0, 1 + n as i32, &item, 5);
                    game.dispatch_route(k, vec![(item, 5)], false)
                        .expect("filling to the cap must succeed");
                }
                (game, tile)
            }),
        ),
    ];

    for (name, build) in cases {
        let (mut game, tile) = build();
        let before_routes = game.world.resource::<crate::resources::Routes>().0.len();
        assert!(
            game.dispatch_outpost_route(tile, false).is_err(),
            "{name} should have been refused"
        );
        assert_eq!(
            game.world.resource::<crate::resources::Routes>().0.len(),
            before_routes,
            "{name} filed a record anyway"
        );
    }
}

#[test]
fn a_legal_outpost_dispatch_records_a_route_with_no_cargo() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8100, tile);

    game.dispatch_outpost_route(tile, true)
        .expect("a legal dispatch");

    let routes = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.destination, RouteEnd::Outpost(tile));
    assert!(
        route.cargo.is_empty(),
        "the outbound leg carries nothing to an outpost"
    );
    assert!(route.standing);
    assert_eq!(route.leg, RouteLeg::Outbound);
}

/// A round trip loads the outpost's stock on arrival and deposits it at
/// home — design spec §7.
#[test]
fn an_outpost_round_trip_brings_stock_home() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8200, tile);
    let item = ItemId::from("cache_grain");
    stock_the_outpost(&mut game, tile, &item, 5);
    // No Depot stands in this fixture, so it lands in the player's pack —
    // `proceeds_land_on_the_player_when_the_base_has_no_depot`'s shape.
    let player = game.player_entity();
    let before = game
        .world
        .get::<Inventory>(player)
        .map(|inv| inv.count(&item))
        .unwrap_or(0);

    game.dispatch_outpost_route(tile, false).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    // Outbound leg lands (loads cargo) and then the inbound leg of the same
    // length lands (deposits it) — `2 * total` ticks covers both.
    for _ in 0..(2 * total) {
        game.run_routes();
    }

    assert!(
        game.world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a one-off outpost route comes home for good"
    );
    assert_eq!(
        outpost_stock_total(&game, tile),
        0,
        "the outpost is drained"
    );
    let after = game
        .world
        .get::<Inventory>(player)
        .map(|inv| inv.count(&item))
        .unwrap_or(0);
    assert_eq!(
        after,
        before + 5,
        "the base must land what the outpost held"
    );
}

/// The outbound leg never loads more than `ROUTE_OUTPOST_CARRY`, summed
/// across every item, whatever the outpost is holding.
#[test]
fn the_carry_is_capped_at_route_outpost_carry() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8300, tile);
    let item = ItemId::from("cache_grain");
    stock_the_outpost(
        &mut game,
        tile,
        &item,
        crate::tuning::ROUTE_OUTPOST_CARRY + 50,
    );

    game.dispatch_outpost_route(tile, false).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..total {
        game.run_routes();
    }

    let route = &game.world.resource::<crate::resources::Routes>().0[0];
    let carried: u32 = route.cargo.iter().map(|(_, qty)| *qty).sum();
    assert_eq!(carried, crate::tuning::ROUTE_OUTPOST_CARRY);
    assert_eq!(
        outpost_stock_total(&game, tile),
        50,
        "the rest stays behind for the next trip"
    );
}

/// A Hostile town beside the line takes its cut of the goods and says so in
/// the log — `a_hostile_town_beside_the_route_taxes_it_and_says_so_in_the_log`'s
/// shape, one endpoint over.
#[test]
fn a_hostile_town_beside_the_outpost_route_taxes_the_goods_and_says_so() {
    let tile = (500, 500);
    let found = (8400..8460).any(|seed| {
        let mut game = an_outpost_ready_base(seed, tile);
        let item = ItemId::from("cache_grain");
        stock_the_outpost(&mut game, tile, &item, 20);

        let (ax, ay) = game.anchor_position().unwrap();
        let midpoint = ((ax + tile.0) / 2, (ay + tile.1) / 2);
        let predator = SettlementKey { rx: 50, ry: 50 };
        register_settlement(&mut game, predator, a_predator_def(), midpoint);
        set_standing(
            &mut game,
            predator,
            crate::tuning::SETTLEMENT_HOSTILE_STANDING,
        );

        game.dispatch_outpost_route(tile, false).unwrap();
        let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
        // Land the outbound leg — the pickup, no predation there.
        for _ in 0..total {
            game.run_routes();
        }
        let picked_up: u32 = game.world.resource::<crate::resources::Routes>().0[0]
            .cargo
            .iter()
            .map(|(_, qty)| *qty)
            .sum();
        let player = game.player_entity();
        let before_pack = game
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&item))
            .unwrap_or(0);
        // Land the inbound leg — predation against the goods has its one
        // roll here, then whatever survives deposits into the player's pack
        // (no Depot stands in this fixture).
        for _ in 0..total {
            game.run_routes();
        }
        let after_pack = game
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&item))
            .unwrap_or(0);
        let delivered = after_pack - before_pack;
        let taxed = delivered < picked_up;
        let logged = game
            .message_log(200)
            .iter()
            .any(|line| line.text.contains(&a_predator_def().name));
        taxed && logged
    });
    assert!(
        found,
        "no seed in the sweep saw the Hostile town tax the outpost's goods and say so"
    );
}

/// If the outpost record vanishes before the caravan arrives, the route
/// stalls with a log line instead of dropping — design spec §7 — and
/// retries every tick, exactly like a settlement's dry-stock stall.
#[test]
fn a_gone_outpost_stalls_the_route_with_a_log_line() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8500, tile);
    game.dispatch_outpost_route(tile, true).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;

    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .remove(&tile);
    for _ in 0..total {
        game.run_routes();
    }

    let route = game.world.resource::<crate::resources::Routes>().0[0].clone();
    assert!(route.stalled, "a missing outpost must stall, not drop");
    assert!(
        game.message_log(50)
            .iter()
            .any(|line| line.text.contains("isn't there anymore")),
        "the stall must say so once"
    );

    // Retried every tick: putting the record back releases it on the next.
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .insert(
            tile,
            crate::outposts::Outpost::new(
                crate::world::Biome::Deadlock,
                crate::tuning::OUTPOST_MAX_INTEGRITY,
            ),
        );
    game.run_routes();
    let route = game.world.resource::<crate::resources::Routes>().0[0].clone();
    assert!(
        !route.stalled,
        "the record reappearing must release the stall"
    );
    assert_eq!(route.leg, RouteLeg::Inbound);
}

/// A standing outpost route reloads and departs again on arrival home.
#[test]
fn a_standing_outpost_route_reloads_on_arrival() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8600, tile);
    let item = ItemId::from("cache_grain");
    stock_the_outpost(&mut game, tile, &item, 5);
    game.dispatch_outpost_route(tile, true).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;

    for _ in 0..(2 * total) {
        game.run_routes();
    }

    let routes = game.world.resource::<crate::resources::Routes>().0.clone();
    assert_eq!(
        routes.len(),
        1,
        "a standing route does not come home for good"
    );
    let route = &routes[0];
    assert_eq!(route.leg, RouteLeg::Outbound, "it has departed again");
    assert_eq!(route.ticks_elapsed, 0);
}

/// Predation is the only thing an outpost's leg completion may draw
/// `GameRng` for — `the_tick_draws_no_rng_when_nothing_preys`'s shape.
#[test]
fn the_tick_draws_no_rng_for_an_outpost_route_when_nothing_preys() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8700, tile);
    let item = ItemId::from("cache_grain");
    stock_the_outpost(&mut game, tile, &item, 5);
    game.dispatch_outpost_route(tile, false).unwrap();
    let total = game.world.resource::<crate::resources::Routes>().0[0].ticks_total;
    for _ in 0..(total - 1) {
        game.run_routes();
    }

    fn peek(g: &mut Game) -> u64 {
        use rand::RngExt;
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    super::support::reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    super::support::reseed_rng(&mut game, 55);
    game.run_routes();
    let with = peek(&mut game);

    assert_eq!(
        without, with,
        "no predator stands near this trip, so completing the leg must not touch GameRng"
    );
}

/// An outpost route in flight survives a real save round trip —
/// `a_route_in_flight_survives_a_real_save_round_trip`'s shape, the new
/// `SaveData::outpost_routes` vector rather than `PlayerSave::routes`.
#[test]
fn an_outpost_route_in_flight_survives_a_real_save_round_trip() {
    let scratch = scratch_assets_dir("outpost_route_inflight_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8800, tile);
    let item = ItemId::from("cache_grain");
    game.dispatch_outpost_route(tile, true).unwrap();
    {
        let mut routes = game.world.resource_mut::<crate::resources::Routes>();
        routes.0[0].cargo = vec![(item.clone(), 4)];
        routes.0[0].ticks_elapsed = 3;
    }

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let routes = loaded
        .world
        .resource::<crate::resources::Routes>()
        .0
        .clone();
    assert_eq!(
        routes.len(),
        1,
        "the outpost route in flight must survive the load"
    );
    let route = &routes[0];
    assert_eq!(route.destination, RouteEnd::Outpost(tile));
    assert_eq!(route.cargo, vec![(item, 4)]);
    assert!(route.standing);
    assert_eq!(route.leg, RouteLeg::Outbound);
    assert_eq!(route.ticks_elapsed, 3);
}

/// A save written before outpost routes existed carries no `outpost_routes`
/// key at all, and must load with none rather than refusing or panicking —
/// `a_pre_routes_save_loads_with_no_routes`'s shape.
#[test]
fn a_pre_outpost_routes_save_loads_with_none() {
    let scratch = scratch_assets_dir("outpost_route_pre_save");
    std::fs::create_dir_all(&*scratch).unwrap();
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8801, tile);
    game.dispatch_outpost_route(tile, true).unwrap();
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();

    let mut data = crate::save::load_from_file(&path).unwrap();
    data.outpost_routes.clear();
    let text = crate::save::to_ron(&data).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("outpost_routes:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the key must have been there to strip, or this proves nothing"
    );
    let old_path = scratch.join("old.bin");
    let stripped_data =
        crate::save::from_ron(&stripped).expect("a pre-outpost-routes save still parses");
    crate::save::save_to_file(&old_path, &stripped_data).unwrap();

    let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
    assert!(
        loaded
            .world
            .resource::<crate::resources::Routes>()
            .0
            .is_empty(),
        "a pre-outpost-routes save has no outpost route in flight"
    );
}

/// The hub's destination list gains a row per founded outpost, after every
/// known settlement — `route_destinations` widened to list both kinds
/// (Part A of the 2026-09-23 plan's dispatch gap).
#[test]
fn route_destinations_lists_a_founded_outpost_after_settlements() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8900, tile);
    // `Game::new` already ran `ensure_local_settlements` — clear what world
    // generation found nearby so only the one town registered below is
    // counted, `route_destinations_lists_every_known_settlement`'s fixture.
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .clear();
    let key = SettlementKey { rx: 2, ry: -1 };
    register_settlement(&mut game, key, a_destination(), (200, -100));

    let rows = game.route_destinations().expect("a Relay stands");
    assert_eq!(rows.len(), 2, "one settlement plus one outpost");
    assert_eq!(rows[0].destination, RouteDestinationId::Settlement(key));
    let outpost_row = &rows[1];
    assert_eq!(outpost_row.destination, RouteDestinationId::Outpost(tile));
    assert!(
        outpost_row.band.is_none(),
        "an outpost carries no diplomatic standing"
    );
    let (ax, ay) = game.anchor_position().unwrap();
    let d = (ax - tile.0).abs().max((ay - tile.1).abs()) as u64;
    assert_eq!(
        outpost_row.ticks,
        crate::tuning::ROUTE_TICKS_BASE + crate::tuning::ROUTE_TICKS_PER_TILE * d
    );
}

/// An outpost route in flight shows up on the hub's own "in flight" list,
/// not only on the outpost's own screen — the second half of the same gap.
#[test]
fn route_reports_includes_an_outpost_route_in_flight() {
    let tile = (500, 500);
    let mut game = an_outpost_ready_base(8901, tile);
    game.dispatch_outpost_route(tile, true).unwrap();

    let reports = game.route_reports();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.destination, RouteDestinationId::Outpost(tile));
    assert!(report.standing);
    assert_eq!(report.leg, RouteLeg::Outbound);
}
