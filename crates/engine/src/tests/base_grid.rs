//! `BaseGrid`: a fresh grid is solid everywhere, `lay_floor` and `open` are
//! the only ways in, and the underlying map iterates in a deterministic
//! order — the property the save encoder will lean on once one exists.

use crate::base_grid::{BaseCell, BaseGrid};

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
