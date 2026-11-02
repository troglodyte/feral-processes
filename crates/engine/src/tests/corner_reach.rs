//! A builder and a digger may stand at a corner of their cell, not only on
//! one of its four sides (`hauling::Touch::AllAround`). Every other base job
//! still touches side to side.

use super::support::*;
use crate::*;

/// A Home with the fixture pocket laid around it, the party standing on it,
/// a pack of Core Fragments to file builds with, and one sturdy worker.
fn base_with_a_worker(seed: u32) -> (Game, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    let worker = spawn_tamed(&mut game, 500, 3);
    (game, worker)
}

/// Off the chamfered corner of the fixture pocket: rock on all four sides,
/// and the one floor cell touching it is the diagonal `(9, 2)`.
const CORNER_ONLY: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 1, 3);

fn is_floor(game: &Game, (x, y): (i32, i32)) -> bool {
    game.world
        .resource::<crate::base_grid::BaseGrid>()
        .is_floor(x, y)
}

fn assert_floor_only_at_the_corner(game: &Game) {
    let (x, y) = CORNER_ONLY;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert!(
            !is_floor(game, (x + dx, y + dy)),
            "precondition: no floor on any side of {CORNER_ONLY:?}"
        );
    }
    assert!(
        is_floor(game, (x - 1, y - 1)),
        "precondition: floor at the corner"
    );
}

fn structure_at(game: &mut Game, (x, y): (i32, i32)) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
}

/// Under the four-sides rule this site had no face at all — `has_station`
/// dropped it as boxed in and nothing ever raised it.
#[test]
fn a_builder_raises_a_site_it_can_only_reach_from_a_corner() {
    let (mut game, _) = base_with_a_worker(1191);
    game.dev_lay_floor([CORNER_ONLY]);
    assert_floor_only_at_the_corner(&game);
    file_build(&mut game, "depot", CORNER_ONLY.0, CORNER_ONLY.1).unwrap();

    for _ in 0..600 {
        if structure_at(&mut game, CORNER_ONLY).is_some() {
            break;
        }
        game.tick();
    }

    assert!(
        structure_at(&mut game, CORNER_ONLY).is_some(),
        "the crew must raise a site it can stand diagonally beside"
    );
}

/// The wall just east of the pocket's middle row, and the one floor cell
/// beside it, `(9, 0)`.
const WALL: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 1, 0);

/// A Depot stands on the one floor cell beside `WALL`, so its only free
/// faces are the diagonals `(9, ±1)`. Under the four-sides rule the mark was
/// boxed in, silently, for as long as the Depot stood.
#[test]
fn a_digger_cuts_a_wall_it_can_only_reach_from_a_corner() {
    let (mut game, worker) = base_with_a_worker(3297);
    set_rock(&mut game, WALL);
    spawn_structure_at(&mut game, "depot", WALL.0 - 1, WALL.1);
    game.toggle_mark_box(WALL, WALL, None);
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("marking a wall spawns its dig site");

    game.tick();
    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(site),
        "a wall with a free corner is a job"
    );
    for _ in 0..2000 {
        if game.ground().walkable(WALL.0, WALL.1) {
            break;
        }
        game.tick();
    }

    assert!(
        game.ground().walkable(WALL.0, WALL.1),
        "the digger must cut the wall from the corner it stands on"
    );
}

/// The corner widens where a digger may *stand*, not what it may cut: a
/// blocker still needs floor or pending floor on a side
/// (`field_mining::minable`) before anyone swings at it, so a mark only a
/// corner touches must not hold a body that would stand there forever.
#[test]
fn a_mark_touching_floor_only_at_a_corner_holds_no_body() {
    let (mut game, worker) = base_with_a_worker(3298);
    set_rock(&mut game, CORNER_ONLY);
    // Marked through a side cell laid just long enough for the mark to be
    // accepted — the state an old save, or a corridor reverted under a
    // standing mark, can hold.
    let beside = (CORNER_ONLY.0 - 1, CORNER_ONLY.1);
    game.dev_lay_floor([beside]);
    game.toggle_mark_box(CORNER_ONLY, CORNER_ONLY, None);
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .revert(beside.0, beside.1);
    assert_floor_only_at_the_corner(&game);
    assert!(
        game.dig_site_at(CORNER_ONLY.0, CORNER_ONLY.1).is_some(),
        "precondition: the mark stands"
    );

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        None,
        "nothing can cut that cell yet, so it is not a job"
    );
}
