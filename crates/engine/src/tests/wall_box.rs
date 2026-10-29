//! `Game::place_wall_box` — painting a box of wall requests, the way the
//! Excavation plan paints a box of dig marks.

use super::support::*;
use crate::resources::GameClock;
use crate::*;

/// A Home, the party on it, and floor laid from (-5, -5) to (5, 5).
fn walled_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base(&mut game);
    lay_floor_box(&mut game, (-5, -5), (5, 5));
    game
}

/// Every filed site as `(cell, structure)`, sorted.
fn sites(game: &mut Game) -> Vec<((i32, i32), String)> {
    let mut rows: Vec<_> = game
        .world
        .query::<(&Position, &BuildSite)>()
        .iter(&game.world)
        .map(|(p, b)| ((p.x, p.y), b.structure.to_string()))
        .collect();
    rows.sort();
    rows
}

fn walls(cells: &[(i32, i32)]) -> Vec<((i32, i32), String)> {
    cells.iter().map(|&c| (c, "wall".to_string())).collect()
}

#[test]
fn a_wall_box_files_a_wall_on_every_cell_of_a_row() {
    let mut game = walled_base(5100);

    let painted = game.place_wall_box((1, 1), (4, 1)).unwrap();

    assert_eq!(
        painted,
        WallBox {
            filed: 4,
            skipped: 0,
            called_off: 0
        }
    );
    assert_eq!(sites(&mut game), walls(&[(1, 1), (2, 1), (3, 1), (4, 1)]));
}

#[test]
fn a_wall_box_fills_the_whole_box_whichever_corner_is_first() {
    let mut game = walled_base(5101);

    game.place_wall_box((2, 3), (1, 2)).unwrap();

    assert_eq!(sites(&mut game), walls(&[(1, 2), (1, 3), (2, 2), (2, 3)]));
}

/// Each cell is asked `place_structure`'s own question, so what a single
/// wall would be refused on is skipped rather than refusing the box.
#[test]
fn a_wall_box_skips_cells_a_single_wall_would_be_refused_on() {
    let mut game = walled_base(5102);
    spawn_structure_at(&mut game, "wall", 1, 2);
    game.place_structure("wall", 1, 3, None).unwrap();

    // (1, 2) stands, (1, 3) is filed, and (1, 6) is past every laid floor.
    let painted = game.place_wall_box((1, 1), (1, 6)).unwrap();

    assert_eq!(
        painted,
        WallBox {
            filed: 3,
            skipped: 3,
            called_off: 0
        }
    );
    assert_eq!(sites(&mut game), walls(&[(1, 1), (1, 3), (1, 4), (1, 5)]));
}

#[test]
fn a_wall_box_with_nowhere_to_stand_a_wall_is_refused() {
    let mut game = walled_base(5103);

    // Past both the laid box and the starting pocket.
    assert!(game.place_wall_box((7, 7), (8, 8)).is_err());
    assert!(sites(&mut game).is_empty());
}

#[test]
fn painting_a_wall_box_spends_no_tick() {
    let mut game = walled_base(5104);
    let before = game.world.resource::<GameClock>().tick;

    game.place_wall_box((1, 1), (4, 1)).unwrap();
    game.place_wall_box((1, 1), (2, 1)).unwrap();

    assert_eq!(game.world.resource::<GameClock>().tick, before);
}

/// The anchor decides, `toggle_mark_box`'s rule: a box drawn from a filed
/// wall calls off the wall requests in it, and nothing else.
#[test]
fn a_wall_box_drawn_from_a_filed_wall_calls_off_the_walls_in_it() {
    let mut game = walled_base(5105);
    game.place_wall_box((1, 1), (4, 1)).unwrap();
    file_build(&mut game, "depot", 2, 2).unwrap();

    let painted = game.place_wall_box((1, 1), (2, 2)).unwrap();

    assert_eq!(
        painted,
        WallBox {
            filed: 0,
            skipped: 0,
            called_off: 2
        }
    );
    let mut left = walls(&[(3, 1), (4, 1)]);
    left.insert(0, ((2, 2), "depot".to_string()));
    left.sort();
    assert_eq!(sites(&mut game), left, "the depot request is not a wall");
}

#[test]
fn a_wall_box_raises_walls_at_once_under_free_build() {
    let mut game = walled_base(5106);
    game.dev_set_free_build(true);

    game.place_wall_box((1, 1), (2, 1)).unwrap();

    assert!(sites(&mut game).is_empty());
    let mut standing: Vec<_> = game
        .world
        .query::<(&Position, &Structure)>()
        .iter(&game.world)
        .filter(|(_, s)| s.kind == "wall")
        .map(|(p, _)| (p.x, p.y))
        .collect();
    standing.sort();
    assert_eq!(standing, vec![(1, 1), (2, 1)]);
}

#[test]
fn a_wall_box_is_a_base_action() {
    let mut game = walled_base(5107);
    game.world.insert_resource(Locale::Surface);

    assert!(game.place_wall_box((1, 1), (2, 1)).is_err());
    assert!(sites(&mut game).is_empty());
}

#[test]
fn only_the_wall_is_placed_by_box() {
    let game = walled_base(5108);

    assert!(game.structure_places_by_box(&StructureId::from("wall")));
    assert!(!game.structure_places_by_box(&StructureId::from("door")));
    assert!(!game.structure_places_by_box(&StructureId::from("depot")));
}
