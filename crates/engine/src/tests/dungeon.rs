//! The dungeon layer: getting in, moving around, and the surface carrying
//! on without you.

use super::support::*;
use crate::dungeon::{CellKind, Dir};
use crate::resources::{CurrentDungeon, Locale};
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Drops the party into depth 1 through an entrance at the player's current
/// tile, which is what walking onto one does.
fn descend(game: &mut Game) -> (i32, i32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_dungeon(pos.x, pos.y);
    (pos.x, pos.y)
}

fn locale(game: &Game) -> Locale {
    game.locale()
}

fn cell_at(game: &Game, x: i32, y: i32) -> CellKind {
    game.world
        .resource::<CurrentDungeon>()
        .0
        .as_ref()
        .unwrap()
        .cell(x, y)
}

/// Faces the party down a direction they can actually walk, so a movement
/// assertion isn't silently testing a wall.
fn face_an_open_way(game: &mut Game) -> Dir {
    for _ in 0..4 {
        let Locale::Dungeon { x, y, facing, .. } = locale(game) else {
            panic!("not underground");
        };
        let (dx, dy) = facing.delta();
        if cell_at(game, x + dx, y + dy).walkable() {
            return facing;
        }
        game.turn_right();
    }
    panic!("the entry cell is walled in on all four sides");
}

#[test]
fn entering_a_dungeon_pins_the_players_surface_position_to_the_entrance() {
    let mut game = game();
    let entrance = descend(&mut game);

    assert!(game.is_underground());
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        entrance,
        "the surface Position must stay on the entrance tile"
    );
}

/// The load-bearing property of the whole design: walking around underground
/// must not drag the player across the zone map.
#[test]
fn walking_underground_never_moves_the_players_surface_position() {
    let mut game = game();
    let entrance = descend(&mut game);
    face_an_open_way(&mut game);

    for _ in 0..20 {
        game.step_forward();
        game.turn_right();
        game.step_forward();
    }

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), entrance);
}

#[test]
fn stepping_forward_advances_along_the_facing() {
    let mut game = game();
    descend(&mut game);
    let facing = face_an_open_way(&mut game);
    let Locale::Dungeon { x, y, .. } = locale(&game) else {
        unreachable!()
    };

    game.step_forward();

    let (dx, dy) = facing.delta();
    let Locale::Dungeon {
        x: nx,
        y: ny,
        facing: after,
        ..
    } = locale(&game)
    else {
        unreachable!()
    };
    assert_eq!((nx, ny), (x + dx, y + dy));
    assert_eq!(after, facing, "walking must not change which way you face");
}

#[test]
fn backing_up_retreats_without_turning_round() {
    let mut game = game();
    descend(&mut game);
    let facing = face_an_open_way(&mut game);
    let Locale::Dungeon { x, y, .. } = locale(&game) else {
        unreachable!()
    };

    game.step_forward();
    game.step_back();

    let Locale::Dungeon {
        x: nx,
        y: ny,
        facing: after,
        ..
    } = locale(&game)
    else {
        unreachable!()
    };
    assert_eq!((nx, ny), (x, y), "backing up should undo the step");
    assert_eq!(after, facing);
}

#[test]
fn stepping_into_rock_does_not_move_the_party() {
    let mut game = game();
    descend(&mut game);

    // Turn until a wall is dead ahead, then shove at it.
    let mut faced_a_wall = false;
    for _ in 0..4 {
        let Locale::Dungeon { x, y, facing, .. } = locale(&game) else {
            unreachable!()
        };
        let (dx, dy) = facing.delta();
        if !cell_at(&game, x + dx, y + dy).walkable() {
            game.step_forward();
            let Locale::Dungeon { x: nx, y: ny, .. } = locale(&game) else {
                unreachable!()
            };
            assert_eq!((nx, ny), (x, y), "walked into solid rock");
            faced_a_wall = true;
            break;
        }
        game.turn_right();
    }
    assert!(
        faced_a_wall,
        "the entry cell should border at least one wall"
    );
}

