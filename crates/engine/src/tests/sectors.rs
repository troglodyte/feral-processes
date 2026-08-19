//! The sector database: loading it, refusing an unplayable or unreadable
//! one, and the census over the sectors the game actually ships.
//!
//! Nothing here touches `resources::GameRng`. A sector's shape is a property
//! of the *place*, and world generation must never draw from the shared
//! stream — see `CLAUDE.md`'s entry on it.

use crate::sectors::{SectorDb, SectorDef, for_zone};
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

/// A well-formed sector: a cold one, raising the Deadlock floor.
const COLD: &str = r#"(
    id: "cold",
    name: "Cold Storage",
    description: "Frost across every surface.",
    shape: (deadlock_temperature: 1.15),
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
        shape.deadlock_temperature,
        neutral.deadlock_temperature + 1.15,
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
        "shape: (deadlock_temperature: 1.15)",
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

// ---------------------------------------------------------------------------
// Which sector a zone gets
// ---------------------------------------------------------------------------
//
// Derived from `(world seed, zone)`, never stored. These live here rather
// than in `tests/zone.rs` because none of them needs a `Game`: the
// derivation is a pure function of two saved numbers and the pool, which is
// exactly the property that lets it survive a reload with no save-format
// change.

/// The shipped pool, which is what the derivation is actually asked about.
fn shipped() -> SectorDb {
    let (db, warnings) = SectorDb::load_dir(&test_assets_dir().join("sectors")).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    db
}

/// Not politeness. `in_opening_ring` fields only species a fresh player can
/// beat, and `habitat_pools` falls back to the biome's *unfiltered* roster
/// when nothing qualifies — so a sector biasing zone 1's biome mix would
/// move the opening roster while looking like a cosmetic change, and could
/// empty the ring while leaving it looking intact.
#[test]
fn zone_one_is_always_neutral_whatever_the_seed() {
    let db = shipped();
    for seed in 0..500u32 {
        assert!(
            for_zone(seed, 1, &db).is_none(),
            "seed {seed} gave zone 1 a sector"
        );
    }
}

/// The whole point of deriving rather than storing: ask twice, get the same
/// answer. A reload is the case that matters, and it is this plus the fact
/// that both inputs are already saved.
#[test]
fn the_same_seed_and_zone_always_derive_the_same_sector() {
    let db = shipped();
    for seed in [1u32, 42, 9001, 0xDEAD_BEEF] {
        for zone in 2..=12u32 {
            let first = for_zone(seed, zone, &db).map(|d| d.id.clone());
            for _ in 0..5 {
                assert_eq!(
                    for_zone(seed, zone, &db).map(|d| d.id.clone()),
                    first,
                    "seed {seed} zone {zone} is not stable"
                );
            }
        }
    }
}

/// The anti-correlation trap `descriptions.rs` hit, reached by a shorter
/// route: one XOR-then-multiply round carries a difference only about the
/// prime's own width upward, so a zone folded in as a single 64-bit word
/// differs nowhere near bit 63 — which is the bit `derive::index` reads.
/// Every zone of a run then lands in the same sector while each individual
/// answer still looks arbitrary, which is what makes it look like a working
/// feature from inside any one zone.
///
/// Measured, not theorised: with the zone folded as one word, seed 1 sends
/// all of zones 2..=20 to a single sector. This test fails against that.
///
/// What it does **not** catch is `%` in place of `derive::index`. That
/// reduction is sound here for the wrong reason — this fold ends on a
/// multiply, so bit 0 does vary with the zone — and the protection against
/// it is structural: `index` is shared rather than copied, and its own doc
/// and `descriptions::every_pair_of_slots_is_independent` carry the
/// argument. Do not read this test as covering the reducer.
#[test]
fn one_seed_does_not_send_every_zone_to_the_same_sector() {
    let db = shipped();
    let pool = db.all().count();
    assert!(pool >= 2, "a pool of {pool} cannot show correlation");

    for seed in [1u32, 7, 42, 1000, 65535, 0xDEAD_BEEF] {
        let seen: std::collections::BTreeSet<String> = (2..=20u32)
            .filter_map(|z| for_zone(seed, z, &db).map(|d| d.id.as_str().to_string()))
            .collect();
        assert!(
            seen.len() >= 2,
            "seed {seed} sent all of zones 2..=20 to {seen:?} — the zone number is \
             not reaching the bits `derive::index` reads"
        );
    }
}

/// The other half of the same trap, read across seeds instead of across
/// zones: one zone must not be pinned to one sector for every world.
#[test]
fn one_zone_does_not_send_every_seed_to_the_same_sector() {
    let db = shipped();
    let pool = db.all().count();
    for zone in [2u32, 3, 7] {
        let seen: std::collections::BTreeSet<String> = (0..400u32)
            .filter_map(|s| for_zone(s, zone, &db).map(|d| d.id.as_str().to_string()))
            .collect();
        assert_eq!(
            seen.len(),
            pool,
            "zone {zone} reached only {seen:?} across 400 seeds, of {pool} sectors"
        );
    }
}

/// Absence is supported: an install with no `assets/sectors/` is the
/// pre-sector game at every zone, not just at zone 1.
#[test]
fn an_empty_db_leaves_every_zone_neutral() {
    let db = SectorDb::default();
    for zone in 1..=30u32 {
        assert!(for_zone(4242, zone, &db).is_none(), "zone {zone}");
    }
}

/// A sector authored before the biome was renamed names its threshold
/// `deadlock_temperature`. The rename is a `serde(alias)` precisely so a
/// third-party sector file does not silently lose its threshold and start
/// generating neutral ground under a name that promises cold.
#[test]
fn a_sector_authored_with_the_old_threshold_key_still_applies_it() {
    const OLD_KEY: &str = r#"(
    id: "cold",
    name: "Cold Storage",
    description: "Frost across every surface.",
    shape: (static_temperature: 1.15),
)"#;
    let dir = sector_dir("sector_old_key", &[("cold.ron", OLD_KEY)]);
    let (db, warnings) = SectorDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    let def = db.all().next().expect("the sector should have loaded");
    assert_eq!(
        def.shape().deadlock_temperature,
        SectorShape::NEUTRAL.deadlock_temperature + 1.15,
        "the old key must still reach the renamed threshold"
    );
}
