//! A settlement's footprint: the square it draws, sized by its kind and
//! `growth::Vitality`, and the one writer that keeps the map entities
//! matching it.
//!
//! `docs/superpowers/specs/2026-09-25-settlement-footprint-design.md` §Testing.
//! `tests/settlement_growth.rs` already drives the growth latch itself;
//! what is here is the square that latch now moves.

use super::support::*;
use crate::components::{CaravanStage, Position, Settlement, SettlementCentre, TownPatrol, Trap};
use crate::resources::{FrameMemory, Outposts, Settlements, StackMemory, Standings, Visit};
use crate::settlements::SettlementKey;
use crate::tuning::*;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

use bevy_ecs::prelude::{Entity, With};
use rand::RngExt;
use std::collections::HashSet;

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

/// How many map entities `key` actually has right now — `footprint(key)
/// .len()` is the *derived* size and moves the instant vitality does,
/// whether or not `sync_settlement_footprint` has run since; this is the
/// one way to see whether the writer actually caught up to it.
fn entity_cells(game: &mut Game, key: SettlementKey) -> usize {
    let mut query = game.world.query::<&Settlement>();
    query
        .iter(&game.world)
        .filter(|settlement| settlement.key == key)
        .count()
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
        entity_cells(&mut game, key),
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
    assert_eq!(
        entity_cells(&mut game, key),
        9,
        "test premise: still a town"
    );

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
    assert_eq!(entity_cells(&mut game, key), 49, "test premise: Thriving");

    {
        let mut standings = game.world.resource_mut::<Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.traded = true;
        relation.commerce = SETTLEMENT_COMMERCE_MIN;
    }
    game.sync_settlement_footprint(key);
    assert_eq!(
        entity_cells(&mut game, key),
        9,
        "pushing a Thriving city to Starved did not despawn its outer cells"
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
    assert_eq!(entity_cells(&mut game, key), 25, "test premise: a 5x5 city");
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
    assert_eq!(
        entity_cells(&mut game, key),
        25,
        "test premise: a Steady city"
    );

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
        entity_cells(&mut game, key),
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
// Displacement
// ---------------------------------------------------------------------------

/// Every cell `key`'s footprint covers right now, as a set — what the
/// displacement tests below check a moved occupant landed *outside*.
fn footprint_set(game: &mut Game, key: SettlementKey) -> HashSet<(i32, i32)> {
    game.footprint(key).into_iter().collect()
}

/// Every `SurfaceLink`'s tile right now — a fresh zone scatters a few of
/// its own, so the Stack-entrance displacement tests snapshot this before
/// and after rather than counting links outright.
fn surface_link_positions(game: &mut Game) -> HashSet<(i32, i32)> {
    let mut query = game.world.query_filtered::<&Position, With<SurfaceLink>>();
    query.iter(&game.world).map(|p| (p.x, p.y)).collect()
}

/// The next `n` values off the shared RNG stream — `tests::caravans::draws`'
/// own shape, repeated here rather than shared across a `pub(super)` seam
/// neither module otherwise needs.
fn draws(game: &mut Game, n: usize) -> Vec<u64> {
    (0..n)
        .map(|_| game.world.resource_mut::<GameRng>().0.random())
        .collect()
}

#[test]
fn a_wild_creature_under_a_growing_footprint_ends_up_outside_it() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 60, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 30, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let creature = spawn_wild_without_routine(&mut game, "scrapper", centre.0 + 1, centre.1);

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let pos = *game.world.get::<Position>(creature).unwrap();
    assert!(
        !footprint.contains(&(pos.x, pos.y)),
        "the wild creature ended up inside the footprint that grew over it"
    );
}

#[test]
fn a_trap_under_a_growing_footprint_ends_up_outside_it() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 70, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 31, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let trap = game
        .world
        .spawn((
            Trap {
                item: ItemId::from("honeypot"),
                next_roll: 999,
                caught: None,
            },
            Position {
                x: centre.0 - 1,
                y: centre.1,
            },
            Glyph {
                ch: '^',
                color: GlyphColor::Yellow,
            },
        ))
        .id();

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let pos = *game.world.get::<Position>(trap).unwrap();
    assert!(
        !footprint.contains(&(pos.x, pos.y)),
        "the trap ended up inside the footprint that grew over it"
    );
}

