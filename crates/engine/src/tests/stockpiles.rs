//! Stockpile zones: a standing mark on laid floor, drawn with the Excavation
//! box but never part of the dig plan.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::items::FloorThing;
use crate::*;

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn in_base(g: &mut Game) {
    stand_in_base_at(g, 0, 0);
}

#[test]
fn a_box_marks_only_laid_floor_as_stockpile() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (1, 0));
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert_eq!(g.stockpile_cells(), vec![(0, 0), (1, 0)]);
}

#[test]
fn a_box_anchored_on_a_stockpile_clears_it() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 0));
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert_eq!(g.stockpile_cells().len(), 3);
    g.toggle_stockpile_box((1, 0), (2, 0));
    assert_eq!(g.stockpile_cells(), vec![(0, 0)]);
    g.toggle_stockpile_box((2, 0), (0, 0)); // anchor unmarked: marks again
    assert_eq!(g.stockpile_cells().len(), 3);
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert!(g.stockpile_cells().is_empty());
}

#[test]
fn stockpile_marks_and_dig_marks_are_independent() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (0, 0));
    g.toggle_mark_box(
        (0, 0),
        (0, 0),
        Some(&FinishOrder::Apply(crate::floors::FloorId::from(
            "cobalt_carpet",
        ))),
    );
    g.toggle_stockpile_box((0, 0), (0, 0));
    let dig = g.marked_cells();
    assert!(!dig.is_empty(), "premise: the cell carries a dig mark");
    assert_eq!(g.stockpile_cells(), vec![(0, 0)]);
    g.toggle_stockpile_box((0, 0), (0, 0));
    assert!(g.stockpile_cells().is_empty());
    assert_eq!(g.marked_cells(), dig);
}

#[test]
fn stockpile_cells_survive_save_and_load() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 1));
    g.toggle_stockpile_box((0, 0), (1, 1));
    let before = g.stockpile_cells();
    assert_eq!(before.len(), 4);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_stockpile_round_trip_{}.sav",
        std::process::id()
    ));
    g.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let grid = loaded.world.resource::<BaseGrid>();
    let after: Vec<_> = grid.stockpile_cells().collect();
    assert_eq!(after, before);
}

#[test]
fn examining_a_stockpile_says_so() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    lay_floor_box(&mut g, (1, 0), (1, 0));
    g.toggle_stockpile_box((1, 0), (1, 0));
    let line = g.describe_base_rock(1, 0, 5).expect("a stockpile answers");
    assert!(line.to_lowercase().contains("a stockpile"), "{line}");
}

#[test]
fn a_stockpile_cell_built_over_stays_marked_but_takes_nothing() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 0));
    g.toggle_stockpile_box((1, 0), (1, 0));
    spawn_structure_at(&mut g, "wall", 1, 0);
    assert!(g.world.resource::<BaseGrid>().is_stockpile(1, 0));
    g.set_down(
        Position { x: 1, y: 0 },
        vec![FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))],
    );
    assert!(
        floor_things(&mut g).iter().all(|(c, _)| *c != (1, 0)),
        "{:?}",
        floor_things(&mut g)
    );
}
