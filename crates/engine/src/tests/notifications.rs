//! Full-screen notifications: what queues one, what stops one queueing
//! twice, and the properties an empty catalogue has to keep.

use super::support::*;
use crate::achievements::Profile;
use crate::game::notify::NoNotify;
use crate::notifications::{NotificationDb, NotificationId, Repeat};
use crate::resources::{Notifications, PendingProfileWrites};
use crate::*;

fn nid(s: &str) -> NotificationId {
    NotificationId::from(s)
}

fn fresh() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Drains whatever the opening of a run queued, so a test measures its own
/// trigger and not the founding tutorial that came before it.
fn drain(game: &mut Game) {
    while game.take_notification().is_some() {}
}

#[test]
fn an_unknown_id_is_a_returned_refusal_and_queues_nothing() {
    let mut game = fresh();
    drain(&mut game);
    assert_eq!(game.notify(&nid("no_such_thing")), Err(NoNotify::Unknown));
    assert_eq!(game.notifications_pending(), 0);
}

#[test]
fn always_queues_every_time_and_once_ever_queues_once() {
    let mut game = fresh();
    drain(&mut game);

    assert_eq!(game.notify(&nid("milestone_breach")), Ok(()));
    assert_eq!(game.notify(&nid("milestone_breach")), Ok(()));
    assert_eq!(game.notifications_pending(), 2, "Always does not latch");
    drain(&mut game);

    assert_eq!(game.notify(&nid("tutorial_first_raid")), Ok(()));
    assert_eq!(
        game.notify(&nid("tutorial_first_raid")),
        Err(NoNotify::AlreadySeen)
    );
    assert_eq!(game.notifications_pending(), 1);
}

/// Only `Game::complete_contract` has a figure worth attaching to the
/// screen. The other two firing sites (`enter_next_zone`, `descend_to`) go
/// through the plain door and must keep queuing `detail: None`.
#[test]
fn firing_without_a_detail_leaves_it_none() {
    let mut game = fresh();
    drain(&mut game);

    game.notify(&nid("milestone_breach")).unwrap();
    let breach = game.take_notification().expect("queued");
    assert_eq!(breach.detail, None);

    descend(&mut game);
    let descent = std::iter::from_fn(|| game.take_notification())
        .find(|n| n.title == "The Stack")
        .expect("descending fires the Stack tutorial");
    assert_eq!(descent.detail, None);
}

/// The queue is FIFO and hands back the *resolved* def, so what the screen
/// draws cannot depend on the catalogue still being on disk.
#[test]
fn the_queue_is_first_in_first_out_and_carries_finished_text() {
    let mut game = fresh();
    drain(&mut game);
    game.notify(&nid("milestone_contract")).unwrap();
    game.notify(&nid("milestone_breach")).unwrap();

    let first = game.take_notification().expect("one queued");
    assert_eq!(first.title, "Contract Closed");
    assert!(!first.body.is_empty());
    let second = game.take_notification().expect("two queued");
    assert_eq!(second.title, "Breach");
    assert!(game.take_notification().is_none());
}

/// The `OnceEver` latch is only worth anything once it reaches disk, and
/// app-core owns the path — so the engine has to say the profile is dirty.
/// Without this the tutorial re-fires on every fresh load.
#[test]
fn a_once_ever_notification_dirties_the_profile_and_always_does_not() {
    let mut game = fresh();
    drain(&mut game);
    assert!(
        !game.take_pending_profile_writes(),
        "a fresh run has not latched anything yet"
    );

    game.notify(&nid("milestone_breach")).unwrap();
    assert!(
        !game.take_pending_profile_writes(),
        "an Always notification stores nothing, so nothing needs writing"
    );

    game.notify(&nid("tutorial_first_raid")).unwrap();
    assert!(game.take_pending_profile_writes());
    assert!(
        game.world
            .resource::<Profile>()
            .seen_notifications
            .contains(&nid("tutorial_first_raid"))
    );
}

