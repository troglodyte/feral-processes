//! The two screens you open, read, and close: the message history and the
//! structure roster. Neither takes an action, which is most of what these
//! tests are checking.

use super::support::*;
use crate::*;

#[test]
fn l_opens_the_history_and_esc_returns_to_the_map() {
    let mut app = test_app(90);
    app.handle_key(GameKey::Char('L'));
    assert_eq!(app.mode, Mode::History);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn b_opens_the_structure_roster_and_esc_returns_to_the_map() {
    let mut app = test_app(91);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Structures);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Up/Down are the scroll, and the popup's window follows the highlighted
/// row (see `popup_layout`) — so moving the highlight is the whole of
/// scrolling, and it must not fall out of the screen doing it.
#[test]
fn up_and_down_scroll_the_history_without_leaving_it() {
    let mut app = test_app(92);
    // Two lines to scroll between. `r` away from any power source logs "You
    // have nothing to recharge from." — an action with a guaranteed line and
    // no dependence on what the seed rolled onto the map.
    app.handle_key(GameKey::Char('r'));
    app.handle_key(GameKey::Char('r'));
    let lines = app
        .game
        .as_ref()
        .unwrap()
        .message_log(MESSAGE_LOG_CAP)
        .len();
    assert!(lines >= 2, "the test needs two lines to scroll between");
    app.handle_key(GameKey::Char('L'));
    // Lines run oldest-first, matching the map's pane, so the newest is the
    // last row — and it is the one the player opened the screen to read.
    assert_eq!(
        app.menu_selected,
        lines - 1,
        "the history opens on its newest line"
    );

    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::History);
    assert_eq!(app.menu_selected, lines - 2, "Up scrolls back in time");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, lines - 1);
    assert_eq!(app.mode, Mode::History, "scrolling never leaves the screen");
}

/// The roster is a list of things, not a timeline, so it opens at the top
/// like every other menu — the Home, which the rest of the base is measured
/// from.
#[test]
fn up_and_down_scroll_the_roster_without_leaving_it() {
    let mut app = test_app(93);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.menu_selected, 0, "the roster opens at its first row");
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::Structures);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Structures);
}

/// Read-only means read-only: no tick, and nothing to resolve. A screen that
/// advanced the clock would let the player pass time by staring at a list —
/// and raids, needs and cronjobs all run off that clock.
#[test]
fn neither_screen_advances_the_game() {
    let mut app = test_app(94);
    let before = app.game.as_ref().unwrap().current_tick();
    for key in [
        GameKey::Char('L'),
        GameKey::Down,
        GameKey::Down,
        GameKey::Up,
        GameKey::Enter,
        GameKey::Char('1'),
        GameKey::Esc,
        GameKey::Char('B'),
        GameKey::Down,
        GameKey::Enter,
        GameKey::Char('1'),
        GameKey::Esc,
    ] {
        app.handle_key(key);
    }
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "reading a list must not pass game time"
    );
}

/// Nothing on either screen is selectable, so the keys that commit a choice
/// everywhere else have nothing to commit here and must not be mistaken for
/// a way out either.
#[test]
fn enter_and_row_shortcuts_do_nothing_on_either_screen() {
    let mut app = test_app(95);
    app.handle_key(GameKey::Char('L'));
    for key in [GameKey::Enter, GameKey::Char('1'), GameKey::Char('a')] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::History, "{key:?} should do nothing here");
    }
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('B'));
    for key in [GameKey::Enter, GameKey::Char('1'), GameKey::Char('a')] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Structures, "{key:?} should do nothing here");
    }
}
