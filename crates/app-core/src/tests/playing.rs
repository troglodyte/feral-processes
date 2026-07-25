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
