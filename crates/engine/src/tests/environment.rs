//! The environment database: loading it, refusing a file that would make
//! the game unplayable, and the one reader that resolves a tile to an
//! effect.
//!
//! Nothing here draws from `resources::GameRng`. Ambient ground is a
//! property of the *place*, resolved from the biome every time it is asked
//! rather than rolled or stored.

use crate::environment::{EnvironmentDb, EnvironmentEffect};
use crate::tests::support::{ScratchAssets, assets_dir_with_environment, scratch_assets_dir};
use crate::tuning::{MAX_ENVIRONMENT_ATTRITION, MAX_ENVIRONMENT_DRAG_TICKS};
use crate::world::Biome;

/// A scratch environment directory holding `files` as `(filename, body)`.
///
/// Built on the `ScratchAssets` RAII guard rather than a hand-rolled `/tmp`
/// path: a panic between creation and a manual cleanup call leaks the
/// directory, and `Drop` runs on an unwind.
fn env_dir(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

const COLD: &str = r#"(
    id: "cold",
    name: "Standing Frost",
    description: "The floor pulls heat out of anything that stops on it.",
    biomes: [Deadlock],
    effect: Attrition(hp_percent: 0.02, min_damage: 1),
)"#;

#[test]
fn a_well_formed_environment_file_loads_and_answers_for_its_biome() {
    let dir = env_dir("env_ok", &[("cold.ron", COLD)]);
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    let def = db.for_biome(Biome::Deadlock).expect("the file claimed it");
    assert_eq!(def.id, "cold");
    assert_eq!(
        def.effect,
        EnvironmentEffect::Attrition {
            hp_percent: 0.02,
            min_damage: 1
        }
    );
    assert!(
        db.for_biome(Biome::OpenGrid).is_none(),
        "an unclaimed biome is neutral ground"
    );
}

#[test]
fn a_malformed_environment_file_is_skipped_and_the_others_still_load() {
    let dir = env_dir(
        "env_bad",
        &[("cold.ron", COLD), ("broken.ron", "(id: \"broken\"")],
    );
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("broken"), "{warnings:?}");
    assert!(
        db.for_biome(Biome::Deadlock).is_some(),
        "one bad mod file must not take the rest of the directory with it"
    );
}

/// The base slab is the one safe ground in the game — nothing spawns there
/// and no ambush fires. That is not a file's decision to revoke.
#[test]
fn an_environment_file_claiming_the_base_slab_is_refused() {
    const SLAB: &str = r#"(
    id: "slab",
    name: "Bad Idea",
    description: "Ground that bites where the base stands.",
    biomes: [Platform],
    effect: Attrition(hp_percent: 0.01, min_damage: 1),
)"#;
    let dir = env_dir("env_slab", &[("slab.ron", SLAB)]);
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("Platform"), "{warnings:?}");
    assert!(db.for_biome(Biome::Platform).is_none());
}

#[test]
fn an_environment_file_over_either_ceiling_is_refused() {
    let bite = format!(
        r#"(
    id: "bite",
    name: "Far Too Much",
    description: "Death in two steps.",
    biomes: [Deadlock],
    effect: Attrition(hp_percent: {}, min_damage: 1),
)"#,
        MAX_ENVIRONMENT_ATTRITION + 0.01
    );
    let drag = format!(
        r#"(
    id: "drag",
    name: "Far Too Slow",
    description: "A step that never finishes.",
    biomes: [NullSector],
    effect: Drag(extra_ticks: {}),
)"#,
        MAX_ENVIRONMENT_DRAG_TICKS + 1
    );
    let dir = env_dir("env_ceiling", &[("bite.ron", &bite), ("drag.ron", &drag)]);
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(db.for_biome(Biome::Deadlock).is_none());
    assert!(db.for_biome(Biome::NullSector).is_none());
}

