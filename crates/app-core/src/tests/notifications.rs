//! The notification's one behavioural rule: it takes the screen
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
    assert_eq!(app.mode, Mode::BaseOverview);

    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.mode,
        Mode::Build,
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

/// Esc dismisses, and the queue drains one at a time rather than the
/// last one arriving alone.
#[test]
fn esc_dismisses_one_and_the_last_returns_to_the_map() {
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

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Notification, "the second one follows");
    let second = app.pending_notification.clone().expect("two queued");
    assert_ne!(first.title, second.title, "not the same one twice");

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_notification.is_none(), "the subject is cleared");
}

/// Anything but Esc or Enter must leave the notification exactly as it was — the
/// player reading it must not lose it to an incidental keypress. `.` and
/// `x` stand in for "any ordinary key"; neither is special-cased anywhere
/// near this path.
#[test]
fn only_esc_or_enter_dismisses_a_notification() {
    let mut app = test_app(9105);
    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::Notification);
    let shown = app.pending_notification.clone().expect("one on screen");

    for key in [GameKey::Char('.'), GameKey::Char('x'), GameKey::Right] {
        app.handle_key(key);
        assert_eq!(
            app.mode,
            Mode::Notification,
            "{key:?} must not dismiss the notification"
        );
        assert_eq!(
            app.pending_notification.as_ref(),
            Some(&shown),
            "{key:?} must not change which notification is on screen"
        );
    }
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
        .place_structure("home", 0, 0, None)
        .unwrap();
    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::Notification);
    assert_eq!(
        app.pending_notification.as_ref().map(|n| n.title.as_str()),
        Some("Your Base")
    );
}

/// Enter closes a notification as Esc does: it is the key a player reaches
/// for to dismiss, and the footer says so.
#[test]
fn enter_dismisses_a_notification() {
    let mut app = test_app(9106);
    queue(
        &mut app,
        feral_processes_engine::notifications::NotificationKind::Breach,
    );
    app.handle_key(GameKey::Char('.'));
    assert_eq!(app.mode, Mode::Notification);

    app.handle_key(GameKey::Enter);

    assert_ne!(app.mode, Mode::Notification);
    assert!(app.pending_notification.is_none());
}

mod ftue_keys {
    use super::*;
    use feral_processes_engine::achievements::Profile;
    use feral_processes_engine::ftue::{FtueEvent, FtueTrigger};

    const KEY: &str = "event:first_raid";

    /// A guidance popup on screen, over a fresh profile file.
    fn on_a_guidance_popup(seed: u32) -> App {
        let mut app = test_app(seed);
        let game = app.game.as_mut().expect("a fixture with a game");
        assert!(game.ftue(FtueTrigger::Event(FtueEvent::FirstRaid)));
        app.handle_key(GameKey::Char('.'));
        assert_eq!(app.mode, Mode::Notification);
        app
    }

    #[test]
    fn d_opts_this_message_out_and_it_reaches_disk() {
        let mut app = on_a_guidance_popup(9111);
        app.handle_key(GameKey::Char('d'));
        assert_eq!(app.mode, Mode::Playing);
        assert!(
            app.game
                .as_ref()
                .unwrap()
                .profile()
                .ftue
                .never
                .contains(&KEY.to_string())
        );
        let (disk, _) = Profile::load(&app.profile_path);
        assert_eq!(disk.ftue.never, [KEY]);
        assert!(disk.ftue.enabled);
    }

    #[test]
    fn t_turns_guidance_off_and_it_survives_the_next_flush() {
        let mut app = on_a_guidance_popup(9112);
        app.handle_key(GameKey::Char('T'));
        assert_eq!(app.mode, Mode::Playing);
        assert!(!app.game.as_ref().unwrap().guidance_enabled());
        let (disk, _) = Profile::load(&app.profile_path);
        assert!(!disk.ftue.enabled);
        // A later earn flushes the engine's copy over the app's.
        app.game
            .as_mut()
            .unwrap()
            .ftue_never("event:first_town_raid");
        app.after_tick();
        let (disk, _) = Profile::load(&app.profile_path);
        assert!(!disk.ftue.enabled);
        assert_eq!(disk.ftue.never, ["event:first_town_raid"]);
        assert!(!app.profile.ftue.enabled);
    }

