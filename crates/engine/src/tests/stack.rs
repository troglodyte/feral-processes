//! The Stack layer: getting in, moving around, and the surface carrying
//! on without you.

use super::support::*;
use crate::game::stack::StackPos;
use crate::resources::{CurrentStack, Locale};
use crate::stack::{CellKind, Dir};
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Drops the party into depth 1 through an entrance at the player's current
/// tile, which is what walking onto one does.
fn descend(game: &mut Game) -> (i32, i32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    (pos.x, pos.y)
}

fn locale(game: &Game) -> Locale {
    game.locale()
}

/// Walking in the Stack rolls for an ambush (`Game::maybe_stack_encounter`),
/// and an open battle refuses every later step. Tests about doors, seals and
/// backing up are not about ambushes, so they shake one off and carry on —
/// otherwise their fixture silently stops moving and the assertion reads as a
/// broken door rather than an interrupted walk. Which step draws an ambush is
/// a property of the seed and of how much `GameRng` everything before it
/// spent, so it moves whenever content is added or an id is renamed.
fn step_forward_clear(game: &mut Game) {
    game.step_forward();
    if game.has_active_battle() {
        flee_until_clear(game);
    }
}

fn step_back_clear(game: &mut Game) {
    game.step_back();
    if game.has_active_battle() {
        flee_until_clear(game);
    }
}

fn cell_at(game: &Game, x: i32, y: i32) -> CellKind {
    game.world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .cell(x, y)
}

/// Teleports the party onto the current frame's way down and returns that
/// cell, so a test about descending doesn't have to walk the maze to reach
/// the link.
fn stand_on_link_down(game: &mut Game) -> (i32, i32) {
    let down = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .link_down
        .expect("this frame should have a way down");
    let Locale::Stack {
        depth,
        frames,
        facing,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: down.0,
        y: down.1,
        facing,
        entrance,
    });
    down
}

/// Teleports the party into a doorway, facing along the corridor it is hung
/// in, and returns both. `place_doors` only ever hangs a door in a cell with
/// exactly two exits opposite each other, so such a heading always exists.
fn stand_in_a_doorway(game: &mut Game) -> ((i32, i32), Dir) {
    let (door, heading) = {
        let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
        let door = (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .find(|&(x, y)| level.cell(x, y) == CellKind::Door)
            .expect("every frame hangs doors");
        let heading = if level.walkable(door.0, door.1 - 1) {
            Dir::North
        } else {
            Dir::East
        };
        (door, heading)
    };
    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: door.0,
        y: door.1,
        facing: heading,
        entrance,
    });
    (door, heading)
}

/// Every cell of the frame the party is standing in, row-major — for
/// asserting that two frames are or aren't the same maze.
fn frame_cells(game: &Game) -> Vec<CellKind> {
    let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
    (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .map(|(x, y)| level.cell(x, y))
        .collect()
}

/// Walks `steps` corridor cells, jacking out of anything that jumps the
/// party on the way.
///
/// Necessary because `face_an_open_way` turns, and turning is refused
/// mid-intrusion — so a naive walk loop panics the moment the encounter
/// roll comes up, on a schedule that varies with the seed.
fn walk_corridors(game: &mut Game, steps: usize) {
    for _ in 0..steps {
        if game.has_active_battle() {
            flee_until_clear(game);
        }
        face_an_open_way(game);
        game.step_forward();
    }
    if game.has_active_battle() {
        flee_until_clear(game);
    }
}

/// Stands the party on a walkable cell with solid rock dead ahead.
///
/// The entry cell used to be guaranteed to border a wall, back when every
/// frame was a maze and the entry was the lattice corner at (1, 1). A
/// `FrameLayout::Rooms` entry is a room's centre with four open neighbours,
/// so a test about shoving at rock now has to go and find some. Every layout
/// is walled in, so a wall-adjacent cell always exists.
fn face_a_wall(game: &mut Game) {
    let level = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .clone();
    let spot = (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .flat_map(|(x, y)| [Dir::North, Dir::East, Dir::South, Dir::West].map(|dir| ((x, y), dir)))
        .find(|&((x, y), dir)| {
            let (dx, dy) = dir.delta();
            level.cell(x, y) == CellKind::Floor && !level.walkable(x + dx, y + dy)
        });
    let ((x, y), facing) = spot.expect("every frame is walled in");

    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        entrance,
        x,
        y,
        facing,
    });
}

/// Faces the party down a direction they can actually walk, so a movement
/// assertion isn't silently testing a wall.
///
/// The way up is deliberately not "a direction they can walk". A caller
/// wants the party to keep exploring this frame, and stepping onto `LinkUp`
/// ends the trip — which barely came up while the entry was the maze
/// lattice's corner and the party wandered away from it, and comes up
/// constantly now that a `Rooms` or `Chambers` entry sits in the middle of
/// an open space the party circles.
fn face_an_open_way(game: &mut Game) -> Dir {
    for _ in 0..4 {
        let Locale::Stack { x, y, facing, .. } = locale(game) else {
            panic!("not underground");
        };
        let (dx, dy) = facing.delta();
        let ahead = cell_at(game, x + dx, y + dy);
        if ahead.walkable() && ahead != CellKind::LinkUp {
            return facing;
        }
        game.turn_right();
    }
    panic!("no way on that doesn't leave the frame");
}

