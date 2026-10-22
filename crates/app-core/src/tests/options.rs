//! The game menu's settings pages. Every setting is written to
//! `profile.ron` the moment it changes.

use super::support::test_app;
use crate::{GameKey, Hub, Mode, OptionKey, OptionKind};
use feral_processes_engine::achievements::Profile;

/// Leaves the run's `game` in place, as `leave_arena` does: where the menu
/// was opened from must not be read off whether a game happens to exist.
fn open_from_main_menu(seed: u32) -> crate::App {
    let mut app = test_app(seed);
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('o'));
    app
}

/// Opens the menu from the map and parks the cursor on the page's first row.
fn on_page(seed: u32, mode: Mode) -> crate::App {
    let mut app = test_app(seed);
    app.handle_key(GameKey::Esc);
    while app.mode != mode {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);
    assert!(!app.hub_on_rail);
    app.menu_selected = 0;
    app
}

#[test]
fn the_main_menu_opens_the_gameplay_page() {
    let app = open_from_main_menu(9001);
    assert_eq!(app.mode, Mode::GameplayOptions);
    assert!(app.hub_on_rail);
}

/// No run behind the menu, so no page to save or quit one.
#[test]
fn from_the_main_menu_the_save_page_is_absent() {
    let mut app = open_from_main_menu(9010);
    let mut seen = vec![app.mode];
    for _ in 0..6 {
        app.handle_key(GameKey::Char(']'));
        seen.push(app.mode);
    }
    assert!(!seen.contains(&Mode::QuitRunConfirm), "{seen:?}");
    assert!(seen.contains(&Mode::AudioOptions));
    assert!(seen.contains(&Mode::GraphicsOptions));
}

#[test]
fn escape_on_the_rail_returns_to_the_main_menu() {
    let mut app = open_from_main_menu(9002);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn escape_on_the_map_opens_the_menu_and_the_rail_closes_it() {
    let mut app = test_app(9011);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::GameplayOptions);
    assert!(app.hub_on_rail);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.game.is_some());
}

#[test]
fn escape_while_watching_only_releases_the_camera() {
    let mut app = test_app(9012);
    app.watching = Some(crate::Entity::PLACEHOLDER);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.watching.is_none());
}

#[test]
fn q_on_the_map_does_nothing() {
    let mut app = test_app(9013);
    app.handle_key(GameKey::Char('q'));
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn a_run_has_four_pages_and_every_one_is_found_by_hub_for() {
    let mut app = test_app(9014);
    app.handle_key(GameKey::Esc);
    let kind = Hub::GameMenu.kind();
    assert_eq!(kind.pages(&app).len(), 4);
    for (i, mode) in [
        Mode::GameplayOptions,
        Mode::AudioOptions,
        Mode::GraphicsOptions,
        Mode::QuitRunConfirm,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            app.hub_for(mode).map(|(h, p, _)| (h, p)),
            Some((Hub::GameMenu, i))
        );
    }
    let menu_only = open_from_main_menu(9015);
    assert_eq!(kind.pages(&menu_only).len(), 3);
    assert_eq!(menu_only.hub_for(Mode::QuitRunConfirm), None);
}

#[test]
fn rows_per_page_and_kind() {
    let mut app = test_app(9003);
    app.mode = Mode::GameplayOptions;
    let rows = app.option_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, OptionKey::TacticalBattles);
    assert_eq!(rows[0].kind, OptionKind::Toggle);
    assert_eq!(rows[0].value, "Off");
    app.profile.tactical_battles = true;
    assert_eq!(app.option_rows()[0].value, "On");

    app.mode = Mode::AudioOptions;
    let rows = app.option_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].key, rows[0].kind),
        (OptionKey::Volume, OptionKind::Level)
    );
    assert_eq!(rows[0].value, "20%");

    app.mode = Mode::GraphicsOptions;
    let keys: Vec<_> = app.option_rows().iter().map(|r| r.key).collect();
    assert_eq!(keys, [OptionKey::Effects, OptionKey::PerfOverlay]);
}

#[test]
fn toggling_writes_the_profile_to_disk() {
    let mut app = on_page(9004, Mode::GameplayOptions);

    app.handle_key(GameKey::Enter);

    assert!(app.profile.tactical_battles, "the toggle did not flip");
    let (from_disk, warning) = Profile::load(&app.profile_path);
    assert_eq!(warning, None);
    assert!(from_disk.tactical_battles);

    app.handle_key(GameKey::Enter);
    assert!(!app.profile.tactical_battles);
    let (from_disk, _) = Profile::load(&app.profile_path);
    assert!(!from_disk.tactical_battles);
}

