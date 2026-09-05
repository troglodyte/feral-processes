//! Caravan routes: the record, its save form and the tick that runs it.
//!
//! Task 1/2 only, per `docs/superpowers/plans/2026-09-05-settlements-phase-6-routes.md`
//! — no dispatch door exists yet, so every fixture here builds a
//! `resources::Route` by hand and pushes it into `resources::Routes`.

use super::support::{scratch_assets_dir, test_assets_dir};
use crate::Game;
use crate::items::ItemId;
use crate::resources::DifficultyMode;
use crate::routes::{Route, RouteLeg};
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
        destination: SettlementKey { rx: 3, ry: -2 },
        destination_def: a_destination(),
        destination_tile: (300, -200),
        cargo: vec![(ItemId("cache_grain".to_string()), 12)],
        standing: true,
        stalled: false,
        leg: RouteLeg::Outbound,
        ticks_total: 400,
        ticks_elapsed: 150,
        proceeds: 0,
        losses: vec!["Test Town skimmed a Wolf pack toll.".to_string()],
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
    assert_eq!(after.destination_def, before.destination_def);
    assert_eq!(after.destination_tile, before.destination_tile);
    assert_eq!(after.cargo, before.cargo);
    assert_eq!(after.standing, before.standing);
    assert_eq!(after.stalled, before.stalled);
    assert_eq!(after.leg, before.leg);
    assert_eq!(after.ticks_total, before.ticks_total);
    assert_eq!(after.ticks_elapsed, before.ticks_elapsed);
    assert_eq!(after.proceeds, before.proceeds);
    assert_eq!(after.losses, before.losses);
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
