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

/// `c` is the collect key, and it is bound on the map rather than being
/// swallowed as an unknown character. Asserted through the log because
/// app-core cannot reach the engine's `World` to look at a buffer — which
/// is the point of the seam, not a limitation of the test.
#[test]
fn c_reaches_the_collect_action() {
    let mut app = test_app(203);
    // `c` collects from the machines around you, and those stand in base
    // space — so the key only reaches the action from inside it.
    stand_in_base(&mut app);
    app.handle_key(GameKey::Char('c'));

    let said = app
        .game
        .as_ref()
        .unwrap()
        .message_log(200)
        .into_iter()
        .any(|e| e.text.contains("nothing to collect"));
    assert!(
        said,
        "pressing C with nothing adjacent should reach Game::collect_adjacent"
    );
}

/// `>` and `<` are the anchor's two doors on the map screen — the same pair
/// the Stack already binds for its link, rather than a third set of keys for
/// a third way of going somewhere.
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

    app.handle_key(GameKey::Char('<'));
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "'<' must reach Game::leave_base"
    );

    app.handle_key(GameKey::Char('>'));
    assert!(
        app.game.as_ref().unwrap().in_base(),
        "'>' must reach Game::enter_base"
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

    app.handle_key(GameKey::Char('>'));

    assert!(!app.game.as_ref().unwrap().in_base());
    let said = app.status_line.clone().expect("a refused key says why");
    assert!(
        said.contains("deploy a Home"),
        "the status line must carry the engine's refusal, got: {said}"
    );
}