/// A nest tethers its guardian by entity, not by tile — see
/// `game/settlement_footprint.rs::displace_nest_at` — so this checks the
/// nest's own `Position` moved and that the guardian's tether still names
/// the same entity, rather than re-deriving `pursuit_field` against it.
#[test]
fn a_nest_and_its_guardian_tether_survive_a_growing_footprint() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 80, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 32, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let nest = spawn_bare_nest(&mut game, centre.0, centre.1 - 1);
    let guardian =
        spawn_pursuing_guardian(&mut game, nest, "scrapper", centre.0 + 30, centre.1 + 30);

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let nest_pos = *game.world.get::<Position>(nest).unwrap();
    assert!(
        !footprint.contains(&(nest_pos.x, nest_pos.y)),
        "the nest ended up inside the footprint that grew over it"
    );
    assert_eq!(
        game.world.get::<NestGuardian>(guardian).map(|g| g.nest),
        Some(nest),
        "the guardian's tether must still name the nest entity after it moved"
    );
}

#[test]
fn a_caravan_under_a_growing_footprint_moves_and_its_arrival_tile_follows() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 90, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 33, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let visit = game.visit_index();
    let caravan_tile = (centre.0, centre.1 + 1);
    let caravan = game
        .world
        .spawn((
            Caravan {
                stage: CaravanStage::Approaching,
                visit,
                arrival_tile: caravan_tile,
                stage_ticks: 0,
                announced_stuck: false,
            },
            Position {
                x: caravan_tile.0,
                y: caravan_tile.1,
            },
            Glyph {
                ch: 'Ω',
                color: GlyphColor::DarkGreen,
            },
        ))
        .id();

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let pos = *game.world.get::<Position>(caravan).unwrap();
    assert!(
        !footprint.contains(&(pos.x, pos.y)),
        "the caravan ended up inside the footprint that grew over it"
    );
    let after = game.world.get::<Caravan>(caravan).unwrap();
    assert_eq!(
        after.arrival_tile,
        (pos.x, pos.y),
        "arrival_tile did not follow the caravan's new position — it would \
         walk home into the settlement that just displaced it"
    );
}

#[test]
fn the_player_under_a_growing_footprint_ends_up_outside_it() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    // One tile from the centre, same as the corner-bump fixture above: a
    // Server's radius-1 square reaches back to cover the player's own
    // standing tile the instant it materializes.
    let centre = (ppos.x + 1, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 34, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert!(
        !footprint.contains(&(pos.x, pos.y)),
        "the player ended up inside the footprint that grew over them"
    );
}

#[test]
fn the_base_anchor_under_a_growing_footprint_moves_outside_it() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 1, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    // Founded before the settlement is placed: the anchor lands wherever the
    // party stands at founding (`Game::move_anchor_to`'s one caller), which
    // is `ppos` here — one tile from the centre the settlement is about to
    // materialize at.
    place_home(&mut game);
    let key = SettlementKey { rx: 35, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);

    game.sync_settlement_footprint(key);

    let footprint = footprint_set(&mut game, key);
    let anchor = game.world.resource::<AnchorEntity>().0;
    let pos = *game.world.get::<Position>(anchor).unwrap();
    assert!(
        !footprint.contains(&(pos.x, pos.y)),
        "the base anchor ended up inside the footprint that grew over it"
    );
}

/// `free_tile_outside` is `relay_landing`'s own search, widened with two
/// entries `relay_landing` never checked before this feature: a trap and an
/// outpost. Blocking the whole ring one band past the footprint with both
/// proves the widened list, not just the wild/nest/link/settlement one it
/// already had.
#[test]
fn free_tile_outside_skips_a_ring_blocked_by_traps_and_outposts() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 150, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 41, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let radius = game
        .settlement_radius(key)
        .expect("test premise: the fixture's own key is materialized");
    let band = radius + 1;

    let mut trap_turn = true;
    for dy in -band..=band {
        for dx in -band..=band {
            if dx.abs() != band && dy.abs() != band {
                continue; // interior of the box, not this ring
            }
            let (x, y) = (centre.0 + dx, centre.1 + dy);
            if trap_turn {
                game.world.spawn((
                    Trap {
                        item: ItemId::from("honeypot"),
                        next_roll: 999,
                        caught: None,
                    },
                    Position { x, y },
                    Glyph {
                        ch: '^',
                        color: GlyphColor::Yellow,
                    },
                ));
            } else {
                game.world
                    .resource_mut::<Outposts>()
                    .0
                    .insert((x, y), crate::outposts::Outpost::new(Biome::OpenGrid, 1));
            }
            trap_turn = !trap_turn;
        }
    }

    let found = game
        .free_tile_outside(key)
        .expect("some tile past the blocked ring must still be free");
    let dist = (found.0 - centre.0).abs().max((found.1 - centre.1).abs());
    assert!(
        dist > band,
        "free_tile_outside picked a tile on the blocked ring: {found:?} (band {band})"
    );
}