#[test]
fn entering_the_stack_pins_the_players_surface_position_to_the_entrance() {
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
    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };

    game.step_forward();

    let (dx, dy) = facing.delta();
    let Locale::Stack {
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
    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };

    step_forward_clear(&mut game);
    step_back_clear(&mut game);

    let Locale::Stack {
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

    face_a_wall(&mut game);
    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    game.step_forward();
    let Locale::Stack { x: nx, y: ny, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!((nx, ny), (x, y), "walked into solid rock");
}

#[test]
fn turning_left_and_right_change_the_facing_and_nothing_else() {
    let mut game = game();
    descend(&mut game);
    let Locale::Stack { x, y, facing, .. } = locale(&game) else {
        unreachable!()
    };

    game.turn_left();
    let Locale::Stack {
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
    let Locale::Stack { facing: back, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!(back, facing);
}

#[test]
fn the_party_arrives_on_the_link_up_facing_north() {
    let mut game = game();
    descend(&mut game);
    let Locale::Stack {
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
    assert_eq!(cell_at(&game, x, y), CellKind::LinkUp);
}

#[test]
fn taking_the_link_down_increments_the_depth_and_regenerates_the_frame() {
    let mut game = game();
    descend(&mut game);
    let first = frame_cells(&game);

    stand_on_link_down(&mut game);
    game.descend();

    let Locale::Stack { depth, .. } = locale(&game) else {
        panic!("descending should leave us underground")
    };
    assert_eq!(depth, 2);
    assert_ne!(first, frame_cells(&game), "depth 2 should be its own frame");
}

/// The descend log line is player-facing narration of "the Stack" vocabulary
/// — it must call a level a frame, not the pre-rename wording.
#[test]
fn descending_names_the_frame_in_the_log() {
    let mut game = game();
    descend(&mut game);
    stand_on_link_down(&mut game);
    game.descend();
    assert!(
        logged(&game, "frame 2 of"),
        "the descend log line should name the frame reached"
    );
}

#[test]
fn climbing_out_of_depth_one_returns_to_the_surface_with_movement_working() {
    let mut game = game();
    let entrance = descend(&mut game);

    game.ascend(); // the party arrives standing on the link up

    assert!(!game.is_underground());
    assert_eq!(locale(&game), Locale::Surface);
    assert!(
        game.world.resource::<CurrentStack>().0.is_none(),
        "surfacing should drop the frame"
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
fn descending_then_climbing_back_lands_on_that_frames_link_down() {
    let mut game = game();
    descend(&mut game);

    let down = stand_on_link_down(&mut game);
    game.descend(); // to depth 2, arriving on its link up
    game.ascend(); // back to depth 1

    let Locale::Stack { depth, x, y, .. } = locale(&game) else {
        panic!("climbing from depth 2 should stay underground")
    };
    assert_eq!(depth, 1);
    assert_eq!(
        (x, y),
        down,
        "climbing must land on the link you went down, not the frame's entry"
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
        "walking the Stack must still pass surface time"
    );
}

#[test]
fn shoving_at_a_wall_still_passes_time() {
    let mut game = game();
    descend(&mut game);
    // Face a wall.
    for _ in 0..4 {
        let Locale::Stack { x, y, facing, .. } = locale(&game) else {
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
        panic!("a Home should not go up inside the Stack");
    };
    assert!(
        reason.contains("open grid"),
        "the refusal should say why, got: {reason}"
    );
}

/// Deploys a Home one tile east, puts the party underground through a link
/// on their own tile, and returns the Home and where it stands.
fn home_then_descend(game: &mut Game) -> (Entity, Position) {
    game.place_structure("home", 1, 0).unwrap();
    let home = game
        .view_entities(5, 5)
        .into_iter()
        .find(|e| e.is_home)
        .expect("the Home just deployed");
    let at = *game.world.get::<Position>(home.entity).unwrap();
    descend(game);
    (home.entity, at)
}

fn stock_for_symlink(game: &mut Game, target: Entity) {
    let cost = game.symlink_cost(target).expect("Home has a symlink");
    let player = game.player_entity();
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    for (item, qty) in &cost {
        inv.add(item.clone(), *qty);
    }
}

/// The symlink is the one guarded action that gets to *change* locale rather
/// than be refused by it: it pulls the party out of the stack and then
/// teleports them. `Position` is never written while `Locale::Stack` is
/// live, which is the thing `require_surface` exists to prevent.
#[test]
fn a_symlink_used_underground_surfaces_the_party_and_teleports_them() {
    let mut game = game();
    let (home, at) = home_then_descend(&mut game);
    stock_for_symlink(&mut game, home);

    game.use_symlink(home).expect("a symlink should reach home");

    assert!(
        !game.is_underground(),
        "the symlink should have surfaced us"
    );
    assert!(
        game.stack_view().is_none(),
        "the frame should have been dropped, not left loaded"
    );
    let player = game.player_entity();
    assert_eq!(
        *game.world.get::<Position>(player).unwrap(),
        at,
        "the party should be standing on the Home they linked to"
    );
}

/// The surfacing happens after every check, so a symlink that cannot be paid
/// for is not a one-way trip out of the Stack.
#[test]
fn a_symlink_that_cannot_be_paid_for_leaves_the_party_underground() {
    let mut game = game();
    let (home, _) = home_then_descend(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
    let before = locale(&game);

    assert!(game.use_symlink(home).is_err());

    assert!(
        game.is_underground(),
        "a refused symlink surfaced the party"
    );
    assert_eq!(
        before,
        locale(&game),
        "a refused symlink moved the party underground"
    );
}

/// The maps of every frame walked are keyed by `(link tile, depth)`, so
/// leaving by symlink costs the descent but not the mapping.
#[test]
fn a_symlink_out_keeps_the_maps_of_the_frames_already_walked() {
    let mut game = game();
    let (home, _) = home_then_descend(&mut game);
    let link = match locale(&game) {
        Locale::Stack { entrance, .. } => entrance,
        Locale::Surface => unreachable!("just descended"),
    };
    walk_corridors(&mut game, 12);
    let before = map(&game).explored;
    stock_for_symlink(&mut game, home);

    game.use_symlink(home).unwrap();
    game.enter_stack(link.0, link.1);

    assert!(
        (map(&game).explored - before).abs() < f32::EPSILON,
        "walking back into the link handed back a blank map"
    );
}

/// A Home sits at the player's own tile before they descend, so its rest
/// radius still covers the pinned entrance `Position` once underground —
/// otherwise `nearby_rest_structure` finds nothing regardless of gate
/// ordering and the test cannot distinguish `require_surface` refusing the
/// rest from there being no rest structure to spend against at all.
#[test]
fn resting_is_refused_underground() {
    let mut game = game();
    spawn_rest_structure_at_player(&mut game);
    descend(&mut game);
    let player = game.player_entity();
    let outlets_before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::OUTLET));
    let before = game.current_tick();

    game.rest();

    assert_eq!(before, game.current_tick(), "rest should not have run");
    assert_eq!(
        outlets_before,
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        "a rest refused by the require_surface gate must spend nothing"
    );
}

#[test]
fn party_management_still_works_underground() {
    let mut game = game();
    let pet = spawn_tamed(&mut game, 20, 5);
    descend(&mut game);
    // Managing the roster four frames down is a thing the genre expects, so
    // it must not be swept up by the surface-only guard.
    assert!(game.owned_pets().iter().any(|p| p.entity == pet));
}

#[test]
fn the_view_is_none_on_the_surface_and_some_underground() {
    let mut game = game();
    assert!(game.stack_view().is_none());
    descend(&mut game);
    assert!(game.stack_view().is_some());
}

#[test]
fn the_view_cone_is_rotated_so_straight_ahead_is_always_the_middle_column() {
    let mut game = game();
    descend(&mut game);

    for _ in 0..4 {
        let Locale::Stack { x, y, facing, .. } = locale(&game) else {
            unreachable!()
        };
        let view = game.stack_view().unwrap();
        let (dx, dy) = facing.delta();

        // Row 0 is the cell the party stands in; row 1 middle is one step
        // ahead along the facing, whichever way that points.
        let ahead = cell_at(&game, x + dx, y + dy);
        let middle = crate::game::stack_view::STACK_VIEW_HALF_WIDTH;
        assert_eq!(
            view.cells[1][middle] == StackCellView::Rock,
            !ahead.walkable()
        );
        assert_eq!(view.facing, facing.label());

        game.turn_right();
    }
}

#[test]
fn the_view_reads_solid_rock_past_the_edge_of_the_frame() {
    let mut game = game();
    descend(&mut game);
    // Stood against a wall rather than at the entry: a Rooms frame arrives
    // the party in the middle of a room, with nothing solid in reach.
    face_a_wall(&mut game);
    let view = game.stack_view().unwrap();
    let middle = crate::game::stack_view::STACK_VIEW_HALF_WIDTH;
    assert_eq!(view.cells[1][middle], StackCellView::Rock);
    assert!(view.cells.len() >= 2);
}

#[test]
fn the_view_names_what_the_party_is_standing_on() {
    let mut game = game();
    descend(&mut game);
    let view = game.stack_view().unwrap();
    assert!(
        view.standing_on
            .as_deref()
            .is_some_and(|s| s.contains("surface")),
        "depth 1's entry should offer the way out, got {:?}",
        view.standing_on
    );
}

#[test]
fn a_new_zone_is_seeded_with_surface_links() {
    let game = game();
    let entrances = game
        .world
        .iter_entities()
        .filter(|e| e.contains::<SurfaceLink>())
        .count();
    assert_eq!(entrances, crate::tuning::STACK_LINKS_PER_ZONE);
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
/// base slab is stamped down *before* the new sector's links are placed
/// (see `enter_next_zone`). On a fresh run no platform exists yet, and a
/// player later stamping a Home over a link is their own doing — that
/// still works, and a Stack mouth inside your base is a fine place for one.
#[test]
fn breaching_with_a_base_never_opens_a_link_inside_the_platform() {
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
            "a link opened at ({x}, {y}), inside the one safe ground in the game"
        );
    }
}

/// A nest and a link on the same tile is a link that can never be
/// used: `move_player` checks `find_nest_at` before `find_surface_link_at`,
/// so walking onto it attacks the nest forever. Nests are placed first in
/// both `Game::new` and `enter_next_zone`, which leaves the placement
/// filter as the only thing that can keep the two apart — and the link
/// this eats may be the near one `STACK_NEAREST_LINK_TILES` exists
/// to guarantee.
#[test]
fn no_entrance_opens_on_top_of_a_nest() {
    // Where this seed puts its links, so a nest can be stood on one.
    let victim = entrance_tiles(&mut game())[0];

    // The same seed again, with that tile already occupied — placement
    // draws from a locally seeded stream, so it reaches for the same tiles
    // in the same order and this one is now taken.
    let mut game = game();
    let entrances: Vec<Entity> = game
        .world
        .query_filtered::<Entity, With<SurfaceLink>>()
        .iter(&game.world)
        .collect();
    for entrance in entrances {
        game.world.despawn(entrance);
    }
    game.world.spawn((
        Nest {
            species: "scrapper".to_string(),
            pending_respawns: Vec::new(),
        },
        Position {
            x: victim.0,
            y: victim.1,
        },
    ));
    game.spawn_surface_links(crate::tuning::STACK_LINKS_PER_ZONE);

    assert!(
        !entrance_tiles(&mut game).contains(&victim),
        "a link opened at {victim:?}, on top of a nest — walking onto it \
         attacks the nest instead of descending, forever"
    );
}

/// Not a rare collision: `STACK_NEAREST_LINK_TILES` puts a zone's first link
/// 5-8 tiles from where the player arrives and `MAX_BUILD_DISTANCE_FROM_HOME`
/// is 7, so a Home built near the arrival point swallows it on a large
/// fraction of seeds. A link under the base platform is unreachable anyway —
/// nothing can spawn on platform floor and the slab is the base's footprint —
/// so it goes the way of the hostiles and nests standing there.
#[test]
fn deploying_a_home_obliterates_a_link_under_the_platform() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let entrances: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<SurfaceLink>>();
        query.iter(&game.world).collect()
    };
    for entrance in entrances {
        game.world.despawn(entrance);
    }
    let swallowed = game
        .world
        .spawn((
            SurfaceLink,
            Position {
                x: ppos.x + 3,
                y: ppos.y,
            },
            Glyph {
                ch: '>',
                color: GlyphColor::Magenta,
            },
        ))
        .id();
    let spared = game
        .world
        .spawn((
            SurfaceLink,
            Position {
                x: ppos.x + 20,
                y: ppos.y,
            },
            Glyph {
                ch: '>',
                color: GlyphColor::Magenta,
            },
        ))
        .id();

    game.place_structure("home", 1, 0)
        .expect("a fresh game can afford its first Home");

    assert!(
        game.world.get::<Position>(swallowed).is_none(),
        "a link 2 tiles from the Home is under the slab and must be gone"
    );
    assert!(
        game.world.get::<Position>(spared).is_some(),
        "a link 19 tiles out is nowhere near the base and must survive"
    );
}

#[test]
fn a_structure_cannot_be_deployed_on_top_of_a_link() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Clear the way, then put a link right where the Home would go.
    game.world.spawn((
        SurfaceLink,
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
        panic!("a structure sharing a tile with a link makes the tile ambiguous to walk onto");
    };
    assert!(reason.contains("link"), "got: {reason}");
}

#[test]
fn walking_onto_an_entrance_descends_and_leaves_the_entrance_standing() {
    let mut game = game();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let target = (ppos.x + 1, ppos.y);
    game.world.spawn((
        SurfaceLink,
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
        game.find_surface_link_at(target.0, target.1).is_some(),
        "the entrance must survive being used"
    );
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), target);
}

#[test]
fn a_stack_position_survives_a_save_and_load_with_an_identical_frame() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    descend(&mut game);
    face_an_open_way(&mut game);
    game.step_forward();
    game.turn_right();
    let before = locale(&game);
    let cells_before: Vec<CellKind> = {
        let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };

    let path = std::env::temp_dir().join(format!(
        "feral_processes_stack_save_{}.bin",
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
        let level = loaded.world.resource::<CurrentStack>().0.as_ref().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    };
    assert_eq!(
        cells_before, cells_after,
        "the frame regenerates from the seed — a different one would strand the party in rock"
    );
}

fn map(game: &Game) -> FrameMapView {
    game.frame_map().expect("underground")
}

fn map_cell(view: &FrameMapView, x: i32, y: i32) -> FrameMapCell {
    view.cells[y as usize][x as usize]
}

/// The dev reveal is a *view*, not a state change: it draws the frame the
/// party has not walked without pretending they walked it. Tested through
/// the inner function rather than by setting the environment variable,
/// because `std::env::set_var` reaches every other test in the process and
/// this suite runs in parallel.
#[test]
fn the_dev_reveal_shows_a_cell_the_party_has_never_seen() {
    let mut game = game();
    descend(&mut game);

    let hidden = {
        let plain = game.frame_map_revealed(false).expect("underground");
        (0..plain.height)
            .flat_map(|y| (0..plain.width).map(move |x| (x, y)))
            .find(|&(x, y)| map_cell(&plain, x, y) == FrameMapCell::Unknown)
            .expect("a frame the party has only stepped into has unseen cells")
    };

    let revealed = game.frame_map_revealed(true).expect("underground");
    assert_ne!(
        map_cell(&revealed, hidden.0, hidden.1),
        FrameMapCell::Unknown,
        "the reveal left a cell dark"
    );
    assert!(
        revealed.revealed,
        "the view does not report that it is revealed"
    );
}

/// The heading still says how much of the frame has actually been walked.
/// A reveal that read 100% mapped would take away the one number worth
/// having while hunting for the wing you have not been down.
#[test]
fn the_dev_reveal_does_not_inflate_the_explored_figure() {
    let mut game = game();
    descend(&mut game);
    let plain = game.frame_map_revealed(false).expect("underground");
    let revealed = game.frame_map_revealed(true).expect("underground");
    assert_eq!(plain.explored, revealed.explored);
    assert!(
        plain.explored < 1.0,
        "a frame just entered is not fully walked"
    );
}

/// Off unless asked for: the shipped game must not ship with the map open.
#[test]
fn the_dev_reveal_is_off_by_default() {
    let mut game = game();
    descend(&mut game);
    assert!(!game.frame_map().unwrap().revealed);
}

#[test]
fn the_surface_has_no_map() {
    let game = game();
    assert!(game.frame_map().is_none());
}

#[test]
fn arriving_maps_what_the_party_can_see_and_nothing_else() {
    let mut game = game();
    descend(&mut game);
    let view = map(&game);

    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!(
        map_cell(&view, x, y),
        FrameMapCell::LinkUp,
        "the cell the party is standing on must be mapped"
    );
    assert!(
        view.cells
            .iter()
            .flatten()
            .any(|&c| c == FrameMapCell::Unknown),
        "standing on the entry should not reveal a 21x21 frame"
    );
    assert!(view.explored > 0.0 && view.explored < 1.0);
}

/// The map is filled from `view_cone`, the same walk the first-person view
/// is built from — so anything the view shows is mapped, and nothing else.
#[test]
fn the_map_records_exactly_what_the_first_person_view_showed() {
    let mut game = game();
    descend(&mut game);
    let facing = face_an_open_way(&mut game);
    game.step_forward();

    let view = game.stack_view().unwrap();
    let mapped = map(&game);
    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };

    // The cell straight ahead is in view, so it must be on the map.
    let (dx, dy) = facing.delta();
    let (ax, ay) = (x + dx, y + dy);
    if (0..mapped.width).contains(&ax) && (0..mapped.height).contains(&ay) {
        assert_ne!(
            map_cell(&mapped, ax, ay),
            FrameMapCell::Unknown,
            "a cell the view is drawing was left off the map"
        );
    }
    assert!(!view.cells.is_empty());
}

/// A door is the one cell that is both walkable and sight-blocking, so it is
/// the one cell the party can stand *inside* an occluder. Their own cell must
/// not stop the cone: letting it means stepping into a doorway blinds them to
/// the corridor they are standing in.
#[test]
fn standing_in_a_doorway_maps_the_corridor_beyond_it() {
    let mut game = game();
    descend(&mut game);
    let (door, heading) = stand_in_a_doorway(&mut game);

    // A full circle, so the party ends on the heading they were placed with
    // and `remember_view` runs for it — teleporting maps nothing by itself.
    for _ in 0..4 {
        game.turn_right();
    }

    // Two cells out, not one: facing across the corridor puts the cell
    // immediately ahead into row 0's lateral span, so it gets mapped even
    // when the cone is truncated and proves nothing.
    let (dx, dy) = heading.delta();
    assert_ne!(
        map_cell(&map(&game), door.0 + dx * 2, door.1 + dy * 2),
        FrameMapCell::Unknown,
        "the party is standing in the doorway looking down the corridor, and \
         it is not on their map past the cell they could touch"
    );
}

#[test]
fn turning_in_place_maps_the_new_heading() {
    let mut game = game();
    descend(&mut game);
    let before = map(&game).explored;

    for _ in 0..3 {
        game.turn_right();
    }
    assert!(
        map(&game).explored >= before,
        "turning to look down a new corridor should map it"
    );
}

#[test]
fn walking_a_frame_maps_more_of_it() {
    let mut game = game();
    descend(&mut game);
    let before = map(&game).explored;

    walk_corridors(&mut game, 40);

    assert!(
        map(&game).explored > before,
        "forty steps mapped nothing new"
    );
}

/// A frame regenerates from its spec, but what the player has *seen* of it
/// does not — losing that on load hands back a blank map of a walked frame.
#[test]
fn the_map_survives_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    walk_corridors(&mut game, 20);
    let before = map(&game);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_stack_map_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let after = map(&loaded);
    assert_eq!(before.cells, after.cells, "the walked map came back blank");
    assert!((before.explored - after.explored).abs() < f32::EPSILON);
}

