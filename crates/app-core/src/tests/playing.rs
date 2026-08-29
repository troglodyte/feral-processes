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

    app.handle_key(GameKey::Right);
    let sounds = app.take_sounds();
    assert_eq!(
        sounds.len(),
        1,
        "a movement key should queue exactly one sound, got {sounds:?}"
    );
    assert!(
        matches!(sounds[0], SoundEvent::Step | SoundEvent::BattleStart),
        "a movement key should queue Step or BattleStart, got {:?}",
        sounds[0]
    );
    assert!(
        app.take_sounds().is_empty(),
        "take_sounds should drain the queue, not just peek it"
    );
}

/// `update_realtime` is the hook a frontend's own loop calls every
/// frame, independent of `handle_key`, so the world keeps advancing
/// while the player is idle — but only in `Mode::Playing`. Backdates
/// `last_realtime_tick` instead of actually sleeping so the test stays
/// fast and deterministic.
#[test]
fn update_realtime_ticks_once_a_second_only_while_playing() {
    let mut app = test_app(303);
    let start_tick = app.game.as_ref().unwrap().current_tick();

    // Not enough wall-clock time has passed yet.
    app.last_realtime_tick = Instant::now();
    app.update_realtime();
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        start_tick,
        "update_realtime shouldn't tick before a full second has elapsed"
    );

    // A full second (backdated) should fire exactly one idle tick.
    app.last_realtime_tick = Instant::now() - Duration::from_secs(2);
    app.update_realtime();
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        start_tick + 1,
        "update_realtime should advance the world by one tick once a second has passed"
    );

    // Paused outside Playing (any menu, or battle via its own Mode) —
    // no tick, and the timer resets rather than banking elapsed time.
    app.mode = Mode::Inventory;
    app.last_realtime_tick = Instant::now() - Duration::from_secs(5);
    app.update_realtime();
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        start_tick + 1,
        "update_realtime shouldn't tick while paused on a non-Playing mode"
    );
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

    inside.handle_key(GameKey::Up);

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

    outside.handle_key(GameKey::Up);

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

/// SPACE doubles the map screen's log pane and back — `App::log_expanded`,
/// read by `hud::layout::regions` in the renderer. Toggling is not an
/// action, the same as `n` above: reading a wider log must not cost a turn.
#[test]
fn space_toggles_the_log_pane_and_spends_no_turn() {
    let mut app = test_app(9103);
    let tick = app.game.as_ref().unwrap().current_tick();
    assert!(!app.log_expanded, "the log pane starts collapsed");

    app.handle_key(GameKey::Char(' '));
    assert!(app.log_expanded, "SPACE did not expand the log pane");
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        tick,
        "expanding the log spent a turn"
    );

    app.handle_key(GameKey::Char(' '));
    assert!(
        !app.log_expanded,
        "SPACE did not collapse the log pane back"
    );
}

/// **The load-bearing one**, `the_digits_work_underground`'s reason: this
/// match runs before the hand-off to `handle_stack_key`, which ends in
/// `_ => {}`, so a key that reached it instead would be a swallowed
/// keypress with no refusal and nothing in the log — how `r` (rest) shipped
/// broken underground. The log pane the toggle resizes is drawn on the
/// Stack view too, so the toggle has to reach both locales.
#[test]
fn space_toggles_the_log_pane_underground_too() {
    let mut app = app_underground(9104);
    assert!(app.game.as_ref().unwrap().is_underground());

    app.handle_key(GameKey::Char(' '));

    assert!(app.log_expanded, "SPACE was swallowed underground");
    assert!(
        app.status_line.is_none(),
        "the key was refused rather than acted on: {:?}",
        app.status_line
    );
}
