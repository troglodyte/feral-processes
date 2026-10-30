use super::*;

fn entry(text: &str, repeats: usize) -> LogEntry {
    LogEntry {
        kind: MessageKind::Info,
        source: MessageSource::Field,
        text: text.to_string(),
        repeats,
    }
}

fn suffix_of(row: &Row) -> Option<&str> {
    match row {
        Row::Item { suffix, .. } => suffix.as_deref(),
        _ => None,
    }
}

/// The count is an annotation on the row, not part of the sentence — so it
/// only appears where there is something to count.
#[test]
fn a_folded_row_carries_its_count_and_a_lone_row_carries_none() {
    let entries = vec![entry("extracted 2 Data Shard.", 14), entry("a raid", 1)];
    let rows = history_rows(&entries, 0);
    assert_eq!(rows.len(), 2, "one row per entry, folded or not");
    assert_eq!(suffix_of(&rows[0]), Some("×14"));
    assert_eq!(suffix_of(&rows[1]), None);
}

#[test]
fn an_empty_history_says_so_instead_of_listing_nothing() {
    let rows = history_rows(&[], 0);
    assert_eq!(rows.len(), 1);
    assert!(matches!(rows[0], Row::Text(_)), "nothing to scroll to");
}

#[test]
fn tile_shade_is_stable_for_a_given_world_coordinate() {
    // The camera slides continuously over a tile now. A shade derived
    // from anything but the world coordinate would shimmer as it went.
    assert_eq!(tile_shade((12, -7)), tile_shade((12, -7)));
}

#[test]
fn tile_shade_stays_within_the_jitter_band() {
    for x in -60..60 {
        for y in -60..60 {
            let s = tile_shade((x, y));
            assert!(
                (1.0 - SHADE_JITTER..=1.0 + SHADE_JITTER).contains(&s),
                "shade {s} at ({x}, {y}) escaped the band"
            );
        }
    }
}

#[test]
fn tile_shade_actually_varies_between_neighbours() {
    let row: Vec<f32> = (0..40).map(|x| tile_shade((x, 5))).collect();
    let first = row[0];
    assert!(
        row.iter().any(|s| (s - first).abs() > SHADE_JITTER / 4.0),
        "a whole row came out flat — the hash isn't spreading"
    );
}

/// A hash that treats the axes alike bands the map along the diagonal,
/// which reads as a pattern rather than as texture.
#[test]
fn tile_shade_distinguishes_the_two_axes() {
    assert_ne!(tile_shade((3, 9)), tile_shade((9, 3)));
}

#[test]
fn the_vignette_leaves_the_centre_of_the_pane_untouched() {
    assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL), 1.0);
}

#[test]
fn the_vignette_bottoms_out_at_its_floor_and_never_below() {
    let f = VIGNETTE_FLOOR_FULL;
    assert!((vignette(400.0, 0.0, 400.0, 300.0, f) - f).abs() < 1e-6);
    // The corners sit past the unit radius and must clamp rather than
    // keep darkening.
    assert!(vignette(400.0, 300.0, 400.0, 300.0, f) >= f);
    assert!(vignette(9999.0, 9999.0, 400.0, 300.0, f) >= f);
}

#[test]
fn the_vignette_darkens_monotonically_outward() {
    let mut previous = f32::MAX;
    for i in 0..=20 {
        let v = vignette(i as f32 * 20.0, 0.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL);
        assert!(
            v <= previous,
            "brightened at step {i}: {v} after {previous}"
        );
        previous = v;
    }
}

/// Normalising by the pane's half-extent is what keeps the gradient the
/// same shape at every zoom step and window size.
#[test]
fn the_vignette_depends_on_position_within_the_pane_not_on_its_size() {
    let small = vignette(100.0, 75.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL);
    let large = vignette(200.0, 150.0, 800.0, 600.0, VIGNETTE_FLOOR_FULL);
    assert!((small - large).abs() < 1e-6, "{small} vs {large}");
}

/// The whole of what Power buys: a full reserve is the vignette this map
/// has always drawn, and draining it deepens the corners and nothing
/// else.
#[test]
fn the_vignette_floor_deepens_as_the_reserve_drains() {
    assert_eq!(vignette_floor(POWER_MAX, POWER_MAX), VIGNETTE_FLOOR_FULL);
    assert_eq!(vignette_floor(0.0, POWER_MAX), VIGNETTE_FLOOR_EMPTY);

    let mut previous = f32::MAX;
    for i in 0..=10 {
        let f = vignette_floor(POWER_MAX * (10 - i) as f32 / 10.0, POWER_MAX);
        assert!(
            f <= previous,
            "brightened at step {i}: {f} after {previous}"
        );
        previous = f;
    }
}

/// A wide reserve at 100 is not full: the fraction is against the
/// entity's own maximum, never the constant.
#[test]
fn the_vignette_floor_reads_the_reserve_against_its_own_maximum() {
    assert!(vignette_floor(100.0, 200.0) < VIGNETTE_FLOOR_FULL);
    assert_eq!(vignette_floor(60.0, 60.0), VIGNETTE_FLOOR_FULL);
}

/// `PowerReserve` clamps itself, but this reads a `PlayerStatus` field
/// rather than the type, and a floor that ran past either constant would
/// either blow out the centre or drive the corners toward black.
#[test]
fn the_vignette_floor_is_clamped_at_both_ends() {
    assert_eq!(
        vignette_floor(POWER_MAX * 4.0, POWER_MAX),
        VIGNETTE_FLOOR_FULL
    );
    assert_eq!(vignette_floor(-40.0, POWER_MAX), VIGNETTE_FLOOR_EMPTY);
}

/// It stays a *vignette* at every reserve: the pane's middle is untouched
/// whatever the floor, so a drained reserve costs the player nothing at
/// the centre of their own attention. A version that dimmed the whole
/// pane would be a wash, and would read as the renderer having faulted
/// rather than as the reserve running down.
#[test]
fn a_drained_reserve_darkens_the_pane_edge_and_not_its_centre() {
    let full = vignette_floor(POWER_MAX, POWER_MAX);
    let empty = vignette_floor(0.0, POWER_MAX);

    assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, full), 1.0);
    assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, empty), 1.0);
    assert!(
        vignette(400.0, 0.0, 400.0, 300.0, empty) < vignette(400.0, 0.0, 400.0, 300.0, full),
        "an empty reserve must darken the edge"
    );
}
