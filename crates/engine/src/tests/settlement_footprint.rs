//! A settlement's footprint: the square it draws, sized by its kind and
//! `growth::Vitality`, and the one writer that keeps the map entities
//! matching it.
//!
//! `docs/superpowers/specs/2026-09-25-settlement-footprint-design.md` §Testing.
//! `tests/settlement_growth.rs` already drives the growth latch itself;
//! what is here is the square that latch now moves.

use super::support::*;
use crate::components::{Position, Settlement, SettlementCentre, TownPatrol};
use crate::resources::{Outposts, Settlements, Standings, Visit};
use crate::settlements::SettlementKey;
use crate::tuning::*;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// The first authored `Server` seed 4242 materializes — sorted, since
/// `Settlements` is a `BTreeMap` keyed by region and this just wants a
/// stable pick, `tests/settlement_growth.rs::a_known_server`'s twin.
fn a_known_server(game: &Game) -> SettlementKey {
    let mut keys: Vec<_> = game
        .world
        .resource::<Settlements>()
        .0
        .iter()
        .filter(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| *key)
        .collect();
    keys.sort_by_key(|key| (key.rx, key.ry));
    *keys
        .first()
        .expect("test premise: seed 4242 materializes at least one authored Server")
}

/// The first authored `Mainframe` seed 4242 materializes.
fn a_known_mainframe(game: &Game) -> SettlementKey {
    let mut keys: Vec<_> = game
        .world
        .resource::<Settlements>()
        .0
        .iter()
        .filter(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Mainframe)
        .map(|(key, _)| *key)
        .collect();
    keys.sort_by_key(|key| (key.rx, key.ry));
    *keys
        .first()
        .expect("test premise: seed 4242 materializes at least one authored Mainframe")
}

/// Open, walkable ground for `radius` around `(x, y)` — `settlement_patrols
/// .rs`'s own fixture, repeated here rather than shared: a footprint test
/// carves a different square every time and a shared helper would just be a
/// second name for `set_override`.
fn carve_open(game: &mut Game, (x, y): (i32, i32), radius: i32) {
    let mut map = game.world.resource_mut::<WorldMap>();
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            map.set_override(
                x + dx,
                y + dy,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }
}

fn tile_of(game: &Game, key: SettlementKey) -> (i32, i32) {
    game.world
        .resource::<Settlements>()
        .0
        .get(&key)
        .unwrap()
        .tile
}

// ---------------------------------------------------------------------------
// The radius table
// ---------------------------------------------------------------------------

#[test]
fn a_server_reads_radius_one_and_a_nine_cell_square() {
    let game = game();
    let key = a_known_server(&game);
    assert_eq!(game.settlement_radius(key), Some(SETTLEMENT_RADIUS_SERVER));
    assert_eq!(game.footprint(key).len(), 9);
}

#[test]
fn a_starved_mainframe_reads_radius_one_and_a_nine_cell_square() {
    let mut game = game();
    let key = a_known_mainframe(&game);
    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.traded = true;
        relation.commerce = SETTLEMENT_COMMERCE_MIN;
    }
    assert_eq!(game.settlement_radius(key), Some(SETTLEMENT_RADIUS_STARVED));
    assert_eq!(game.footprint(key).len(), 9);
}

#[test]
fn a_steady_mainframe_reads_radius_two_and_a_twenty_five_cell_square() {
    let game = game();
    let key = a_known_mainframe(&game);
    // Untouched: `growth::vitality_floor` holds an untraded city at Steady.
    assert_eq!(game.settlement_radius(key), Some(SETTLEMENT_RADIUS_STEADY));
    assert_eq!(game.footprint(key).len(), 25);
}

#[test]
fn a_thriving_mainframe_reads_radius_three_and_a_forty_nine_cell_square() {
    let mut game = game();
    let key = a_known_mainframe(&game);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = SETTLEMENT_COMMERCE_MAX;
    assert_eq!(
        game.settlement_radius(key),
        Some(SETTLEMENT_RADIUS_THRIVING)
    );
    assert_eq!(game.footprint(key).len(), 49);
}

// ---------------------------------------------------------------------------
// Materialization
// ---------------------------------------------------------------------------

