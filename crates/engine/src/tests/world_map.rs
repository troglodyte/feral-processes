//! The world map's stored state: surface fog, and its save coverage.

use super::support::*;
use crate::components::Position;
use crate::resources::{DifficultyMode, ExploredChunks};
use crate::tuning::WORLD_MAP_REVEAL_RADIUS_CHUNKS;
use crate::world::CHUNK_SIZE;
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn explored(game: &Game) -> std::collections::BTreeSet<(i32, i32)> {
    game.world.resource::<ExploredChunks>().0.clone()
}

fn move_party_to(game: &mut Game, x: i32, y: i32) {
    let p = game.player_entity();
    *game.world.get_mut::<Position>(p).unwrap() = Position { x, y };
}

#[test]
fn walking_reveals_the_chunks_within_the_radius() {
    let mut game = game();
    move_party_to(&mut game, 10 * CHUNK_SIZE + 3, -4 * CHUNK_SIZE + 5);
    game.idle_tick();
    let r = WORLD_MAP_REVEAL_RADIUS_CHUNKS;
    let set = explored(&game);
    for cy in (-4 - r)..=(-4 + r) {
        for cx in (10 - r)..=(10 + r) {
            assert!(set.contains(&(cx, cy)), "({cx},{cy}) should be revealed");
        }
    }
    assert!(!set.contains(&(10 + r + 1, -4)));
}

#[test]
fn negative_tiles_floor_to_the_chunk_below() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, -1, -1);
    game.idle_tick();
    assert!(explored(&game).contains(&(-1, -1)));
}

#[test]
fn nothing_is_revealed_in_base_space() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, 50 * CHUNK_SIZE, 50 * CHUNK_SIZE);
    stand_in_base(&mut game);
    game.idle_tick();
    assert!(explored(&game).is_empty());
}

#[test]
fn nothing_is_revealed_in_the_stack() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, 50 * CHUNK_SIZE, 50 * CHUNK_SIZE);
    descend(&mut game);
    game.idle_tick();
    assert!(explored(&game).is_empty());
}

#[test]
fn fog_survives_a_breach() {
    let mut game = game();
    game.idle_tick();
    let before = explored(&game);
    assert!(!before.is_empty());
    game.enter_next_zone();
    assert!(before.is_subset(&explored(&game)));
}