#[test]
fn turning_left_and_right_change_the_facing_and_nothing_else() {
    let mut game = game();
    descend(&mut game);
    let Locale::Dungeon { x, y, facing, .. } = locale(&game) else {
        unreachable!()
    };

    game.turn_left();
    let Locale::Dungeon {
        x: lx,
        y: ly,
        facing: left,
        ..
    } = locale(&game)
    else {
        unreachable!()
    };
    assert_eq!(left, facing.turn_left());
    assert_eq!((lx, ly), (x, y));

    game.turn_right();
    let Locale::Dungeon { facing: back, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!(back, facing);
}

#[test]
fn the_party_arrives_on_the_stairs_up_facing_north() {
    let mut game = game();
    descend(&mut game);
    let Locale::Dungeon {
        depth,
        x,
        y,
        facing,
        ..
    } = locale(&game)
    else {
        unreachable!()
    };
    assert_eq!(depth, 1);
    assert_eq!(facing, Dir::North);
    assert_eq!(cell_at(&game, x, y), CellKind::StairsUp);
}

#[test]
fn taking_the_stairs_down_increments_the_depth_and_regenerates_the_level() {
    let mut game = game();
    descend(&mut game);
    let first: Vec<CellKind> = {
        let level = game.world.resource::<CurrentDungeon>().0.as_ref().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };

    // Stand on the way down, then take it.
    let down = game
        .world
        .resource::<CurrentDungeon>()
        .0
        .as_ref()
        .unwrap()
        .stairs_down;
    let Locale::Dungeon {
        facing, entrance, ..
    } = locale(&game)
    else {
        unreachable!()
    };
    game.world.insert_resource(Locale::Dungeon {
        depth: 1,
        x: down.0,
        y: down.1,
        facing,
        entrance,
    });
    game.descend();

    let Locale::Dungeon { depth, .. } = locale(&game) else {
        panic!("descending should leave us underground")
    };
    assert_eq!(depth, 2);

    let second: Vec<CellKind> = {
        let level = game.world.resource::<CurrentDungeon>().0.as_ref().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };
    assert_ne!(first, second, "depth 2 should be its own level");
}

#[test]
fn climbing_out_of_depth_one_returns_to_the_surface_with_movement_working() {
    let mut game = game();
    let entrance = descend(&mut game);

    game.ascend(); // the party arrives standing on the stairs up

    assert!(!game.is_underground());
    assert_eq!(locale(&game), Locale::Surface);
    assert!(
        game.world.resource::<CurrentDungeon>().0.is_none(),
        "surfacing should drop the level"
    );

    // And surface movement works again — it was refused while underground.
    let before = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((before.x, before.y), entrance);
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
        game.move_player(dx, dy);
        let after = *game.world.get::<Position>(game.player_entity()).unwrap();
        if (after.x, after.y) != (before.x, before.y) {
            return;
        }
    }
    panic!("no direction moved the player after surfacing");
}

#[test]
fn descending_then_climbing_back_lands_on_that_levels_stairs_down() {
    let mut game = game();
    descend(&mut game);

    let down = game
        .world
        .resource::<CurrentDungeon>()
        .0
        .as_ref()
        .unwrap()
        .stairs_down;
    let Locale::Dungeon {
        facing, entrance, ..
    } = locale(&game)
    else {
        unreachable!()
    };
    game.world.insert_resource(Locale::Dungeon {
        depth: 1,
        x: down.0,
        y: down.1,
        facing,
        entrance,
    });
    game.descend(); // to depth 2, arriving on its stairs up
    game.ascend(); // back to depth 1

    let Locale::Dungeon { depth, x, y, .. } = locale(&game) else {
        panic!("climbing from depth 2 should stay underground")
    };
    assert_eq!(depth, 1);
    assert_eq!(
        (x, y),
        down,
        "climbing must land on the stairs you went down, not the level's entry"
    );
}

#[test]
fn surface_movement_is_refused_while_underground() {
    let mut game = game();
    let entrance = descend(&mut game);
    let before = locale(&game);

    game.move_player(1, 0);
    game.move_player(0, 1);

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), entrance);
    assert_eq!(
        locale(&game),
        before,
        "surface keys must not move the party"
    );
}

/// The whole point of keeping the player's `Position` on the surface: the
/// base keeps running while they're down a hole.
#[test]
fn the_surface_simulation_keeps_ticking_while_the_party_is_underground() {
    let mut game = game();
    let before = game.current_tick();
    descend(&mut game);
    face_an_open_way(&mut game);

    for _ in 0..10 {
        game.step_forward();
        game.turn_right();
    }

    assert!(
        game.current_tick() > before,
        "walking a dungeon must still pass surface time"
    );
}

#[test]
fn shoving_at_a_wall_still_passes_time() {
    let mut game = game();
    descend(&mut game);
    // Face a wall.
    for _ in 0..4 {
        let Locale::Dungeon { x, y, facing, .. } = locale(&game) else {
            unreachable!()
        };
        let (dx, dy) = facing.delta();
        if !cell_at(&game, x + dx, y + dy).walkable() {
            break;
        }
        game.turn_right();
    }
    let before = game.current_tick();
    game.step_forward();
    assert!(game.current_tick() > before);
}

