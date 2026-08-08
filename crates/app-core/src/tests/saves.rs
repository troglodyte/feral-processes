//! Save listing, loading and deletion.

use crate::*;

#[test]
fn starting_a_new_game_creates_a_listed_save_that_can_be_loaded_and_deleted() {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let saves_dir = std::env::temp_dir().join(format!(
        "feral_processes_appcore_test_savelist_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&saves_dir);
    std::fs::create_dir_all(&saves_dir).unwrap();
    let history_path = std::env::temp_dir().join(format!(
        "feral_processes_appcore_test_savelist_{}.log",
        std::process::id()
    ));
    let profile_path = std::env::temp_dir().join(format!(
        "feral_processes_appcore_test_savelist_{}_profile.ron",
        std::process::id()
    ));
    let mut app = App::new(
        assets_dir,
        saves_dir.clone(),
        history_path,
        profile_path,
        super::support::arenas_dir(),
    );

    app.start_new_game(DifficultyMode::Forgiving);
    assert!(
        app.mode == Mode::Playing,
        "starting a new game should enter Playing"
    );
    let saves = app.list_saves();
    assert_eq!(
        saves.len(),
        1,
        "starting a new game should immediately create one listed save"
    );
    assert!(
        saves[0].summary.is_some(),
        "a freshly saved game should be readable back"
    );

    // Back to the main menu, then load that save from the list.
    app.game = None;
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('l'));
    assert!(
        app.mode == Mode::LoadGame,
        "'l' should open the load list once a save exists"
    );
    app.handle_key(GameKey::Char('1'));
    assert!(
        app.mode == Mode::SaveAction,
        "picking a save should open the load/delete choice"
    );
    app.handle_key(GameKey::Char('l'));
    assert!(
        app.mode == Mode::Playing,
        "loading should return to Playing"
    );
    assert!(app.game.is_some(), "loading should populate the game");

    // Delete it.
    app.game = None;
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('x'));
    assert!(
        app.list_saves().is_empty(),
        "deleting the only save should empty the list"
    );

    let _ = std::fs::remove_dir_all(&saves_dir);
}
