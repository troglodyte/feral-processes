//! `BaseGrid`: a fresh grid is solid everywhere, `lay_floor` and `open` are
//! the only ways in, and the underlying map iterates in a deterministic
//! order — the property the save encoder will lean on once one exists.
//!
//! The save round trip lives here too, per this repo's convention that each
//! feature tests its own save round trip in its own file rather than a
//! shared save-test module.

use crate::Game;
use crate::base_grid::{BaseCell, BaseGrid};
use crate::floors::{FloorDb, FloorId};
use crate::resources::DifficultyMode;
use crate::tests::support::test_assets_dir;

#[test]
fn a_fresh_grid_is_solid_everywhere() {
    let grid = BaseGrid::default();
    for (x, y) in [(0, 0), (5, -3), (-100, 100), (1, 1)] {
        assert!(grid.is_solid(x, y));
        assert!(!grid.is_floor(x, y));
        assert!(!grid.walkable(x, y));
        assert_eq!(grid.cell(x, y), None);
    }
    assert_eq!(grid.floor_count(), 0);
}

#[test]
fn lay_floor_makes_a_coordinate_floor_and_not_solid() {
    let mut grid = BaseGrid::default();
    grid.lay_floor(3, 4);

    assert!(grid.is_floor(3, 4));
    assert!(!grid.is_solid(3, 4));
    assert!(grid.walkable(3, 4));
    assert_eq!(grid.cell(3, 4), Some(BaseCell::Floor));
    assert_eq!(grid.floor_count(), 1);
}

#[test]
fn open_makes_a_coordinate_walkable_but_not_floor() {
    let mut grid = BaseGrid::default();
    grid.open(1, 2, 7);

    assert!(grid.walkable(1, 2));
    assert!(!grid.is_floor(1, 2));
    assert!(!grid.is_solid(1, 2));
    assert_eq!(grid.cell(1, 2), Some(BaseCell::Open { mined_at: 7 }));
    assert_eq!(grid.floor_count(), 0);
}

#[test]
fn lay_floor_over_an_open_cell_replaces_it_rather_than_stacking() {
    let mut grid = BaseGrid::default();
    grid.open(0, 0, 42);
    grid.lay_floor(0, 0);

    assert_eq!(grid.cell(0, 0), Some(BaseCell::Floor));
    assert_eq!(grid.floor_count(), 1);
}

#[test]
fn cells_inserted_in_scrambled_order_iterate_in_ascending_key_order() {
    let mut grid = BaseGrid::default();
    for (x, y) in [(5, 5), (-2, 3), (0, 0), (2, -1), (-2, -2)] {
        grid.open(x, y, 0);
    }

    let keys: Vec<(i32, i32)> = grid.iter().map(|(&k, _)| k).collect();
    let mut sorted = keys.clone();
    sorted.sort();

    assert_eq!(keys, sorted, "BTreeMap iteration must already be sorted");
}

/// Save format v32's whole point: `BaseGrid` round-trips through a real
/// `Game::save`/`Game::load`, not only the RON round trip. A round trip
/// through `to_ron`/`from_ron` alone cannot catch `#[serde(skip)]` on
/// `SaveData::base_grid` or a load path that forgets to insert the restored
/// resource — both would leave `loaded`'s grid solid everywhere, identical
/// to a grid nobody ever touched, which is exactly what a hand-laid grid
/// with both a `Floor` and an `Open` cell rules out.
#[test]
fn a_hand_laid_grid_survives_a_real_save_and_load() {
    let mut game = Game::new(4001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        grid.lay_floor(2, 3);
        grid.lay_floor(-5, 1);
        grid.open(9, -4, 42);
    }

    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_grid_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let grid = loaded.world.resource::<BaseGrid>();
    assert_eq!(grid.cell(2, 3), Some(BaseCell::Floor));
    assert_eq!(grid.cell(-5, 1), Some(BaseCell::Floor));
    assert_eq!(grid.cell(9, -4), Some(BaseCell::Open { mined_at: 42 }));
    assert_eq!(
        grid.floor_count(),
        2,
        "only the two Floor cells count, not the Open one"
    );
    assert!(
        grid.is_solid(0, 0),
        "an untouched coordinate must still read solid after the round trip"
    );
}

// ---------------------------------------------------------------------------
// Floor finishes
// ---------------------------------------------------------------------------