#[test]
fn deploying_a_structure_is_refused_underground() {
    let mut game = game();
    descend(&mut game);
    let Err(reason) = game.place_structure("home", 1, 0) else {
        panic!("a Home should not go up inside a dungeon");
    };
    assert!(
        reason.contains("open grid"),
        "the refusal should say why, got: {reason}"
    );
}

#[test]
fn a_symlink_cannot_be_used_underground() {
    let mut game = game();
    game.place_structure("home", 1, 0).unwrap();
    let home = game
        .view_entities(5, 5)
        .into_iter()
        .find(|e| e.is_home)
        .expect("the Home just deployed");
    descend(&mut game);

    let Err(reason) = game.use_symlink(home.entity) else {
        panic!("a symlink underground would relocate the entrance we climb out of");
    };
    assert!(reason.contains("open grid"), "got: {reason}");
}

#[test]
fn resting_is_refused_underground() {
    let mut game = game();
    descend(&mut game);
    let before = game.current_tick();
    game.rest();
    assert_eq!(before, game.current_tick(), "rest should not have run");
}

#[test]
fn party_management_still_works_underground() {
    let mut game = game();
    let pet = spawn_tamed(&mut game, 20, 5);
    descend(&mut game);
    // Managing the roster four levels down is a thing the genre expects, so
    // it must not be swept up by the surface-only guard.
    assert!(game.owned_pets().iter().any(|p| p.entity == pet));
}

#[test]
fn the_view_is_none_on_the_surface_and_some_underground() {
    let mut game = game();
    assert!(game.dungeon_view().is_none());
    descend(&mut game);
    assert!(game.dungeon_view().is_some());
}

#[test]
fn the_view_cone_is_rotated_so_straight_ahead_is_always_the_middle_column() {
    let mut game = game();
    descend(&mut game);

    for _ in 0..4 {
        let Locale::Dungeon { x, y, facing, .. } = locale(&game) else {
            unreachable!()
        };
        let view = game.dungeon_view().unwrap();
        let (dx, dy) = facing.delta();

        // Row 0 is the cell the party stands in; row 1 middle is one step
        // ahead along the facing, whichever way that points.
        let ahead = cell_at(&game, x + dx, y + dy);
        let middle = crate::game::dungeon::DUNGEON_VIEW_HALF_WIDTH;
        assert_eq!(
            view.cells[1][middle] == DungeonCellView::Rock,
            !ahead.walkable()
        );
        assert_eq!(view.facing, facing.label());

        game.turn_right();
    }
}

#[test]
fn the_view_reads_solid_rock_past_the_edge_of_the_level() {
    let mut game = game();
    descend(&mut game);
    // The entry sits at (1, 1) facing north — one step off the top edge.
    let view = game.dungeon_view().unwrap();
    let middle = crate::game::dungeon::DUNGEON_VIEW_HALF_WIDTH;
    assert_eq!(view.cells[1][middle], DungeonCellView::Rock);
    assert!(view.cells.len() >= 2);
}

#[test]
fn the_view_names_what_the_party_is_standing_on() {
    let mut game = game();
    descend(&mut game);
    let view = game.dungeon_view().unwrap();
    assert!(
        view.standing_on
            .as_deref()
            .is_some_and(|s| s.contains("surface")),
        "depth 1's entry should offer the way out, got {:?}",
        view.standing_on
    );
}

#[test]
fn a_new_zone_is_seeded_with_dungeon_entrances() {
    let game = game();
    let entrances = game
        .world
        .iter_entities()
        .filter(|e| e.contains::<DungeonEntrance>())
        .count();
    assert_eq!(entrances, crate::tuning::DUNGEON_ENTRANCES_PER_ZONE);
}

#[test]
fn no_entrance_opens_onto_unwalkable_ground() {
    let mut game = game();
    let tiles = entrance_tiles(&mut game);
    assert!(!tiles.is_empty());
    for (x, y) in tiles {
        let tile = game.world.resource_mut::<WorldMap>().tile(x, y);
        assert!(tile.walkable, "entrance at ({x}, {y}) sits in a wall");
    }
}

/// The Platform check only has anything to do on a zone breach, where the
/// base slab is stamped down *before* the new sector's breaches are placed
/// (see `enter_next_zone`). On a fresh run no platform exists yet, and a
/// player later stamping a Home over a breach is their own doing — that
/// still works, and a dungeon mouth inside your base is a fine place for one.
#[test]
fn breaching_with_a_base_never_opens_a_breach_inside_the_platform() {
    let mut game = game();
    // Home to the south so it doesn't share the portal's tile.
    game.place_structure("home", 0, 1).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));
    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);

    let tiles = entrance_tiles(&mut game);
    assert!(!tiles.is_empty());
    for (x, y) in tiles {
        let tile = game.world.resource_mut::<WorldMap>().tile(x, y);
        assert_ne!(
            tile.biome,
            Biome::Platform,
            "a breach opened at ({x}, {y}), inside the one safe ground in the game"
        );
    }
}

