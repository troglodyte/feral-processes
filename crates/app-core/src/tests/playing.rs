//! Movement on the map, and the idle clock.

use super::support::*;
use crate::*;

/// `SoundEvent`s are the seam frontends use to play movement/battle
/// sound effects — this doesn't try to reach every variant (the
/// engine's own battle tests already cover the mechanics that decide
/// which one fires), just locks in that a movement key queues exactly
/// one of `Step`/`BattleStart`, that a non-movement key queues neither,
/// and that `take_sounds` actually drains the queue rather than
/// leaking across keypresses.
#[test]
fn movement_keys_queue_exactly_one_step_or_battle_start_sound() {
    let mut app = test_app(202);
    assert!(
        app.take_sounds().is_empty(),
        "a fresh App should start with no queued sounds"
    );

    app.handle_key(GameKey::Char('.'));
    assert!(
        app.take_sounds().is_empty(),
        "waiting isn't a movement key and shouldn't queue a movement sound"
    );

    // Queuing a step alone fires nothing — `travel-on-the-clock` moved the
    // cue to the tick that actually spends it (`walk`, below).
    app.handle_key(GameKey::Right);
    assert!(
        app.take_sounds().is_empty(),
        "queuing a step should not itself queue a sound"
    );
    app.update_realtime(1.0 / app.world_speed.ticks_per_second());
    let sounds = app.take_sounds();
    assert_eq!(
        sounds.len(),
        1,
        "a movement key should queue exactly one sound, got {sounds:?}"
    );
    assert!(
        // `Hit` is the third legal answer: a step onto ground carrying an
        // attrition condition sounds like taking damage rather than like
        // walking — see `a_step_that_attrits_sounds_like_taking_a_hit`. It
        // cannot arise at this seed's starting zone, which is gated out of
        // conditions entirely, but naming it here keeps this test a
        // statement about the cue set rather than a tripwire on the zone.
        matches!(
            sounds[0],
            SoundEvent::Step | SoundEvent::BattleStart | SoundEvent::Hit
        ),
        "a movement key should queue Step, BattleStart or Hit, got {:?}",
        sounds[0]
    );
    assert!(
        app.take_sounds().is_empty(),
        "take_sounds should drain the queue, not just peek it"
    );
}