/// Two links are two stacks, so they are two maps. Sharing one would
/// pre-reveal a frame the party has never set foot in.
///
/// Asserted on arrival rather than after a walk, and in both directions.
/// The old shape walked twenty steps and compared explored fractions, which
/// was a proxy for the claim and a lottery on what the walk hit — twenty
/// steps through the open layouts finds a Breakpoint that reveals the whole
/// frame, a Fault that drops the party a level, or a fight that reboots them
/// to the surface, none of which this test is about. Arriving reveals the
/// view cone, which is all the map either half needs.
#[test]
fn each_link_keeps_its_own_map() {
    let mut game = game();
    let tiles = entrance_tiles(&mut game);
    assert!(tiles.len() >= 2);

    game.enter_stack(tiles[0].0, tiles[0].1);
    let first = map(&game).cells.clone();
    game.ascend();

    game.enter_stack(tiles[1].0, tiles[1].1);
    assert_ne!(
        map(&game).cells,
        first,
        "the second link opened onto the first one's map"
    );
    game.ascend();

    game.enter_stack(tiles[0].0, tiles[0].1);
    assert_eq!(
        map(&game).cells,
        first,
        "the first link's map was lost to the second"
    );
}

#[test]
fn a_stack_fight_is_pinned_to_the_corridor_it_happened_in() {
    let mut game = game();
    descend(&mut game);
    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    game.remember_fight();

    let marks = map(&game).marks;
    assert!(
        marks.contains(&((x, y), FrameMapMark::Fight)),
        "a fight should leave a mark on the map"
    );
    assert_eq!(
        marks.last().map(|&(_, m)| m),
        Some(FrameMapMark::Party),
        "the party must be drawn last, or a fight marker hides them"
    );
}

/// Breaching does not despawn what a zone accumulated, so anything
/// zone-local has to be wiped by name — the trap `BuybackLedger` already
/// documents, and one a Stack map falls into just as readily.
#[test]
fn maps_do_not_ride_a_breach_into_the_next_zone() {
    let mut game = game();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    // Turning maps the entry's surroundings without leaving the cell, so the
    // climb back out below is from the link the party arrived on — a
    // breach can only be taken from the surface.
    for _ in 0..3 {
        game.turn_right();
    }
    game.ascend();
    assert!(!game.is_underground(), "the fixture must surface to breach");
    assert!(
        !game.world.resource::<StackMemory>().0.is_empty(),
        "the fixture should have mapped something to lose"
    );

    game.enter_next_zone();
    assert!(
        game.world.resource::<StackMemory>().0.is_empty(),
        "last sector's maps rode the breach through"
    );
}

/// Teleports the party to the mouth of a cache's dead end, facing it, and
/// returns the cache's cell. A single `step_forward` then walks onto it,
/// which is how a cache is actually reached — teleporting *onto* one would
/// test a state the game never produces, and `step_back` off a dead end is
/// only sometimes possible depending on which way the party happens to face.
///
/// Caches sit at dead ends, so walking to one honestly would mean solving
/// the maze first.
fn stand_before_a_cache(game: &mut Game) -> (i32, i32) {
    let level = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .clone();
    let cache = (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .find(|&(x, y)| level.cell(x, y) == CellKind::Cache)
        .expect("every frame should hide at least one cache");

    // A dead end has exactly one open neighbour; stand there looking in.
    let (facing, mouth) = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .find_map(|dir| {
            let (dx, dy) = dir.delta();
            // The neighbour is behind us, so we face the *opposite* way.
            let neighbour = (cache.0 + dx, cache.1 + dy);
            level
                .walkable(neighbour.0, neighbour.1)
                .then_some((dir.turn_left().turn_left(), neighbour))
        })
        .expect("a dead end must have one way in");

    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: mouth.0,
        y: mouth.1,
        facing,
        entrance,
    });
    cache
}

fn credits(game: &Game) -> u32 {
    let id = game.trade_currency();
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .iter()
        .find(|(i, _)| *i == id)
        .map(|&(_, qty)| qty)
        .unwrap_or(0)
}

#[test]
fn a_frame_hides_caches_in_its_dead_ends() {
    let mut game = game();
    descend(&mut game);
    let level = game.world.resource::<CurrentStack>().0.clone().unwrap();

    let caches: Vec<(i32, i32)> = (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .filter(|&(x, y)| level.cell(x, y) == CellKind::Cache)
        .collect();
    assert_eq!(caches.len(), crate::tuning::STACK_CACHES_PER_FRAME);
    for (x, y) in caches {
        let exits = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .iter()
            .filter(|(dx, dy)| level.walkable(x + dx, y + dy))
            .count();
        assert_eq!(exits, 1, "the cache at {x},{y} is not in a dead end");
    }
}

#[test]
fn walking_onto_a_cache_pays_out() {
    let mut game = game();
    descend(&mut game);
    let cache = stand_before_a_cache(&mut game);
    let before = credits(&game);

    game.step_forward();

    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!((x, y), cache, "the fixture should walk onto the cache");
    assert!(credits(&game) > before, "walking onto a cache paid nothing");
    assert!(logged(&game, "cache"));
}

#[test]
fn a_cache_pays_credits_and_never_the_breaching_currency() {
    let mut game = game();
    descend(&mut game);
    stand_before_a_cache(&mut game);
    let fragments_before = fragments(&game);

    game.step_forward();

    assert_eq!(
        fragments(&game),
        fragments_before,
        "a stack's progress toward the next zone is what the party fights the lair for, \
         not what they find in the walls on the way to it"
    );
}

/// Otherwise a cache is an infinite credit tap: step off, step back on.
#[test]
fn a_cache_only_pays_out_once() {
    let mut game = game();
    descend(&mut game);
    stand_before_a_cache(&mut game);
    game.step_forward();
    let after_first = credits(&game);

    for _ in 0..5 {
        if game.has_active_battle() {
            flee_until_clear(&mut game);
        }
        game.step_back();
        game.step_forward();
    }
    assert_eq!(
        credits(&game),
        after_first,
        "the cache refilled itself when the party stepped off and back on"
    );
}

#[test]
fn an_emptied_cache_stays_emptied_across_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    let cache = stand_before_a_cache(&mut game);
    game.step_forward();
    if game.has_active_battle() {
        flee_until_clear(&mut game);
    }
    let after_looting = credits(&game);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_cache_looted_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(credits(&loaded), after_looting);
    loaded.step_back();
    loaded.step_forward();
    assert_eq!(
        credits(&loaded),
        after_looting,
        "loading refilled a cache the party had already emptied"
    );
    assert_eq!(
        loaded.frame_map().unwrap().cells[cache.1 as usize][cache.0 as usize],
        FrameMapCell::Floor,
        "an emptied cache should stop being advertised on the map"
    );
}

/// The map answers "where is there still something", so an emptied cache
/// drops off it while an untouched one stays.
#[test]
fn the_map_marks_caches_the_party_has_seen_and_not_opened() {
    let mut game = game();
    descend(&mut game);
    let cache = stand_before_a_cache(&mut game);
    // Seen from the mouth of the dead end, not yet stepped on.
    game.turn_left();
    game.turn_right();
    let seen = map(&game);
    assert_eq!(map_cell(&seen, cache.0, cache.1), FrameMapCell::Cache);

    game.step_forward();
    assert_eq!(map_cell(&map(&game), cache.0, cache.1), FrameMapCell::Floor);
}

#[test]
fn a_deeper_cache_pays_better() {
    let payout_at = |depth: u32| {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.enter_stack(pos.x, pos.y);
        let Locale::Stack {
            x,
            y,
            facing,
            entrance,
            ..
        } = locale(&game)
        else {
            unreachable!()
        };
        game.world.insert_resource(Locale::Stack {
            depth,
            frames: 9,
            x,
            y,
            facing,
            entrance,
        });
        let before = credits(&game);
        stand_before_a_cache(&mut game);
        game.step_forward();
        credits(&game) - before
    };

    assert!(
        payout_at(4) > payout_at(1),
        "depth has to pay better than it costs, or the bottom of a stack has no draw"
    );
}

/// Walks the party to the bottom of the stack they are in and stands them
/// outside the sealed door guarding the lair, facing that door. Returns the
/// lair's cell.
///
/// Outside the *seal*, not beside the lair: a sealed door is walkable as far
/// as the frame is concerned (the generator has to see through it), so
/// standing on the lair's neighbour would put the party already past the
/// lock this is meant to exercise.
fn stand_before_the_lair(game: &mut Game) -> (i32, i32) {
    loop {
        let Locale::Stack { depth, frames, .. } = locale(game) else {
            unreachable!("not underground")
        };
        if depth >= frames {
            break;
        }
        stand_on_link_down(game);
        game.descend();
    }

    let level = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .clone();
    let lair = (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .find(|&(x, y)| level.cell(x, y) == CellKind::Lair)
        .expect("the bottom frame should hold a lair");

    let seal = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .map(|dir| {
            let (dx, dy) = dir.delta();
            (lair.0 + dx, lair.1 + dy)
        })
        .find(|&(x, y)| level.cell(x, y) == CellKind::SealedDoor)
        .expect("the lair must be sealed and reachable");

    let (facing, mouth) = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .find_map(|dir| {
            let (dx, dy) = dir.delta();
            let outside = (seal.0 + dx, seal.1 + dy);
            (outside != lair && level.walkable(outside.0, outside.1))
                .then_some((dir.turn_left().turn_left(), outside))
        })
        .expect("the seal must have a way up to it");

    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!()
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: mouth.0,
        y: mouth.1,
        facing,
        entrance,
    });
    lair
}

/// Enough Integrity to still be standing after however many parting volleys
/// `flee_until_clear` has to eat.
///
/// A flatlined party is ejected to the surface on Forgiving
/// (`difficulty::death_handling_system`), which would leave a test about
/// what a *jack-out* does asserting against a frame the party is no longer
/// in. Only the tests that flee need this; walking in and being hit once
/// does not risk it.
fn outlast_the_guardian(game: &mut Game) {
    let player = game.player_entity();
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.max_hp = 10_000;
    stats.hp = 10_000;
}

/// Shoves through the seal and walks into the lair, which is what rouses
/// whatever is in it.
fn walk_into_the_lair(game: &mut Game) -> (i32, i32) {
    let lair = stand_before_the_lair(game);
    step_forward_clear(game); // through the seal, shaking off any ambush
    game.step_forward(); // into the lair — this fight is the point
    lair
}

/// The bottom frame puts a lair where a frame with a way down puts its
/// link — the deepest room of the stack, and the only place its guardian
/// could sensibly be.
#[test]
fn only_the_bottom_frame_of_a_stack_holds_a_lair() {
    let mut game = game();
    descend(&mut game);

    let lairs = |game: &Game| {
        let level = game.world.resource::<CurrentStack>().0.clone().unwrap();
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| level.cell(x, y) == CellKind::Lair)
            .count()
    };

    let Locale::Stack { frames, .. } = locale(&game) else {
        unreachable!()
    };
    for _ in 1..frames {
        assert_eq!(lairs(&game), 0, "a frame with a way down held a lair");
        stand_on_link_down(&mut game);
        game.descend();
    }
    assert_eq!(lairs(&game), 1, "the bottom of the stack held no lair");
}

#[test]
fn walking_into_the_lair_starts_a_fight() {
    let mut game = game();
    descend(&mut game);
    let lair = walk_into_the_lair(&mut game);

    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!((x, y), lair, "the fixture should walk into the lair");
    assert!(
        game.has_active_battle(),
        "the deepest room of the stack was empty"
    );
}

/// Jacking out of the boss fight has to leave the boss there. Otherwise the
/// bottom of every stack is cleared by walking in and immediately leaving.
#[test]
fn fleeing_the_lair_leaves_it_held() {
    let mut game = game();
    descend(&mut game);
    let lair = walk_into_the_lair(&mut game);
    outlast_the_guardian(&mut game);
    flee_until_clear(&mut game);

    assert_eq!(
        map_cell(&map(&game), lair.0, lair.1),
        FrameMapCell::Lair,
        "fleeing cleared the lair"
    );

    // Step out and back in: it should rouse again.
    game.step_back();
    game.step_forward();
    assert!(
        game.has_active_battle(),
        "the guardian did not come back after being fled from"
    );
}

