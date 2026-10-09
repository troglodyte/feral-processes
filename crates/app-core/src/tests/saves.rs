//! Save listing, loading and deletion.

use crate::*;

use feral_processes_engine::save;

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
        std::env::temp_dir().join("feral_processes_saves_telemetry.jsonl"),
    );

    app.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
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

/// Permadeath's whole guarantee, end to end through the screens the player
/// actually touches. Before 0.13.65 every assertion below failed: nothing
/// wrote the flatline to disk, so the slot sat in the load list holding the
/// last autosave and reloaded into a run with no memory of the death.
#[test]
fn a_flatlined_permadeath_run_cannot_be_reloaded_from_the_load_list() {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let saves_dir = std::env::temp_dir().join(format!(
        "feral_processes_appcore_test_flatline_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&saves_dir);
    std::fs::create_dir_all(&saves_dir).unwrap();
    let tag = format!(
        "feral_processes_appcore_test_flatline_{}",
        std::process::id()
    );
    let mut app = App::new(
        assets_dir.clone(),
        saves_dir.clone(),
        std::env::temp_dir().join(format!("{tag}.log")),
        std::env::temp_dir().join(format!("{tag}_profile.ron")),
        super::support::arenas_dir(),
        std::env::temp_dir().join(format!("{tag}_telemetry.jsonl")),
    );

    app.start_new_game(DifficultyMode::Permadeath, &CharacterChoice::default());
    while app.game.as_mut().unwrap().take_notification().is_some() {}
    let path = app
        .current_save_path
        .clone()
        .expect("a new run owns a slot");

    // The tick below is what `death_handling_system` reacts to, exactly as a
    // killing blow would be.
    super::support::zero_the_players_hp(&mut app, &path);

    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::GameOver, "the run ends on the screen");

    assert!(
        save::load_from_file(&path).unwrap().game_over.is_some(),
        "the flatline has to reach disk, or the slot is still the last autosave"
    );

    let saves = app.list_saves();
    assert_eq!(saves.len(), 1);
    assert!(
        saves[0].summary.as_deref().unwrap().contains("FLATLINED"),
        "the load list says the run is over: {:?}",
        saves[0].summary
    );

    // And the one gesture that used to undo the death.
    app.handle_key(GameKey::Char(' '));
    assert_eq!(app.mode, Mode::MainMenu);
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('l'));
    assert_ne!(app.mode, Mode::Playing, "a dead run does not reopen");
    assert!(
        app.game.is_none(),
        "and nothing is loaded behind the screen"
    );
    assert!(
        app.status_line
            .as_deref()
            .unwrap_or("")
            .contains("flatlined"),
        "the refusal says why: {:?}",
        app.status_line
    );

    let _ = std::fs::remove_dir_all(&saves_dir);
}

/// The main menu and the load list call `list_saves` every frame, and a
/// whole-file parse of every save per frame cost over half a core on a menu
/// that draws seven rows. A file is parsed again only once its size or
/// modification time moves.
#[test]
fn list_saves_parses_a_file_again_only_once_it_changes() {
    let tag = format!(
        "feral_processes_appcore_test_savecache_{}",
        std::process::id()
    );
    let saves_dir = std::env::temp_dir().join(&tag);
    let _ = std::fs::remove_dir_all(&saves_dir);
    std::fs::create_dir_all(&saves_dir).unwrap();
    let mut app = App::new(
        super::support::test_assets_dir(),
        saves_dir.clone(),
        std::env::temp_dir().join(format!("{tag}.log")),
        std::env::temp_dir().join(format!("{tag}_profile.ron")),
        super::support::arenas_dir(),
        std::env::temp_dir().join(format!("{tag}_telemetry.jsonl")),
    );
    app.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
    let path = app
        .current_save_path
        .clone()
        .expect("a new run owns a slot");
    let summary = app.list_saves()[0].summary.clone();
    assert!(summary.is_some(), "a fresh save reads back");

    // Same length, same timestamp, unreadable contents: only a cached
    // entry can still answer with the old summary.
    let len = std::fs::metadata(&path).unwrap().len() as usize;
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, vec![0u8; len]).unwrap();
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(modified).unwrap();
    drop(file);
    assert_eq!(
        app.list_saves()[0].summary,
        summary,
        "an unchanged file is not parsed again"
    );

    // A different length is a changed file.
    std::fs::write(&path, vec![0u8; len + 1]).unwrap();
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(modified).unwrap();
    drop(file);
    assert_eq!(
        app.list_saves()[0].summary,
        None,
        "a changed file is parsed again"
    );

    let _ = std::fs::remove_dir_all(&saves_dir);
}
