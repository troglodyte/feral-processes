//! Program extraction, phase 1: `items::DownedProgram`, its
//! `components::DownedPrograms` store, and the save field that carries it.
//!
//! Nothing consumes a downed program yet — no tool, no `extract_program`,
//! no site leaves one at a kill. This file is the object and the store in
//! isolation; later phases append to it rather than starting a sibling
//! file. See `docs/superpowers/specs/2026-09-04-program-extraction-design.md`.

use super::support::*;
use crate::items::DownedProgram;
use crate::*;

fn program(condition: u8, rarity: Rarity, level: u32) -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity,
        boss: false,
        condition,
    }
}

#[test]
fn grade_rises_monotonically_with_each_axis_held_fixed() {
    // Condition, rarity and level fixed in turn while the other two hold
    // still — `DownedProgram::grade` must move the same direction as each
    // axis in isolation, or a yield formula built on it (a later phase)
    // could reward a worse-condition, lower-level or more-common program
    // over a better one.
    let by_condition: Vec<f32> = [10u8, 40, 70, 100]
        .into_iter()
        .map(|condition| program(condition, Rarity::Gold, 20).grade())
        .collect();
    assert!(
        by_condition.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with condition alone: {by_condition:?}"
    );

    let by_rarity: Vec<f32> = Rarity::ALL
        .into_iter()
        .map(|rarity| program(70, rarity, 20).grade())
        .collect();
    assert!(
        by_rarity.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with rarity's rung alone: {by_rarity:?}"
    );

    let by_level: Vec<f32> = [1u32, 10, 30, 80]
        .into_iter()
        .map(|level| program(70, Rarity::Gold, level).grade())
        .collect();
    assert!(
        by_level.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with level alone: {by_level:?}"
    );
}

#[test]
fn downed_programs_survive_a_save_load_round_trip() {
    // Save -> load, not a RON round trip: `PlayerSave::downed_programs` is
    // `#[serde(default)]`, and a RON round trip can't catch a field that
    // silently defaults away — only `Game::save`/`Game::load` exercise the
    // path that would drop it.
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = vec![
        program(15, Rarity::Ordinary, 3),
        program(88, Rarity::Prismatic, 47),
        DownedProgram {
            species: "sentinel".to_string(),
            level: 12,
            rarity: Rarity::Silver,
            boss: true,
            condition: 60,
        },
    ];
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = held.clone();

    let path = std::env::temp_dir().join(format!(
        "feral_downed_programs_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .world
        .get::<DownedPrograms>(loaded.player_entity())
        .unwrap();
    assert_eq!(
        restored.0, held,
        "three distinct downed programs must come back exactly as saved"
    );
}