/// Shoving at a wall is not travel — the encounter roll is already built
/// that way. The lair has to make the same call: a party that jacked out
/// and misjudged which way it was facing would otherwise conjure a second,
/// full-HP guardian by walking into rock it is already standing next to.
#[test]
fn shoving_at_a_wall_in_a_held_lair_does_not_rouse_the_guardian_again() {
    let mut game = game();
    descend(&mut game);
    walk_into_the_lair(&mut game);
    outlast_the_guardian(&mut game);
    flee_until_clear(&mut game);

    // Turning is safe here: the lair is held rather than roused, so there
    // is no fight to be refused by.
    let mut faced_a_wall = false;
    for _ in 0..4 {
        let Locale::Stack { x, y, facing, .. } = locale(&game) else {
            unreachable!()
        };
        let (dx, dy) = facing.delta();
        if !cell_at(&game, x + dx, y + dy).walkable() {
            game.step_forward();
            faced_a_wall = true;
            break;
        }
        game.turn_right();
    }
    assert!(faced_a_wall, "the lair should border at least one wall");
    assert!(
        !game.has_active_battle(),
        "shoving at the lair's wall roused the guardian a second time"
    );
}

/// Beating the guardian has to clear the lair for good, or the bottom of a
/// stack is a treadmill rather than an ending.
#[test]
fn killing_the_guardian_clears_the_lair_for_good() {
    let mut game = game();
    descend(&mut game);
    // Overwhelming force, so this is testing what a win does rather than
    // whether a level-1 party can manage one.
    {
        let player = game.player_entity();
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100_000;
        stats.hp = 100_000;
        stats.atk = 100_000;
        stats.def = 100_000;
    }

    let lair = walk_into_the_lair(&mut game);
    assert!(game.has_active_battle(), "the lair should have roused");

    let mut rounds = 0;
    while game.has_active_battle() && rounds < 60 {
        player_attacks(&mut game);
        rounds += 1;
    }
    assert!(!game.has_active_battle(), "60 rounds and it is still up");

    assert_eq!(
        map_cell(&map(&game), lair.0, lair.1),
        FrameMapCell::Floor,
        "a cleared lair should stop being marked"
    );
    // Counting the lair's own line rather than asserting no battle at all:
    // an ordinary encounter can roll on any step, and would otherwise fail
    // this test at random.
    let before = roused_count(&game);
    game.step_back();
    game.step_forward();
    assert_eq!(
        roused_count(&game),
        before,
        "the guardian roused again after being killed"
    );
}

/// How many times the lair has announced itself — see `Game::rouse_lair`.
/// The line is a `MessageKind::Outcome`, so it survives the prune when a
/// battle ends rather than vanishing with the blow-by-blow.
fn roused_count(game: &Game) -> usize {
    game.message_log(200)
        .iter()
        .filter(|e| e.text.contains("very large"))
        .count()
}

#[test]
fn a_cleared_lair_stays_cleared_across_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    {
        let player = game.player_entity();
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100_000;
        stats.hp = 100_000;
        stats.atk = 100_000;
        stats.def = 100_000;
    }
    walk_into_the_lair(&mut game);
    let mut rounds = 0;
    while game.has_active_battle() && rounds < 60 {
        player_attacks(&mut game);
        rounds += 1;
    }
    assert!(!game.has_active_battle());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_lair_cleared_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let before = roused_count(&loaded);
    loaded.step_back();
    loaded.step_forward();
    assert_eq!(
        roused_count(&loaded),
        before,
        "loading refilled a lair the party had already cleared"
    );
}

/// Which program guards a stack is a property of the stack, seeded off its
/// frame spec — so leaving and coming back cannot reroll it into something
/// easier.
#[test]
fn the_same_stack_always_fields_the_same_guardian() {
    let name_of_guardian = || {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.enter_stack(pos.x, pos.y);
        walk_into_the_lair(&mut game);
        game.battle_view().map(|v| v.groups[0].species_name.clone())
    };
    let first = name_of_guardian();
    assert!(first.is_some(), "the lair should have fielded something");
    assert_eq!(first, name_of_guardian());
}

/// The party has to get through a sealed door to reach the guardian, so
/// `stand_before_the_lair` puts them on one. Returns its cell.
fn a_seal_before_the_lair(game: &Game, lair: (i32, i32)) -> (i32, i32) {
    let level = game.world.resource::<CurrentStack>().0.clone().unwrap();
    [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .map(|dir| {
            let (dx, dy) = dir.delta();
            (lair.0 + dx, lair.1 + dy)
        })
        .find(|&(x, y)| level.cell(x, y) == CellKind::SealedDoor)
        .expect("the lair should be sealed off")
}

#[test]
fn the_lair_is_sealed_off_behind_doors() {
    let mut game = game();
    descend(&mut game);
    let lair = stand_before_the_lair(&mut game);
    let level = game.world.resource::<CurrentStack>().0.clone().unwrap();

    let ways_in: Vec<CellKind> = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .map(|dir| {
            let (dx, dy) = dir.delta();
            level.cell(lair.0 + dx, lair.1 + dy)
        })
        .filter(|kind| kind.walkable())
        .collect();
    assert!(!ways_in.is_empty(), "the lair must be reachable at all");
    assert!(
        ways_in.iter().all(|&k| k == CellKind::SealedDoor),
        "an unsealed way into the lair: {ways_in:?}"
    );
}

/// The seal is a barrier to be shoved through, not a lock to be paid off:
/// nothing in the party's pack has any bearing on whether it gives.
#[test]
fn a_sealed_door_opens_for_a_party_carrying_nothing() {
    let mut game = game();
    descend(&mut game);
    let lair = stand_before_the_lair(&mut game);
    let seal = a_seal_before_the_lair(&game, lair);
    let carried = game
        .world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .clone();

    game.step_forward();

    let Locale::Stack { x, y, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!((x, y), seal, "the seal refused a party that had nothing");
    assert_eq!(
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .items,
        carried,
        "forcing a seal should cost the party no item at all"
    );
}

/// The record of an opened seal outlives the step that opened it, because
/// both Stack views read it: a seal that re-shut behind the party would
/// redraw the way back out as a wall.
#[test]
fn a_door_once_opened_stays_open() {
    let mut game = game();
    descend(&mut game);
    let lair = stand_before_the_lair(&mut game);
    let seal = a_seal_before_the_lair(&game, lair);
    step_forward_clear(&mut game);
    let pos = game.stack_pos().unwrap();
    assert!(game.seal_open(pos, seal), "the seal was not recorded open");

    step_back_clear(&mut game);
    let before = locale(&game);
    step_forward_clear(&mut game);

    assert_ne!(locale(&game), before, "the door sealed itself behind us");
    assert!(game.seal_open(pos, seal));
}

#[test]
fn an_opened_door_stays_open_across_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    stand_before_the_lair(&mut game);
    game.step_forward();
    game.step_back();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_door_open_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let before = loaded.locale();
    loaded.step_forward();
    assert_ne!(
        loaded.locale(),
        before,
        "loading re-sealed a door the party had already opened"
    );
}

/// A door you cannot see past is the whole reason plain doors exist. The
/// map is filled from the same cone, so this asserts on what it recorded.
#[test]
fn a_shut_door_stops_the_view() {
    let mut game = game();
    descend(&mut game);
    let lair = stand_before_the_lair(&mut game);
    // Standing at the mouth looking at a sealed door with the lair behind it.
    assert_eq!(
        map_cell(&map(&game), lair.0, lair.1),
        FrameMapCell::Unknown,
        "the view saw straight through a shut door"
    );
}

#[test]
fn a_frame_hangs_doorways_in_corridors_not_junctions() {
    let mut game = game();
    descend(&mut game);
    let level = game.world.resource::<CurrentStack>().0.clone().unwrap();

    let doors: Vec<(i32, i32)> = (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .filter(|&(x, y)| level.cell(x, y) == CellKind::Door)
        .collect();
    assert!(!doors.is_empty(), "the frame hung no doorways at all");
    assert!(doors.len() <= crate::tuning::STACK_DOORS_PER_FRAME);

    for (x, y) in doors {
        let vertical = level.walkable(x, y - 1) && level.walkable(x, y + 1);
        let horizontal = level.walkable(x + 1, y) && level.walkable(x - 1, y);
        assert!(
            vertical != horizontal,
            "the door at {x},{y} is in a junction, not a corridor"
        );
    }
}

/// True if any of the last `n` log lines contains `needle`.
fn logged(game: &Game, needle: &str) -> bool {
    game.message_log(12).iter().any(|e| e.text.contains(needle))
}

#[test]
fn a_link_further_from_the_arrival_point_runs_deeper() {
    let spawn = (100, -50);
    let near = crate::game::stack::frames_for((105, -50), spawn);
    let far = crate::game::stack::frames_for((138, -50), spawn);
    assert_eq!(
        near,
        crate::tuning::STACK_FRAMES_MIN,
        "a link inside the opening viewport should be the shallow one"
    );
    assert!(
        far > near,
        "walking {} tiles further bought no extra depth",
        138 - 105
    );
    assert!(far <= crate::tuning::STACK_FRAMES_MAX);
}

#[test]
fn stack_depth_is_capped_however_far_out_the_link_sits() {
    let frames = crate::game::stack::frames_for((10_000, 10_000), (0, 0));
    assert_eq!(frames, crate::tuning::STACK_FRAMES_MAX);
}

/// The bottom frame is generated with no link down at all, so a stack ends
/// rather than running forever — which is what it did before links had a
/// depth: `descend` incremented past any number you like.
#[test]
fn a_stack_bottoms_out_and_says_so() {
    let mut game = game();
    descend(&mut game);
    let Locale::Stack { frames, .. } = locale(&game) else {
        unreachable!()
    };

    for _ in 1..frames {
        stand_on_link_down(&mut game);
        game.descend();
    }

    let Locale::Stack { depth, .. } = locale(&game) else {
        panic!("still underground at the bottom")
    };
    assert_eq!(depth, frames, "should have walked the stack to its end");
    assert_eq!(
        game.world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .unwrap()
            .link_down,
        None,
        "the bottom frame laid a way down into nothing"
    );

    game.descend();
    let Locale::Stack { depth, .. } = locale(&game) else {
        unreachable!()
    };
    assert_eq!(depth, frames, "descending past the bottom moved the party");
    assert!(
        logged(&game, "bottoms out"),
        "the bottom of a stack should say so, not just refuse"
    );
}

/// Before the entrance tile went into the frame seed, every link in a
/// sector opened onto the same maze — three holes, one stack, and no
/// reason to walk to the far one.
#[test]
fn two_links_in_a_sector_open_onto_different_stacks() {
    let mut game = game();
    let tiles = entrance_tiles(&mut game);
    assert!(tiles.len() >= 2, "this seed should field several links");

    game.enter_stack(tiles[0].0, tiles[0].1);
    let first = frame_cells(&game);
    game.ascend();

    game.enter_stack(tiles[1].0, tiles[1].1);
    assert_ne!(first, frame_cells(&game), "two links carved the same maze");
}

#[test]
fn how_deep_a_stack_runs_survives_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let tiles = entrance_tiles(&mut game);
    let far = *tiles.last().unwrap();
    game.enter_stack(far.0, far.1);
    let before = locale(&game);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_stack_depth_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.locale(), before);
}

fn entrance_tiles(game: &mut Game) -> Vec<(i32, i32)> {
    let mut query = game.world.query_filtered::<&Position, With<SurfaceLink>>();
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
    assert!(loaded.stack_view().is_none());
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
    fresh.spawn_surface_links(crate::tuning::STACK_LINKS_PER_ZONE);
    let after: Vec<u32> = {
        let mut rng = fresh.world.resource_mut::<GameRng>();
        (0..8).map(|_| rng.0.random_range(0..1_000_000)).collect()
    };

    assert_eq!(
        before, after,
        "entrance placement drew from the shared stream and moved it"
    );
}

/// The same zone of the same world always opens onto the same links, so
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
/// player links existed at all. At the default zoom the map pane shows
/// roughly +/-16 by +/-9 tiles.
const OPENING_VIEW_HALF_W: i32 = 16;
const OPENING_VIEW_HALF_H: i32 = 9;

#[test]
fn every_seed_puts_one_link_inside_the_opening_view() {
    for seed in [16u32, 43, 77, 101, 2024, 7, 999, 31337] {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let origin = *game.world.get::<Position>(game.player_entity()).unwrap();
        let visible = entrance_tiles(&mut game).into_iter().any(|(x, y)| {
            (x - origin.x).abs() <= OPENING_VIEW_HALF_W
                && (y - origin.y).abs() <= OPENING_VIEW_HALF_H
        });
        assert!(
            visible,
            "seed {seed} starts the player with no link on screen and no way to know one exists"
        );
    }
}

#[test]
fn the_remaining_links_are_still_a_trip() {
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
        "if every link is underfoot there is nothing left to explore for"
    );
}

#[test]
fn arriving_in_a_zone_scans_for_links_and_says_where_the_nearest_is() {
    let game = game();
    let scan = game
        .message_log(50)
        .into_iter()
        .find(|e| e.text.contains("Deep scan"))
        .map(|e| e.text);
    let Some(scan) = scan else {
        panic!("arriving in a zone should report what the scan found");
    };
    assert!(scan.contains("links"), "got: {scan}");
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
            .any(|e| e.text.contains("Deep scan")),
        "a fresh sector needs its own scan, or the layer is invisible again"
    );
}