fn tick_of(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

fn player_pos(app: &App) -> (i32, i32) {
    app.game.as_ref().unwrap().player_status().position
}

/// `travel-on-the-clock`'s own claim: an unpaused arrow queues a step
/// rather than spending one, so `handle_key` alone must not move the
/// player or the clock — only `update_realtime` may.
#[test]
fn an_arrow_moves_the_player_on_the_next_tick_and_not_in_handle_key() {
    let mut app = test_app(2601);
    clear_the_area_around_player(&mut app);
    let start_tick = tick_of(&app);
    let start_pos = player_pos(&app);

    app.handle_key(GameKey::Right);
    assert_eq!(tick_of(&app), start_tick, "handle_key alone spent a tick");
    assert_eq!(
        player_pos(&app),
        start_pos,
        "handle_key alone moved the player"
    );

    app.update_realtime(1.0 / app.world_speed.ticks_per_second());
    assert_eq!(
        tick_of(&app),
        start_tick + 1,
        "the queued step did not spend the tick it was owed"
    );
    assert_eq!(
        player_pos(&app),
        (start_pos.0 + 1, start_pos.1),
        "the queued step did not move the player"
    );
}

/// **The regression this feature exists to fix.** Before `travel-on-the-
/// clock`, `handle_key` spent a tick per press and gui's key repeat fired
/// far faster than any `WorldSpeed` (`REPEAT_INTERVAL` is 0.09s, against
/// Normal's 0.5s tick) — holding a direction ran the world at ~11 ticks/s
/// regardless of the speed setting. This re-sends the key several times a
/// tick, faster than the clock itself, and checks that `N` seconds of that
/// still spends and walks exactly `ticks_per_second * N` — the clock, not
/// the keyboard, deciding how far the party got. A held key's repeats all
/// overwrite the same pending `Walk::Step`, so any repeat rate is legal
/// input for this claim; `REPEATS_PER_TICK` only has to be more than one.
#[test]
fn holding_an_arrow_spends_exactly_the_clocks_ticks() {
    const REPEATS_PER_TICK: u32 = 4;
    const SECONDS: u32 = 1;

    let mut app = test_app(2602);
    clear_the_area_around_player(&mut app);
    let start_tick = tick_of(&app);
    let start_pos = player_pos(&app);
    let ticks_per_second = app.world_speed.ticks_per_second();
    let expected_ticks = (ticks_per_second as u32) * SECONDS;
    let frame = 1.0 / (ticks_per_second * REPEATS_PER_TICK as f32);

    for _ in 0..(expected_ticks * REPEATS_PER_TICK) {
        app.handle_key(GameKey::Right);
        app.update_realtime(frame);
    }

    assert_eq!(
        tick_of(&app),
        start_tick + expected_ticks as u64,
        "holding an arrow must spend exactly the clock's own ticks, not one per repeat"
    );
    assert_eq!(
        player_pos(&app),
        (start_pos.0 + expected_ticks as i32, start_pos.1),
        "holding an arrow must walk exactly the clock's own cells, not one per repeat"
    );
}

/// Pause keeps the turn-based path: an arrow still steps immediately and
/// spends exactly one tick, `acting_while_paused_still_spends_a_turn`'s own
/// case for a key that is now queued rather than acted on everywhere else.
#[test]
fn a_paused_arrow_still_steps_immediately_and_spends_one_tick() {
    let mut app = test_app(2603);
    clear_the_area_around_player(&mut app);
    app.handle_key(GameKey::Char(' '));
    assert!(app.paused);
    let start_tick = tick_of(&app);
    let start_pos = player_pos(&app);

    app.handle_key(GameKey::Right);

    assert_eq!(
        tick_of(&app),
        start_tick + 1,
        "a paused arrow did not spend a turn immediately"
    );
    assert_eq!(
        player_pos(&app),
        (start_pos.0 + 1, start_pos.1),
        "a paused arrow did not move the player immediately"
    );
}

/// **The regression finding 1 exists for.** A queued step that bounces off
/// base rock with mining off spends no world tick inside the engine at
/// all — `Game::move_in_base` refuses it for free. `spend_walk_tick` must
/// still spend the clock's own tick regardless, through `Game::idle_tick`,
/// or holding a direction into a wall freezes the world rather than merely
/// refusing to walk through it.
#[test]
fn a_step_that_moves_nobody_still_spends_the_clocks_tick() {
    let mut app = test_app(2604);
    found_the_base(&mut app);
    // (5, 0) sits outside the starting pocket `found_the_base` lays
    // (`tuning::STARTING_POCKET_RADIUS` is 4), so it stays solid rock and
    // mining is off by default — every eastward bump from (4, 0) refuses
    // for free, spending no tick of its own.
    stand_in_base_at(&mut app, 4, 0);
    let start_tick = tick_of(&app);

    const HELD_SECONDS: u32 = 3;
    let ticks_per_second = app.world_speed.ticks_per_second();
    let expected_ticks = (ticks_per_second as u32) * HELD_SECONDS;
    for _ in 0..expected_ticks {
        app.handle_key(GameKey::Right);
        app.update_realtime(1.0 / ticks_per_second);
    }

    assert_eq!(
        tick_of(&app),
        start_tick + expected_ticks as u64,
        "a step that moved nobody must still spend exactly the clock's own ticks"
    );
}

/// A queued `Walk::Step` is cleared by any key that isn't itself a step,
/// exactly as a pending `Walk::Travel` already was — the player asked for
/// something else, and a stale step spent on the *next* tick after an
/// unrelated action would move them without being asked again.
#[test]
fn a_queued_step_is_cleared_by_a_following_non_move_key() {
    let mut app = test_app(2605);
    clear_the_area_around_player(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.walk,
        Some(Walk::Step(1, 0)),
        "test premise: a step is queued"
    );

    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.walk, None,
        "a non-move key must clear a queued step, not just a queued travel"
    );
}

