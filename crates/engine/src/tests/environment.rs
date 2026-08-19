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

/// Attrition with room to mitigate and enough bite to be lethal from 1 HP.
const HARSH: &str = r#"(
    id: "harsh",
    name: "Hard Frost",
    description: "The floor takes what it can get.",
    biomes: [Deadlock],
    effect: Attrition(hp_percent: 0.05, min_damage: 4),
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

// ------------------------------------------------------------- the hook

use crate::components::{ActiveFieldBuff, BuffSource, FieldBuffKind, Position, Stats};
use crate::resources::{MessageLog, Party};

/// Stands the player on `from` with `to` one step east, both written
/// through the override overlay, and clears anything squatting on the
/// destination — walking into a program, a nest or a structure is a fight
/// or a door, not travel.
fn step_from_onto(game: &mut Game, from: Biome, to: Biome, to_walkable: bool) {
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);
    let squatters: Vec<crate::Entity> = {
        let mut q = game.world.query::<(crate::Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != player && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    let mut map = game.world.resource_mut::<WorldMap>();
    map.set_override(
        pos.x,
        pos.y,
        Tile {
            biome: from,
            walkable: true,
        },
    );
    map.set_override(
        nx,
        ny,
        Tile {
            biome: to,
            walkable: to_walkable,
        },
    );
}

fn player_hp(game: &Game) -> i32 {
    game.world.get::<Stats>(game.player_entity()).unwrap().hp
}

/// A game past zone 1 with `files` installed, standing on Open Grid with
/// one step east onto `onto`.
fn game_about_to_step(tag: &str, files: &[(&str, &str)], onto: Biome) -> Game {
    let dir = assets_dir_with_environment(tag, files);
    let mut game = Game::new(16, DifficultyMode::Forgiving, &dir).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::OpenGrid, onto, true);
    game
}

#[test]
fn a_step_onto_attrition_ground_costs_integrity() {
    let mut game = game_about_to_step("env_bite", &[("cold.ron", COLD)], Biome::Deadlock);
    let max_hp = game
        .world
        .get::<Stats>(game.player_entity())
        .unwrap()
        .max_hp;
    let before = player_hp(&game);

    game.move_player(1, 0);

    let expected = ((max_hp as f32 * 0.02).round() as i32).max(1);
    assert_eq!(before - player_hp(&game), expected);
}

#[test]
fn a_step_that_bounces_off_a_wall_costs_no_integrity() {
    let dir = assets_dir_with_environment("env_bite_wall", &[("cold.ron", COLD)]);
    let mut game = Game::new(16, DifficultyMode::Forgiving, &dir).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    // Unwalkable Deadlock is unreachable on a generated map, which is the
    // point: what is under test is that the bite rides the *step*, and the
    // only way to hold everything else steady is to make the tile refuse.
    step_from_onto(&mut game, Biome::OpenGrid, Biome::Deadlock, false);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before, "shoving at a wall is not travel");
}

/// The player alone takes environment damage. Corrupting the party would
/// route program deaths — and, on Permadeath, the run-ending path — through
/// something that is not a fight.
#[test]
fn attrition_never_touches_the_party() {
    let mut game = game_about_to_step("env_party", &[("cold.ron", COLD)], Biome::Deadlock);
    let member = crate::tests::support::spawn_tamed(&mut game, 10, 3);
    game.add_companion(member).unwrap();
    let before = game.world.get::<Stats>(member).unwrap().hp;
    assert!(game.world.resource::<Party>().0.contains(&member));

    game.move_player(1, 0);

    assert_eq!(game.world.get::<Stats>(member).unwrap().hp, before);
}

/// Free only because the bite goes through `Game::apply_damage`, the one
/// code path that lowers a creature's HP. This test is what stops someone
/// "simplifying" the hook into a direct write to `Stats::hp`.
#[test]
fn a_mitigation_buff_reduces_the_bite() {
    let mut game = game_about_to_step("env_mitigated", &[("harsh.ron", HARSH)], Biome::Deadlock);
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 400;
        stats.hp = 400;
    }
    let unmitigated = {
        let mut probe = game_about_to_step("env_probe", &[("harsh.ron", HARSH)], Biome::Deadlock);
        let p = probe.player_entity();
        {
            let mut stats = probe.world.get_mut::<Stats>(p).unwrap();
            stats.max_hp = 400;
            stats.hp = 400;
        }
        probe.move_player(1, 0);
        400 - player_hp(&probe)
    };
    assert!(unmitigated > 1, "the fixture has to have room to mitigate");

    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Ablative Layer".into(),
            power: 50,
            remaining: 100,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    game.move_player(1, 0);

    assert!(
        400 - player_hp(&game) < unmitigated,
        "mitigation must apply to the ground the same way it applies to a hit"
    );
}

/// The one place in this phase where two systems meet at a lethal edge:
/// ground that kills must not then roll an ambush onto the corpse.
#[test]
fn attrition_that_kills_does_not_then_start_an_ambush() {
    // Permadeath, because a Forgiving death reboots the player to full
    // Integrity inside the very tick this is asserting about — the corpse
    // the test needs to look at would be gone before it could.
    let dir = assets_dir_with_environment("env_lethal", &[("harsh.ron", HARSH)]);
    let mut game = Game::new(16, DifficultyMode::Permadeath, &dir).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::OpenGrid, Biome::Deadlock, true);
    {
        let mut stats = game.world.get_mut::<Stats>(game.player_entity()).unwrap();
        stats.hp = 1;
    }

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), 0);
    assert!(
        !game.has_active_battle(),
        "a fight started against a dead player is unwinnable and unloseable"
    );
}

/// Both halves in one test on purpose: the effect half alone passes against
/// a bare early return that also swallowed the name.
#[test]
fn zone_one_takes_no_bite_but_still_names_the_ground() {
    let dir = assets_dir_with_environment("env_zone1_hook", &[("cold.ron", COLD)]);
    let mut game = Game::new(16, DifficultyMode::Forgiving, &dir).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    step_from_onto(&mut game, Biome::OpenGrid, Biome::Deadlock, true);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before, "zone 1 is neutral ground");
    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(Biome::Deadlock.name())),
        "the ground is named from the first step of a run"
    );
}
