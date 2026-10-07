//! The Basin Exit and the ending: the build gate, the confirmation, the
//! escape, the fallbacks.

use super::support::*;
use crate::components::PhaseKeys;
use crate::story::EndingText;
use crate::*;

const ALL_KEYS: u16 = (1 << crate::tuning::PHASE_KEY_COUNT) - 1;

fn exit_game() -> Game {
    let mut game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(crate::items::ids::PORTAL_FRAGMENT), 200);
    stand_in_base(&mut game);
    game
}

fn hold(game: &mut Game, held: u16) {
    let player = game.player_entity();
    game.world.get_mut::<PhaseKeys>(player).unwrap().held = held;
}

fn open_game() -> Game {
    let mut game = exit_game();
    hold(&mut game, ALL_KEYS);
    game.world.resource_mut::<ZoneLevel>().0 = crate::tuning::PHASE_KEY_COUNT;
    game
}

fn keys(game: &Game) -> PhaseKeys {
    *game.world.get::<PhaseKeys>(game.player_entity()).unwrap()
}

fn built_exit() -> Game {
    let mut game = open_game();
    place_now(&mut game, "basin_exit", 1, 0).unwrap();
    game
}

#[test]
fn the_exit_needs_every_key() {
    let mut game = exit_game();
    game.world.resource_mut::<ZoneLevel>().0 = 10;
    hold(&mut game, ALL_KEYS & !1);
    let err = game.place_structure("basin_exit", 1, 0, None).unwrap_err();
    assert!(err.contains("Phase Keys"), "{err}");
    assert!(find_structure_by_kind(&mut game, "basin_exit").is_none());
    hold(&mut game, ALL_KEYS);
    place_now(&mut game, "basin_exit", 1, 0).unwrap();
}

#[test]
fn the_exit_needs_zone_ten() {
    let mut game = exit_game();
    hold(&mut game, ALL_KEYS);
    game.world.resource_mut::<ZoneLevel>().0 = 9;
    assert!(game.place_structure("basin_exit", 1, 0, None).is_err());
    game.world.resource_mut::<ZoneLevel>().0 = 12;
    place_now(&mut game, "basin_exit", 1, 0).unwrap();
}

#[test]
fn the_menu_lists_the_exit_only_once_it_can_be_built() {
    let mut game = exit_game();
    let listed = |game: &Game| game.buildable_structure_defs().iter().any(|d| d.basin_exit);
    assert!(!listed(&game));
    hold(&mut game, ALL_KEYS);
    game.world.resource_mut::<ZoneLevel>().0 = 10;
    assert!(listed(&game));
}

#[test]
fn stepping_on_the_exit_asks_and_does_not_act() {
    let mut game = built_exit();
    let tick = game.world.resource::<GameClock>().tick;

    game.move_player(1, 0);

    assert!(game.take_basin_exit_prompt());
    assert!(!game.take_basin_exit_prompt(), "taking clears it");
    assert!(!keys(&game).story_complete);
    assert!(find_structure_by_kind(&mut game, "basin_exit").is_some());
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        tick,
        "asking is free"
    );
}

#[test]
fn confirming_escapes_and_removes_the_exit() {
    let mut game = built_exit();
    game.move_player(1, 0);
    assert!(game.take_basin_exit_prompt());

    game.escape_basin().unwrap();

    assert!(keys(&game).story_complete);
    assert_eq!(
        keys(&game).count(),
        crate::tuning::PHASE_KEY_COUNT,
        "keys are kept"
    );
    assert!(find_structure_by_kind(&mut game, "basin_exit").is_none());
}

#[test]
fn escaping_unlocks_the_achievement() {
    use crate::achievements::{AchievementId, Profile};
    let mut game = built_exit();
    game.escape_basin().unwrap();
    assert!(
        game.world
            .resource::<Profile>()
            .contains(&AchievementId::from("basin_escaped"))
    );
}

#[test]
fn escaping_without_an_exit_is_refused() {
    let mut game = open_game();
    assert!(game.escape_basin().is_err());
    assert!(!keys(&game).story_complete);
}

#[test]
fn a_rebuild_is_refused_after_escaping() {
    let mut game = built_exit();
    game.escape_basin().unwrap();
    let err = game.place_structure("basin_exit", 1, 0, None).unwrap_err();
    assert!(err.contains("already"), "{err}");
    assert!(!game.buildable_structure_defs().iter().any(|d| d.basin_exit));
}

#[test]
fn play_continues_after_the_ending() {
    let mut game = built_exit();
    game.escape_basin().unwrap();
    let tick = game.world.resource::<GameClock>().tick;
    game.move_player(-1, 0);
    assert!(game.world.resource::<GameClock>().tick > tick, "ticks run");
    assert!(game.is_game_over().is_none());
}

// ---- the fallbacks ----

#[test]
fn the_shipped_exit_is_the_marked_structure() {
    let game = exit_game();
    let defs: Vec<_> = game
        .structure_defs()
        .into_iter()
        .filter(|d| d.basin_exit)
        .collect();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].id, "basin_exit");
}

#[test]
fn a_missing_exit_file_falls_back_to_the_built_in_def() {
    let dir = scratch_assets_dir("basin_nofile");
    std::fs::create_dir_all(&*dir).unwrap();
    let (db, warnings) = crate::structures::StructureDb::load_dir(&dir).unwrap();
    let def = db.get(crate::structures::BASIN_EXIT_ID).expect("fallback");
    assert!(def.basin_exit);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_malformed_exit_file_falls_back_to_the_built_in_def() {
    let dir = scratch_assets_dir("basin_bad");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(dir.join("basin_exit.ron"), "(id: ").unwrap();
    let (db, warnings) = crate::structures::StructureDb::load_dir(&dir).unwrap();
    assert!(db.get(crate::structures::BASIN_EXIT_ID).unwrap().basin_exit);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn the_shipped_ending_has_screens_and_no_warnings() {
    let (text, warnings) = EndingText::load_dir(&test_assets_dir().join("story")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.screens().len() > 1);
    assert_ne!(text, EndingText::default(), "the shipped file was read");
}

#[test]
fn a_missing_ending_file_is_silent_and_falls_back() {
    let dir = scratch_assets_dir("ending_none");
    let (text, warnings) = EndingText::load_dir(&dir).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(text, EndingText::default());
    assert!(!text.screens().is_empty());
}

#[test]
fn a_malformed_or_empty_ending_falls_back_with_a_warning() {
    for body in ["(screens: [", "(screens: [])"] {
        let dir = scratch_assets_dir("ending_bad");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(dir.join("ending.ron"), body).unwrap();
        let (text, warnings) = EndingText::load_dir(&dir).unwrap();
        assert_eq!(text, EndingText::default(), "{body}");
        assert_eq!(warnings.len(), 1, "{body}");
    }
}

#[test]
fn the_game_exposes_the_ending_screens() {
    let game = exit_game();
    assert!(!game.ending_screens().is_empty());
}
