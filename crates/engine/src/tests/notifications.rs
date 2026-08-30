//! Full-screen notifications: what queues one, and what stops one queueing
//! twice.

use super::support::*;
use crate::achievements::Profile;
use crate::notifications::{NotificationKind, Repeat};
use crate::resources::{Notifications, PendingProfileWrites};
use crate::*;

/// Whether the profile has latched `kind`. The store is plain strings on
/// purpose (`Profile::seen_notifications`), so every assertion about it goes
/// through `latch_key` rather than restating one.
fn latched(game: &Game, kind: NotificationKind) -> bool {
    game.world
        .resource::<Profile>()
        .seen_notifications
        .iter()
        .any(|seen| seen == kind.latch_key())
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
fn always_queues_every_time_and_once_ever_queues_once() {
    let mut game = fresh();
    drain(&mut game);

    assert!(game.notify(NotificationKind::Breach));
    assert!(game.notify(NotificationKind::Breach));
    assert_eq!(game.notifications_pending(), 2, "Always does not latch");
    drain(&mut game);

    assert!(game.notify(NotificationKind::FirstRaid));
    assert!(
        !game.notify(NotificationKind::FirstRaid),
        "a spent OnceEver latch is the only refusal left"
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

    assert!(game.notify(NotificationKind::Breach));
    let breach = game.take_notification().expect("queued");
    assert_eq!(breach.detail, None);

    descend(&mut game);
    let descent = std::iter::from_fn(|| game.take_notification())
        .find(|n| n.title == "The Stack")
        .expect("descending fires the Stack tutorial");
    assert_eq!(descent.detail, None);
}

/// The queue is FIFO and hands back *resolved* text, which is what lets an
/// achievement push one built from its own prose rather than from a kind.
#[test]
fn the_queue_is_first_in_first_out_and_carries_finished_text() {
    let mut game = fresh();
    drain(&mut game);
    assert!(game.notify(NotificationKind::ContractClosed));
    assert!(game.notify(NotificationKind::Breach));

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

    assert!(game.notify(NotificationKind::Breach));
    assert!(
        !game.take_pending_profile_writes(),
        "an Always notification stores nothing, so nothing needs writing"
    );

    assert!(game.notify(NotificationKind::FirstRaid));
    assert!(game.take_pending_profile_writes());
    assert!(latched(&game, NotificationKind::FirstRaid));
}

/// The latch has to survive the run it was set in — that is the whole
/// difference between a tutorial and a milestone. Asserted through a real
/// profile round trip, because a same-session second call passes against a
/// latch that is never written anywhere.
#[test]
fn a_once_ever_latch_survives_a_profile_round_trip() {
    let mut game = fresh();
    game.notify(NotificationKind::FirstDescent);
    let dir = scratch_assets_dir("notification_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    game.world.resource::<Profile>().save(&path).unwrap();

    let (reloaded, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert!(
        reloaded
            .seen_notifications
            .iter()
            .any(|seen| seen == NotificationKind::FirstDescent.latch_key())
    );

    let mut next_run = fresh();
    next_run.world.insert_resource(reloaded);
    drain(&mut next_run);
    assert!(
        !next_run.notify(NotificationKind::FirstDescent),
        "a tutorial seen in one run stays seen in the next"
    );
}

/// **The latch keys are a file format.** They were the ids of the deleted
/// `assets/notifications/*.ron`, and a player who has already been shown a
/// tutorial holds them in `profile.ron` today. Renaming one re-shows that
/// tutorial to everybody, which nothing else in the suite can see — the
/// round trip above writes and reads the *same* build's keys and passes
/// whatever they say.
#[test]
fn a_profile_written_before_this_refactor_keeps_its_latches() {
    let dir = scratch_assets_dir("notification_legacy_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(
        &path,
        "(earned: [], seen_notifications: [\"tutorial_first_descent\", \"tutorial_first_raid\"])",
    )
    .unwrap();

    let (profile, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");

    let mut game = fresh();
    game.world.insert_resource(profile);
    drain(&mut game);
    assert!(!game.notify(NotificationKind::FirstDescent));
    assert!(!game.notify(NotificationKind::FirstRaid));
    assert!(
        game.notify(NotificationKind::BaseFounding),
        "one this profile has not seen still fires"
    );
}

/// A key no build has copy for any more must sit there inertly. `load`
/// discards the *whole* profile on a parse error — achievements included —
/// which is why `seen_notifications` is `Vec<String>` and not a typed id.
#[test]
fn a_retired_latch_key_does_not_cost_the_profile() {
    let dir = scratch_assets_dir("notification_retired_key");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(
        &path,
        "(earned: [], seen_notifications: [\"tutorial_gone\"])",
    )
    .unwrap();

    let (profile, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(
        profile.seen_notifications,
        vec!["tutorial_gone".to_string()]
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

/// Every kind is fired by something. There is no `trigger:` field to derive
/// this from — the copy is a table and the triggers are hooks into
/// particular functions — so this census is the whole rule,
/// `MEMORY_TRIGGERS`' shape.
///
/// **The match is what makes it a census.** A new variant fails to compile
/// here until somebody names the site that fires it, which a `&[(kind,
/// site)]` table would not; the walk over `all()` is then only checking that
/// nobody wrote an empty string.
#[test]
fn every_notification_kind_is_fired_by_a_named_site() {
    // Achievements are deliberately absent: they build their notification
    // from their own def and name no kind at all.
    fn site(kind: NotificationKind) -> &'static str {
        match kind {
            NotificationKind::BaseFounding => "Game::place_structure, founding",
            NotificationKind::FirstDescent => "Game::descend_to",
            NotificationKind::FirstRaid => "Game::run_raid",
            NotificationKind::FirstWorkOrder => "Game::queue_work_order",
            NotificationKind::Breach => "Game::enter_next_zone",
            NotificationKind::ContractClosed => "Game::complete_contract",
            NotificationKind::OnboardingMission => "Game::ensure_tutorial_held",
        }
    }

    for kind in NotificationKind::all() {
        assert!(!site(kind).is_empty(), "{kind} is fired by nothing");
    }
}

/// Every tutorial is `OnceEver` and every milestone is `Always`. Getting
/// this backwards is silent: a tutorial that re-fires reads as a bug in the
/// screen, and a milestone that fires once reads as one in the trigger.
///
/// A second match rather than a fold over the variant names, for the reason
/// above — and because the grouping the enum expresses with a comment is
/// exactly what a name-prefix rule used to express with a string.
#[test]
fn tutorials_latch_and_milestones_do_not() {
    for kind in NotificationKind::all() {
        let expected = match kind {
            NotificationKind::BaseFounding
            | NotificationKind::FirstDescent
            | NotificationKind::FirstRaid
            | NotificationKind::FirstWorkOrder => Repeat::OnceEver,
            // The chain runs on every new game, so a briefing latched across
            // runs would leave a second playthrough's missions unexplained.
            NotificationKind::Breach
            | NotificationKind::ContractClosed
            | NotificationKind::OnboardingMission => Repeat::Always,
        };
        assert_eq!(kind.def().repeat, expected, "{kind}");
    }
}

/// `Notifications` is session state and must never reach the save. Asserted
/// by pushing one and reloading, because "I did not add a save field" is not
/// something a round trip can see on its own.
#[test]
fn the_queue_does_not_survive_a_save() {
    let mut game = fresh();
    assert!(game.notify(NotificationKind::Breach));
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
        .push(NotificationKind::BaseFounding);
    assert!(game.take_pending_profile_writes());
    assert!(!game.take_pending_profile_writes());
}
/// A new run opens on its onboarding briefing and **nothing else**. Worth
/// pinning because `enter_next_zone` fires the breach milestone from its
/// first line, and anything that later routes world setup through it would
/// greet every player with "Breach" before they had moved.
///
/// The briefing itself is deliberate: `Game::new` hands out the chain's
/// first mission, and a mission handed out is a mission explained.
#[test]
fn a_fresh_run_opens_on_its_briefing_and_no_other_notice() {
    let mut game = fresh();
    let queued: Vec<String> = std::iter::from_fn(|| game.take_notification())
        .map(|n| n.title)
        .collect();
    let first = game
        .active_contracts()
        .into_iter()
        .find(|row| row.tutorial)
        .expect("a new run holds the chain's first mission");
    assert_eq!(
        queued,
        vec![first.name],
        "one notice, and it is the mission just handed out"
    );
}

// ---------------------------------------------------------------------------
// The onboarding chain's briefing
// ---------------------------------------------------------------------------

/// A hole the caller filled is gone, and one it did not name is left alone
/// rather than becoming an empty string — a body reading "Build a  now" is
/// a worse failure than one still showing its placeholder, because only the
/// second is visible in a census.
#[test]
fn filling_replaces_the_holes_the_caller_named() {
    let mut game = fresh();
    drain(&mut game);
    assert!(game.notify_filled(NotificationKind::Breach, &[("nothing", "x")]));
    assert!(game.take_notification().is_some());
}

/// The briefing carries **the contract's own words**. It is the whole point
/// of templating one file rather than authoring eleven: the mission's name
/// and description exist once, in `assets/contracts/`.
#[test]
fn handing_out_a_mission_briefs_it_in_the_contracts_own_words() {
    let mut game = fresh();
    let held = game
        .active_contracts()
        .into_iter()
        .find(|row| row.tutorial)
        .expect("a new run holds the chain's first mission");

    let shown = std::iter::from_fn(|| game.take_notification())
        .find(|n| n.body.contains(&held.description))
        .unwrap_or_else(|| panic!("no briefing carried {}'s description", held.id));

    assert!(
        shown.title.contains(&held.name),
        "the briefing is titled for the mission: {:?}",
        shown.title
    );
    assert!(
        shown.body.contains(&held.objective_line),
        "and says what it asks for: {:?}",
        shown.body
    );
    assert!(
        !shown.body.contains('{'),
        "every hole is filled: {:?}",
        shown.body
    );
}

/// Finishing one briefs the next, in the same tick it is handed out.
#[test]
fn finishing_a_mission_briefs_the_next_one() {
    let mut game = fresh();
    drain(&mut game);
    game.note_deed(crate::contracts::Deed::Examined);
    // Step 10 is `Build(home)`; step 20 is the `Examined` one. Walk to it by
    // filing the first as done rather than by building a Home.
    let first = game
        .active_contracts()
        .into_iter()
        .find(|row| row.tutorial)
        .expect("holding one");
    game.world
        .resource_mut::<crate::resources::ActiveContracts>()
        .active
        .retain(|c| c.def.id != first.id);
    game.world
        .resource_mut::<crate::resources::ActiveContracts>()
        .done
        .push(first.id.clone());
    game.tick();

    let next = game
        .active_contracts()
        .into_iter()
        .find(|row| row.tutorial)
        .expect("the next step is in hand");
    assert_ne!(next.id, first.id, "the chain moved on");
    assert!(
        std::iter::from_fn(|| game.take_notification()).any(|n| n.title.contains(&next.name)),
        "the step that was just handed out is the one briefed"
    );
}
