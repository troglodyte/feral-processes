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

/// Writes `files` as `.ron` into a fresh temp dir and loads a `ToolDb` from
/// it. Duplicated rather than shared with `abilities`'s own version of this
/// helper: that one is private to its module's `#[cfg(test)]` block.
fn load_tools(tag: &str, files: &[(&str, &str)]) -> (ToolDb, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("feral_tools_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
    }
    let result = ToolDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

const VALID_TOOL: &str = r#"(
    id: "test_good_tool",
    name: "Test Good Tool",
    description: "d",
    category: Materials,
    yields: [("core_fragment", 1.0)],
    tier: 1,
    ticks: 5,
)"#;

/// `ToolDb::load_dir` is the second `load_dir` this phase writes, alongside
/// `items::DownedProgram`'s own store — see `AffixDb::load_dir`'s rule,
/// which `ToolDb` follows rather than `AbilityDb::load_dir`'s: abilities are
/// mandatory content and refuse a missing directory outright, but a tool
/// catalogue has no floor to enforce yet.
#[test]
fn an_absent_tools_directory_loads_silently_empty() {
    let dir = std::env::temp_dir().join(format!("feral_tools_absent_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!dir.exists(), "the directory must genuinely not exist");

    let (db, warnings) = ToolDb::load_dir(&dir).unwrap();
    assert!(
        db.all().next().is_none(),
        "an absent directory must load no tools"
    );
    assert!(
        warnings.is_empty(),
        "an absent directory must warn about nothing: {warnings:?}"
    );
}

/// Mirrors `abilities`'s own malformed-file coverage: a file that fails to
/// parse is skipped with a warning naming it, and its well-formed neighbour
/// still loads — never a panic that would take the whole game down over one
/// bad mod file.
#[test]
fn a_malformed_tool_file_is_skipped_while_its_neighbour_still_loads() {
    let (db, warnings) = load_tools(
        "malformed",
        &[("good", VALID_TOOL), ("bad", "not valid ron at all {{{")],
    );
    assert!(
        db.get("test_good_tool").is_some(),
        "the well-formed neighbour must still load"
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("bad"),
        "the warning should name the bad file: {}",
        warnings[0]
    );
}