/// The same clearing rule, exercised through a key that opens a screen
/// rather than one that merely waits — a popup taking the mode away from
/// `Playing` must not leave a stale step for the clock to spend once the
/// player is back, which the same early check in `handle_playing_key`
/// already gives for free.
#[test]
fn a_popup_key_clears_a_pending_walk_before_it_opens() {
    let mut app = test_app(2606);
    clear_the_area_around_player(&mut app);
    app.handle_key(GameKey::Right);
    assert!(app.walk.is_some(), "test premise: a step is queued");

    app.handle_key(GameKey::Char('i'));

    assert_eq!(app.walk, None, "opening a screen must clear a pending walk");
    assert_eq!(app.mode, Mode::Inventory);
}

/// `update_realtime` is the hook a frontend's own loop calls every frame,
/// independent of `handle_key`, so the world keeps advancing while the
/// player is idle. It paces against the frame's `dt` with a carry, so a
/// frame shorter than a tick banks its share rather than losing it.
#[test]
fn update_realtime_paces_idle_ticks_against_dt() {
    let mut app = test_app(303);
    let start = tick_of(&app);
    let tick = 1.0 / WorldSpeed::Normal.ticks_per_second();

    app.update_realtime(tick * 0.6);
    assert_eq!(tick_of(&app), start, "ticked before a tick's worth of time");

    app.update_realtime(tick * 0.6);
    assert_eq!(
        tick_of(&app),
        start + 1,
        "two short frames did not add up to the tick they bought"
    );
}

/// Every mode but `Playing` holds the clock, and the carry resets rather
/// than banking the time spent in a menu.
#[test]
fn update_realtime_holds_the_clock_off_the_map() {
    let mut app = test_app(304);
    let start = tick_of(&app);
    let tick = 1.0 / WorldSpeed::Normal.ticks_per_second();

    app.update_realtime(tick * 0.9);
    app.mode = Mode::Inventory;
    app.update_realtime(tick * 5.0);
    assert_eq!(tick_of(&app), start, "the world ran behind a menu");

    app.mode = Mode::Playing;
    app.update_realtime(tick * 0.2);
    assert_eq!(
        tick_of(&app),
        start,
        "time from before the menu was banked across it"
    );
}

/// A frame of seconds — the window dragged, the app backgrounded — must
/// not come back as a burst of idle ticks the player never saw happen.
#[test]
fn a_long_frame_spends_at_most_the_per_frame_cap() {
    let mut app = test_app(305);
    let start = tick_of(&app);
    app.world_speed = WorldSpeed::Fastest;

    app.update_realtime(30.0);

    assert_eq!(tick_of(&app), start + MAX_IDLE_TICKS_PER_FRAME as u64);
}

/// `Normal` is the world's own two ticks a second and `Fastest` is four
/// times that, over the same second of frames. Literal rates, not
/// `ticks_per_second()`, or a wrong multiple would be checked against itself.
#[test]
fn a_faster_speed_spends_more_ticks_per_second() {
    for (seed, speed, rate) in [
        (306, WorldSpeed::Normal, 2),
        (307, WorldSpeed::Fast, 4),
        (308, WorldSpeed::Fastest, 8),
    ] {
        let mut app = test_app(seed);
        app.world_speed = speed;
        let start = tick_of(&app);
        // A power-of-two frame, so the carry sums exactly in `f32`.
        for _ in 0..64 {
            app.update_realtime(1.0 / 64.0);
        }
        assert_eq!(
            app.mode,
            Mode::Playing,
            "seed {seed} left the map, so the second was cut short"
        );
        assert_eq!(tick_of(&app) - start, rate, "{speed:?}");
    }
}

/// `c` is the transfer key, and it is bound on the map rather than being
/// swallowed as an unknown character. Asserted through the log because
/// app-core cannot reach the engine's `World` to look at a buffer — which
/// is the point of the seam, not a limitation of the test.
#[test]
fn c_reaches_the_transfer_action() {
    let mut app = test_app(203);
    // `c` moves cargo between you and the machines around you, and those
    // stand in base space — so the key only reaches the action from inside
    // it.
    stand_in_base(&mut app);
    app.handle_key(GameKey::Char('c'));

    let said = app
        .game
        .as_ref()
        .unwrap()
        .message_log(200)
        .into_iter()
        .any(|e| e.text.contains("nothing here to take from or put into"));
    assert!(
        said,
        "pressing c with nothing adjacent should reach Game::refuse_transfer"
    );
}