#[test]
fn a_structure_cannot_be_deployed_on_top_of_a_breach() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Clear the way, then put a breach right where the Home would go.
    game.world.spawn((
        DungeonEntrance,
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
        Glyph {
            ch: '>',
            color: GlyphColor::Magenta,
        },
    ));
    let Err(reason) = game.place_structure("home", 1, 0) else {
        panic!("a structure sharing a tile with a breach makes the tile ambiguous to walk onto");
    };
    assert!(reason.contains("breach"), "got: {reason}");
}

#[test]
fn walking_onto_an_entrance_descends_and_leaves_the_entrance_standing() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let target = (ppos.x + 1, ppos.y);
    game.world.spawn((
        DungeonEntrance,
        Position {
            x: target.0,
            y: target.1,
        },
        Glyph {
            ch: '%',
            color: GlyphColor::Magenta,
        },
    ));

    game.move_player(1, 0);

    assert!(game.is_underground());
    // Unlike a zone portal, an entrance is a place you come back to.
    assert!(
        game.find_dungeon_entrance_at(target.0, target.1).is_some(),
        "the entrance must survive being used"
    );
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), target);
}

#[test]
fn a_dungeon_position_survives_a_save_and_load_with_an_identical_level() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    descend(&mut game);
    face_an_open_way(&mut game);
    game.step_forward();
    game.turn_right();
    let before = locale(&game);
    let cells_before: Vec<CellKind> = {
        let level = game.world.resource::<CurrentDungeon>().0.as_ref().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };

    let path = std::env::temp_dir().join(format!(
        "feral_processes_dungeon_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.locale(),
        before,
        "depth, cell and facing must all survive"
    );
    let cells_after: Vec<CellKind> = {
        let level = loaded
            .world
            .resource::<CurrentDungeon>()
            .0
            .as_ref()
            .unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };
    assert_eq!(
        cells_before, cells_after,
        "the level regenerates from the seed — a different one would strand the party in rock"
    );
}

fn entrance_tiles(game: &mut Game) -> Vec<(i32, i32)> {
    let mut query = game
        .world
        .query_filtered::<&Position, With<DungeonEntrance>>();
    let mut tiles: Vec<(i32, i32)> = query.iter(&game.world).map(|p| (p.x, p.y)).collect();
    tiles.sort();
    tiles
}

#[test]
fn entrances_survive_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let before = entrance_tiles(&mut game);
    assert!(!before.is_empty());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_entrance_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(before, entrance_tiles(&mut loaded));
}

#[test]
fn a_save_made_on_the_surface_loads_back_onto_the_surface() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_surface_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.locale(), Locale::Surface);
    assert!(loaded.dungeon_view().is_none());
}

/// Placing entrances must not touch `GameRng`. It did once, and shifting the
/// shared stream silently rewrote the outcome of a seeded combat test three
/// files away — the failure surfaced nowhere near the cause.
#[test]
fn seeding_a_zones_entrances_does_not_disturb_the_shared_rng_stream() {
    let mut untouched = game();
    let before: Vec<u32> = {
        let mut rng = untouched.world.resource_mut::<GameRng>();
        (0..8).map(|_| rng.0.random_range(0..1_000_000)).collect()
    };

    let mut fresh = game();
    fresh.spawn_dungeon_entrances(crate::tuning::DUNGEON_ENTRANCES_PER_ZONE);
    let after: Vec<u32> = {
        let mut rng = fresh.world.resource_mut::<GameRng>();
        (0..8).map(|_| rng.0.random_range(0..1_000_000)).collect()
    };

    assert_eq!(
        before, after,
        "entrance placement drew from the shared stream and moved it"
    );
}

/// The same zone of the same world always opens onto the same breaches, so
/// loading a save and re-entering a zone can't shuffle them.
#[test]
fn entrance_placement_is_a_pure_function_of_the_seed_and_zone() {
    let mut a = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut b = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(entrance_tiles(&mut a), entrance_tiles(&mut b));
    assert!(!entrance_tiles(&mut a).is_empty());
}

/// The bug this guards: with every entrance scattered to the full radius,
/// most seeds put all three off screen on arrival, and nothing told the
/// player breaches existed at all. At the default zoom the map pane shows
/// roughly +/-16 by +/-9 tiles.
const OPENING_VIEW_HALF_W: i32 = 16;
const OPENING_VIEW_HALF_H: i32 = 9;