/// The latch has to survive the run it was set in — that is the whole
/// difference between a tutorial and a milestone. Asserted through a real
/// profile round trip, because a same-session second call passes against a
/// latch that is never written anywhere.
#[test]
fn a_once_ever_latch_survives_a_profile_round_trip() {
    let mut game = fresh();
    game.notify(&nid("tutorial_first_descent")).ok();
    let dir = scratch_assets_dir("notification_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    game.world.resource::<Profile>().save(&path).unwrap();

    let (reloaded, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert!(
        reloaded
            .seen_notifications
            .contains(&nid("tutorial_first_descent"))
    );

    let mut next_run = fresh();
    next_run.world.insert_resource(reloaded);
    drain(&mut next_run);
    assert_eq!(
        next_run.notify(&nid("tutorial_first_descent")),
        Err(NoNotify::AlreadySeen),
        "a tutorial seen in one run stays seen in the next"
    );
}

/// A profile written before notifications existed has no key for them, and
/// must load rather than being refused — the `#[serde(default)]` promise.
#[test]
fn a_profile_written_before_notifications_still_loads() {
    let dir = scratch_assets_dir("notification_old_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(&path, "(earned: [])").unwrap();
    let (profile, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert!(profile.seen_notifications.is_empty());
}

/// Deleting `assets/notifications/` is a supported way to play. Every
/// trigger keeps firing and every one of them is a no-op — nothing is
/// gated on the database anywhere, which is the property that makes the
/// omission safe at *every* site rather than at the ones someone remembered.
#[test]
fn an_empty_catalogue_leaves_every_trigger_a_no_op() {
    let mut game = fresh();
    game.world.insert_resource(NotificationDb::default());
    drain(&mut game);

    for id in [
        "tutorial_base_founding",
        "tutorial_first_descent",
        "tutorial_first_raid",
        "tutorial_first_work_order",
        "milestone_breach",
        "milestone_contract",
    ] {
        assert_eq!(game.notify(&nid(id)), Err(NoNotify::Unknown), "{id}");
    }
    assert_eq!(game.notifications_pending(), 0);
    assert!(!game.take_pending_profile_writes());
}

/// A notification queued while the player is somewhere it must not be drawn
/// waits rather than being dropped. The engine holds the queue; deciding
/// *when* it is safe to show is app-core's, so nothing here may consume one
/// on its own.
#[test]
fn a_notification_queued_underground_is_still_waiting_on_the_surface() {
    let mut game = fresh();
    drain(&mut game);
    descend(&mut game);
    assert!(
        game.notifications_pending() > 0,
        "descending fires the Stack tutorial"
    );
    let queued = game.notifications_pending();
    for _ in 0..5 {
        game.wait();
    }
    assert_eq!(
        game.notifications_pending(),
        queued,
        "nothing in the engine drains the queue behind the frontend's back"
    );
}

/// Founding the Home is the run's first tutorial, and it fires from the
/// player's own verb rather than from a system polling for a Home.
#[test]
fn founding_the_home_queues_its_tutorial_once() {
    let mut game = fresh();
    drain(&mut game);
    place_home(&mut game);
    let titles: Vec<String> = std::iter::from_fn(|| game.take_notification())
        .map(|n| n.title)
        .collect();
    assert!(
        titles.iter().any(|t| t == "Base Space"),
        "founding says what base space is: {titles:?}"
    );
}

/// An achievement's notification quotes the achievement's own prose. A
/// second `.ron` file repeating it is the pattern this test exists to stop.
#[test]
fn an_achievement_notification_quotes_the_achievement_def() {
    let mut game = fresh();
    drain(&mut game);
    set_zone(&mut game, 2);
    game.wait();

    let earned = game.world.resource::<Profile>().earned.clone();
    assert!(!earned.is_empty(), "reaching zone 2 earns a rung");
    let def = game
        .world
        .resource::<crate::achievements::AchievementDb>()
        .iter()
        .find(|d| d.id == earned[0].id)
        .expect("the rung has a def")
        .clone();

    let shown: Vec<_> = std::iter::from_fn(|| game.take_notification()).collect();
    let hit = shown
        .iter()
        .find(|n| n.title.contains(&def.name))
        .unwrap_or_else(|| panic!("no notification named {}: {shown:?}", def.name));
    assert_eq!(
        hit.body, def.description,
        "the body is the def's own description, not a copy"
    );
}

/// Every shipped def is fired by something. There is no `trigger:` field to
/// derive this from — the catalogue is data and the triggers are Rust — so
/// this census is the whole rule, `MEMORY_TRIGGERS`' shape.
#[test]
fn every_shipped_notification_is_fired_by_a_named_site() {
    /// `(id, the Rust site that fires it)`. Achievements are deliberately
    /// absent: they build their notification from their own def and author
    /// no file here.
    const TRIGGERS: &[(&str, &str)] = &[
        ("tutorial_base_founding", "Game::place_structure, founding"),
        ("tutorial_first_descent", "Game::descend_to"),
        ("tutorial_first_raid", "Game::run_raid"),
        ("tutorial_first_work_order", "Game::queue_work_order"),
        ("milestone_breach", "Game::enter_next_zone"),
        ("milestone_contract", "Game::complete_contract"),
    ];

    let (db, warnings) =
        NotificationDb::load_dir(&test_assets_dir().join("notifications")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");

    for def in db.iter() {
        assert!(
            TRIGGERS.iter().any(|(id, _)| *id == def.id.as_str()),
            "{} is shipped but nothing fires it — add it to TRIGGERS with \
             the site, or delete the file",
            def.id
        );
        assert!(!def.title.is_empty(), "{}", def.id);
        assert!(!def.body.is_empty(), "{}", def.id);
    }
    for (id, site) in TRIGGERS {
        assert!(
            db.get(&nid(id)).is_some(),
            "{site} fires {id}, which no file defines"
        );
    }
}

/// Every tutorial is `OnceEver` and every milestone is `Always`. Getting
/// this backwards is silent: a tutorial that re-fires reads as a bug in the
/// screen, and a milestone that fires once reads as one in the trigger.
#[test]
fn tutorials_latch_and_milestones_do_not() {
    let (db, _) = NotificationDb::load_dir(&test_assets_dir().join("notifications")).unwrap();
    for def in db.iter() {
        let expected = if def.id.as_str().starts_with("tutorial_") {
            Repeat::OnceEver
        } else {
            Repeat::Always
        };
        assert_eq!(def.repeat, expected, "{}", def.id);
    }
}

/// `Notifications` is session state and must never reach the save. Asserted
/// by pushing one and reloading, because "I did not add a save field" is not
/// something a round trip can see on its own.
#[test]
fn the_queue_does_not_survive_a_save() {
    let mut game = fresh();
    game.notify(&nid("milestone_breach")).unwrap();
    assert!(game.notifications_pending() > 0);

    let dir = scratch_assets_dir("notification_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    // The founding tutorial has not fired for this `Game`, so anything
    // queued came from the load itself.
    assert!(
        loaded
            .world
            .get_resource::<Notifications>()
            .is_some_and(|q| q.is_empty())
    );
    assert!(loaded.take_notification().is_none());
    let _ = loaded.take_pending_profile_writes();
}

/// Two reasons the profile can be dirty, one resource — so a drain has to
/// clear both halves or the second one writes the profile forever.
#[test]
fn draining_the_profile_channel_clears_both_halves() {
    let mut game = fresh();
    game.world
        .resource_mut::<PendingProfileWrites>()
        .seen
        .push(nid("x"));
    assert!(game.take_pending_profile_writes());
    assert!(!game.take_pending_profile_writes());
}
/// A new run opens on the map, not on a notice. Worth pinning because
/// `enter_next_zone` fires the breach milestone from its first line, and
/// anything that later routes world setup through it would greet every
/// player with "Breach" before they had moved.
#[test]
fn a_fresh_run_opens_on_the_map_and_not_on_a_notice() {
    let game = fresh();
    assert_eq!(game.notifications_pending(), 0);
}