#[test]
fn a_bearing_names_the_direction_you_would_actually_walk() {
    use crate::game::stack::bearing;
    // North is -y, matching stack::Dir and the renderer.
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
    use crate::game::stack::bearing;
    // Mostly east with a slight northerly lean is still east — calling it
    // north-east would send the player off at an angle.
    assert_eq!(bearing(20, -3), "east");
    assert_eq!(bearing(3, -20), "north");
    assert_eq!(bearing(0, 0), "here");
}

/// Breaching does not despawn structures — the base travels — so anything
/// zone-local has to be wiped by name in `enter_next_zone`. Entrances are
/// zone-local: each opens onto a frame generated for its own sector.
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
        crate::tuning::STACK_LINKS_PER_ZONE,
        "the new sector should hold its own links and no more — old ones rode the breach along"
    );
    assert_ne!(before, after, "the new sector needs its own links");
}

/// A link on the arrival tile means starting the run standing on one; a
/// link one step away means the first movement key of the run drops the
/// player into the Stack they never chose to enter.
#[test]
fn no_link_opens_on_top_of_the_player_or_within_a_step_of_them() {
    for seed in 0u32..40 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let origin = *game.world.get::<Position>(game.player_entity()).unwrap();
        for (x, y) in entrance_tiles(&mut game) {
            let distance = (x - origin.x).abs().max((y - origin.y).abs());
            assert!(
                distance >= crate::tuning::STACK_MIN_LINK_TILES,
                "seed {seed}: link at ({x}, {y}) is {distance} tiles from the player"
            );
        }
    }
}

/// Walks until an encounter fires, or gives up. Deterministic: the seeded
/// `GameRng` decides, and the walk itself is a fixed pattern.
fn walk_until_a_fight(game: &mut Game, steps: usize) -> bool {
    for i in 0..steps {
        if game.has_active_battle() {
            return true;
        }
        game.step_forward();
        if i % 3 == 0 {
            game.turn_right();
        }
    }
    game.has_active_battle()
}

#[test]
fn walking_the_stack_eventually_draws_an_encounter() {
    let mut game = game();
    descend(&mut game);
    assert!(
        walk_until_a_fight(&mut game, 400),
        "400 steps of corridor drew no fights at all"
    );
}

#[test]
fn shoving_at_a_wall_cannot_draw_an_encounter() {
    let mut game = game();
    descend(&mut game);
    // Face a wall and grind at it. A blocked step is not travel, so it must
    // never roll for a fight, however many times it is repeated.
    face_a_wall(&mut game);
    for _ in 0..500 {
        game.step_forward();
        assert!(
            !game.has_active_battle(),
            "walking into solid rock started a fight"
        );
    }
}

#[test]
fn a_stack_pack_is_drawn_from_the_biome_the_link_opens_in() {
    let mut game = game();
    let entrance = descend(&mut game);
    let biome = game
        .world
        .resource_mut::<WorldMap>()
        .tile(entrance.0, entrance.1)
        .biome;
    assert!(walk_until_a_fight(&mut game, 400), "no fight to inspect");

    let species: Vec<SpeciesId> = {
        let mut query = game.world.query_filtered::<&Creature, With<StackSpawn>>();
        query.iter(&game.world).map(|c| c.species.clone()).collect()
    };
    assert!(!species.is_empty(), "the pack should be tagged StackSpawn");
    for id in species {
        let def = game
            .species_defs()
            .into_iter()
            .find(|s| s.id == id)
            .expect("a spawned species is in the db");
        assert!(
            def.habitats.contains(&biome),
            "{id} does not live in {biome:?}, the biome the link opens in"
        );
    }
}

#[test]
fn deeper_frames_field_tougher_programs() {
    // Same species, same link, same everything but depth.
    let power_at = |depth: u32| {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.enter_stack(pos.x, pos.y);
        if depth > 1 {
            let Locale::Stack {
                x,
                y,
                facing,
                entrance,
                ..
            } = locale(&game)
            else {
                unreachable!()
            };
            game.world.insert_resource(Locale::Stack {
                depth,
                // Deep enough that this test's depths are all above the
                // bottom; it is measuring the stat curve, not the stack.
                frames: 9,
                x,
                y,
                facing,
                entrance,
            });
        }
        // Through the pack path a Stack encounter actually uses: depth
        // is carried into the spawn as an argument, not read back off the
        // locale, so this is the scaling the game applies rather than a
        // proxy for it. `is_boss` only to pin the group at one member.
        let esc = game.stack_escalation(game.stack_pos().map_or(1, |p| p.depth));
        let pack = game.spawn_pack("scrapper", true, pos.x, pos.y, esc);
        game.world.get::<Stats>(pack[0]).unwrap().power()
    };

    let shallow = power_at(1);
    let deep = power_at(4);
    assert!(
        deep > shallow,
        "depth 4 fielded {deep} power against depth 1's {shallow} — descending must cost something"
    );
}

#[test]
fn the_surface_is_untouched_by_the_depth_multiplier() {
    let mut game = game();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!(
        game.stack_depth_multiplier(),
        1.0,
        "the multiplier must be inert above ground"
    );
    let wild = game.spawn_wild_creature("scrapper", pos.x, pos.y).unwrap();
    assert!(game.world.get::<Stats>(wild).unwrap().hp > 0);
}

/// The surface keeps running while the party is underground — that is the
/// point of pinning `Position` to the link — so `tick` goes on rolling
/// ambient spawns and nest respawns the whole way down. Those are surface
/// programs standing on surface tiles, untagged and never swept by
/// `end_battle`, and they are still there when the party climbs out. Depth
/// must not reach them: it is the property of a Stack encounter, not of
/// the clock.
#[test]
fn a_surface_spawn_is_not_scaled_by_how_deep_the_party_is() {
    let power_at_depth = |depth: Option<u32>| {
        let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        if let Some(depth) = depth {
            // Neither entering nor rewriting the locale draws from
            // `GameRng` — frames carve from their own stream — so both
            // arms of this reach the spawn with the same rolls queued up.
            game.enter_stack(pos.x, pos.y);
            let Locale::Stack {
                x,
                y,
                facing,
                entrance,
                ..
            } = locale(&game)
            else {
                unreachable!()
            };
            game.world.insert_resource(Locale::Stack {
                depth,
                frames: 9,
                x,
                y,
                facing,
                entrance,
            });
        }
        let wild = game.spawn_wild_creature("scrapper", pos.x, pos.y).unwrap();
        game.world.get::<Stats>(wild).unwrap().power()
    };

    assert_eq!(
        power_at_depth(Some(5)),
        power_at_depth(None),
        "a program spawned on the surface was scaled by a stack the party \
         happened to be standing in"
    );
}

/// A pack conjured for a Stack fight has no business outliving it: it
/// stands at surface coordinates around the link mouth, and would be
/// waiting there when the party climbs out.
#[test]
fn a_stack_pack_that_survives_a_jack_out_does_not_linger_on_the_surface() {
    let mut game = game();
    descend(&mut game);
    assert!(walk_until_a_fight(&mut game, 400), "no fight to flee");

    let before = {
        let mut query = game.world.query_filtered::<Entity, With<StackSpawn>>();
        query.iter(&game.world).count()
    };
    assert!(before > 0);

    flee_until_clear(&mut game);

    let after = {
        let mut query = game.world.query_filtered::<Entity, With<StackSpawn>>();
        query.iter(&game.world).count()
    };
    assert_eq!(after, 0, "{before} Stack programs outlived the fight");
}

#[test]
fn an_encounter_underground_leaves_the_players_surface_position_alone() {
    let mut game = game();
    let entrance = descend(&mut game);
    assert!(walk_until_a_fight(&mut game, 400), "no fight");
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), entrance);
}

// ---- Trace ------------------------------------------------------------

fn trace(game: &Game) -> u32 {
    game.world.resource::<crate::resources::Trace>().0
}

fn set_trace(game: &mut Game, n: u32) {
    game.world.insert_resource(crate::resources::Trace(n));
}

/// Rewrites the depth of the live `Locale::Stack`, the way `set_trace`
/// rewrites the Trace resource. A test about the depth curve wants a stated
/// depth, not a walk down to it through however many frames the seed's stack
/// happens to run — `frames` is widened to match so the position stays a
/// legal one.
fn set_depth(game: &mut Game, depth: u32) {
    let Locale::Stack {
        frames,
        x,
        y,
        facing,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames: frames.max(depth),
        x,
        y,
        facing,
        entrance,
    });
}

/// The reason Trace is a resource and not a field on the `Locale::Stack`
/// variant. `descend_to` and `ascend_to` each *construct* a fresh variant
/// rather than mutating the live one, so a field there is silently zeroed on
/// every frame change — precisely when Trace is supposed to be accumulating.
#[test]
fn trace_survives_descending_and_ascending() {
    let mut game = game();
    descend(&mut game);
    set_trace(&mut game, 50);

    stand_on_link_down(&mut game);
    game.descend();
    assert_eq!(trace(&game), 50, "descending a frame must not shed Trace");

    game.ascend();
    assert_eq!(trace(&game), 50, "climbing a frame must not shed Trace");
}

#[test]
fn surfacing_clears_trace() {
    let mut game = game();
    descend(&mut game);
    set_trace(&mut game, 50);

    game.ascend(); // from depth 1 this leaves the Stack entirely

    assert_eq!(locale(&game), Locale::Surface);
    assert_eq!(trace(&game), 0, "the Stack stops caring once you are out");
}

/// The other way out. CLAUDE.md records `use_symlink` as going *through*
/// `clear_stack` rather than around it, and this is the assertion that keeps
/// it true — a second exit that skipped the reset would leave Trace live on
/// the surface, where nothing can ever clear it again.
#[test]
fn a_symlink_out_of_the_stack_clears_trace() {
    let mut game = game();
    let (home, _) = home_then_descend(&mut game);
    stock_for_symlink(&mut game, home);
    set_trace(&mut game, 50);

    game.use_symlink(home).expect("a symlink should reach home");

    assert!(!game.is_underground());
    assert_eq!(trace(&game), 0);
}

#[test]
fn trace_survives_a_save_and_load_mid_dive() {
    let assets = test_assets_dir();
    let mut game = Game::new(16, DifficultyMode::Forgiving, &assets).unwrap();
    descend(&mut game);
    set_trace(&mut game, 77);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_trace_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(
        trace(&loaded),
        77,
        "without persistence, saving mid-dive is a free Trace reset"
    );
}

/// Each threshold constant reads as "from", so a value sitting exactly on one
/// belongs to the band above it.
#[test]
fn band_thresholds_are_half_open() {
    use crate::resources::TraceBand::{self, *};
    use crate::tuning::{TRACE_HUNTED, TRACE_NOTICED, TRACE_TRACED};

    assert_eq!(TraceBand::from_trace(0), Quiet);
    assert_eq!(TraceBand::from_trace(TRACE_NOTICED - 1), Quiet);
    assert_eq!(TraceBand::from_trace(TRACE_NOTICED), Noticed);
    assert_eq!(TraceBand::from_trace(TRACE_TRACED - 1), Noticed);
    assert_eq!(TraceBand::from_trace(TRACE_TRACED), Traced);
    assert_eq!(TraceBand::from_trace(TRACE_HUNTED - 1), Traced);
    assert_eq!(TraceBand::from_trace(TRACE_HUNTED), Hunted);
    assert_eq!(TraceBand::from_trace(u32::MAX), Hunted);
}

/// Trace pays for *taking*, so the three things a stack can be robbed of
/// each raise it, and walking does not.
#[test]
fn cracking_a_cache_raises_trace() {
    use crate::tuning::TRACE_PER_CACHE;
    let mut game = game();
    descend(&mut game);
    stand_before_a_cache(&mut game);

    game.step_forward();
    assert_eq!(trace(&game), TRACE_PER_CACHE);

    // The cache is spent, so stepping off and back on must not charge again.
    game.step_back();
    game.step_forward();
    assert_eq!(
        trace(&game),
        TRACE_PER_CACHE,
        "an emptied cache should not keep paying Trace"
    );
}

#[test]
fn forcing_a_seal_raises_trace() {
    use crate::tuning::TRACE_PER_SEAL;
    let mut game = game();
    descend(&mut game);
    stand_before_the_lair(&mut game);

    game.step_forward();
    assert_eq!(trace(&game), TRACE_PER_SEAL);

    // A door already standing open makes no second noise.
    game.step_back();
    game.step_forward();
    assert_eq!(trace(&game), TRACE_PER_SEAL, "the seal was already forced");
}