#[test]
fn every_seed_puts_one_breach_inside_the_opening_view() {
    for seed in [16u32, 43, 77, 101, 2024, 7, 999, 31337] {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let origin = *game.world.get::<Position>(game.player_entity()).unwrap();
        let visible = entrance_tiles(&mut game).into_iter().any(|(x, y)| {
            (x - origin.x).abs() <= OPENING_VIEW_HALF_W
                && (y - origin.y).abs() <= OPENING_VIEW_HALF_H
        });
        assert!(
            visible,
            "seed {seed} starts the player with no breach on screen and no way to know one exists"
        );
    }
}

#[test]
fn the_remaining_breaches_are_still_a_trip() {
    let mut game = Game::new(2024, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let origin = *game.world.get::<Position>(game.player_entity()).unwrap();
    let far = entrance_tiles(&mut game)
        .into_iter()
        .filter(|(x, y)| {
            (x - origin.x).abs() > OPENING_VIEW_HALF_W || (y - origin.y).abs() > OPENING_VIEW_HALF_H
        })
        .count();
    assert!(
        far > 0,
        "if every breach is underfoot there is nothing left to explore for"
    );
}

#[test]
fn arriving_in_a_zone_scans_for_breaches_and_says_where_the_nearest_is() {
    let game = game();
    let scan = game
        .message_log(50)
        .into_iter()
        .find(|(_, line)| line.contains("Deep scan"))
        .map(|(_, line)| line);
    let Some(scan) = scan else {
        panic!("arriving in a zone should report what the scan found");
    };
    assert!(scan.contains("breaches"), "got: {scan}");
    assert!(
        ["north", "south", "east", "west"]
            .iter()
            .any(|d| scan.contains(d)),
        "the scan must give a bearing to walk, got: {scan}"
    );
}

#[test]
fn breaching_a_zone_scans_the_new_sector_too() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));
    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);
    assert!(
        game.message_log(50)
            .iter()
            .any(|(_, line)| line.contains("Deep scan")),
        "a fresh sector needs its own scan, or the layer is invisible again"
    );
}

#[test]
fn a_bearing_names_the_direction_you_would_actually_walk() {
    use crate::game::dungeon::bearing;
    // North is -y, matching dungeon::Dir and the renderer.
    assert_eq!(bearing(0, -10), "north");
    assert_eq!(bearing(0, 10), "south");
    assert_eq!(bearing(10, 0), "east");
    assert_eq!(bearing(-10, 0), "west");
    assert_eq!(bearing(10, -10), "north-east");
    assert_eq!(bearing(-10, -10), "north-west");
    assert_eq!(bearing(10, 10), "south-east");
    assert_eq!(bearing(-10, 10), "south-west");
}

#[test]
fn a_bearing_only_goes_diagonal_when_neither_axis_dominates() {
    use crate::game::dungeon::bearing;
    // Mostly east with a slight northerly lean is still east — calling it
    // north-east would send the player off at an angle.
    assert_eq!(bearing(20, -3), "east");
    assert_eq!(bearing(3, -20), "north");
    assert_eq!(bearing(0, 0), "here");
}

/// Breaching does not despawn structures — the base travels — so anything
/// zone-local has to be wiped by name in `enter_next_zone`. Entrances are
/// zone-local: each opens onto a level generated for its own sector.
#[test]
fn a_breach_leaves_the_previous_sectors_entrances_behind() {
    let mut game = game();
    let before = entrance_tiles(&mut game);
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));
    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);

    let after = entrance_tiles(&mut game);
    assert_eq!(
        after.len(),
        crate::tuning::DUNGEON_ENTRANCES_PER_ZONE,
        "the new sector should hold its own breaches and no more — old ones rode the breach"
    );
    assert_ne!(before, after, "the new sector needs its own breaches");
}

/// A breach on the arrival tile means starting the run standing on one; a
/// breach one step away means the first movement key of the run drops the
/// player into a dungeon they never chose to enter.
#[test]
fn no_breach_opens_on_top_of_the_player_or_within_a_step_of_them() {
    for seed in 0u32..40 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let origin = *game.world.get::<Position>(game.player_entity()).unwrap();
        for (x, y) in entrance_tiles(&mut game) {
            let distance = (x - origin.x).abs().max((y - origin.y).abs());
            assert!(
                distance >= crate::tuning::DUNGEON_MIN_ENTRANCE_TILES,
                "seed {seed}: breach at ({x}, {y}) is {distance} tiles from the player"
            );
        }
    }
}
