//! The sector database: loading it, refusing an unplayable or unreadable
//! one, and the census over the sectors the game actually ships.
//!
//! Nothing here touches `resources::GameRng`. A sector's shape is a property
//! of the *place*, and world generation must never draw from the shared
//! stream — see `CLAUDE.md`'s entry on it.

use crate::sectors::{SectorDb, SectorDef};
use crate::tests::support::{ScratchAssets, scratch_assets_dir, test_assets_dir};
use crate::world::SectorShape;

/// A scratch sector directory holding `files` as `(filename, body)`.
///
/// Built on the `ScratchAssets` RAII guard rather than a hand-rolled `/tmp`
/// path, for the reason `descriptions::bank_dir` records: a panic between
/// creation and a manual cleanup call leaks the directory, and `Drop` runs
/// on an unwind.
fn sector_dir(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

/// A well-formed sector: a cold one, raising the Static Field floor.
const COLD: &str = r#"(
    id: "cold",
    name: "Cold Storage",
    description: "Frost across every surface.",
    shape: (static_temperature: 1.15),
    palette: (ground_hue: 205.0, hazard_hue: 12.0),
)"#;

#[test]
fn a_well_formed_sector_resolves_its_deltas_onto_neutral() {
    let dir = sector_dir("sector_ok", &[("cold.ron", COLD)]);
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    let def = db.all().next().expect("the sector should have loaded");
    let shape = def.shape();
    let neutral = SectorShape::NEUTRAL;
    assert_eq!(
        shape.static_temperature,
        neutral.static_temperature + 1.15,
        "the delta must be applied on top of NEUTRAL, not replace it"
    );
    // Every threshold the file said nothing about stays neutral, which is
    // what makes a sector authorable by naming one number.
    assert_eq!(shape.void_elevation, neutral.void_elevation);
    assert_eq!(shape.black_ice_elevation, neutral.black_ice_elevation);
    assert_eq!(shape.null_temperature, neutral.null_temperature);
    assert_eq!(shape.null_moisture, neutral.null_moisture);
    assert_eq!(shape.mainframe_moisture, neutral.mainframe_moisture);
}

#[test]
fn a_malformed_sector_file_is_skipped_and_the_others_still_load() {
    let dir = sector_dir(
        "sector_malformed",
        &[("cold.ron", COLD), ("broken.ron", "( id: \"oops\" ")],
    );
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("broken.ron"),
        "the warning must name the file: {}",
        warnings[0]
    );
    assert_eq!(db.all().count(), 1, "the good file must still be loaded");
}

#[test]
fn a_ground_hue_outside_the_cool_band_is_refused() {
    let hot_ground = COLD.replace("ground_hue: 205.0", "ground_hue: 20.0");
    let dir = sector_dir("sector_hot_ground", &[("cold.ron", &hot_ground)]);
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert_eq!(db.all().count(), 0, "the sector must be skipped");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("ground hue"),
        "the warning must say which hue: {}",
        warnings[0]
    );
}

#[test]
fn a_hazard_hue_outside_the_warm_band_is_refused() {
    let cool_hazard = COLD.replace("hazard_hue: 12.0", "hazard_hue: 190.0");
    let dir = sector_dir("sector_cool_hazard", &[("cold.ron", &cool_hazard)]);
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert_eq!(db.all().count(), 0, "the sector must be skipped");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("hazard hue"),
        "the warning must say which hue: {}",
        warnings[0]
    );
}

/// A sector with almost no ground is a stranded run, not merely an ugly one:
/// `enter_next_zone` calls `find_walkable_start` on the new map, and every
/// spawn, structure and Stack link refuses an unwalkable tile.
#[test]
fn a_sector_that_leaves_no_ground_to_stand_on_is_refused() {
    let drowned = COLD.replace(
        "shape: (static_temperature: 1.15)",
        "shape: (void_elevation: 1.0)",
    );
    let dir = sector_dir("sector_drowned", &[("cold.ron", &drowned)]);
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert_eq!(db.all().count(), 0, "the sector must be skipped");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("walkable"),
        "the warning must say what is wrong: {}",
        warnings[0]
    );
}

/// The same absence-is-supported property affixes and the enemy policy
/// have. An install with no sectors is the pre-sector game, and that is a
/// supported way to play rather than an accident.
#[test]
fn an_absent_directory_loads_an_empty_db_without_an_error() {
    let dir = scratch_assets_dir("sector_absent");
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert_eq!(db.all().count(), 0);
    assert!(warnings.is_empty(), "warnings were {warnings:?}");
}

/// The census over the real `assets/sectors/`: every shipped sector is
/// playable and reads correctly on the map.
///
/// The non-zero count assertion is not decoration. `for def in db.all()`
/// over an empty directory passes while asserting nothing, which reads as
/// coverage and is not — and this file is exactly the sort a later tidy-up
/// might empty.
#[test]
fn every_shipped_sector_is_loadable_playable_and_in_its_colour_bands() {
    let (db, warnings) = SectorDb::load_dir(&test_assets_dir().join("sectors")).unwrap();

    assert!(
        warnings.is_empty(),
        "a shipped sector failed to load: {warnings:?}"
    );
    let defs: Vec<&SectorDef> = db.all().collect();
    assert!(
        defs.len() >= 2,
        "the game ships {} sectors; the census is vacuous below two",
        defs.len()
    );
    for def in defs {
        assert!(
            def.fault().is_none(),
            "shipped sector {:?} is unusable: {:?}",
            def.id.as_str(),
            def.fault()
        );
        assert!(
            !def.name.is_empty() && !def.description.is_empty(),
            "shipped sector {:?} has no player-facing text",
            def.id.as_str()
        );
    }
}