/// Directory order is not stable across platforms, so resolving a clash
/// silently by whichever file was read first would make a modder's game
/// differ from the one they tested. It is an authoring error and it says so,
/// naming both files.
#[test]
fn two_files_claiming_one_biome_refuse_the_second_and_name_both() {
    const RIVAL: &str = r#"(
    id: "rival",
    name: "Also Frost",
    description: "The same ground, claimed twice.",
    biomes: [Deadlock],
    effect: Drag(extra_ticks: 1),
)"#;
    let dir = env_dir("env_clash", &[("a_cold.ron", COLD), ("b_rival.ron", RIVAL)]);
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("cold"), "{warnings:?}");
    assert!(warnings[0].contains("rival"), "{warnings:?}");
    let def = db
        .for_biome(Biome::Deadlock)
        .expect("the first still loads");
    assert_eq!(def.id, "cold");
}

/// A hole in the map is unreachable, not wrong. A mod naming all six biomes
/// for convenience must not be nagged about the two nobody can stand on.
#[test]
fn an_environment_file_claiming_an_unwalkable_biome_loads_without_complaint() {
    const HOLES: &str = r#"(
    id: "holes",
    name: "Nowhere",
    description: "Ground that cannot be reached.",
    biomes: [DataVoid, BlackIce],
    effect: Drag(extra_ticks: 1),
)"#;
    let dir = env_dir("env_holes", &[("holes.ron", HOLES)]);
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert!(db.for_biome(Biome::BlackIce).is_some());
}

/// Deleting `assets/environment/` must restore today's game exactly, the
/// same supported way deleting `assets/sectors/` already is.
#[test]
fn an_absent_environment_directory_loads_silently_to_an_empty_db() {
    let dir = scratch_assets_dir("env_absent");
    let (db, warnings) = EnvironmentDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    for biome in [
        Biome::DataVoid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::BlackIce,
        Biome::Platform,
    ] {
        assert!(db.for_biome(biome).is_none());
    }
}

// ---------------------------------------------------------------- the reader

use crate::resources::ZoneLevel;
use crate::world::{Tile, WorldMap};
use crate::{DifficultyMode, Game};

/// A game standing on `biome` at `zone`, with the environment directory
/// holding exactly `files`.
fn game_standing_on(tag: &str, files: &[(&str, &str)], zone: u32, biome: Biome) -> Game {
    let dir = assets_dir_with_environment(tag, files);
    let mut game = Game::new(16, DifficultyMode::Forgiving, &dir).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = zone;
    let pos = *game
        .world
        .get::<crate::components::Position>(game.player_entity())
        .unwrap();
    game.world.resource_mut::<WorldMap>().set_override(
        pos.x,
        pos.y,
        Tile {
            biome,
            walkable: true,
        },
    );
    game
}

fn player_tile(game: &Game) -> (i32, i32) {
    let pos = *game
        .world
        .get::<crate::components::Position>(game.player_entity())
        .unwrap();
    (pos.x, pos.y)
}

#[test]
fn ground_effect_answers_for_a_claimed_biome_past_zone_one() {
    let mut game = game_standing_on("env_read", &[("cold.ron", COLD)], 2, Biome::Deadlock);
    let (x, y) = player_tile(&game);

    assert_eq!(
        game.ground_effect(x, y).map(|d| d.id.as_str()),
        Some("cold")
    );
}

/// The gate lives inside `ground_effect` so it cannot lapse at a second
/// call site. A test that read the db directly would be asserting about
/// the wrong thing entirely.
#[test]
fn ground_effect_is_empty_at_zone_one() {
    let mut game = game_standing_on("env_zone1", &[("cold.ron", COLD)], 1, Biome::Deadlock);
    let (x, y) = player_tile(&game);

    assert!(game.ground_effect(x, y).is_none());
}

#[test]
fn ground_effect_never_answers_for_the_base_slab() {
    let mut game = game_standing_on("env_slab_read", &[("cold.ron", COLD)], 2, Biome::Platform);
    let (x, y) = player_tile(&game);

    assert!(game.ground_effect(x, y).is_none());
}

#[test]
fn ground_effect_is_empty_for_an_unclaimed_biome() {
    let mut game = game_standing_on("env_unclaimed", &[("cold.ron", COLD)], 2, Biome::OpenGrid);
    let (x, y) = player_tile(&game);

    assert!(game.ground_effect(x, y).is_none());
}
