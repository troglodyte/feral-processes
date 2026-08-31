//! Permadeath's one guarantee: a run that flatlines cannot be played on.
//!
//! Until 0.13.65 the mode bought nothing on disk. `resources::GameOver` is a
//! resource and was not persisted, so the slot left behind by a flatline was
//! the last autosave — at most `AUTOSAVE_INTERVAL_TICKS` before the death,
//! and with no memory of having died. It sat in the load list and reloaded
//! into a clean run.

use crate::Game;
use crate::components::Stats;
use crate::resources::DifficultyMode;
use crate::save;
use crate::tests::support::test_assets_dir;

fn scratch(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_permadeath_{tag}_{}.bin",
        std::process::id()
    ))
}

/// Drops the player's Integrity to zero and lets the schedule notice, which
/// is the one thing `difficulty::death_handling_system` branches on.
fn die(seed: u32, mode: DifficultyMode) -> Game {
    let mut game = Game::new(seed, mode, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 0;
    game.tick();
    game
}

#[test]
fn a_flatlined_run_writes_its_ending_into_the_save() {
    let mut game = die(7001, DifficultyMode::Permadeath);
    assert!(
        game.is_game_over().is_some(),
        "zero Integrity under Permadeath ends the run"
    );

    let path = scratch("written");
    game.save(&path).unwrap();
    let data = save::load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        data.game_over,
        game.is_game_over(),
        "the save carries the run's own verdict, not a bare flag"
    );
}

#[test]
fn a_flatlined_save_refuses_to_load() {
    let mut game = die(7002, DifficultyMode::Permadeath);
    let path = scratch("refused");
    game.save(&path).unwrap();

    let Err(err) = Game::load(&path, &test_assets_dir()) else {
        panic!("a dead run must not reopen");
    };
    let _ = std::fs::remove_file(&path);

    assert!(
        err.to_string().contains("flatlined"),
        "the refusal names the ending it is refusing: {err}"
    );
}

/// The control the two above are worthless without: a refusal that fires on
/// every save would pass both of them and delete the game.
#[test]
fn a_run_still_going_loads_exactly_as_before() {
    let mut game = Game::new(7003, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let path = scratch("living");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.is_ok(),
        "a Permadeath run that has not died is an ordinary save"
    );
}

/// The flag tracks the *run ending*, not the death — which is why nothing
/// here branches on `DifficultyMode`. A Forgiving death is a reboot, and
/// `GameOver::reason` is never written for one.
#[test]
fn a_forgiving_death_leaves_its_save_loadable() {
    let mut game = die(7004, DifficultyMode::Forgiving);
    assert!(
        game.is_game_over().is_none(),
        "a Forgiving death reboots rather than ending the run"
    );

    let path = scratch("forgiving");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);

    assert!(loaded.is_ok(), "the reboot is playable and so is its save");
}

/// Additive behind `#[serde(default)]`, so a file written before the field
/// existed loads as a run still going — which is exactly what it was.
#[test]
fn a_save_written_before_the_field_existed_still_loads() {
    let mut game = Game::new(7005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = scratch("legacy");
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim().starts_with("game_over:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the field has to be present to strip, or this test proves nothing"
    );
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);
    assert!(loaded.is_ok(), "no migration and no version bump");
}
