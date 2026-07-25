//! Auto-repeat for held keys.
//!
//! macroquad's `is_key_pressed` is edge-triggered and deliberately drops the
//! backend's auto-repeat events: `key_down_event` records a press only when
//! the event is flagged `repeat == false`, and the pressed set is cleared
//! every frame. A held arrow key therefore delivers exactly one press no
//! matter how long it's held. Letter keys escape this only by arriving
//! through `get_char_pressed`, whose queue is fed regardless of the flag —
//! which is why `hjkl` repeat and the arrows do not.
//!
//! `KeyRepeat` reconstructs the repeat from the *held* state instead, so its
//! cadence is the game's rather than the player's desktop key settings.

use macroquad::prelude::KeyCode;

/// How long a key must be held before it starts repeating.
const REPEAT_DELAY: f64 = 0.25;
/// Spacing between repeats once one has started.
const REPEAT_INTERVAL: f64 = 0.09;

enum Held {
    /// Wall-clock time this key is next due to fire.
    Due(f64),
    /// Held, but silenced until released and pressed again.
    Blocked,
}

pub struct KeyRepeat {
    keys: Vec<(KeyCode, Held)>,
}

impl KeyRepeat {
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Given every key currently held down, returns the ones to deliver as
    /// presses this frame: each key fires the moment it goes down, then
    /// again every `REPEAT_INTERVAL` once it has been held `REPEAT_DELAY`.
    pub fn tick(&mut self, now: f64, held: &[KeyCode]) -> Vec<KeyCode> {
        self.keys.retain(|(key, _)| held.contains(key));
        let mut fired = Vec::new();
        for &key in held {
            match self.keys.iter_mut().find(|(k, _)| *k == key) {
                None => {
                    self.keys.push((key, Held::Due(now + REPEAT_DELAY)));
                    fired.push(key);
                }
                Some((_, Held::Due(due))) if now >= *due => {
                    *due = now + REPEAT_INTERVAL;
                    fired.push(key);
                }
                Some(_) => {}
            }
        }
        fired
    }

    /// Silences every held key until it is released. Walking into a wild
    /// creature swaps the map for the battle menu mid-hold, and the arrow
    /// key that took that step must not go on to scroll the menu it just
    /// opened.
    pub fn block_held(&mut self) {
        for (_, state) in &mut self.keys {
            *state = Held::Blocked;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tap has to stay a tap: the repeat starts *later*, it doesn't turn
    /// every frame of a press into a move.
    #[test]
    fn a_new_press_fires_once_and_then_waits_out_the_delay() {
        let mut repeat = KeyRepeat::new();
        assert_eq!(repeat.tick(0.0, &[KeyCode::Up]), vec![KeyCode::Up]);
        for frame in 1..15 {
            let now = frame as f64 / 60.0;
            assert!(now < REPEAT_DELAY, "test only covers the pre-delay frames");
            assert!(
                repeat.tick(now, &[KeyCode::Up]).is_empty(),
                "a key held for {now}s is still inside the delay"
            );
        }
    }

    /// The reported bug: holding an arrow key moved the player exactly one
    /// tile, because macroquad's `is_key_pressed` drops the backend's
    /// auto-repeat events.
    #[test]
    fn a_held_key_repeats_at_the_interval_once_the_delay_has_passed() {
        let mut repeat = KeyRepeat::new();
        assert_eq!(repeat.tick(0.0, &[KeyCode::Right]), vec![KeyCode::Right]);

        assert!(
            repeat
                .tick(REPEAT_DELAY - 0.001, &[KeyCode::Right])
                .is_empty()
        );
        assert_eq!(
            repeat.tick(REPEAT_DELAY, &[KeyCode::Right]),
            vec![KeyCode::Right]
        );
        assert!(
            repeat
                .tick(REPEAT_DELAY + REPEAT_INTERVAL - 0.001, &[KeyCode::Right])
                .is_empty(),
            "repeats are paced by the interval, not by the frame rate"
        );
        assert_eq!(
            repeat.tick(REPEAT_DELAY + REPEAT_INTERVAL, &[KeyCode::Right]),
            vec![KeyCode::Right]
        );
    }

    /// Releasing and pressing again moves immediately — a second tap must
    /// not be made to sit out the repeat delay.
    #[test]
    fn releasing_a_key_lets_the_next_press_fire_immediately() {
        let mut repeat = KeyRepeat::new();
        repeat.tick(0.0, &[KeyCode::Left]);
        repeat.tick(0.1, &[]);
        assert_eq!(repeat.tick(0.11, &[KeyCode::Left]), vec![KeyCode::Left]);
    }

    /// A key pressed while another is already held starts its own delay
    /// rather than inheriting the elapsed one.
    #[test]
    fn each_key_keeps_its_own_schedule() {
        let mut repeat = KeyRepeat::new();
        repeat.tick(0.0, &[KeyCode::Up]);

        assert_eq!(
            repeat.tick(REPEAT_DELAY, &[KeyCode::Up, KeyCode::Down]),
            vec![KeyCode::Up, KeyCode::Down],
            "Up is due to repeat, Down is a fresh press"
        );
        assert!(
            repeat
                .tick(REPEAT_DELAY + REPEAT_INTERVAL, &[KeyCode::Down])
                .is_empty(),
            "Down waits out its own delay, not Up's"
        );
    }

    /// Walking into a wild creature swaps the map for the battle menu
    /// mid-hold. The key that took that last step must not go on to scroll
    /// the menu it just opened.
    #[test]
    fn a_blocked_key_stays_quiet_until_it_is_released() {
        let mut repeat = KeyRepeat::new();
        repeat.tick(0.0, &[KeyCode::Down]);
        repeat.block_held();

        for frame in 1..=120 {
            let now = frame as f64 / 60.0;
            assert!(
                repeat.tick(now, &[KeyCode::Down]).is_empty(),
                "a blocked key must stay quiet while it stays held ({now}s)"
            );
        }

        repeat.tick(2.1, &[]);
        assert_eq!(
            repeat.tick(2.2, &[KeyCode::Down]),
            vec![KeyCode::Down],
            "releasing it clears the block"
        );
    }
}