#[test]
fn killing_a_hostile_raises_trace() {
    use crate::tuning::TRACE_PER_KILL;
    let mut game = game();
    descend(&mut game);
    let wild = spawn_wild_on_player_tile(&mut game);

    game.award_loot(wild);

    assert_eq!(trace(&game), TRACE_PER_KILL);
}

/// `award_loot` fires for every kill in the game, the overwhelming majority
/// of them on the surface. The guard lives inside `raise_trace` so there is
/// one of it rather than one per hook.
#[test]
fn a_surface_kill_raises_no_trace() {
    let mut game = game();
    let wild = spawn_wild_on_player_tile(&mut game);

    game.award_loot(wild);

    assert!(!game.is_underground());
    assert_eq!(trace(&game), 0);
}

/// The load-bearing choice of the whole phase: a meter driven by time or
/// distance would tax exploration and map-making, rewarding the beeline and
/// punishing the careful player.
#[test]
fn a_plain_step_raises_no_trace() {
    let mut game = game();
    descend(&mut game);

    for _ in 0..12 {
        game.step_forward();
        game.turn_left();
    }

    assert_eq!(trace(&game), 0, "walking must be free");
}

/// Escalating ambushes with no visible cause are experienced as bad luck
/// rather than as consequence, so every band crossing announces itself —
/// and as `Outcome`, which `retain_outcomes_since_battle` keeps. A kill-driven
/// crossing is logged during a battle teardown, where a plain `Info` line
/// would be pruned before the player ever saw it.
#[test]
fn crossing_a_band_logs_an_outcome_line() {
    use crate::tuning::{TRACE_NOTICED, TRACE_PER_CACHE};
    let mut game = game();
    descend(&mut game);
    set_trace(&mut game, TRACE_NOTICED - TRACE_PER_CACHE);
    stand_before_a_cache(&mut game);

    game.step_forward();

    assert_eq!(trace(&game), TRACE_NOTICED);
    assert!(
        game.message_log(12)
            .iter()
            .any(|e| e.kind == MessageKind::Outcome && e.text.contains("turns to look at you")),
        "crossing into Noticed should announce itself as an Outcome"
    );
}

#[test]
fn staying_inside_a_band_logs_nothing() {
    let mut game = game();
    descend(&mut game);
    stand_before_a_cache(&mut game);

    game.step_forward();

    assert!(
        !logged(&game, "turns to look at you"),
        "a rise that crosses no threshold should be silent"
    );
}

/// The measurement the whole Trace tuning table rests on.
///
/// A frame's kill-to-cache ratio is what makes Trace a greed meter rather
/// than a combat meter: `STACK_ENCOUNTER_CHANCE` at 0.08 per step over a
/// ~300-step exhaustive crawl draws roughly 24 fights against these 3
/// caches, which is why `TRACE_PER_KILL` is a fifth of `TRACE_PER_CACHE`
/// and not comparable to it.
///
/// Left unasserted, a later change to frame size or cache count moves that
/// ratio and silently turns the meter into something else, with the whole
/// suite still green.
///
/// The band was 190-220, measured when the maze was the only carver. Three
/// layouts widen it to the 150-230 they share — see
/// `stack::tests::every_layout_fills_the_frame_to_about_the_same_extent`,
/// which holds the same numbers from the generator's side. Note what the
/// widening does *not* say: cell count is a proxy for step count, and a
/// looping layout is crossed in fewer steps than a maze of the same size,
/// so the fights-per-cache ratio moves somewhat however tightly this is
/// pinned. This catches a carver that halves the frame; it cannot catch a
/// carver that merely makes it quicker to walk.
///
/// Ranges rather than equalities because the generator
/// legitimately varies per depth.
#[test]
fn a_frames_shape_still_matches_what_trace_was_tuned_against() {
    for depth in 1..=4u32 {
        let spec = crate::stack::FrameSpec {
            world_seed: 12345,
            entrance: (30, 30),
            depth,
            frames: 4,
        };
        let frame = crate::stack::generate(spec);
        let cells = || (0..frame.height).flat_map(|y| (0..frame.width).map(move |x| (x, y)));

        let walkable = cells().filter(|&(x, y)| frame.walkable(x, y)).count();
        let caches = cells()
            .filter(|&(x, y)| frame.cell(x, y) == CellKind::Cache)
            .count();
        let seals = cells()
            .filter(|&(x, y)| frame.cell(x, y) == CellKind::SealedDoor)
            .count();

        assert!(
            (150..=230).contains(&walkable),
            "depth {depth}: {walkable} walkable cells, outside the 150-230 \
             the encounter-to-cache ratio was measured against"
        );
        assert!(
            (2..=3).contains(&caches),
            "depth {depth}: {caches} caches, outside the 2-3 TRACE_PER_CACHE assumes"
        );
        if depth < 4 {
            assert_eq!(seals, 0, "depth {depth}: only the bottom frame is sealed");
        } else {
            assert!(
                seals > 0,
                "the bottom frame walls its lair off behind seals"
            );
        }
    }
}

#[test]
fn trace_scales_the_encounter_roll() {
    use crate::tuning::{STACK_ENCOUNTER_CHANCE, TRACE_HUNTED, TRACE_NOTICED};
    let mut game = game();
    descend(&mut game);

    assert_eq!(game.trace_encounter_mult(), 1.0, "Quiet is the baseline");

    set_trace(&mut game, TRACE_NOTICED);
    assert!(game.trace_encounter_mult() > 1.0);

    set_trace(&mut game, TRACE_HUNTED);
    let hunted = STACK_ENCOUNTER_CHANCE * game.trace_encounter_mult();
    assert!(
        (hunted - 0.16).abs() < 1e-9,
        "Hunted should double the 0.08 base, got {hunted}"
    );
}

/// Folded into `stack_depth_multiplier` rather than applied at the ambush
/// alone, so the lair guardian inherits it too — a party that looted its way
/// to Hunted meets a harder boss, having chosen to.
#[test]
fn trace_scales_enemy_stats_and_reaches_the_lair_through_depth() {
    use crate::tuning::{STACK_DEPTH_STAT_GROWTH, TRACE_HUNTED};
    let mut game = game();
    descend(&mut game);
    stand_on_link_down(&mut game);
    game.descend(); // depth 2

    let quiet = game.stack_depth_multiplier();
    assert!((quiet - STACK_DEPTH_STAT_GROWTH.powi(1)).abs() < 1e-5);

    set_trace(&mut game, TRACE_HUNTED);
    let hunted = game.stack_depth_multiplier();
    assert!(
        (hunted - STACK_DEPTH_STAT_GROWTH.powi(1) * 1.45).abs() < 1e-5,
        "Hunted should compound with depth, got {hunted}"
    );
}

/// Trace pushes a pack toward its zone's ceiling faster. It must never raise
/// that ceiling: `zone_group_cap` is a balance bound on how big any fight in
/// a zone can get, and a meter the player runs up themselves should not
/// vault it. The zone-1 case is why the lever is inert there.
#[test]
fn trace_reaches_the_group_ceiling_faster_but_never_past_it() {
    use crate::game::spawning::trace_group_ceiling;

    assert_eq!(trace_group_ceiling(1, 1, 9), 1, "Quiet changes nothing");
    assert_eq!(
        trace_group_ceiling(2, 3, 9),
        6,
        "Hunted triples a small pack"
    );
    assert_eq!(
        trace_group_ceiling(4, 3, 9),
        9,
        "the zone cap still bounds it"
    );
    assert_eq!(
        trace_group_ceiling(1, 3, 1),
        1,
        "zone 1 pins every group to one member, whatever Trace says"
    );
}

/// The leak this phase was most at risk of. `spawn_pack`'s doc records the
/// same mistake being made once already with `depth_mult`: ambient spawns
/// and nest respawns keep rolling on every `tick` while the party is
/// underground, so a scaling factor read off a resource inside the spawn
/// scaled those too, leaving oversized packs waiting at the link mouth for
/// the climb out. Group scaling is a parameter for exactly this reason.
#[test]
fn a_surface_spawn_is_unscaled_while_the_party_is_hunted() {
    use crate::tuning::TRACE_HUNTED;

    fn surface_pack_size(trace_value: u32) -> usize {
        let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.insert_resource(ZoneLevel(3));
        descend(&mut game);
        set_trace(&mut game, trace_value);

        let (x, y) = multi_group_ground(&mut game);
        let before = game.world.query::<&Creature>().iter(&game.world).count();
        game.try_spawn_habitat_creature(x, y);
        game.world.query::<&Creature>().iter(&game.world).count() - before
    }

    assert_eq!(
        surface_pack_size(TRACE_HUNTED),
        surface_pack_size(0),
        "Trace must not reach a spawn happening on the surface"
    );
}

/// `maybe_stack_encounter` refuses a boss with its own stated reason — a
/// fight you never saw coming should not also be the hardest fight
/// available. Escalation was designed *around* that rule rather than
/// through it: the phase-2 sketch called for Hunted to open the boss pool,
/// and it was cut, because reversing a decision that carries its own
/// reasoning needs a better argument than wanting a spike.
#[test]
fn a_hunted_ambush_is_still_never_a_boss() {
    use crate::tuning::TRACE_HUNTED;
    let mut game = game();
    let entrance = descend(&mut game);
    set_trace(&mut game, TRACE_HUNTED);

    let biome = game
        .world
        .resource_mut::<WorldMap>()
        .tile(entrance.0, entrance.1)
        .biome;
    assert!(
        !game
            .world
            .resource::<SpeciesDb>()
            .boss_habitat_matches(biome)
            .is_empty(),
        "this test is only meaningful where a boss pool exists to be drawn from"
    );

    assert!(walk_until_a_fight(&mut game, 400), "no fight to inspect");
    // Scoped to `StackSpawn`: surface ambient spawns keep rolling while the
    // party is underground (see `spawn_pack`'s doc), and those *may* roll a
    // boss — an unscoped query is asserting about the surface, not about
    // the ambush this test is named for.
    let bosses: Vec<String> = game
        .world
        .query::<(&Creature, &Hostile, &StackSpawn)>()
        .iter(&game.world)
        .map(|(c, _, _)| c.species.clone())
        .filter(|id| {
            game.world
                .resource::<SpeciesDb>()
                .get(id)
                .is_some_and(|s| s.is_boss)
        })
        .collect();
    assert!(
        bosses.is_empty(),
        "Hunted drew a boss into an ambush: {bosses:?}"
    );
}

/// The band is the only form the player ever sees Trace in — a threat
/// readout rather than a progress bar, since a visible integer invites
/// playing to the threshold instead of to the risk.
#[test]
fn the_stack_view_reports_the_trace_band() {
    use crate::tuning::TRACE_HUNTED;
    let mut game = game();
    descend(&mut game);

    assert_eq!(game.stack_view().unwrap().trace, "Quiet");

    set_trace(&mut game, TRACE_HUNTED);
    assert_eq!(game.stack_view().unwrap().trace, "Hunted");
}

/// The group lever, asserted where it actually lands.
///
/// `trace_group_ceiling` being right is not enough: `spawn_pack` decides how
/// many bodies exist, but `group_pack` decides how many of them fight, and
/// it caps each species group independently. Scaling only the spawn made
/// `TRACE_GROUP_MULT` a no-op in every zone — the surplus was capped back
/// out at battle assembly and then swept by `end_battle`'s `StackSpawn`
/// cleanup, so a Hunted ambush fielded exactly as many programs as a Quiet
/// one while every unit test still passed.
#[test]
fn a_hunted_ambush_fields_more_of_the_pack_than_a_quiet_one() {
    use crate::tuning::TRACE_HUNTED;
    let mut game = game();
    game.world.insert_resource(ZoneLevel(3));
    descend(&mut game);

    let pack: Vec<Entity> = (0..6)
        .map(|_| spawn_wild_on_player_tile(&mut game))
        .collect();
    let fielded = |game: &Game, pack: &[Entity]| -> usize {
        game.group_pack(pack.to_vec())
            .iter()
            .map(|g| g.members.len())
            .sum()
    };

    let quiet = fielded(&game, &pack);
    set_trace(&mut game, TRACE_HUNTED);
    let hunted = fielded(&game, &pack);

    assert!(
        hunted > quiet,
        "Hunted fielded {hunted} of the pack, Quiet fielded {quiet} — the \
         band multiplier never reached the fight"
    );
}

// ---- Depth as the Stack's distance -------------------------------------

