//! The compass: one derivation of where the run's known destinations are,
//! and the selection that points at one of them.

use super::support::*;
use crate::settlements::{CompassTarget, SettlementKey};
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn player_at(game: &Game) -> (i32, i32) {
    let p = game.world.get::<Position>(game.player_entity()).unwrap();
    (p.x, p.y)
}

/// Clears the links a fresh zone spawns, so a test about ordering is not at
/// the mercy of where the seed happened to put three holes in the ground.
fn clear_links(game: &mut Game) {
    let links: Vec<Entity> = {
        let mut q = game
            .world
            .query_filtered::<Entity, With<crate::components::SurfaceLink>>();
        q.iter(&game.world).collect()
    };
    for e in links {
        game.world.despawn(e);
    }
}

#[test]
fn the_rows_run_home_then_settlements_then_links_each_nearest_first() {
    let mut game = game();
    clear_links(&mut game);
    // `Game::new` runs `ensure_local_settlements` over the 3x3 region block,
    // so a fresh world already holds towns — clearing them is what makes
    // this a test about ordering rather than about the seed.
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .clear();
    let (px, py) = player_at(&game);
    place_settlement(&mut game, SettlementKey { rx: 1, ry: 0 }, px + 20, py);
    place_settlement(&mut game, SettlementKey { rx: 2, ry: 0 }, px + 5, py);
    game.world.spawn((
        crate::components::SurfaceLink,
        Position { x: px + 40, y: py },
    ));
    game.world.spawn((
        crate::components::SurfaceLink,
        Position { x: px + 9, y: py },
    ));

    let rows = game.compass_targets();
    let kinds: Vec<&str> = rows
        .iter()
        .map(|r| match r.target {
            CompassTarget::Home => "home",
            CompassTarget::Town(_) => "town",
            CompassTarget::Link(_) => "link",
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["home", "town", "town", "link", "link"],
        "home, then settlements, then links"
    );
    assert_eq!(
        rows[1].target,
        CompassTarget::Town(SettlementKey { rx: 2, ry: 0 })
    );
    assert_eq!(rows[3].target, CompassTarget::Link((px + 9, py)));
}

#[test]
fn an_unreached_settlement_gives_a_bearing_and_withholds_its_name_and_distance() {
    let mut game = game();
    let (px, py) = player_at(&game);
    let key = SettlementKey { rx: 1, ry: 0 };
    place_settlement(&mut game, key, px, py + 12);

    let row = game
        .compass_targets()
        .into_iter()
        .find(|r| r.target == CompassTarget::Town(key))
        .expect("the town is listed");
    assert_eq!(row.bearing, "south", "the bearing is the whole first tier");
    assert_eq!(row.distance, None);
    assert!(!row.visited);
    assert_ne!(
        row.label,
        game.settlement_report(key).name,
        "a town the party has never walked to does not give up its name"
    );
}

#[test]
fn a_reached_settlement_gives_its_name_and_a_distance() {
    let mut game = game();
    let (px, py) = player_at(&game);
    let key = SettlementKey { rx: 1, ry: 0 };
    place_settlement(&mut game, key, px + 1, py);
    game.move_player(1, 0);

    let row = game
        .compass_targets()
        .into_iter()
        .find(|r| r.target == CompassTarget::Town(key))
        .expect("the town is listed");
    assert_eq!(row.label, game.settlement_report(key).name);
    assert_eq!(row.distance, Some(1));
    assert!(row.visited);
}

#[test]
fn home_is_always_reached_because_it_is_the_partys_own() {
    let mut game = game();
    let row = game
        .compass_targets()
        .into_iter()
        .find(|r| r.target == CompassTarget::Home)
        .expect("the anchor is listed");
    assert!(row.visited);
    assert!(row.distance.is_some());
}

#[test]
fn a_selection_naming_a_link_that_no_longer_exists_is_dropped() {
    let mut game = game();
    let (px, py) = player_at(&game);
    let tile = (px + 7, py + 7);
    let link = game
        .world
        .spawn((
            crate::components::SurfaceLink,
            Position {
                x: tile.0,
                y: tile.1,
            },
        ))
        .id();
    game.set_compass_bearing(Some(CompassTarget::Link(tile)));
    assert!(game.compass_bearing().is_some(), "the link is still there");

    game.world.despawn(link);

    assert_eq!(
        game.compass_bearing(),
        None,
        "the derivation drops a target that stopped existing — no cleanup hook"
    );
}

#[test]
fn the_compass_is_empty_off_the_zone_surface() {
    let mut game = game();
    let (px, py) = player_at(&game);
    place_settlement(&mut game, SettlementKey { rx: 1, ry: 0 }, px + 4, py);
    assert!(
        !game.compass_targets().is_empty(),
        "on the surface it lists"
    );

    stand_in_base(&mut game);
    assert!(
        game.compass_targets().is_empty(),
        "in base space `Position` is pinned to the anchor, so a bearing would \
         be frozen while reading as live"
    );

    game.world.insert_resource(Locale::Surface);
    descend(&mut game);
    assert!(
        game.compass_targets().is_empty(),
        "underground `Position` is pinned to the entrance tile"
    );
}

#[test]
fn the_selection_survives_a_save_and_a_load() {
    let mut game = game();
    let key = SettlementKey { rx: 3, ry: 1 };
    let (px, py) = player_at(&game);
    place_settlement(&mut game, key, px + 6, py);
    game.set_compass_bearing(Some(CompassTarget::Town(key)));

    let path = std::env::temp_dir().join(format!(
        "feral_processes_compass_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).expect("save");
    let loaded = Game::load(&path, &test_assets_dir()).expect("load");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded
            .world
            .resource::<crate::resources::CompassBearing>()
            .0,
        Some(CompassTarget::Town(key)),
        "the selection is saved state, not a cue about this instant"
    );
}
