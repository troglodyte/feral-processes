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
    game.take_stairs();

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

    game.take_stairs(); // the party arrives standing on the stairs up

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
    game.take_stairs(); // to depth 2, arriving on its stairs up
    game.take_stairs(); // back to depth 1

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
