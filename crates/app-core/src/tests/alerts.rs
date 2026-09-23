//! `Mode::Alerts`, opened with `N`: `App::handle_alerts_key`'s dismiss and
//! selection shape. The engine's own `alerts` module and every source that
//! posts into it are covered in the engine crate; this file only exercises
//! the app-core door onto the board.
//!
//! `Game::post_alert` is `pub(crate)` to the engine, so the only door
//! app-core has onto a seeded board is the save round trip — the same
//! trick `support.rs` uses for `resources::Locale` and everything else the
//! engine's `World` keeps private.

use feral_processes_engine::alerts::{Alert, AlertKind};
use feral_processes_engine::save::{self};

use crate::tests::support::*;
use crate::*;

fn seeded(seed: u32, alerts: Vec<Alert>) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("alerts_seed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.alerts = alerts;
    save::save_to_file(&path, &data).unwrap();
    app.game = feral_processes_engine::Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app
}

fn alert(kind: AlertKind, subject: &str, text: &str) -> Alert {
    Alert {
        kind,
        subject: subject.to_string(),
        text: text.to_string(),
        count: 1,
        unread: true,
    }
}

#[test]
fn n_opens_the_board_from_the_surface_and_marks_everything_read() {
    let mut app = seeded(
        20001,
        vec![alert(
            AlertKind::DepotsFull,
            "depots",
            "Every Depot is full.",
        )],
    );
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 1);

    app.handle_key(GameKey::Char('N'));

    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
}

#[test]
fn n_opens_the_board_from_the_stack() {
    let mut app = app_owning_a_program_and_a_compiler_deep(20002, &[], &[], true);
    assert!(app.game.as_ref().unwrap().is_underground());

    app.handle_key(GameKey::Char('N'));

    assert_eq!(app.mode, Mode::Alerts);
}

/// `n` distinct alerts, seeded (and so displayed) in `alert-0`..`alert-{n-1}`
/// order, so `menu_selected == i` always highlights `alert-{i}`.
fn numbered_alerts(n: usize) -> Vec<Alert> {
    (0..n)
        .map(|i| {
            alert(
                AlertKind::DepotsFull,
                &format!("subject-{i}"),
                &format!("alert-{i}"),
            )
        })
        .collect()
}

fn texts(app: &App) -> Vec<String> {
    app.game
        .as_ref()
        .unwrap()
        .alerts()
        .into_iter()
        .map(|a| a.text)
        .collect()
}

/// `App::selected_index` letters row 0 as `a`, row 1 as `b`, and so on from
/// `DIGIT_ROWS` (9) — so `d` would resolve to row 12 and `x` to row 32 if
/// either fell through to it. A board wide enough for both collisions to be
/// *reachable* rows (`ALERT_BOARD_CAP` is 50) is what makes this test fail
/// if `x`/`d` ever stop being matched ahead of `selected_index`: a fixture
/// with only a handful of rows would pass whether or not the ordering held,
/// since neither shortcut would resolve to a real row either way.
#[test]
fn x_dismisses_the_highlighted_row_not_its_letter_shortcut() {
    let mut app = seeded(20003, numbered_alerts(40));
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);

    app.menu_selected = 5;
    app.handle_key(GameKey::Char('x'));

    let remaining = texts(&app);
    assert_eq!(remaining.len(), 39);
    assert!(
        !remaining.contains(&"alert-5".to_string()),
        "x must dismiss the highlighted row"
    );
    assert!(
        remaining.contains(&"alert-32".to_string()),
        "x must not fall through to its row-32 shortcut and dismiss that instead"
    );
}

#[test]
fn d_dismisses_the_highlighted_row_not_its_letter_shortcut() {
    let mut app = seeded(20004, numbered_alerts(40));
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);

    app.menu_selected = 6;
    app.handle_key(GameKey::Char('d'));

    let remaining = texts(&app);
    assert_eq!(remaining.len(), 39);
    assert!(
        !remaining.contains(&"alert-6".to_string()),
        "d must dismiss the highlighted row"
    );
    assert!(
        remaining.contains(&"alert-12".to_string()),
        "d must not fall through to its row-12 shortcut and dismiss that instead"
    );
}

/// The ordering half of the collapse contract, and that dismissing walks
/// the newest-first list `menu_selected` actually indexes.
#[test]
fn x_and_d_dismiss_exactly_the_indexed_row_in_display_order() {
    let mut app = seeded(
        20005,
        vec![
            alert(AlertKind::SiegeIncoming, "siege", "three"),
            alert(AlertKind::SweepIncoming, "sweep", "two"),
            alert(AlertKind::DepotsFull, "depots", "one"),
        ],
    );
    app.handle_key(GameKey::Char('N'));
    assert_eq!(
        texts(&app),
        vec!["three".to_string(), "two".to_string(), "one".to_string()]
    );

    app.menu_selected = 1; // highlight "two"
    app.handle_key(GameKey::Char('x'));
    assert_eq!(texts(&app), vec!["three".to_string(), "one".to_string()]);

    app.menu_selected = 0; // highlight "three"
    app.handle_key(GameKey::Char('d'));
    assert_eq!(texts(&app), vec!["one".to_string()]);
}

#[test]
fn dismissing_the_last_row_clamps_the_selection() {
    let mut app = seeded(
        20006,
        vec![
            alert(AlertKind::DepotsFull, "depots", "one"),
            alert(AlertKind::SweepIncoming, "sweep", "two"),
        ],
    );
    app.handle_key(GameKey::Char('N'));
    app.menu_selected = 1;
    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.game.as_ref().unwrap().alerts().len(), 1);
    assert_eq!(app.menu_selected, 0);

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.game.as_ref().unwrap().alerts().len(), 0);
    assert_eq!(app.menu_selected, 0);
}

#[test]
fn esc_closes_the_board() {
    let mut app = test_app(20005);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// The board does not gate `App::show_next_notification` — that already
/// fires only from `Mode::Playing` — so a notification queued while the
/// board is open waits, and is not dropped, exactly as it does for every
/// other screen (see `tests::notifications`).
#[test]
fn a_notification_waits_for_the_board_to_close() {
    let mut app = test_app(20007);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);

    assert!(
        app.game
            .as_mut()
            .unwrap()
            .notify(feral_processes_engine::notifications::NotificationKind::Breach),
        "an Always notification queues every time"
    );
    app.handle_key(GameKey::Up);
    assert_eq!(
        app.mode,
        Mode::Alerts,
        "a queued notification must not interrupt the board"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::Notification,
        "closing the board lets the waiting notification through"
    );
}