/// The surface escalates a fight by distance from the danger origin. The
/// Stack could not: the party's `Position` is pinned to the entrance tile
/// they walked in through, so `max_group_size` measured the *base's own
/// doorstep* however far down they had gone, and every frame at every depth
/// fielded a single program in a single group. Depth is what pushing out
/// means underground, and it now feeds the same curve.
#[test]
fn a_deeper_frame_fields_more_of_the_pack_than_a_shallow_one() {
    let mut game = game();
    // Zone 4's cap is 27, so the depth curve rather than the zone clamp is
    // what this measures — at zone 2 both depths would land on the cap of 3.
    game.world.insert_resource(ZoneLevel(4));
    descend(&mut game);

    let pack: Vec<Entity> = (0..8)
        .map(|_| spawn_wild_on_player_tile(&mut game))
        .collect();
    let fielded = |game: &Game| -> usize {
        game.group_pack(pack.clone())
            .iter()
            .map(|g| g.members.len())
            .sum()
    };

    set_depth(&mut game, 1);
    let shallow = fielded(&game);
    set_depth(&mut game, 4);
    let deep = fielded(&game);

    assert_eq!(shallow, 1, "the first frame is still one program");
    assert!(
        deep > shallow,
        "depth 4 fielded {deep} of the pack, depth 1 fielded {shallow} — \
         descending has to escalate the fight the way walking out does"
    );
}

/// The same leak `a_surface_spawn_is_unscaled_while_the_party_is_hunted`
/// guards, for the second lever that rides the party's own state. Ambient
/// spawns and nest respawns keep rolling on every `tick` while the party is
/// four frames down, and a depth read off the locale *inside* the spawn
/// would size those from the party's depth — leaving oversized packs waiting
/// at the link mouth for the climb out. Depth is a parameter for exactly the
/// reason Trace's multiplier is.
#[test]
fn a_surface_spawn_is_unscaled_while_the_party_is_deep() {
    fn surface_pack_size(depth: u32) -> usize {
        let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.insert_resource(ZoneLevel(4));
        descend(&mut game);
        set_depth(&mut game, depth);

        let (x, y) = multi_group_ground(&mut game);
        let before = game.world.query::<&Creature>().iter(&game.world).count();
        game.try_spawn_habitat_creature(x, y);
        game.world.query::<&Creature>().iter(&game.world).count() - before
    }

    assert_eq!(
        surface_pack_size(6),
        surface_pack_size(1),
        "the party's depth must not reach a spawn happening on the surface"
    );
}

/// `group_pack` groups by *species*, and `maybe_stack_encounter` drew one
/// species — so a Stack fight was a single group however many
/// `max_enemy_groups` allowed. Raising the count alone would have been a
/// no-op, the same way scaling only the spawn made `TRACE_GROUP_MULT` one.
/// The encounter now draws a pick per group the depth has earned.
#[test]
fn a_deep_encounter_draws_a_pack_per_group_the_depth_allows() {
    let mut game = game();
    game.world.insert_resource(ZoneLevel(4));
    descend(&mut game);
    set_depth(&mut game, 4);

    let groups = game.max_enemy_groups(Some(4));
    assert!(
        groups > 1,
        "depth 4 should have earned more than one group, got {groups}"
    );

    let pack = game.stack_encounter_pack();
    assert!(
        pack.len() >= groups,
        "{groups} groups' worth of picks should have put at least {groups} \
         programs on the field, got {}",
        pack.len()
    );
}

/// The opening ring is load-bearing — `in_opening_ring` and the fresh-player
/// species checks both depend on a zone-1 fight being one program — and the
/// first frame of a stack is the underground equivalent. Depth 1 must stay
/// exactly where it was.
#[test]
fn the_first_frame_is_no_wider_than_it_was() {
    let mut game = game();
    descend(&mut game);

    assert_eq!(
        game.max_group_size(Some(1)),
        game.max_group_size(None),
        "depth 1 is the entrance tile's own curve, not a step past it"
    );
    assert_eq!(game.max_enemy_groups(Some(1)), 1);
}

// ─────────────────────────────────────────────────────────────────────────
// Phase 3 — cell kinds
// ─────────────────────────────────────────────────────────────────────────

/// Puts the party on a walkable neighbour of the frame's first cell of
/// `kind`, facing it, so `step_forward` walks onto it through the real step
/// path rather than the test teleporting the party on top of it.
///
/// Returns the target cell. `None` when the frame has no such cell — which
/// is a real case for `Fault` on a bottom frame, and a test that wants one
/// should say so rather than unwrap blindly.
fn stand_facing(game: &mut Game, kind: CellKind) -> Option<(i32, i32)> {
    let (target, from, facing) = {
        let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
        let target = (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .find(|&(x, y)| level.cell(x, y) == kind)?;
        let dirs = [Dir::North, Dir::East, Dir::South, Dir::West];
        let (from, facing) = dirs.into_iter().find_map(|dir| {
            let (dx, dy) = dir.delta();
            // Step *from* the neighbour opposite the way we want to face.
            let from = (target.0 - dx, target.1 - dy);
            level.walkable(from.0, from.1).then_some((from, dir))
        })?;
        (target, from, facing)
    };
    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: from.0,
        y: from.1,
        facing,
        entrance,
    });
    Some(target)
}

fn trace_of(game: &Game) -> u32 {
    game.world.resource::<crate::resources::Trace>().0
}

fn player_hp(game: &Game) -> i32 {
    game.world.get::<Stats>(game.player_entity()).unwrap().hp
}

/// What the party has mapped of the frame they are standing in.
fn seen_cells(game: &Game) -> std::collections::BTreeSet<(i32, i32)> {
    let Locale::Stack {
        depth, entrance, ..
    } = locale(game)
    else {
        unreachable!("not underground")
    };
    game.world
        .resource::<crate::resources::StackMemory>()
        .0
        .get(&(entrance, depth))
        .expect("standing in a frame it has a memory of")
        .seen
        .clone()
}

fn frame_dims(game: &Game) -> (i32, i32) {
    let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
    (level.width, level.height)
}

struct Jack {
    game: Game,
    port: (i32, i32),
    trace_before: u32,
    seed: u64,
}

/// The same walk onto the same port with the port already burnt out, so what
/// the jack-in itself showed the party is the difference between the two
/// seen sets. Without this control a test can only measure the reveal
/// against the radius it was drawn from, which is true however small the
/// reveal is — including not happening at all.
fn walk_onto_a_spent_port(seed: u64) -> Game {
    let mut game = game();
    descend(&mut game);
    let port = stand_facing(&mut game, CellKind::Breakpoint).expect("every frame exposes a port");
    let pos = game.stack_pos().expect("underground");
    game.frame_memory_mut(pos).jacked.insert(port);
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(seed)));
    game.step_forward();
    game
}

/// A jack-in is a `GameRng` roll, so a test that wants one outcome searches
/// seeds for it rather than naming a seed that any change upstream of the
/// roll would shift. Returns the game standing on the spent port.
fn jack_in(succeeds: bool) -> Jack {
    for seed in 0..64u64 {
        let mut game = game();
        descend(&mut game);
        let port =
            stand_facing(&mut game, CellKind::Breakpoint).expect("every frame exposes a port");
        game.world
            .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(seed)));
        let trace_before = trace_of(&game);
        game.step_forward();
        let (w, h) = frame_dims(&game);
        if (seen_cells(&game).len() == (w * h) as usize) == succeeds {
            return Jack {
                game,
                port,
                trace_before,
                seed,
            };
        }
    }
    panic!("no seed under 64 produced a jack-in that succeeded: {succeeds}");
}

/// The payout: the whole frame, walls included, in one step. Asserted as
/// *every* in-bounds cell rather than every walkable one — a map drawn from
/// floors alone is a floor plan floating in nothing.
#[test]
fn a_jack_in_that_takes_maps_the_entire_frame() {
    let game = jack_in(true).game;
    let (w, h) = frame_dims(&game);
    let seen = seen_cells(&game);
    for y in 0..h {
        for x in 0..w {
            assert!(seen.contains(&(x, y)), "({x}, {y}) went unmapped");
        }
    }
    assert_eq!(seen.len(), (w * h) as usize);
}

/// The consolation: a failed jack shows the party something the walk did not
/// — measured against `walk_onto_a_spent_port`, since the reveal cannot be
/// checked against the radius it was drawn from — and nothing outside the
/// patch around them.
#[test]
fn a_jack_in_that_fails_maps_the_patch_around_the_party_and_no_more() {
    let Jack {
        game, port, seed, ..
    } = jack_in(false);
    let walked = seen_cells(&walk_onto_a_spent_port(seed));
    let seen = seen_cells(&game);
    let r = crate::tuning::STACK_BREAKPOINT_PARTIAL_RADIUS;

    let handed_over: Vec<(i32, i32)> = seen.difference(&walked).copied().collect();
    assert!(
        !handed_over.is_empty(),
        "a failed jack showed the party nothing the walk to the port hadn't"
    );
    for cell in &handed_over {
        assert!(
            (cell.0 - port.0).abs() <= r && (cell.1 - port.1).abs() <= r,
            "{cell:?} is outside the patch — a failed jack reached further \
             than the substrate the party is standing in"
        );
    }
    let (w, h) = frame_dims(&game);
    assert!(
        seen.len() < (w * h) as usize,
        "a failed jack handed over the whole frame"
    );
}

/// One try. A port that could be jacked again until it took would make the
/// roll a delay rather than a risk.
#[test]
fn a_jack_in_that_fails_still_burns_the_port() {
    let Jack { mut game, port, .. } = jack_in(false);
    let after_first = trace_of(&game);
    assert_ne!(
        game.frame_map().unwrap().cells[port.1 as usize][port.0 as usize],
        crate::views::FrameMapCell::Breakpoint,
        "the spent port is still advertised on the map"
    );

    // Off and back on, through the same real step path.
    game.step_back();
    game.step_forward();
    assert_eq!(
        trace_of(&game),
        after_first,
        "the port paid out a second time — the jacked record is not holding"
    );
}

/// Trace is the price of jacking in, not of what it gave you: the substrate
/// heard you either way.
#[test]
fn jacking_into_a_breakpoint_is_the_loudest_thing_the_party_can_do() {
    for succeeds in [true, false] {
        let jack = jack_in(succeeds);
        assert_eq!(
            trace_of(&jack.game) - jack.trace_before,
            crate::tuning::TRACE_PER_BREAKPOINT,
            "a jack-in that succeeded: {succeeds} charged the wrong Trace"
        );
    }
}

/// The spent-ness half. Without `FrameMemory::jacked` the port refills the
/// moment the party steps off and back on, and Trace becomes a tap you can
/// leave running for free maps.
#[test]
fn a_spent_breakpoint_stays_spent_when_the_party_steps_off_and_back_on() {
    let mut game = game();
    descend(&mut game);
    stand_facing(&mut game, CellKind::Breakpoint).unwrap();
    game.step_forward();
    let after_first = trace_of(&game);

    // Off and back on, through the same real step path.
    game.step_back();
    game.step_forward();
    assert_eq!(
        trace_of(&game),
        after_first,
        "the port paid out a second time — the jacked record is not holding"
    );
}

/// A fault is the one cell that moves the party between frames on a plain
/// step. It has to land them somewhere they could stand, and *not* on the
/// way back up — landing on the up-link would make a fall a free ride.
#[test]
fn falling_through_a_fault_lands_a_frame_down_and_not_on_the_way_up() {
    let mut game = game();
    descend(&mut game);
    let Locale::Stack { depth: before, .. } = locale(&game) else {
        unreachable!()
    };
    stand_facing(&mut game, CellKind::Fault).expect("a non-bottom frame lays a fault");
    game.step_forward();

    let Locale::Stack { depth, x, y, .. } = locale(&game) else {
        unreachable!("the fall left the Stack entirely")
    };
    assert_eq!(depth, before + 1, "a fault must drop exactly one frame");

    let level = game.world.resource::<CurrentStack>().0.as_ref().unwrap();
    assert_eq!(
        level.cell(x, y),
        CellKind::Floor,
        "landed on {:?} — a fall must come down on plain floor, never on a \
         cache, a lair or another fault",
        level.cell(x, y)
    );
    assert_ne!(
        (x, y),
        level.entry,
        "landed on the frame's own way up, which makes a fall a free ride"
    );
}

/// Falling is clumsy, not loud. Trace is a meter for what the party takes,
/// and a fall is something that happens to them.
#[test]
fn falling_through_a_fault_raises_no_trace() {
    let mut game = game();
    descend(&mut game);
    stand_facing(&mut game, CellKind::Fault).unwrap();
    let before = trace_of(&game);
    game.step_forward();
    assert_eq!(trace_of(&game), before);
}