#[test]
fn a_stack_entrance_under_a_growing_footprint_relocates_and_drops_its_memory() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 100, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    // Snapshotted before this test's own entrance is spawned, since a fresh
    // zone already scatters a few links of its own (`spawn_surface_links`)
    // and this test cares only about the one it placed.
    let before: HashSet<(i32, i32)> = surface_link_positions(&mut game);
    let key = SettlementKey { rx: 36, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let entrance = (centre.0 + 1, centre.1);
    game.spawn_entrance_at(entrance.0, entrance.1);
    game.world
        .resource_mut::<StackMemory>()
        .0
        .insert((entrance, 1), FrameMemory::default());

    game.sync_settlement_footprint(key);

    assert!(
        game.find_surface_link_at(entrance.0, entrance.1).is_none(),
        "the old entrance is still standing inside the footprint that grew over it"
    );
    assert!(
        !game
            .world
            .resource::<StackMemory>()
            .0
            .contains_key(&(entrance, 1)),
        "the collapsed entrance's own memory was not dropped"
    );
    let footprint = footprint_set(&mut game, key);
    let after = surface_link_positions(&mut game);
    let new_ones: Vec<&(i32, i32)> = after.difference(&before).collect();
    assert_eq!(
        new_ones.len(),
        1,
        "the entrance did not reopen exactly once: {after:?}"
    );
    assert!(
        !footprint.contains(new_ones[0]),
        "the reopened entrance landed back inside the footprint"
    );
}

#[test]
fn a_stack_entrance_is_not_relocated_while_the_party_stands_inside_it() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 110, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 37, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let entrance = (centre.0 + 1, centre.1);
    game.spawn_entrance_at(entrance.0, entrance.1);
    {
        let mut pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        pos.x = entrance.0;
        pos.y = entrance.1;
    }
    descend(&mut game);
    assert_eq!(
        game.stack_pos().map(|pos| pos.entrance),
        Some(entrance),
        "test premise: the party is inside the stack under this very entrance"
    );

    game.sync_settlement_footprint(key);

    assert!(
        game.find_surface_link_at(entrance.0, entrance.1).is_some(),
        "the entrance under the party was relocated out from under them"
    );
}

#[test]
fn a_deferred_stack_entrance_relocates_once_the_party_surfaces() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 120, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 38, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let entrance = (centre.0 + 1, centre.1);
    game.spawn_entrance_at(entrance.0, entrance.1);
    {
        let mut pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        pos.x = entrance.0;
        pos.y = entrance.1;
    }
    descend(&mut game);
    game.sync_settlement_footprint(key);
    assert!(
        game.find_surface_link_at(entrance.0, entrance.1).is_some(),
        "test premise: the entrance is still deferred while the party is inside"
    );

    game.ascend(); // from depth 1, standing on the link up: this leaves the Stack

    assert_eq!(
        game.locale(),
        Locale::Surface,
        "test premise: the party surfaced"
    );
    game.sync_settlement_footprint(key);

    assert!(
        game.find_surface_link_at(entrance.0, entrance.1).is_none(),
        "a deferred entrance never relocated once the party surfaced"
    );
    assert!(
        !game
            .world
            .resource::<StackMemory>()
            .0
            .contains_key(&(entrance, 1)),
        "the deferred entrance's memory was not dropped on relocation"
    );
    let footprint = footprint_set(&mut game, key);
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert!(
        !footprint.contains(&(player_pos.x, player_pos.y)),
        "the player, standing on the old entrance after surfacing, must also \
         be shoved off the footprint"
    );
}