#[test]
fn fog_survives_a_real_save_and_load() {
    let mut game = game();
    move_party_to(&mut game, 7 * CHUNK_SIZE, 7 * CHUNK_SIZE);
    game.idle_tick();
    let before = explored(&game);
    assert!(before.contains(&(7, 7)));
    let path = std::env::temp_dir().join(format!("feral_fog_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(explored(&loaded), before);
}

// ------------------------------------------------------------ the view

use crate::resources::{Outposts, Routes, Settlements, Standings};
use crate::routes::{Route, RouteEnd, RouteLeg};
use crate::settlements::growth::TownTrend;
use crate::settlements::{CompassTarget, SettlementKey};
use crate::views::{WorldMapCell, WorldMapMarkKind};

const KEY: SettlementKey = SettlementKey { rx: 40, ry: 40 };

/// A surface with nothing the seed placed: no towns, links, outposts,
/// routes or fog, so a test sees only what it adds.
fn bare() -> Game {
    let mut game = game();
    game.world.resource_mut::<Settlements>().0.clear();
    game.world.resource_mut::<Outposts>().0.clear();
    game.world.resource_mut::<Routes>().0.clear();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    let doomed: Vec<Entity> = {
        let mut q = game.world.query_filtered::<Entity, Or<(
            With<crate::components::SurfaceLink>,
            With<crate::components::Nest>,
        )>>();
        q.iter(&game.world).collect()
    };
    for e in doomed {
        game.world.despawn(e);
    }
    game
}

fn outpost_at(game: &mut Game, tile: (i32, i32)) {
    game.world.resource_mut::<Outposts>().0.insert(
        tile,
        crate::outposts::Outpost::new(crate::world::Biome::Deadlock, 10),
    );
}

fn chunk_tile(cx: i32, cy: i32) -> (i32, i32) {
    (cx * CHUNK_SIZE + 3, cy * CHUNK_SIZE + 3)
}

#[test]
fn the_view_is_none_in_base_space_and_the_stack() {
    let mut game = bare();
    assert!(game.world_map((0, 0), 3).is_some());
    descend(&mut game);
    assert!(game.world_map((0, 0), 3).is_none());
    let mut game = bare();
    stand_in_base(&mut game);
    assert!(game.world_map((0, 0), 3).is_none());
}

#[test]
fn derived_reveals_show_without_being_stored() {
    let mut game = bare();
    let far = chunk_tile(30, 30);
    outpost_at(&mut game, far);
    let view = game.world_map((30, 30), 2).unwrap();
    assert_eq!(view.cells.len(), 5);
    assert!(matches!(view.cells[2][2], WorldMapCell::Explored(_)));
    assert!(matches!(view.cells[0][0], WorldMapCell::Unknown));
    assert!(
        view.marks
            .iter()
            .any(|m| m.target == Some(CompassTarget::Outpost(far)))
    );
    assert!(
        explored(&game).is_empty(),
        "derived reveals are never stored"
    );
}

#[test]
fn the_anchor_and_a_route_corridor_are_revealed() {
    let mut game = bare();
    let anchor = game.anchor_position().unwrap();
    let (ac, _) = (anchor.0.div_euclid(CHUNK_SIZE), ());
    let end = (anchor.0 + 6 * CHUNK_SIZE, anchor.1);
    outpost_at(&mut game, end);
    game.world.resource_mut::<Routes>().0.push(Route {
        destination: RouteEnd::Outpost(end),
        cargo: Vec::new(),
        standing: false,
        stalled: false,
        leg: RouteLeg::Outbound,
        ticks_total: 10,
        ticks_elapsed: 0,
        proceeds: 0,
    });
    let ay = anchor.1.div_euclid(CHUNK_SIZE);
    let view = game.world_map((ac + 3, ay), 4).unwrap();
    let mid = &view.cells[4][4];
    assert!(
        matches!(mid, WorldMapCell::Explored(_)),
        "mid-corridor chunk"
    );
    assert_eq!(view.routes.len(), 1);
    assert_eq!(view.routes[0].from_chunk, (ac, ay));
    assert_eq!(view.routes[0].to_chunk, (ac + 6, ay));
}

#[test]
fn a_known_town_in_an_unrevealed_chunk_is_absent() {
    let mut game = bare();
    let far = chunk_tile(25, 25);
    place_settlement(&mut game, KEY, far.0, far.1);
    let view = game.world_map((25, 25), 2).unwrap();
    assert!(
        view.marks
            .iter()
            .all(|m| m.target != Some(CompassTarget::Town(KEY)))
    );

    game.world
        .resource_mut::<ExploredChunks>()
        .0
        .insert((25, 25));
    let view = game.world_map((25, 25), 2).unwrap();
    let town = view
        .marks
        .iter()
        .find(|m| m.target == Some(CompassTarget::Town(KEY)))
        .expect("revealed now");
    assert_eq!(town.label, "a settlement");
    assert!(matches!(town.kind, WorldMapMarkKind::Town { .. }));
}

#[test]
fn marks_carry_the_matching_compass_target_and_a_nest_has_none() {
    let mut game = bare();
    let (px, py) = {
        let p = game.world.get::<Position>(game.player_entity()).unwrap();
        (p.x, p.y)
    };
    game.world
        .resource_mut::<ExploredChunks>()
        .0
        .insert((px.div_euclid(CHUNK_SIZE), py.div_euclid(CHUNK_SIZE)));
    game.world.spawn((
        crate::components::SurfaceLink,
        Position { x: px + 1, y: py },
    ));
    game.world.spawn((
        crate::components::Nest {
            species: "x".to_string(),
            pending_respawns: Vec::new(),
        },
        Position { x: px + 2, y: py },
    ));
    let view = game
        .world_map((px.div_euclid(CHUNK_SIZE), py.div_euclid(CHUNK_SIZE)), 1)
        .unwrap();
    let target =
        |k: fn(&WorldMapMarkKind) -> bool| view.marks.iter().find(|m| k(&m.kind)).map(|m| m.target);
    assert_eq!(
        target(|k| matches!(k, WorldMapMarkKind::Home)),
        Some(Some(CompassTarget::Home))
    );
    assert_eq!(
        target(|k| matches!(k, WorldMapMarkKind::StackLink)),
        Some(Some(CompassTarget::Link((px + 1, py))))
    );
    assert_eq!(target(|k| matches!(k, WorldMapMarkKind::Nest)), Some(None));
}

#[test]
fn a_routes_preyed_by_is_route_predators() {
    let mut game = bare();
    let anchor = game.anchor_position().unwrap();
    let end = (anchor.0 + 5 * CHUNK_SIZE, anchor.1);
    outpost_at(&mut game, end);
    game.world.resource_mut::<Routes>().0.push(Route {
        destination: RouteEnd::Outpost(end),
        cargo: Vec::new(),
        standing: false,
        stalled: false,
        leg: RouteLeg::Outbound,
        ticks_total: 10,
        ticks_elapsed: 0,
        proceeds: 0,
    });
    place_settlement(&mut game, KEY, anchor.0 + 40, anchor.1 + 2);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(KEY)
        .or_default()
        .standing = -1000;
    let expected = game.route_predators(anchor, end);
    assert_eq!(
        expected,
        vec![KEY],
        "the fixture town must prey on the route"
    );
    let ac = (
        anchor.0.div_euclid(CHUNK_SIZE),
        anchor.1.div_euclid(CHUNK_SIZE),
    );
    let view = game.world_map(ac, 2).unwrap();
    assert_eq!(view.routes[0].preyed_by, expected);
}

#[test]
fn the_view_writes_nothing_and_a_mainframe_reads_its_pending_decay_as_falling() {
    let mut game = bare();
    let anchor = game.anchor_position().unwrap();
    place_settlement(&mut game, KEY, anchor.0 + 5, anchor.1);
    game.world
        .resource_mut::<Settlements>()
        .0
        .get_mut(&KEY)
        .unwrap()
        .def
        .kind = crate::settlements::SettlementKind::Mainframe;
    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(KEY).or_default();
        relation.commerce = 50;
        relation.commerce_at_epoch = 50;
        relation.commerce_epoch = 0;
    }
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 2);
    let before = game.world.resource::<Standings>().clone();
    let ac = (
        anchor.0.div_euclid(CHUNK_SIZE),
        anchor.1.div_euclid(CHUNK_SIZE),
    );
    let view = game.world_map(ac, 1).unwrap();
    assert_eq!(*game.world.resource::<Standings>(), before);
    let town = view
        .marks
        .iter()
        .find(|m| m.target == Some(CompassTarget::Town(KEY)))
        .unwrap();
    match town.kind {
        WorldMapMarkKind::Town {
            trend,
            outlook,
            vitality,
            ..
        } => {
            assert_eq!(trend, Some(TownTrend::Falling));
            assert_eq!(outlook, None);
            assert!(vitality.is_some());
        }
        _ => panic!("not a town mark"),
    }
}

#[test]
fn the_views_pending_drift_is_what_the_settler_then_writes() {
    let mut game = bare();
    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(KEY).or_default();
        relation.commerce = 80;
    }
    let epoch = 7;
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * epoch);
    place_settlement(&mut game, KEY, 100, 100);
    let relation = game.world.resource::<Standings>().0[&KEY];
    let pending = crate::settlements::growth::settle_commerce(&relation, epoch, false);
    game.settle_commerce_drift(KEY);
    let written = game.world.resource::<Standings>().0[&KEY];
    assert_eq!(written.commerce, pending.commerce);
    assert_eq!(written.commerce_epoch, pending.commerce_epoch);
    assert_eq!(written.commerce_at_epoch, pending.commerce_at_epoch);
}