#[test]
fn stepping_onto_corruption_costs_the_player_hp() {
    let mut game = game();
    descend(&mut game);
    stand_facing(&mut game, CellKind::Corruption).expect("every frame grows corruption");
    let before = player_hp(&game);
    game.step_forward();

    let max_hp = game
        .world
        .get::<Stats>(game.player_entity())
        .unwrap()
        .max_hp;
    let expected = ((max_hp as f32 * crate::tuning::STACK_CORRUPTION_HP_PERCENT).round() as i32)
        .max(crate::tuning::STACK_CORRUPTION_MIN_DAMAGE);
    assert_eq!(before - player_hp(&game), expected);
}

/// The proof that corruption routes through `Game::apply_damage` rather
/// than writing `Stats::hp` directly. A direct write would ignore the
/// Mitigation buff entirely and the two figures would match — so this fails
/// against the shortcut and passes against the real path.
#[test]
fn corruption_goes_through_apply_damage_and_so_mitigation_blunts_it() {
    let unmitigated = {
        let mut game = game();
        descend(&mut game);
        stand_facing(&mut game, CellKind::Corruption).unwrap();
        let before = player_hp(&game);
        game.step_forward();
        before - player_hp(&game)
    };

    let mut game = game();
    descend(&mut game);
    stand_facing(&mut game, CellKind::Corruption).unwrap();
    let player = game.player_entity();
    game.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "test".to_string(),
            power: 50,
            remaining: 99,
            interval: 1,
            source: BuffSource::Consumable,
        }],
    });
    let before = player_hp(&game);
    game.step_forward();
    let mitigated = before - player_hp(&game);

    assert!(
        mitigated < unmitigated,
        "mitigation did not blunt corruption ({mitigated} vs {unmitigated}) — \
         the damage is bypassing apply_damage"
    );
}

/// The player alone. Corrupting the party would route program deaths and
/// the permadeath path through something that is not a fight.
#[test]
fn corruption_does_not_touch_the_party() {
    let mut game = game();
    descend(&mut game);
    let pet = spawn_tamed(&mut game, 40, 5);
    let before = game.world.get::<Stats>(pet).unwrap().hp;
    stand_facing(&mut game, CellKind::Corruption).unwrap();
    game.step_forward();
    assert_eq!(game.world.get::<Stats>(pet).unwrap().hp, before);
}

/// The relationship the 2026-08-01 retune rests on, and the reason those
/// thresholds moved at all.
///
/// A frame holds `STACK_CACHES_PER_FRAME` caches worth `TRACE_PER_CACHE`
/// each. Emptying every one of them is the greediest thing that can be done
/// to a single floor, and it has to be enough to cross the first band — at
/// the original 40 against three caches paying 10, it was not, so a player
/// could strip a frame bare and watch the meter do nothing.
///
/// Written against the constants rather than the numbers so it keeps
/// meaning this after the next retune: raise `TRACE_NOTICED` past what a
/// frame's caches pay, or cut the cache count, and this fails rather than
/// the mechanic quietly going inert again.
#[test]
fn stripping_a_frames_caches_is_enough_to_be_noticed() {
    use crate::resources::TraceBand;
    use crate::tuning::{STACK_CACHES_PER_FRAME, TRACE_NOTICED, TRACE_PER_CACHE};

    let whole_frame = TRACE_PER_CACHE * STACK_CACHES_PER_FRAME as u32;
    assert!(
        whole_frame >= TRACE_NOTICED,
        "a frame's {STACK_CACHES_PER_FRAME} caches pay {whole_frame} Trace \
         against a first band at {TRACE_NOTICED} — stripping an entire floor \
         leaves the player Quiet, and the meter never announces itself"
    );
    assert_eq!(TraceBand::from_trace(whole_frame), TraceBand::Noticed);
}

// ─────────────────────────────────────────────────────────────────────────
// Phase 4 — the orphaned process
// ─────────────────────────────────────────────────────────────────────────

/// Walks the party onto the frame's orphan and returns its cell.
///
/// `None` when the frame has none — about one frame in four has no plain
/// dead end left after the caches take theirs, so a test that needs one
/// says so rather than unwrapping blindly. See
/// `most_frames_place_an_orphan_and_none_places_two`.
fn stand_on_orphan(game: &mut Game) -> Option<(i32, i32)> {
    let cell = stand_facing(game, CellKind::Orphan)?;
    game.step_forward();
    Some(cell)
}

/// The other half of a cell that can be used up. Without the record the
/// orphan refills the moment the party steps off and back on, and both
/// views keep advertising a dead end with nothing in it.
#[test]
fn an_adopted_orphan_reads_as_plain_floor_in_both_views() {
    let mut game = game();
    descend(&mut game);
    let cell = stand_on_orphan(&mut game).expect("this seed's depth 1 leaves an orphan");

    let view = game.stack_view().unwrap();
    assert!(
        view.cells[0].contains(&StackCellView::Orphan),
        "the party is standing on it and the view does not show one"
    );
    assert_eq!(
        game.frame_map().unwrap().cells[cell.1 as usize][cell.0 as usize],
        FrameMapCell::Orphan
    );

    let Locale::Stack {
        depth, entrance, ..
    } = locale(&game)
    else {
        unreachable!()
    };
    game.world
        .resource_mut::<crate::resources::StackMemory>()
        .0
        .entry((entrance, depth))
        .or_default()
        .adopted
        .insert(cell);

    let view = game.stack_view().unwrap();
    assert!(
        !view.cells[0].contains(&StackCellView::Orphan),
        "an adopted orphan is still drawn down the corridor"
    );
    assert_eq!(
        game.frame_map().unwrap().cells[cell.1 as usize][cell.0 as usize],
        FrameMapCell::Floor,
        "an adopted orphan is still marked on the map"
    );
    assert_eq!(
        game.stack_view().unwrap().standing_on,
        None,
        "an adopted orphan still offers itself underfoot"
    );
}

/// The invariant the whole of `orphan_species` exists for. The party has to
/// see what a program is before paying an `ice_breaker` for it, so the
/// answer has to survive a save/load — which a `GameRng` draw would not,
/// since that stream's position is not persisted. A test that merely called
/// it twice in one session would pass against exactly the implementation
/// this forbids.
#[test]
fn the_species_a_frame_offers_survives_a_save_and_load() {
    let mut game = game();
    descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let before = game.orphan_species(pos);
    assert!(before.is_some(), "the entrance biome fields nothing at all");

    let path = std::env::temp_dir().join(format!(
        "feral_orphan_species_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let pos = loaded.stack_pos().unwrap();
    assert_eq!(
        loaded.orphan_species(pos),
        before,
        "the frame offered a different program after a reload"
    );
}

/// Two frames of one stack share an entrance and therefore a biome pool, so
/// this pins that the *seed* differs by depth rather than the pool doing the
/// work. Without the depth in the salt every frame of a stack would offer
/// the same program.
#[test]
fn two_depths_of_one_stack_draw_their_orphans_independently() {
    let mut game = game();
    descend(&mut game);
    let pos = game.stack_pos().unwrap();

    let mut seen = Vec::new();
    for depth in 1..=6 {
        seen.push(game.orphan_species(StackPos { depth, ..pos }));
    }
    assert!(
        seen.iter().any(|s| *s != seen[0]),
        "every depth of the stack offered {:?} — the depth is not in the salt",
        seen[0]
    );
}

/// Puts the party on this frame's orphan with one `ice_breaker` in hand.
/// `None` when the frame left no orphan.
fn ready_to_adopt(game: &mut Game) -> Option<(i32, i32)> {
    let cell = stand_on_orphan(game)?;
    set_inventory(game, &[(ids::ICE_BREAKER, 1)]);
    Some(cell)
}

fn ice_breakers(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::ICE_BREAKER))
}

#[test]
fn adopting_an_orphan_puts_a_program_in_the_roster() {
    let mut game = game();
    descend(&mut game);
    let cell = ready_to_adopt(&mut game).expect("this seed's depth 1 leaves an orphan");
    let before = game.pet_count();
    let player = game.player_entity();

    game.adopt_orphan().expect("standing on one, holding one");

    assert_eq!(game.pet_count(), before + 1, "the roster did not grow");
    assert_eq!(ice_breakers(&game), 0, "the catalyst was not spent");
    let adopted = game
        .world
        .iter_entities()
        .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
        .count();
    assert_eq!(adopted, before + 1);

    let Locale::Stack {
        depth, entrance, ..
    } = locale(&game)
    else {
        unreachable!()
    };
    assert!(
        game.world
            .resource::<crate::resources::StackMemory>()
            .0
            .get(&(entrance, depth))
            .is_some_and(|m| m.adopted.contains(&cell)),
        "the cell was not recorded as spent"
    );
}

#[test]
fn adopting_off_an_orphan_is_refused_and_costs_nothing() {
    let mut game = game();
    descend(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    let before = game.pet_count();

    assert!(game.adopt_orphan().is_err());
    assert_eq!(ice_breakers(&game), 1);
    assert_eq!(game.pet_count(), before);
}

/// The refusal has to land *before* anything is spawned. A message-only
/// assertion passes against an implementation that has already created the
/// creature and then declined to charge for it.
#[test]
fn adopting_without_a_catalyst_is_refused_before_anything_is_spawned() {
    let mut game = game();
    descend(&mut game);
    ready_to_adopt(&mut game).expect("this seed's depth 1 leaves an orphan");
    set_inventory(&mut game, &[]);
    let before = game.pet_count();

    let err = game.adopt_orphan().unwrap_err();
    assert!(err.contains("no taming catalyst"), "{err}");
    assert_eq!(game.pet_count(), before, "a refused adoption still spawned");
}

/// The ordering bug this phase is most likely to ship: refusing for a full
/// roster *after* the catalyst has been taken charges the player for
/// nothing.
#[test]
fn adopting_with_a_full_roster_is_refused_before_the_catalyst_is_spent() {
    let mut game = game();
    descend(&mut game);
    ready_to_adopt(&mut game).expect("this seed's depth 1 leaves an orphan");
    while game.pet_count() < game.pet_capacity() {
        spawn_tamed(&mut game, 10, 2);
    }
    let before = game.pet_count();

    let err = game.adopt_orphan().unwrap_err();
    assert!(err.contains("roster is full"), "{err}");
    assert_eq!(
        ice_breakers(&game),
        1,
        "a refused adoption spent the catalyst"
    );
    assert_eq!(game.pet_count(), before);
}

/// The spent-ness half, through the real step path. Without
/// `FrameMemory::adopted` the dead end refills every time the party steps
/// off and back on, and one `ice_breaker` buys a program per lap.
#[test]
fn an_orphan_does_not_recur_when_the_party_steps_off_and_back_on() {
    let mut game = game();
    descend(&mut game);
    let cell = ready_to_adopt(&mut game).expect("this seed's depth 1 leaves an orphan");
    game.adopt_orphan().unwrap();
    let after_first = game.pet_count();

    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    game.step_back();
    game.step_forward();
    // A step underground can roll an encounter, and a live battle refuses
    // every action for its own reason — which passed this test against an
    // `adopt_orphan` with no spent-ness check in it at all.
    flee_until_clear(&mut game);
    assert_eq!(
        game.stack_pos().map(|p| (p.x, p.y)),
        Some(cell),
        "the party did not walk back onto the cell"
    );

    let err = game.adopt_orphan().unwrap_err();
    assert!(
        err.contains("nothing like that here"),
        "the orphan came back: {err}"
    );
    assert_eq!(
        game.stack_view().unwrap().standing_on,
        None,
        "the emptied dead end still offers itself"
    );
    assert_eq!(game.pet_count(), after_first);
    assert_eq!(ice_breakers(&game), 1);
}

/// Forgiving mode reboots you at the nearest construction. Underground,
/// `death_handling_system` was writing that construction's tile into
/// `Position` while `Locale::Stack` stayed live — so the party was warped
/// home on paper and left standing in the maze, with the entrance tile
/// `Position` had been pinned to overwritten into the bargain.
#[test]
fn a_forgiving_death_underground_surfaces_the_party_at_their_base() {
    let mut game = game();
    place_home(&mut game, 3, 0);
    let home = *game
        .world
        .iter_entities()
        .find(|e| e.contains::<Structure>())
        .and_then(|e| e.get::<Position>())
        .expect("place_home should have deployed one");
    descend(&mut game);
    game.raise_trace(4);

    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 0;
    game.tick();

    assert!(
        !game.is_underground(),
        "a Forgiving reboot has to leave the Stack, not just move Position while in it"
    );
    assert_eq!(
        *game.world.get::<Position>(player).unwrap(),
        home,
        "and it lands on the construction it rebooted at"
    );
    assert!(
        game.world.resource::<CurrentStack>().0.is_none(),
        "the frame goes with the locale — clear_stack's whole point"
    );
    assert_eq!(
        game.trace(),
        0,
        "and Trace resets, as it does on every other way out"
    );
}