    #[test]
    fn a_once_per_run_notification_ignores_d_and_t() {
        let mut app = test_app(9113);
        queue(
            &mut app,
            feral_processes_engine::notifications::NotificationKind::SweepsBegin,
        );
        app.handle_key(GameKey::Char('.'));
        assert_eq!(app.mode, Mode::Notification);
        app.handle_key(GameKey::Char('d'));
        app.handle_key(GameKey::Char('t'));
        assert_eq!(app.mode, Mode::Notification);
        assert!(app.game.as_ref().unwrap().guidance_enabled());
    }
}

mod focused {
    use super::*;
    use feral_processes_engine::NotifyArgs;
    use feral_processes_engine::notifications::{NotificationKind, NotificationSubject};

    /// A base with one staff member, standing in it, with a `ProgramDecompiled`
    /// notice about that staff member on screen (or without a subject).
    fn on_a_notice(seed: u32, with_subject: bool) -> (App, Entity) {
        let mut app = app_owning_distant_programs(seed, 1);
        found_the_base(&mut app);
        stand_in_base(&mut app);
        for _ in 0..4 {
            app.handle_key(GameKey::Char('.'));
        }
        let staff = app.game.as_ref().unwrap().base_staff()[0];
        let game = app.game.as_mut().unwrap();
        assert!(game.notify_with(
            NotificationKind::ProgramDecompiled,
            NotifyArgs {
                fills: &[("name", "Pip")],
                subject: with_subject.then_some(NotificationSubject::Entity(staff)),
                ..NotifyArgs::default()
            },
        ));
        app.handle_key(GameKey::Char('.'));
        assert_eq!(app.mode, Mode::Notification);
        (app, staff)
    }

    #[test]
    fn w_on_an_entity_subject_starts_the_watch_and_dismisses() {
        let (mut app, staff) = on_a_notice(9201, true);
        assert!(
            app.notification_focus().is_some(),
            "fixture must be placeable"
        );
        app.handle_key(GameKey::Char('w'));
        assert_eq!(app.watching, Some(staff));
        assert_eq!(app.mode, Mode::Playing);
        assert_eq!(app.notification_focus(), None, "nothing pending, no focus");
    }

    #[test]
    fn w_with_no_subject_is_inert() {
        let (mut app, _) = on_a_notice(9202, false);
        assert_eq!(app.notification_focus(), None);
        app.handle_key(GameKey::Char('W'));
        assert_eq!(app.watching, None);
        assert_eq!(app.mode, Mode::Notification);
    }

    #[test]
    fn d_silences_a_repeating_kind_and_it_does_not_queue_again() {
        let (mut app, _) = on_a_notice(9203, true);
        app.handle_key(GameKey::Char('D'));
        assert_eq!(app.mode, Mode::Playing);
        let (disk, _) = feral_processes_engine::achievements::Profile::load(&app.profile_path);
        assert_eq!(disk.notifications_never, ["milestone_program_decompiled"]);
        assert!(
            !app.game
                .as_mut()
                .unwrap()
                .notify(NotificationKind::ProgramDecompiled)
        );
    }

    #[test]
    fn d_on_a_guidance_popup_still_goes_to_the_ftue_path() {
        // Covered in full by `ftue_keys`; this pins that it files no kind.
        let mut app = test_app(9204);
        let game = app.game.as_mut().unwrap();
        assert!(game.ftue(feral_processes_engine::ftue::FtueTrigger::Event(
            feral_processes_engine::ftue::FtueEvent::FirstRaid
        )));
        app.handle_key(GameKey::Char('.'));
        app.handle_key(GameKey::Char('d'));
        assert!(
            app.game
                .as_ref()
                .unwrap()
                .profile()
                .notifications_never
                .is_empty()
        );
    }
}
