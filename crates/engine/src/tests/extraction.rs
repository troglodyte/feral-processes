//! Program extraction, phase 1: `items::DownedProgram`, its
//! `components::DownedPrograms` store, the `tools::ToolDb` catalogue, and
//! the player's `components::Tools` slots with the starter grant.
//!
//! Nothing extracts a downed program yet — no `Game::extract_program`, no
//! site leaves one at a kill. This file is the object, the store and the
//! tool kit in isolation; later phases append to it rather than starting a
//! sibling file. See
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`.

use super::support::*;
use crate::items::DownedProgram;
use crate::tools::ToolId;
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

#[test]
fn tool_slots_grow_one_a_step_from_a_full_base_slot() {
    // `tools::player_tool_slots` has its own unit test over the raw formula;
    // this is the shape assertion from the seat of a real save/`Game`
    // rather than the arithmetic — the controller's ruling was one slot at
    // base and one per step, deliberately not `abilities::
    // ROUTINE_SLOTS_PER_STEP`'s two, since a level-1 player holding a spare
    // slot beside the starter tool would have nothing yet to choose
    // between.
    assert_eq!(
        crate::tools::player_tool_slots(1),
        1,
        "the starter tool fills the only slot at level 1"
    );
    assert_eq!(
        crate::tools::player_tool_slots(tuning::TOOL_SLOT_PER_LEVEL),
        2,
        "one slot at the first step, not routines' two"
    );
    assert_eq!(
        crate::tools::player_tool_slots(9_999),
        tuning::TOOL_SLOT_CAP as usize,
        "never above the modest cap, however high level climbs"
    );
}

#[test]
fn a_new_game_starts_with_the_starter_tool_in_slot_one() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let installed = game.installed_tools();
    assert_eq!(
        installed.len(),
        1,
        "exactly the base slot, filled — nothing else grants a tool yet"
    );
    assert_eq!(installed[0].id, ToolId(tuning::STARTER_TOOL_ID.to_string()));
}

#[test]
fn game_load_never_re_grants_the_starter_tool() {
    // The profile rule: pay at `Game::new`, never at `Game::load`. Emptying
    // the loadout by hand stands in for a future `uninstall_tool` (phase 2)
    // — what matters here is that `load` is not a second door the starter
    // grant can walk back through.
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Tools>(player).unwrap().0.clear();
    assert!(game.installed_tools().is_empty());

    let path =
        std::env::temp_dir().join(format!("feral_tools_no_regrant_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.installed_tools().is_empty(),
        "a load must never re-grant the starter tool into an emptied loadout"
    );
}

#[test]
fn a_tool_loadout_survives_a_save_load_round_trip() {
    // Save -> load, not a RON round trip — `PlayerSave::tools` is
    // `#[serde(default)]`, and only `Game::save`/`Game::load` exercise the
    // path that would silently drop a skipped field.
    let mut game = Game::new(2210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = vec![
        ToolId("salvage_clamp".to_string()),
        ToolId("core_tap".to_string()),
    ];
    game.world.get_mut::<Tools>(player).unwrap().0 = held.clone();

    let path =
        std::env::temp_dir().join(format!("feral_tools_roundtrip_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded.world.get::<Tools>(loaded.player_entity()).unwrap();
    assert_eq!(
        restored.0, held,
        "a two-tool loadout must come back in the same slot order"
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