/// `<` and `>` are the anchor's two doors on the map screen — the same pair
/// the Stack already binds for its link, rather than a third set of keys for
/// a third way of going somewhere.
///
/// **The sense is the opposite of the Stack's, and that is the assertion.**
/// The pair reads as up and down before it reads as in and out, and base
/// space is a platform the party steps *onto*: `<` rises into it, `>` drops
/// back to the grid. Bound the other way round it told the player they were
/// descending into a base that stands above the ground they left.
///
/// Driven out and back in, because a fixture with a Home standing is the
/// only one that can be entered at all: the anchor leads nowhere until one
/// is deployed. Going out first also proves the entry half is not passing on
/// a locale the fixture handed it.
#[test]
fn the_link_keys_walk_out_of_the_base_and_back_in_through_the_anchor() {
    let mut app = app_inside_a_small_base_with_programs(215, false, 1);
    assert!(
        app.game.as_ref().unwrap().in_base(),
        "the fixture must start inside the base"
    );

    app.handle_key(GameKey::Char('>'));
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "'>' must reach Game::leave_base — down off the platform"
    );

    app.handle_key(GameKey::Char('<'));
    assert!(
        app.game.as_ref().unwrap().in_base(),
        "'<' must reach Game::enter_base — up onto the platform"
    );
}

/// A refused crossing puts the engine's own words on the status line. Wired
/// the way the Stack path already wires `o` and `Z`: a refusal is not an
/// action, so it must not be cleared by the bookkeeping that follows one.
///
/// The refusal asserted on is the *no Home* one specifically — a fresh run
/// starts standing on the anchor, so anything about position would mean the
/// key had reached something other than `Game::enter_base`.
#[test]
fn a_refused_crossing_reports_the_engines_own_reason() {
    let mut app = test_app(216);

    app.handle_key(GameKey::Char('<'));

    assert!(!app.game.as_ref().unwrap().in_base());
    let said = app.status_line.clone().expect("a refused key says why");
    assert!(
        said.contains("deploy a Home"),
        "the status line must carry the engine's refusal, got: {said}"
    );
}

/// `.` is a zone-surface and Stack verb, and does nothing at all in base
/// space — no turn, no refusal, nothing on the line. A **dead key**, not a
/// refused one, which is the deliberate exception to the rule the rest of
/// this screen follows: `r`, `<`, `>` and `v` all hand the engine's own
/// sentence to `App::refuse` rather than fall through. Base space is time
/// you spend by walking it, and a key that stands still there is meant to
/// read as not being a key at all.
///
/// The surface half is the control, and is not optional: a `.` arm that had
/// been deleted outright rather than gated would pass the base assertions
/// below without it.
#[test]
fn waiting_is_a_dead_key_in_base_space() {
    let mut app = test_app(219);

    let surface_tick = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('.'));
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        surface_tick + 1,
        "the control: `.` on the zone surface still spends the turn"
    );

    stand_in_base(&mut app);
    // Set *after* the fixture is in place, so what it proves is that the
    // keypress left it standing — `App::after_world_action` clears this line
    // on any real action, so a `.` that still ticked would wipe it.
    app.status_line = Some("an earlier refusal".to_string());
    let tick = app.game.as_ref().unwrap().current_tick();

    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        tick,
        "`.` must spend no turn in base space"
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("an earlier refusal"),
        "and must say nothing of its own — a dead key, not a refusal"
    );
}

