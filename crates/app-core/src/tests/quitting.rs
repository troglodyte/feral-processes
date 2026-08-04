//! Leaving a run, and closing the process.

use super::support::*;
use crate::*;

/// A run with a real save slot behind it, so save-and-quit has somewhere to
/// write. `test_app` leaves `current_save_path` unset, which would make every
/// save a silent no-op.
fn app_with_a_save_slot(name: &str) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir = std::env::temp_dir().join(format!(
        "feral_processes_appcore_quit_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&saves_dir);
    std::fs::create_dir_all(&saves_dir).unwrap();
    let history_path = std::env::temp_dir().join(format!(
        "feral_processes_appcore_quit_{name}_{}.log",
        std::process::id()
    ));
    let profile_path = std::env::temp_dir().join(format!(
        "feral_processes_appcore_quit_{name}_{}_profile.ron",
        std::process::id()
    ));
    let mut app = App::new(assets_dir, saves_dir, history_path, profile_path);
    app.start_new_game(DifficultyMode::Forgiving);
    app
}

/// The whole point of the screen: `q` must not be the keypress that throws the
/// run away. It used to clear `App::game` outright, so one mistyped key cost
/// every tick since the last autosave with nothing asked and nothing said.
#[test]
fn q_in_a_run_asks_before_abandoning_it() {
    let mut app = test_app(701);

    app.handle_key(GameKey::Char('q'));

    assert_eq!(app.mode, Mode::QuitRunConfirm);
    assert!(
        app.game.is_some(),
        "the run has to still be there while the question is on screen"
    );
}

/// Both ways of saying no put the player back where they were, with the run
/// intact — Esc because that is what Esc does everywhere else, and `n`
/// because the row is on screen offering it.
#[test]
fn declining_the_quit_confirm_returns_to_the_run() {
    for decline in [GameKey::Esc, GameKey::Char('n')] {
        let mut app = test_app(702);
        app.handle_key(GameKey::Char('q'));
        assert_eq!(app.mode, Mode::QuitRunConfirm);

        app.handle_key(decline);

        assert_eq!(
            app.mode,
            Mode::Playing,
            "{decline:?} should cancel the quit"
        );
        assert!(app.game.is_some(), "{decline:?} must not drop the run");
    }
}

#[test]
fn quitting_without_saving_drops_the_run_and_returns_to_the_menu() {
    let mut app = test_app(703);
    app.handle_key(GameKey::Char('q'));

    app.handle_key(GameKey::Char('q'));

    assert_eq!(app.mode, Mode::MainMenu);
    assert!(app.game.is_none(), "quitting should release the run");
}

/// Save-and-quit has to actually write before it leaves, or it is just the
/// discard path with a friendlier label.
#[test]
fn save_and_quit_writes_the_run_before_leaving() {
    let mut app = app_with_a_save_slot("saveandquit");
    // Move first, so the state on disk would differ from the state at the
    // last autosave if the save were skipped.
    app.handle_key(GameKey::Char('.'));
    let tick_before = app.game.as_ref().unwrap().current_tick();
    // Without this the test could pass on an autosave having already written
    // this tick, proving nothing about save-and-quit. The clock has to be
    // ahead of the last autosave for the assertion below to have teeth.
    assert!(
        app.last_autosave_tick < tick_before,
        "an autosave already covered tick {tick_before}, so this test cannot \
         tell a real save-and-quit from a skipped one"
    );

    app.handle_key(GameKey::Char('q'));
    assert_eq!(app.mode, Mode::QuitRunConfirm);
    app.handle_key(GameKey::Char('s'));

    assert_eq!(
        app.mode,
        Mode::MainMenu,
        "save-and-quit should leave the run"
    );
    assert!(app.game.is_none());

    let saves = app.list_saves();
    assert_eq!(saves.len(), 1, "the run should have left exactly one save");
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('l'));
    assert_eq!(app.mode, Mode::Playing, "the written save should load back");
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        tick_before,
        "save-and-quit wrote a save from before the last action"
    );
}

/// A save that fails holds the player on the confirm rather than leaving
/// anyway. Leaving would discard the run right after being asked to preserve
/// it, which is worse than the unconfirmed `q` this screen replaced.
#[test]
fn a_failed_save_does_not_quit() {
    let mut app = app_with_a_save_slot("failedsave");
    // A path under a *file* rather than a directory: the write cannot
    // succeed, and nothing about the run itself has to be broken to arrange
    // it.
    let blocker = std::env::temp_dir().join(format!(
        "feral_processes_appcore_quit_blocker_{}",
        std::process::id()
    ));
    std::fs::write(&blocker, b"not a directory").unwrap();
    app.current_save_path = Some(blocker.join("save.bin"));

    app.handle_key(GameKey::Char('q'));
    app.handle_key(GameKey::Char('s'));

    assert_eq!(
        app.mode,
        Mode::QuitRunConfirm,
        "a failed save must not be treated as a successful one"
    );
    assert!(app.game.is_some(), "the run has to survive a failed save");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("Save failed")),
        "the player has to be told why they are still here, got {:?}",
        app.status_line
    );
}

/// The main menu's `q` ends the process, so it asks too — it sits one key
/// from `n` and `l`.
#[test]
fn q_at_the_main_menu_asks_before_closing_the_process() {
    let mut app = test_app(704);
    app.game = None;
    app.mode = Mode::MainMenu;

    app.handle_key(GameKey::Char('q'));

    assert_eq!(app.mode, Mode::QuitAppConfirm);
    assert!(!app.quit, "the question must not have answered itself");

    app.handle_key(GameKey::Char('y'));
    assert!(app.quit, "'y' should close the process");
}

#[test]
fn declining_the_app_quit_confirm_returns_to_the_main_menu() {
    for decline in [GameKey::Esc, GameKey::Char('n')] {
        let mut app = test_app(705);
        app.game = None;
        app.mode = Mode::MainMenu;
        app.handle_key(GameKey::Char('q'));

        app.handle_key(decline);

        assert_eq!(
            app.mode,
            Mode::MainMenu,
            "{decline:?} should cancel the quit"
        );
        assert!(!app.quit, "{decline:?} must not close the process");
    }
}

/// Both confirms are reachable by arrow keys and Enter as well as by their
/// letters, the same as every other menu — `selected_index` is what provides
/// that, and it only works if the option list is the one being indexed.
#[test]
fn both_quit_confirms_take_arrows_and_enter() {
    let mut app = test_app(706);
    app.handle_key(GameKey::Char('q'));
    // Row 2 is "Keep playing".
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::Playing,
        "arrowing to Keep playing and pressing Enter should cancel"
    );

    app.game = None;
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('q'));
    // Row 1 is "No, stay".
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::MainMenu);
    assert!(!app.quit);
}
