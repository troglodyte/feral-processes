//! Full-screen notifications: what queues one, and what stops one queueing
//! twice.

use super::support::*;
use crate::achievements::Profile;
use crate::ftue::{FtueEvent, FtueTrigger};
use crate::notifications::NotificationKind;
use crate::resources::{Notifications, PendingProfileWrites};
use crate::*;

/// The run set key of a named event.
fn event_key(event: FtueEvent) -> String {
    FtueTrigger::Event(event).key()
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
fn milestones_queue_every_time_and_ftue_queues_once_per_run() {
    let mut game = fresh();
    drain(&mut game);

    assert!(game.notify(NotificationKind::Breach));
    assert!(game.notify(NotificationKind::Breach));
    assert_eq!(
        game.notifications_pending(),
        2,
        "a milestone does not latch"
    );
    drain(&mut game);

    assert!(game.ftue(FtueTrigger::Event(FtueEvent::FirstRaid)));
    assert!(
        !game.ftue(FtueTrigger::Event(FtueEvent::FirstRaid)),
        "the run set is the only refusal"
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
    assert_eq!(second.title, "Escalation");
    assert!(game.take_notification().is_none());
}

/// FTUE firing is per run: a new `Game` shows it again, and a save -> load
/// keeps it spent. Through a real save file, since a same-session second call
/// passes against a set that is never written.
#[test]
fn an_ftue_message_is_once_per_run_across_save_and_load() {
    let mut game = fresh();
    drain(&mut game);
    assert!(game.ftue(FtueTrigger::Event(FtueEvent::FirstDescent)));
    let queued = game.take_notification().expect("queued");
    assert_eq!(queued.ftue_key.as_deref(), Some("event:first_descent"));

    let dir = scratch_assets_dir("ftue_run_set");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert!(loaded.ftue_shown("event:first_descent"));
    assert!(
        !loaded.ftue(FtueTrigger::Event(FtueEvent::FirstDescent)),
        "a reload is the same run"
    );

    let mut next_run = fresh();
    drain(&mut next_run);
    assert!(
        next_run.ftue(FtueTrigger::Event(FtueEvent::FirstDescent)),
        "a new game shows it again"
    );
}

/// Guidance off or a "never" key latches the run set but queues nothing, so
/// turning guidance back on mid-run does not replay what was skipped.
#[test]
fn a_suppressed_message_is_still_latched() {
    let mut game = fresh();
    drain(&mut game);
    game.set_guidance(false);
    assert!(!game.ftue(FtueTrigger::Event(FtueEvent::FirstRaid)));
    assert_eq!(game.notifications_pending(), 0);
    game.set_guidance(true);
    assert!(!game.ftue(FtueTrigger::Event(FtueEvent::FirstRaid)));

    game.ftue_never("event:first_descent");
    assert!(!game.ftue(FtueTrigger::Event(FtueEvent::FirstDescent)));
    assert!(game.ftue_shown("event:first_descent"));

    game.reset_guidance();
    assert!(game.guidance_enabled());
    assert!(game.ftue(FtueTrigger::Event(FtueEvent::FirstWorkOrder)));
}

/// The opt-outs reach the profile and mark it dirty.
#[test]
fn ftue_choices_dirty_the_profile() {
    let mut game = fresh();
    drain(&mut game);
    let _ = game.take_pending_profile_writes();
    game.ftue_never("event:first_raid");
    assert!(game.take_pending_profile_writes());
    assert_eq!(
        game.world.resource::<Profile>().ftue.never,
        vec!["event:first_raid".to_string()]
    );
}

/// `Profile::ftue` round-trips through `profile.ron`, and a profile from
/// before the field (carrying the retired `seen_notifications`) still loads
/// with its achievements and guidance on.
#[test]
fn a_profile_from_before_ftue_still_loads_and_ftue_round_trips() {
    let dir = scratch_assets_dir("ftue_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(
        &path,
        "(earned: [(id: \"first_kill\", first_tick: 12)], seen_notifications: [\"tutorial_first_raid\"])",
    )
    .unwrap();
    let (profile, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(profile.earned.len(), 1);
    assert!(profile.ftue.enabled);
    assert!(profile.ftue.never.is_empty());

    let mut profile = profile;
    profile.ftue.enabled = false;
    profile.ftue.never.push("event:first_raid".into());
    profile.save(&path).unwrap();
    let (back, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(back.ftue, profile.ftue);
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
        titles.iter().any(|t| t == "Your Base"),
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
            NotificationKind::Breach => "Game::enter_next_zone",
            NotificationKind::SweepsBegin => "Game::enter_next_zone, the breach onto RAID_MIN_ZONE",
            NotificationKind::ContractClosed => "Game::complete_contract",
            NotificationKind::OnboardingComplete => "Game::complete_contract, the onboarding arm",
            NotificationKind::OnboardingMission => "Game::ensure_tutorial_held",
            NotificationKind::SettlementGrown => "Game::announce_growth, on the latch's flip",
            NotificationKind::LevelCapReached => {
                "Game::award_player_xp, the level that lands on the cap"
            }
            NotificationKind::ResearchComplete => "Game::settle_research",
            NotificationKind::ResearchDiscovered => "Game::discover_research",
            NotificationKind::PhaseKeyFound => "Game::roll_phase_key, via announce_phase_key",
            NotificationKind::ProgramDecompiled => "Game::settle_jail_attempt",
        }
    }

    for kind in NotificationKind::all() {
        assert!(!site(kind).is_empty(), "{kind} is fired by nothing");
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

/// A drain clears the dirty flag or the profile is written forever.
#[test]
fn draining_the_profile_channel_clears_it() {
    let mut game = fresh();
    game.world.resource_mut::<PendingProfileWrites>().ftue = true;
    assert!(game.take_pending_profile_writes());
    assert!(!game.take_pending_profile_writes());
}

/// A new run opens on its onboarding briefing and **nothing else**. Worth
/// pinning because `enter_next_zone` fires the breach milestone from its
/// first line, and anything that later routes world setup through it would
/// greet every player with "Escalation" before they had moved.
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
    assert!(game.notify_filled(NotificationKind::Breach, &[("nothing", "x")], None));
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

/// **The low-Power notice fires at the threshold the penalty starts at**,
/// `tuning::LOW_POWER_ATTACK_THRESHOLD`, and not at a fraction of its own:
/// a second number here would let the screen say "your attacks are
/// weakening" on a tick where they are not.
///
/// Driven by ticking rather than by writing the reserve and calling the
/// hook, because the drain lives in `needs_tick_system` inside the schedule
/// and the whole question is whether anything reads it back afterwards. The
/// reserve is set by *inserting* one — `PowerReserve`'s float is private and
/// its seven operations match the shipped call sites, so a test setter would
/// be an eighth.
#[test]
fn crossing_the_low_power_threshold_notifies_once() {
    let mut game = fresh();
    drain(&mut game);
    let player = game.player_entity();
    let above = crate::tuning::LOW_POWER_ATTACK_THRESHOLD + 5.0;
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(above, crate::components::POWER_MAX));

    game.tick();
    assert_eq!(
        game.notifications_pending(),
        0,
        "above the threshold there is nothing to say"
    );

    // A hair above, so the passive drain alone carries it under on the very
    // next tick and the crossing is the only thing that changed.
    game.world.entity_mut(player).insert(PowerReserve::new(
        crate::tuning::LOW_POWER_ATTACK_THRESHOLD + 0.1,
        crate::components::POWER_MAX,
    ));
    game.tick();
    assert_eq!(
        game.notifications_pending(),
        1,
        "crossing under is what the player is told about"
    );
    assert!(game.ftue_shown(&event_key(FtueEvent::LowPower)));
    drain(&mut game);

    for _ in 0..5 {
        game.tick();
    }
    assert_eq!(
        game.notifications_pending(),
        0,
        "staying low is not news — the latch is what makes it a tutorial"
    );
}

/// A benched program is the one state the player cannot fix by playing on:
/// `Downed` is a one-way door with no Bay standing, and nothing else in the
/// game says so.
#[test]
fn benching_a_program_with_no_bay_asks_for_one() {
    let mut game = fresh();
    let program = spawn_tamed(&mut game, 1, 0);
    drain(&mut game);

    game.bench_or_dissolve(program);

    assert_eq!(game.notifications_pending(), 1, "the bench is the trigger");
    assert!(game.ftue_shown(&event_key(FtueEvent::DownedProgram)));
}

/// ...and a base that already has one is not told to build it. The gate is
/// `StructureDef::recovery` rather than the Bay's id, `dispatches_sorties`'
/// rule: a mod's own recovery structure answers the player's problem just as
/// well, and naming `"repair_bay"` in Rust would put content in the engine.
#[test]
fn benching_a_program_with_a_bay_standing_says_nothing() {
    let mut game = fresh();
    place_home(&mut game);
    spawn_machine_at(&mut game, "repair_bay", 2, 0);
    let program = spawn_tamed(&mut game, 1, 0);
    drain(&mut game);

    game.bench_or_dissolve(program);

    assert_eq!(
        game.notifications_pending(),
        0,
        "the program walks itself to the Bay — there is nothing to ask for"
    );
}

/// Reaching the sector's ceiling says so, and says it again at the next one.
///
/// `Repeat::Always` rather than a tutorial latch: the ceiling is a different
/// number in every sector, so "you are capped" becomes news again at each of
/// them. It cannot repeat *within* a sector, and no check enforces that —
/// `progression::add_xp` returns before levelling once `level >= cap`, so the
/// `gain.levels > 0` branch this fires from is unreachable until a breach
/// raises the ceiling. That is why the third leg below asserts silence rather
/// than a guard.
#[test]
fn reaching_the_level_cap_announces_itself_once_per_sector() {
    let mut game = fresh();
    let player = game.player_entity();
    let cap = game.level_cap();

    // A level that lands short of the ceiling is not what this announces.
    set_level(&mut game, player, cap - 2);
    drain(&mut game);
    game.award_player_xp(player, crate::progression::xp_for_level(cap - 2));
    assert_eq!(
        game.world.get::<Experience>(player).unwrap().level,
        cap - 1,
        "exactly one level, still short of the ceiling"
    );
    assert_eq!(
        game.notifications_pending(),
        0,
        "levelling below the cap is not news"
    );

    // The level that lands on it is.
    game.award_player_xp(player, crate::progression::xp_for_level(cap - 1));
    assert_eq!(game.world.get::<Experience>(player).unwrap().level, cap);
    assert_eq!(
        game.take_notification().map(|n| n.title),
        Some("Ceiling Reached".to_string()),
        "the level that lands on the cap says so"
    );

    // Nothing more this sector: further XP banks instead of levelling, so the
    // branch it fires from is never reached again.
    game.award_player_xp(player, crate::progression::xp_for_level(cap) * 4);
    assert_eq!(
        game.notifications_pending(),
        0,
        "XP at the cap banks, and banking is not a second announcement"
    );

    // A breach raises the ceiling, and the next one announces itself too.
    game.enter_next_zone();
    drain(&mut game);
    let raised = game.level_cap();
    assert!(
        raised > cap,
        "a breach has to raise the ceiling for this leg"
    );
    set_level(&mut game, player, raised - 1);
    game.award_player_xp(player, crate::progression::xp_for_level(raised - 1));
    assert_eq!(game.world.get::<Experience>(player).unwrap().level, raised);
    assert_eq!(
        game.take_notification().map(|n| n.title),
        Some("Ceiling Reached".to_string()),
        "Always, so the second sector's ceiling is news again"
    );
}

/// The warning that sweeps have started is hung on the breach *into*
/// `RAID_MIN_ZONE` and nowhere else. A gate written as `>=` would re-fire it
/// at every breach for the rest of the run, and one written against a
/// literal would keep firing at sector 2 after the constant moved — so the
/// call site reads the constant and this reads it too.
#[test]
fn the_sweep_warning_fires_on_the_breach_into_the_first_swept_sector() {
    let mut game = fresh();

    for _ in 1..tuning::RAID_MIN_ZONE {
        drain(&mut game);
        game.enter_next_zone();
        assert!(
            titles(&mut game).contains(&"Sweeps Begin".to_string()),
            "the breach into sector {} says nothing about sweeps",
            tuning::RAID_MIN_ZONE
        );
    }

    for _ in 0..2 {
        drain(&mut game);
        game.enter_next_zone();
        assert!(
            !titles(&mut game).contains(&"Sweeps Begin".to_string()),
            "a later breach repeats the warning"
        );
    }
}

/// Every title the queue is holding, drained.
fn titles(game: &mut Game) -> Vec<String> {
    std::iter::from_fn(|| game.take_notification())
        .map(|n| n.title)
        .collect()
}

// --- FTUE trigger hooks. `Game::ftue` latches a key even with no entry, so
// each hook is observable through `ftue_shown` without a shipped message. ---

#[test]
fn finishing_research_fires_its_ftue_key() {
    let mut game = fresh();
    let def = game
        .world
        .resource::<crate::research::ResearchDb>()
        .get("automation")
        .cloned()
        .expect("automation ships");
    assert!(!game.ftue_shown("research:automation"));
    game.grant_research(&def);
    assert!(game.ftue_shown("research:automation"));
}

#[test]
fn a_crew_built_structure_fires_its_ftue_key() {
    let mut game = fresh();
    stand_in_base(&mut game);
    place_home(&mut game);
    assert!(game.ftue_shown("built:home"), "the instant Home path");
    assert!(!game.ftue_shown("built:depot"));
    // `place_now` raises through `spawn_structure` directly; the crew's
    // completion is `finish_site_now`, which runs `complete_new_build`.
    file_build(&mut game, "depot", 3, 0).unwrap();
    let site = game
        .world
        .query::<(bevy_ecs::prelude::Entity, &BuildSite)>()
        .iter(&game.world)
        .map(|(e, _)| e)
        .next()
        .expect("the depot is filed");
    game.finish_site_now(site);
    assert!(game.ftue_shown("built:depot"), "the crew-built path");
}

#[test]
fn hand_crafting_fires_its_ftue_key() {
    let mut game = fresh();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
    let key = format!("crafted:{}", ids::ICE_BREAKER);
    assert!(!game.ftue_shown(&key));
    game.craft(&ItemId::from(ids::ICE_BREAKER), 1, false)
        .unwrap();
    assert!(game.ftue_shown(&key));
}

/// Stock on hand satisfies an order without anything being made, so it must
/// not spend the run's `crafted:` message.
#[test]
fn stock_that_satisfies_a_work_order_is_not_a_craft() {
    let mut game = fresh();
    stand_in_base(&mut game);
    place_home(&mut game);
    let mine = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    game.world
        .get_mut::<crate::components::Stock>(mine)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 5);
    game.tick();
    assert!(!game.ftue_shown(&format!("crafted:{}", ids::CORE_FRAGMENT)));
}