/// `stepped` reads the clock rather than assuming, and slice 2 is what
/// makes that pay: a movement key into base-space rock used to be refused
/// for free and is a swing now (`Game::strike_rock`), so the same keypress
/// changed from no action to one — without a line of app-core changing.
/// The status line explaining an earlier refusal clears, because something
/// really happened.
///
/// The surface half is the control: the identical keypress there is an
/// action too, so a `stepped` that always answered `true` would pass this
/// twice over — which is why the turn is asserted as well as the line.
#[test]
fn a_swing_at_rock_in_base_space_is_an_action() {
    let mut inside = app_inside_a_small_base_with_programs(217, false, 1);
    assert!(
        inside.game.as_ref().is_some_and(|g| g.in_base()),
        "the fixture must start inside the base, where the rock is"
    );
    // The pocket's north edge, so the step up leaves it and meets rock.
    stand_in_base_at(
        &mut inside,
        0,
        -feral_processes_engine::tuning::STARTING_POCKET_RADIUS,
    );
    // Armed first: a disarmed bump is refused for free, which is the whole
    // of `MiningMode`. What this test is about is that an *armed* swing is
    // an action.
    inside.handle_key(GameKey::Char('n'));
    inside.status_line = Some("an earlier refusal".to_string());
    let _ = inside.take_sounds();
    let tick = inside.game.as_ref().unwrap().current_tick();

    walk(&mut inside, GameKey::Up);

    assert_eq!(
        inside.game.as_ref().unwrap().current_tick(),
        tick + 1,
        "a swing at rock spends the turn a step would have"
    );
    assert_eq!(
        inside.status_line, None,
        "and clears the line, because something really happened"
    );
    assert_eq!(
        inside.take_sounds().len(),
        1,
        "and cues the movement key's one sound, as shoving at a wall on the \
         surface already does"
    );

    let mut outside = test_app(218);
    outside.status_line = Some("an earlier refusal".to_string());
    let _ = outside.take_sounds();

    walk(&mut outside, GameKey::Up);

    assert_eq!(
        outside.status_line, None,
        "the control: a real step on the open grid still clears the line"
    );
    assert_eq!(
        outside.take_sounds().len(),
        1,
        "and still plays exactly one sound"
    );
}

/// `n` arms and disarms the player's own bump into rock, and costs no turn
/// doing it: picking a tool up is not an action, so the handler `return`s
/// rather than falling through to the tick.
#[test]
fn n_toggles_the_players_bump_and_spends_no_turn() {
    let mut app = test_app(9101);
    stand_in_base(&mut app);
    let tick = app.game.as_ref().unwrap().current_tick();
    assert!(!app.game.as_ref().unwrap().mining());

    app.handle_key(GameKey::Char('n'));
    assert!(app.game.as_ref().unwrap().mining(), "n did not arm mining");
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        tick,
        "arming a tool spent a turn"
    );

    app.handle_key(GameKey::Char('n'));
    assert!(
        !app.game.as_ref().unwrap().mining(),
        "n did not disarm mining"
    );
}

/// Out on the surface there is no rock to cut, so `n` says so rather than
/// arming a tool that can never fire — the same refusal `d` and `m` make,
/// and for the same reason.
#[test]
fn n_is_refused_outside_base_space() {
    let mut app = test_app(9102);
    assert!(!app.game.as_ref().unwrap().in_base());

    app.handle_key(GameKey::Char('n'));

    assert!(!app.game.as_ref().unwrap().mining());
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("through the anchor")),
        "no refusal shown: {:?}",
        app.status_line
    );
}

/// SPACE pauses the idle clock and SPACE again resumes it. Neither press is
/// an action, so neither spends a turn.
#[test]
fn space_pauses_the_idle_clock_and_resumes_it() {
    let mut app = test_app(9103);
    let start = tick_of(&app);
    assert!(!app.paused, "a game starts running");

    app.handle_key(GameKey::Char(' '));
    assert!(app.paused, "SPACE did not pause");
    assert_eq!(tick_of(&app), start, "pausing spent a turn");
    app.update_realtime(5.0);
    assert_eq!(tick_of(&app), start, "the world ran while paused");

    app.handle_key(GameKey::Char(' '));
    assert!(!app.paused, "SPACE did not resume");
    app.update_realtime(1.0);
    assert!(
        tick_of(&app) > start,
        "the world did not run after resuming"
    );
}

/// Pause holds the *idle* clock only: an action still spends its own tick
/// through `handle_key`'s tail, so a paused game is a turn-based one rather
/// than a frozen one.
#[test]
fn acting_while_paused_still_spends_a_turn() {
    let mut app = test_app(9106);
    app.handle_key(GameKey::Char(' '));
    let start = tick_of(&app);

    app.handle_key(GameKey::Char('.'));

    assert!(app.paused);
    assert_eq!(
        tick_of(&app),
        start + 1,
        "waiting while paused spent no turn"
    );
}