#[test]
fn set_finish_refuses_an_open_cell_and_a_solid_cell_but_accepts_floor() {
    let mut grid = BaseGrid::default();
    grid.open(1, 1, 0);
    assert!(
        !grid.set_finish(1, 1, FloorId::from("cobalt_carpet")),
        "an Open cell took a finish"
    );
    assert!(grid.finish_at(1, 1).is_none());

    assert!(
        !grid.set_finish(9, 9, FloorId::from("cobalt_carpet")),
        "solid rock took a finish"
    );
    assert!(grid.finish_at(9, 9).is_none());

    grid.lay_floor(0, 0);
    assert!(grid.set_finish(0, 0, FloorId::from("cobalt_carpet")));
    assert_eq!(grid.finish_at(0, 0), Some(&FloorId::from("cobalt_carpet")));
}

#[test]
fn clear_finish_leaves_the_cell_floor() {
    let mut grid = BaseGrid::default();
    grid.lay_floor(0, 0);
    grid.set_finish(0, 0, FloorId::from("cobalt_carpet"));

    assert!(grid.clear_finish(0, 0));

    assert!(grid.finish_at(0, 0).is_none());
    assert!(grid.is_floor(0, 0), "stripping a finish unfloored the cell");
}

#[test]
fn revert_on_a_finished_cell_leaves_no_finish() {
    let mut grid = BaseGrid::default();
    grid.lay_floor(0, 0);
    grid.set_finish(0, 0, FloorId::from("cobalt_carpet"));

    grid.revert(0, 0);

    assert!(grid.finish_at(0, 0).is_none());
    assert!(grid.is_solid(0, 0));
}

fn floor_db() -> FloorDb {
    let (db, warnings) = FloorDb::load_dir(&test_assets_dir().join("floors")).unwrap();
    assert!(
        warnings.is_empty(),
        "shipped floor assets warned: {warnings:?}"
    );
    db
}

/// `set_finish` refuses both of these, so the bad entries are built by
/// round-tripping through RON directly — the same way a mod's stale id or a
/// save from before this check existed could arrive.
#[test]
fn prune_finishes_drops_an_unknown_id_and_a_non_floor_entry_and_keeps_a_good_one() {
    let mut grid = BaseGrid::default();
    grid.lay_floor(0, 0);
    grid.open(1, 1, 0);
    grid.lay_floor(2, 2);
    grid.set_finish(2, 2, FloorId::from("cobalt_carpet"));

    let text = ron::to_string(&grid).unwrap();
    // Inject the two bad entries the public setter would have refused: an
    // unknown id on a real floor cell, and a known id on a cell that is not
    // floor.
    let text = text.replace(
        "finishes:{(2,2):\"cobalt_carpet\"}",
        "finishes:{(0,0):\"no_such_finish\",(1,1):\"cobalt_carpet\",(2,2):\"cobalt_carpet\"}",
    );
    let mut grid: BaseGrid = ron::from_str(&text).unwrap();
    assert_eq!(
        grid.finish_at(0, 0),
        Some(&FloorId::from("no_such_finish")),
        "the round trip must have actually inserted the bad entry"
    );

    let warnings = grid.prune_finishes(&floor_db());

    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(
        grid.finish_at(0, 0).is_none(),
        "an unknown id survived pruning"
    );
    assert!(
        grid.finish_at(1, 1).is_none(),
        "a finish on a non-floor cell survived pruning"
    );
    assert_eq!(
        grid.finish_at(2, 2),
        Some(&FloorId::from("cobalt_carpet")),
        "a good entry was dropped along with the bad ones"
    );
}

/// Save format's own gate: a real `Game::save`/`Game::load`, not a RON
/// round trip, which cannot catch `#[serde(skip)]` on `BaseGrid::finishes`
/// or a load path that forgets to restore it.
#[test]
fn a_finished_floor_survives_a_real_save_and_load() {
    let mut game = Game::new(4002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        grid.lay_floor(2, 3);
        grid.set_finish(2, 3, FloorId::from("cobalt_carpet"));
    }

    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_grid_finish_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let grid = loaded.world.resource::<BaseGrid>();
    assert_eq!(grid.finish_at(2, 3), Some(&FloorId::from("cobalt_carpet")));
}