/// Every mover `displace` runs writes a `Position` outright — no roll of its
/// own — `stack::generate`'s world-generation rule extended to settlement
/// growth. Every occupant kind stands somewhere in the footprint at once, so
/// a mover added later that forgets this would still be exercised here.
#[test]
fn settlement_displacement_draws_no_game_rng() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 130, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 6);
    let key = SettlementKey { rx: 39, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    spawn_wild_without_routine(&mut game, "scrapper", centre.0 + 1, centre.1);
    game.world.spawn((
        Trap {
            item: ItemId::from("honeypot"),
            next_roll: 999,
            caught: None,
        },
        Position {
            x: centre.0 - 1,
            y: centre.1,
        },
        Glyph {
            ch: '^',
            color: GlyphColor::Yellow,
        },
    ));
    spawn_bare_nest(&mut game, centre.0, centre.1 + 1);
    let visit = game.visit_index();
    game.world.spawn((
        Caravan {
            stage: CaravanStage::Approaching,
            visit,
            arrival_tile: (centre.0, centre.1 - 1),
            stage_ticks: 0,
            announced_stuck: false,
        },
        Position {
            x: centre.0,
            y: centre.1 - 1,
        },
        Glyph {
            ch: 'Ω',
            color: GlyphColor::DarkGreen,
        },
    ));

    reseed_rng(&mut game, 4242);
    let control = draws(&mut game, 6);
    reseed_rng(&mut game, 4242);
    game.sync_settlement_footprint(key);
    let after = draws(&mut game, 6);

    assert_eq!(
        control, after,
        "settlement displacement drew from the shared RNG stream"
    );
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

/// `Game::try_spawn_habitat_creature` is the one gate `populate_chunk` and
/// `spawn_wild_nearby` both land in, so this covers the whole wild-spawn
/// chain rather than one caller of it.
#[test]
fn no_wild_creature_ever_spawns_on_a_settlement_footprint_cell() {
    let mut game = game();
    let centre = (300, 300);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 2);
    let key = SettlementKey { rx: 90, ry: 90 };
    place_settlement(&mut game, key, centre.0, centre.1);
    game.sync_settlement_footprint(key);
    let footprint = game.footprint(key);
    assert_eq!(footprint.len(), 9, "test premise: a full Server footprint");

    for &(x, y) in &footprint {
        assert!(
            !game.try_spawn_habitat_creature(x, y),
            "a wild creature spawned on footprint cell ({x}, {y})"
        );
    }

    // Non-vacuous: the carved ground one tile past the footprint is exactly
    // as walkable and habitat-matched as the cells inside it, so it spawns
    // freely — proving the cells above were refused *because* of the
    // footprint and not for want of open ground or a matching species.
    let outside = (centre.0 + SETTLEMENT_RADIUS_SERVER + 1, centre.1);
    assert!(
        game.find_settlement_at(outside.0, outside.1).is_none(),
        "test premise: the control tile must sit outside the footprint"
    );
    assert!(
        game.try_spawn_habitat_creature(outside.0, outside.1),
        "test premise: carved ground outside the footprint should spawn freely"
    );
}

/// The RNG-stream trap this seam warns about: a settlement check placed
/// *after* `pick_habitat_species` has already drawn would shift every
/// seeded spawn test that happens to roll near a town by a different amount
/// than one that never does. The guard sits ahead of every draw instead, the
/// same place the unwalkable-tile check already lived.
#[test]
fn refusing_a_footprint_cell_draws_nothing_from_the_shared_rng() {
    let mut game = game();
    let centre = (400, 400);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 2);
    let key = SettlementKey { rx: 91, ry: 91 };
    place_settlement(&mut game, key, centre.0, centre.1);
    game.sync_settlement_footprint(key);

    reseed_rng(&mut game, 4242);
    let control = draws(&mut game, 4);
    reseed_rng(&mut game, 4242);
    assert!(!game.try_spawn_habitat_creature(centre.0, centre.1));
    let after = draws(&mut game, 4);

    assert_eq!(
        control, after,
        "a refused footprint cell drew from the shared RNG stream"
    );
}