/// **The load-bearing one**, `the_digits_work_underground`'s reason: this
/// match runs before the hand-off to `handle_stack_key`, which ends in
/// `_ => {}`, so a key that reached it instead would be swallowed with no
/// refusal. The clock runs underground too, so pause has to reach it.
#[test]
fn space_pauses_underground_too() {
    let mut app = app_underground(9104);
    assert!(app.game.as_ref().unwrap().is_underground());

    app.handle_key(GameKey::Char(' '));

    assert!(app.paused, "SPACE was swallowed underground");
    assert!(app.status_line.is_none(), "{:?}", app.status_line);
}

/// `,` cycles the speed and wraps back to `Normal`, and is not an action.
#[test]
fn comma_cycles_the_world_speed() {
    let mut app = test_app(9107);
    let start = tick_of(&app);
    assert_eq!(app.world_speed, WorldSpeed::Normal);

    app.handle_key(GameKey::Char(','));
    assert_eq!(app.world_speed, WorldSpeed::Fast);
    app.handle_key(GameKey::Char(','));
    assert_eq!(app.world_speed, WorldSpeed::Fastest);
    app.handle_key(GameKey::Char(','));
    assert_eq!(app.world_speed, WorldSpeed::Normal, "did not wrap");
    assert_eq!(tick_of(&app), start, "changing speed spent a turn");
}

/// The speed key reaches the Stack for SPACE's reason.
#[test]
fn comma_cycles_the_world_speed_underground_too() {
    let mut app = app_underground(9108);

    app.handle_key(GameKey::Char(','));

    assert_eq!(
        app.world_speed,
        WorldSpeed::Fast,
        "`,` was swallowed underground"
    );
}

/// TAB doubles the map screen's log pane and back — `App::log_expanded`,
/// read by `hud::layout::regions` in the renderer. Toggling is not an
/// action: reading a wider log must not cost a turn.
#[test]
fn tab_toggles_the_log_pane_and_spends_no_turn() {
    let mut app = test_app(9105);
    let start = tick_of(&app);
    assert!(!app.log_expanded, "the log pane starts collapsed");

    app.handle_key(GameKey::Tab);
    assert!(app.log_expanded, "TAB did not expand the log pane");
    assert_eq!(app.mode, Mode::Playing, "TAB changed the mode");
    assert_eq!(tick_of(&app), start, "expanding the log spent a turn");

    app.handle_key(GameKey::Tab);
    assert!(!app.log_expanded, "TAB did not collapse the log pane back");
}

/// The log pane is drawn on the Stack view too, so TAB has to reach it.
#[test]
fn tab_toggles_the_log_pane_underground_too() {
    let mut app = app_underground(9109);

    app.handle_key(GameKey::Tab);

    assert!(app.log_expanded, "TAB was swallowed underground");
    assert!(app.status_line.is_none(), "{:?}", app.status_line);
}

/// Ground that costs Integrity has to *sound* like it. The bite is
/// otherwise indistinguishable from the step that caused it: same key, same
/// footstep, and the HP bar is the only tell.
///
/// The battle `Hit` cue rather than one of its own, deliberately — this is
/// taking damage, and a player who has fought already knows what it means.
///
/// Driven through `after_world_action` rather than by walking until the
/// ground happens to bite: what decides the cue is the figure the engine
/// hands back, and a walk that has to dodge ambushes to reach an attriting
/// tile would be testing worldgen. The engine's
/// `move_player_reports_exactly_what_the_ground_took` pins the other end of
/// that figure.
#[test]
fn a_step_that_attrits_sounds_like_taking_a_hit() {
    let mut app = test_app(202);
    app.take_sounds();

    app.after_world_action(true, true, 3);
    assert_eq!(
        app.take_sounds(),
        vec![SoundEvent::Hit],
        "a step the ground took Integrity off should sound like a hit"
    );

    app.after_world_action(true, true, 0);
    assert_eq!(
        app.take_sounds(),
        vec![SoundEvent::Step],
        "clean ground still sounds like walking"
    );

    app.after_world_action(true, false, 3);
    assert!(
        app.take_sounds().is_empty(),
        "an action that was not a step queues no movement cue, bite or not"
    );
}
