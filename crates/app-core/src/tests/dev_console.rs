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