/// A mid-run flip reaches the running game, which holds its own copy.
#[test]
fn the_tactical_toggle_reaches_the_running_game() {
    let mut app = on_page(9005, Mode::GameplayOptions);
    app.handle_key(GameKey::Enter);
    assert!(app.game.as_ref().unwrap().profile().tactical_battles);
    app.handle_key(GameKey::Enter);
    assert!(!app.game.as_ref().unwrap().profile().tactical_battles);
}

#[test]
fn volume_steps_by_ten_clamps_and_persists() {
    let mut app = on_page(9006, Mode::AudioOptions);
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.mode,
        Mode::AudioOptions,
        "Right must not leave a Level row"
    );
    assert_eq!(app.profile.volume_percent, 30);
    let (from_disk, _) = Profile::load(&app.profile_path);
    assert_eq!(from_disk.volume_percent, 30);

    for _ in 0..12 {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(app.profile.volume_percent, 100);
    for _ in 0..12 {
        app.handle_key(GameKey::Left);
    }
    assert_eq!(app.profile.volume_percent, 0);
    assert_eq!(
        app.mode,
        Mode::AudioOptions,
        "Left must not fall to the rail"
    );
    assert!(!app.hub_on_rail);
}

#[test]
fn left_on_a_toggle_row_still_goes_to_the_rail() {
    let mut app = on_page(9007, Mode::GraphicsOptions);
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
}

#[test]
fn square_brackets_turn_pages() {
    let mut app = test_app(9016);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::AudioOptions);
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::GameplayOptions);
}

#[test]
fn effects_and_overlay_persist() {
    let mut app = on_page(9008, Mode::GraphicsOptions);
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert!(!app.profile.effects);
    assert!(app.profile.perf_overlay);
    let (from_disk, _) = Profile::load(&app.profile_path);
    assert!(!from_disk.effects);
    assert!(from_disk.perf_overlay);
}

/// A flush of the run's earnings replaces the profile with the engine's
/// copy; a setting changed since must not be reverted by it.
#[test]
fn a_profile_flush_keeps_a_just_changed_setting() {
    let mut app = on_page(9009, Mode::AudioOptions);
    app.handle_key(GameKey::Right);
    assert_eq!(app.profile.volume_percent, 30);

    // Something earned, so the engine reports its profile dirty and the
    // flush replaces the app's copy with the engine's.
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .notify(feral_processes_engine::notifications::NotificationKind::FirstRaid)
    );
    app.after_tick();

    assert_eq!(app.profile.volume_percent, 30);
}

#[test]
fn a_failed_profile_write_keeps_the_value_and_says_so() {
    let mut app = on_page(9017, Mode::AudioOptions);
    let blocker = std::env::temp_dir().join("feral_processes_options_blocker_9017");
    std::fs::write(&blocker, b"not a directory").unwrap();
    app.profile_path = blocker.join("profile.ron");

    app.handle_key(GameKey::Right);

    assert_eq!(app.profile.volume_percent, 30);
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("Could not write profile")),
        "{:?}",
        app.status_line
    );
}

/// Loading or starting a run re-reads `profile.ron`; the in-memory setting
/// wins over a stale file.
#[test]
fn loading_a_run_keeps_an_unsaved_setting() {
    let mut app = on_page(9018, Mode::AudioOptions);
    let blocker = std::env::temp_dir().join("feral_processes_options_blocker_9018");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let good = std::mem::replace(&mut app.profile_path, blocker.join("profile.ron"));
    app.handle_key(GameKey::Right);
    app.profile_path = good;
    let path = std::env::temp_dir().join("feral_processes_options_9018.bin");
    app.game.as_mut().unwrap().save(&path).unwrap();

    app.load_game(path);

    assert_eq!(app.profile.volume_percent, 30);
}

#[test]
fn install_profile_carries_the_toggle() {
    let mut app = on_page(9019, Mode::GameplayOptions);
    app.handle_key(GameKey::Enter);
    let mut game = app.game.take().expect("the fixture has a game");
    game.install_profile(app.profile.clone());
    assert!(game.profile().tactical_battles);
}