#[test]
fn a_server_spawns_nine_cells_with_exactly_one_centre() {
    let mut game = game();
    let key = a_known_server(&game);
    let mut query = game
        .world
        .query::<(&Settlement, Option<&SettlementCentre>)>();
    let (cells, centres): (usize, usize) = query
        .iter(&game.world)
        .filter(|(settlement, _)| settlement.key == key)
        .fold((0, 0), |(cells, centres), (_, centre)| {
            (cells + 1, centres + centre.is_some() as usize)
        });
    assert_eq!(
        cells, 9,
        "a materialized Server drew something other than 9 cells"
    );
    assert_eq!(
        centres, 1,
        "a materialized Server carries more or less than one centre"
    );
}

#[test]
fn bumping_a_corner_cell_queues_the_visit_and_does_not_move_the_player() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 10, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 2);
    let key = SettlementKey { rx: 5, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    game.sync_settlement_footprint(key);
    assert_eq!(
        game.footprint(key).len(),
        9,
        "test premise: a full 3x3 square"
    );

    // The corner, approached from one tile further out on both axes so the
    // step onto it is a single diagonal move rather than a walk through the
    // footprint's own ground.
    let corner = (
        centre.0 - SETTLEMENT_RADIUS_SERVER,
        centre.1 - SETTLEMENT_RADIUS_SERVER,
    );
    let approach = (corner.0 - 1, corner.1 - 1);
    {
        let mut pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        pos.x = approach.0;
        pos.y = approach.1;
    }

    game.move_player(1, 1);

    let after = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!(
        (after.x, after.y),
        approach,
        "a corner cell of the footprint admitted the player instead of queuing a visit"
    );
    assert_eq!(
        game.take_visit(),
        Some(Visit::Settlement(key)),
        "the corner bump did not name this settlement"
    );
}

// ---------------------------------------------------------------------------
// Growth
// ---------------------------------------------------------------------------

/// The whole point: latching growth does not just repaint a glyph in place,
/// it grows the square underneath it — a fresh Mainframe reads Steady
/// (`growth::vitality_floor`), radius 2 against a Server's 1.
#[test]
fn latching_growth_moves_a_town_from_nine_cells_to_twenty_five_and_repaints_them_all() {
    let mut game = game();
    let key = a_known_server(&game);
    assert_eq!(game.footprint(key).len(), 9, "test premise: still a town");

    // Jumping the clock straight to `due` also hands `settle_commerce_drift`
    // however many epochs it crossed to get there, and a decay off a
    // starting commerce of 0 goes negative -- which pushes the due date
    // *later* and the growth this test wants never fires. Pre-seeding
    // `commerce_epoch` at the epoch `due` falls in makes the elapsed count
    // zero, so nothing decays and the town grows with no commerce boost:
    // the untouched case, reading Steady off `growth::vitality_floor`.
    let due = crate::settlements::growth::due_tick(
        game.world.resource::<crate::world::WorldMap>().seed(),
        key,
    );
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce_epoch = due / SETTLEMENT_COMMERCE_DECAY_TICKS;
    game.set_tick_for_test(due);
    game.settlement_growth_tick();

    let cells = game.footprint(key);
    assert_eq!(cells.len(), 25, "growth did not grow the square");

    let mut query = game
        .world
        .query::<(&Settlement, &crate::components::Glyph)>();
    let glyphs: Vec<char> = query
        .iter(&game.world)
        .filter(|(settlement, _)| settlement.key == key)
        .map(|(_, drawn)| drawn.ch)
        .collect();
    assert_eq!(glyphs.len(), 25);
    assert!(
        glyphs
            .iter()
            .all(|&ch| ch == crate::settlements::SettlementKind::Mainframe.glyph()),
        "not every cell was repainted to the Mainframe glyph: {glyphs:?}"
    );
}

// ---------------------------------------------------------------------------
// Vitality
// ---------------------------------------------------------------------------

#[test]
fn a_thriving_citys_forty_nine_cells_shrink_to_nine_when_pushed_to_starved() {
    let mut game = game();
    let key = a_known_mainframe(&game);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = SETTLEMENT_COMMERCE_MAX;
    game.sync_settlement_footprint(key);
    assert_eq!(game.footprint(key).len(), 49, "test premise: Thriving");

    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.traded = true;
        relation.commerce = SETTLEMENT_COMMERCE_MIN;
    }
    game.sync_settlement_footprint(key);
    assert_eq!(
        game.footprint(key).len(),
        9,
        "pushing a Thriving city to Starved did not shrink its footprint"
    );
}

// ---------------------------------------------------------------------------
// Outposts
// ---------------------------------------------------------------------------

