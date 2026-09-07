//! The dev console — a keypad for provoking events that are otherwise
//! rare enough to make a visual feature untestable by hand.
//!
//! What is asserted here is *dispatch*: that the gate holds, that the
//! screen opens and closes, and that firing a row reaches the engine. What
//! each trigger actually does to the world is the engine's own suite
//! (`tests/raids.rs`), because `Game::world` is private and app-core cannot
//! set up a structure to raid.

use super::support::*;
use crate::*;

/// The gate is a field read once in `App::new`, not a live env lookup, so a
/// test can open it without touching an environment the parallel suite
/// shares — the same reasoning `dev_arena_enabled` records.
fn console_app(seed: u32) -> App {
    let mut app = test_app(seed);
    app.enable_dev_console_for_test();
    app
}

#[test]
fn the_console_stays_shut_without_the_dev_flag() {
    let mut app = test_app(1);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the console must be unreachable in a build a player is running"
    );
}

#[test]
fn the_console_opens_from_the_map_and_esc_returns() {
    let mut app = console_app(2);

    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    assert_eq!(app.mode, Mode::DevConsole);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Firing a trigger leaves the console standing. Every use of this thing is
/// "do it again and watch harder" — a screen that closed on each press would
/// make a four-key sequence into twelve.
#[test]
fn firing_a_trigger_leaves_the_console_open() {
    let mut app = console_app(3);
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::DevConsole);
}

/// The renderer draws `rows[i].label` and the handler dispatches
/// `rows[i].action`, so the table has to be the only source of both — the
/// group-menu invariant applied to a smaller screen.
#[test]
fn every_console_row_has_a_label_and_the_selection_stays_in_range() {
    let mut app = console_app(4);
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    let rows = App::dev_console_rows();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(!row.label.is_empty());
    }

    for _ in 0..rows.len() * 2 {
        app.handle_key(GameKey::Down);
        assert!(app.menu_selected < rows.len());
    }
    for _ in 0..rows.len() * 2 {
        app.handle_key(GameKey::Up);
        assert!(app.menu_selected < rows.len());
    }
}

/// Rows are dispatched by their `action`, so two rows sharing one is a row
/// that silently cannot be reached — the copy-paste mistake this table
/// invites, and invisible on screen because both labels still draw.
#[test]
fn no_two_console_rows_fire_the_same_action() {
    let rows = App::dev_console_rows();
    for (i, row) in rows.iter().enumerate() {
        for other in &rows[i + 1..] {
            assert_ne!(
                row.action, other.action,
                "'{}' and '{}' both fire {:?}",
                row.label, other.label, row.action
            );
        }
    }
}

/// The one trigger whose effect app-core can see for itself: needs decay
/// every tick, so burning cycles has to move them.
#[test]
fn the_tick_trigger_advances_the_world() {
    let mut app = console_app(5);
    let before = app.game.as_ref().unwrap().player_status().power;
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    let row = App::dev_console_rows()
        .iter()
        .position(|r| r.action == DevAction::AdvanceTicks)
        .expect("the table must offer a tick trigger");
    app.menu_selected = row;

    app.handle_key(GameKey::Enter);

    let after = app.game.as_ref().unwrap().player_status().power;
    assert!(
        after < before,
        "burning {DEV_CONSOLE_TICKS} cycles should have moved Power: {before} -> {after}"
    );
}

/// The master switch's resolution, exercised as the pure decision it is.
///
/// `resolve_tool_flag` is split from `dev_tool_flag` precisely so these can
/// run without `set_var` — the parallel suite shares one environment, and a
/// test that wrote `FERAL_DEV` would decide the gate for whatever else was
/// running at the time. The same split `launcher::paths::layout` makes for
/// `FERAL_ASSETS_DIR`, for the same reason.
mod master_flag {
    use crate::app::dev_console::resolve_tool_flag;
    use std::ffi::OsStr;

    /// Reads as "is this tool on, given its own value and the master's".
    fn on(specific: Option<&str>, master: Option<&str>) -> bool {
        resolve_tool_flag(
            specific.map(OsStr::new),
            master.map(OsStr::new),
        )
    }

    #[test]
    fn a_player_build_sets_neither_and_gets_nothing() {
        assert!(!on(None, None));
    }

    #[test]
    fn the_master_flag_alone_opens_a_tool() {
        assert!(
            on(None, Some("1")),
            "FERAL_DEV=1 is the whole point: one name for the everyday case"
        );
    }

    #[test]
    fn a_specific_flag_alone_still_opens_its_own_tool() {
        assert!(
            on(Some("1"), None),
            "every script and doc that names a single flag must keep working"
        );
    }

    #[test]
    fn a_specific_zero_vetoes_the_master() {
        assert!(
            !on(Some("0"), Some("1")),
            "FERAL_DEV=1 FERAL_DEV_ARENA=0 is how one tool is left out"
        );
    }

    #[test]
    fn a_master_zero_opens_nothing() {
        assert!(!on(None, Some("0")));
    }

    /// Empty is unset, not off — the rule `dev_flag` has always had, and the
    /// one `paths::layout` cites. So it falls through rather than vetoing.
    #[test]
    fn an_empty_specific_value_falls_through_to_the_master() {
        assert!(on(Some(""), Some("1")));
        assert!(!on(Some(""), Some("0")));
        assert!(!on(Some(""), None));
    }

    /// The `!= "0"` half is a single-value exception, not a truthiness
    /// parser: `dev_flag` has never accepted "false" as off and this must
    /// not quietly start.
    #[test]
    fn any_other_value_reads_as_on() {
        assert!(on(Some("true"), None));
        assert!(on(None, Some("false")));
        assert!(on(Some("00"), None));
    }
}
