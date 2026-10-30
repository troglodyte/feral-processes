use super::*;
use crate::tactical::footprint_cells_at;

#[test]
fn radius_grows_two_a_tier() {
    let d = RadiusDef {
        base: 4,
        per_tier: 2,
    };
    assert_eq!(
        (1..=5).map(|t| radius_at(&d, t)).collect::<Vec<_>>(),
        vec![4, 6, 8, 10, 12]
    );
}

#[test]
fn a_touched_tile_is_covered() {
    assert!(covers((0, 0), 4, (0, 0)));
    assert!(covers((0, 0), 4, (4, 0)));
    assert!(!covers((0, 0), 4, (5, 0)));
    assert!(covers((0, 0), 4, (3, 3)));
    assert!(!covers((0, 0), 4, (4, 3)));
}

#[test]
fn covered_tiles_matches_covers() {
    for r in 1..=12u32 {
        let list = covered_tiles((0, 0), r);
        assert!(list.windows(2).all(|w| w[0] < w[1]), "sorted");
        for x in -15..15 {
            for y in -15..15 {
                assert_eq!(list.contains(&(x, y)), covers((0, 0), r, (x, y)));
            }
        }
    }
}

#[test]
fn a_footprint_corner_is_enough() {
    assert!(!covers_any((0, 0), 4, &footprint_cells_at((4, 3), 2)));
    assert!(covers_any((0, 0), 4, &footprint_cells_at((3, 3), 2)));
    // The anchor is outside the circle; only a far cell of the footprint is in.
    assert!(!covers((0, 0), 4, (-5, 0)));
    assert!(covers_any((0, 0), 4, &footprint_cells_at((-5, 0), 2)));
}
