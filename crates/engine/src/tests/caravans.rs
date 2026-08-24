//! Periodic caravan traders: the catalogue, the derived schedule and shelf,
//! the journey in and out, and what the player may take off it.

use super::support::*;
use crate::caravans::{CaravanDb, CaravanDef};

fn caravans_dir() -> std::path::PathBuf {
    test_assets_dir().join("caravans")
}

fn shipped_db() -> CaravanDb {
    let (db, warnings) = CaravanDb::load_dir(&caravans_dir()).unwrap();
    assert!(warnings.is_empty(), "shipped caravans warned: {warnings:?}");
    db
}

#[test]
fn the_shipped_directory_loads_clean() {
    let db = shipped_db();
    assert!(
        db.all().count() >= 2,
        "two traders is the minimum that makes 'which trader visits' mean anything"
    );
}

/// A broken file costs the game that one trader and nothing else. Written to
/// a scratch dir — mutating `assets/` is how a timed-out run once left a
/// shipped item edited.
#[test]
fn a_malformed_file_is_skipped_with_one_warning() {
    let dir = scratch_assets_dir("caravans_malformed");
    std::fs::create_dir_all(&*dir).unwrap();
    for entry in std::fs::read_dir(caravans_dir()).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|e| e.to_str()) == Some("ron") {
            std::fs::copy(entry.path(), &*dir.join(entry.file_name())).unwrap();
        }
    }
    let shipped = std::fs::read_dir(&*dir).unwrap().count();
    std::fs::write(&*dir.join("broken.ron"), "( id: \"nope\"").unwrap();

    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();

    assert_eq!(warnings.len(), 1, "one bad file, one warning: {warnings:?}");
    assert_eq!(
        db.all().count(),
        shipped,
        "every other file in the directory still loaded"
    );
}

/// A def the schema refuses is skipped the same way a syntactically broken
/// one is — `complaint` runs at load so an unusable trader is a startup
/// warning rather than an empty shelf nobody can explain.
#[test]
fn a_def_with_no_rows_or_no_weights_is_refused() {
    let dir = scratch_assets_dir("caravans_invalid");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        &*dir.join("rowless.ron"),
        "(id: \"a\", name: \"A\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 0, \
         weights: (gear: 1), min_zone: 1, max_zone: 9)",
    )
    .unwrap();
    std::fs::write(
        &*dir.join("weightless.ron"),
        "(id: \"b\", name: \"B\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 3, \
         weights: (), min_zone: 1, max_zone: 9)",
    )
    .unwrap();
    std::fs::write(
        &*dir.join("inverted.ron"),
        "(id: \"c\", name: \"C\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 3, \
         weights: (gear: 1), min_zone: 9, max_zone: 1)",
    )
    .unwrap();

    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();

    assert_eq!(db.all().count(), 0, "none of the three is usable");
    assert_eq!(warnings.len(), 3, "each says why: {warnings:?}");
}

#[test]
fn for_zone_keeps_only_the_window_and_sorts_by_id() {
    let dir = scratch_assets_dir("caravans_window");
    std::fs::create_dir_all(&*dir).unwrap();
    // Filenames deliberately in the opposite order to the ids, so a walk
    // that returned directory order rather than id order comes out wrong.
    for (file, id, lo, hi) in [
        ("z.ron", "aardvark", 1u32, 3u32),
        ("m.ron", "middle", 3, 5),
        ("a.ron", "zulu", 6, 9),
    ] {
        std::fs::write(
            &*dir.join(file),
            format!(
                "(id: \"{id}\", name: \"N\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
                 rows: 3, weights: (gear: 1), min_zone: {lo}, max_zone: {hi})"
            ),
        )
        .unwrap();
    }
    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");

    let ids = |zone| -> Vec<String> {
        db.for_zone(zone)
            .into_iter()
            .map(|d| d.id.clone())
            .collect()
    };

    assert_eq!(ids(1), vec!["aardvark"], "below the second window");
    assert_eq!(
        ids(3),
        vec!["aardvark", "middle"],
        "both windows contain 3, and the answer is in id order"
    );
    assert_eq!(ids(4), vec!["middle"]);
    assert!(ids(10).is_empty(), "past every window");
}

/// A census over the real directory. What it holds is what `complaint`
/// refuses, asserted here as well because `complaint` skipping a file is
/// silent to anyone reading the shipped set.
#[test]
fn every_shipped_caravan_is_stockable() {
    for def in shipped_db().all() {
        let CaravanDef {
            id,
            rows,
            weights,
            min_zone,
            max_zone,
            ..
        } = def;
        assert!(*rows >= 1, "{id} would stand there with nothing to sell");
        assert!(weights.gear + weights.routines + weights.programs + weights.materials > 0);
        assert!(min_zone <= max_zone, "{id}'s window is inverted");
    }
}