#[test]
fn founding_an_outpost_at_the_footprint_ceiling_of_a_town_is_refused_and_spends_nothing() {
    let mut game = game();
    let key = a_known_server(&game);
    let tile = tile_of(&game, key);
    let site = (tile.0 + SETTLEMENT_FOOTPRINT_MAX_RADIUS, tile.1);
    carve_open(&mut game, site, 1);
    let before = game.world.resource::<Outposts>().0.len();

    let result = game.found_outpost(site);

    assert!(
        result.is_err(),
        "founding within SETTLEMENT_FOOTPRINT_MAX_RADIUS of a known town must be refused"
    );
    assert_eq!(
        game.world.resource::<Outposts>().0.len(),
        before,
        "a refused founding still recorded an outpost"
    );
}

// ---------------------------------------------------------------------------
// Trading reach
// ---------------------------------------------------------------------------

#[test]
fn the_player_beside_a_corner_of_a_five_by_five_city_can_trade() {
    let mut game = game();
    let key = a_known_mainframe(&game);
    // Untouched, so it reads Steady — radius 2, a 5x5 square.
    assert_eq!(game.footprint(key).len(), 25, "test premise: a 5x5 city");
    let tile = tile_of(&game, key);
    let radius = game.settlement_radius(key).unwrap();
    let beside_corner = (tile.0 - radius - 1, tile.1 - radius - 1);
    {
        let mut pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        pos.x = beside_corner.0;
        pos.y = beside_corner.1;
    }
    assert!(
        game.settlement_reach(key),
        "standing beside a 5x5 city's own corner must be in reach"
    );
}

// ---------------------------------------------------------------------------
// Patrols
// ---------------------------------------------------------------------------

#[test]
fn a_patrol_stays_tethered_through_a_footprint_shrink() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 6, ppos.y);
    carve_open(
        &mut game,
        centre,
        SETTLEMENT_RADIUS_STEADY + SETTLEMENT_PATROL_RING_MAX + 2,
    );
    let key = SettlementKey { rx: 3, ry: 0 };
    let town = place_settlement(&mut game, key, centre.0, centre.1);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    game.sync_settlement_footprint(key);
    assert_eq!(game.footprint(key).len(), 25, "test premise: a Steady city");

    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = SETTLEMENT_HOSTILE_STANDING;
    game.field_patrol();
    let members: Vec<(Entity, Entity)> = {
        let mut query = game.world.query::<(Entity, &TownPatrol)>();
        query
            .iter(&game.world)
            .map(|(entity, patrol)| (entity, patrol.town))
            .collect()
    };
    assert_eq!(members.len(), 1, "test premise: one patrol member fielded");
    let (member, town_before) = members[0];
    assert_eq!(
        town_before, town,
        "test premise: tethered to the fixture's own centre entity"
    );

    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.traded = true;
        relation.commerce = SETTLEMENT_COMMERCE_MIN;
    }
    game.sync_settlement_footprint(key);
    assert_eq!(
        game.footprint(key).len(),
        9,
        "test premise: the shrink actually happened"
    );

    let town_after = game
        .world
        .get::<TownPatrol>(member)
        .map(|patrol| patrol.town);
    assert_eq!(
        town_after,
        Some(town_before),
        "the patrol's tether moved off the centre entity when the footprint shrank"
    );
    assert!(
        game.world.get::<Settlement>(town).is_some(),
        "the centre entity itself did not survive the shrink"
    );
}

// ---------------------------------------------------------------------------
// Save/load
// ---------------------------------------------------------------------------

/// `settlements::placement::tests::a_settlement_survives_a_save_and_load`
/// already checks this at whatever size a fresh run's towns happen to be;
/// this one forces a Thriving city first, so the size under test is not the
/// one every town starts at.
#[test]
fn a_thriving_citys_footprint_rebuilds_at_its_derived_size_after_a_load() {
    let dir = scratch_assets_dir("settlement_footprint_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game();
    let key = a_known_mainframe(&game);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = SETTLEMENT_COMMERCE_MAX;
    game.sync_settlement_footprint(key);
    assert_eq!(game.footprint(key).len(), 49, "test premise: Thriving");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.footprint(key).len(),
        49,
        "a Thriving city's footprint did not rebuild at 49 cells after a load"
    );
    let mut query = loaded
        .world
        .query::<(&Settlement, Option<&SettlementCentre>)>();
    let centres = query
        .iter(&loaded.world)
        .filter(|(settlement, _)| settlement.key == key)
        .filter(|(_, centre)| centre.is_some())
        .count();
    assert_eq!(
        centres, 1,
        "the loaded footprint carries more or less than one centre"
    );
}