/// A pack's own anchor passes `try_spawn_habitat_creature`'s check, but the
/// rest of the pack scatters around it through `scatter_open_tile`, which
/// has to ask the same question on its own account — an anchor placed just
/// outside a footprint's edge is well within a size-12 pack's own scatter
/// radius.
#[test]
fn no_pack_member_ever_scatters_onto_a_settlement_footprint_cell() {
    for seed in 0..40u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let centre = (300, 300);
        carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 10);
        let key = SettlementKey { rx: 92, ry: 92 };
        place_settlement(&mut game, key, centre.0, centre.1);
        game.sync_settlement_footprint(key);

        let anchor = (centre.0 + SETTLEMENT_RADIUS_SERVER + 1, centre.1);
        assert!(
            game.find_settlement_at(anchor.0, anchor.1).is_none(),
            "test premise: the anchor sits outside the footprint"
        );

        let pack = game.spawn_group(
            "overseer",
            12,
            anchor.0,
            anchor.1,
            crate::game::spawning::SpawnEscalation::surface(),
            false,
        );

        for entity in pack {
            let pos = *game.world.get::<Position>(entity).unwrap();
            assert!(
                game.find_settlement_at(pos.x, pos.y).is_none(),
                "seed {seed}: a pack member scattered onto the footprint at {pos:?}"
            );
        }
    }
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
    assert_eq!(entity_cells(&mut game, key), 49, "test premise: Thriving");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        entity_cells(&mut loaded, key),
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

/// CRITICAL: `restore_settlements` used to run while `Locale` was still the
/// default `Surface` (`restore_locale` runs last in `Game::load`), so a load
/// taken while the party stood inside a Stack whose entrance a footprint
/// covers saw `stack_pos()` answer `None` — the deferral this seam depends on
/// never fired, and the load itself relocated the entrance, dropped its
/// memory, and shoved the pinned player `Position` off the entrance tile.
#[test]
fn a_load_defers_a_stack_entrance_a_footprint_covers_until_the_party_surfaces() {
    let dir = scratch_assets_dir("settlement_footprint_load_defer");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let centre = (ppos.x + 200, ppos.y);
    carve_open(&mut game, centre, SETTLEMENT_RADIUS_SERVER + 4);
    let key = SettlementKey { rx: 50, ry: 0 };
    place_settlement(&mut game, key, centre.0, centre.1);
    let entrance = (centre.0 + 1, centre.1);
    game.spawn_entrance_at(entrance.0, entrance.1);
    game.world
        .resource_mut::<StackMemory>()
        .0
        .insert((entrance, 1), FrameMemory::default());
    {
        let mut pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        pos.x = entrance.0;
        pos.y = entrance.1;
    }
    descend(&mut game);
    assert_eq!(
        game.stack_pos().map(|pos| pos.entrance),
        Some(entrance),
        "test premise: the party is inside the stack under this very entrance"
    );

    // Grows the footprint over the entrance while the party stands inside it
    // — the deferral state the design calls out.
    game.sync_settlement_footprint(key);
    assert!(
        game.find_surface_link_at(entrance.0, entrance.1).is_some(),
        "test premise: the entrance is still deferred while the party is inside"
    );
    let before_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.stack_pos().map(|pos| pos.entrance),
        Some(entrance),
        "a load relocated the deferred entrance's own locale"
    );
    assert!(
        loaded
            .find_surface_link_at(entrance.0, entrance.1)
            .is_some(),
        "a load relocated the Stack entrance the party is standing inside"
    );
    assert!(
        loaded
            .world
            .resource::<StackMemory>()
            .0
            .contains_key(&(entrance, 1)),
        "a load dropped the deferred entrance's own StackMemory"
    );
    let after_pos = *loaded
        .world
        .get::<Position>(loaded.player_entity())
        .unwrap();
    assert_eq!(
        (after_pos.x, after_pos.y),
        (before_pos.x, before_pos.y),
        "a load moved the player's Position, pinned to the entrance while underground"
    );

    loaded.ascend();
    assert_eq!(
        loaded.locale(),
        Locale::Surface,
        "test premise: ascending from depth 1 on the link up surfaces the party"
    );
    loaded.sync_settlement_footprint(key);
    assert!(
        loaded
            .find_surface_link_at(entrance.0, entrance.1)
            .is_none(),
        "surfacing after a load never relocated the deferred entrance"
    );
}

// Findings 2-4's reproducers land in later commits, once each fix is green.
