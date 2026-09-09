//! The settings screen. One toggle, written to `profile.ron` the moment it
//! is flipped.

use super::support::test_app;
use crate::{GameKey, Mode, OptionKey};
use feral_processes_engine::achievements::Profile;

/// The screen exists and the main menu can reach it. `[O]` is uppercase in
/// the label and matched case-insensitively, exactly as the five rows
/// already on that menu are.
#[test]
fn the_main_menu_opens_the_options_screen() {
    let mut app = test_app(9001);
    app.mode = Mode::MainMenu;

    app.handle_key(GameKey::Char('o'));

    assert_eq!(
        app.mode,
        Mode::Options,
        "[O] did not open the options screen"
    );
}

#[test]
fn escape_returns_to_the_main_menu() {
    let mut app = test_app(9002);
    app.mode = Mode::Options;

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::MainMenu);
}

/// One row, and its value reads the profile rather than any second copy of
/// the state.
#[test]
fn the_toggle_row_reports_the_profile() {
    let mut app = test_app(9003);

    let rows = app.option_rows();
    assert_eq!(
        rows.len(),
        1,
        "one option only, per the spec's YAGNI clause"
    );
    assert_eq!(rows[0].key, OptionKey::TacticalBattles);
    assert_eq!(rows[0].value, "Off");

    app.profile.tactical_battles = true;
    assert_eq!(app.option_rows()[0].value, "On");
}

/// Enter on the highlighted row flips it **and writes `profile.ron`
/// immediately** — the same rule the icon write follows, and for the same
/// reason: nothing else on this screen is going to save it.
#[test]
fn toggling_writes_the_profile_to_disk() {
    let mut app = test_app(9004);
    app.mode = Mode::Options;
    app.menu_selected = 0;

    app.handle_key(GameKey::Enter);

    assert!(app.profile.tactical_battles, "the toggle did not flip");
    let (from_disk, warning) = Profile::load(&app.profile_path);
    assert_eq!(warning, None, "the profile did not reload: {warning:?}");
    assert!(
        from_disk.tactical_battles,
        "the toggle flipped in memory but was never written"
    );

    app.handle_key(GameKey::Enter);
    assert!(
        !app.profile.tactical_battles,
        "the toggle does not flip back"
    );
    let (from_disk, _) = Profile::load(&app.profile_path);
    assert!(
        !from_disk.tactical_battles,
        "the second flip was never written"
    );
}

/// The toggle rides `install_profile`, which is the one hand-off from this
/// screen to a running game. Asserted here rather than left to a later
/// phase: a field that does not survive that copy makes every reader see
/// `false` forever, and nothing else in the suite would say so.
#[test]
fn install_profile_carries_the_toggle() {
    let mut app = test_app(9005);
    app.mode = Mode::Options;
    app.handle_key(GameKey::Enter);
    assert!(app.profile.tactical_battles);

    let mut game = app.game.take().expect("the fixture has a game");
    game.install_profile(app.profile.clone());

    assert!(
        game.profile().tactical_battles,
        "install_profile dropped the toggle"
    );
}
