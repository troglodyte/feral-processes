//! The full-screen notification's one behavioural rule: it takes the screen
//! from the map and from nowhere else.

use crate::tests::support::*;
use crate::*;

fn queue(app: &mut App, kind: feral_processes_engine::notifications::NotificationKind) {
    assert!(
        app.game
            .as_mut()
            .expect("a fixture with a game")
            .notify(kind),
        "an Always notification queues every time"
    );
}

/// The whole timing rule in one test. Queued while a picker is open, the
/// notification waits — it does not interrupt, and it is not dropped either.
#[test]
fn a_notification_waits_for_the_map_and_is_not_dropped() {
    let mut app = test_app(9101);
    found_the_base(&mut app);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseMenu);

    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.mode,
        Mode::BaseMenu,
        "a menu must not be interrupted by a notification"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::Notification,
        "back on the map, the waiting notification takes the screen"
    );
    assert!(app.pending_notification.is_some());
}

/// Any key dismisses, and the queue drains one at a time rather than the
/// last one arriving alone.
#[test]
fn each_key_dismisses_one_and_the_last_returns_to_the_map() {
    let mut app = test_app(9102);
    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::ContractClosed,
    );
    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::Notification);
    let first = app.pending_notification.clone().expect("one on screen");

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::Notification, "the second one follows");
    let second = app.pending_notification.clone().expect("two queued");
    assert_ne!(first.title, second.title, "not the same one twice");

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_notification.is_none(), "the subject is cleared");
}

/// `Mode::Playing` and nothing else. The four stood in here are the four
/// kinds of screen the rule protects — a fight, a picker, text entry and the
/// excavation plan — but the exhaustiveness is not theirs: it comes from
/// `show_next_notification` testing one equality rather than listing modes,
/// so a mode added tomorrow is covered without being named anywhere.
#[test]
fn only_the_map_gives_up_the_screen() {
    for guarded in [Mode::Battle, Mode::Build, Mode::FuseName, Mode::Excavate] {
        let mut app = test_app(9103);
        queue(
            &mut app,
            feral_processes_engine::notifications::NotificationKind::Breach,
        );
        app.mode = guarded;
        app.show_next_notification();
        assert_eq!(app.mode, guarded, "{guarded:?} was interrupted");
        assert!(app.pending_notification.is_none());
    }

    let mut app = test_app(9103);
    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    app.mode = Mode::Playing;
    app.show_next_notification();
    assert_eq!(app.mode, Mode::Notification);
}

/// Founding the base is a tutorial the player actually meets, through the
/// real key path — the fixture that founds silently is deliberately not used
/// here, because what is under test is that the screen opens at all.
#[test]
fn founding_the_base_opens_the_tutorial() {
    let mut app = test_app(9104);
    app.game
        .as_mut()
        .unwrap()
        .place_structure("home", 0, 0)
        .unwrap();
    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::Notification);
    assert_eq!(
        app.pending_notification.as_ref().map(|n| n.title.as_str()),
        Some("Base Space")
    );
}
