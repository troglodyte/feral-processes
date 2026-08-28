//! A meter as block glyphs.
//!
//! Returns strings rather than drawing, so a bar is just another pair of
//! runs on the baseline it shares with its label and `Painter::ui_runs`
//! draws it. That is what lets a bar sit *inline* in a border strip: there
//! is no new drawing primitive here and none is wanted.
//!
//! `bars.rs::draw_bar` is the other shape a meter takes in this frontend — a
//! label over a track, occupying two rows of a panel. It has four callers and
//! is left alone; it is simply the wrong shape for a strip.

/// A filled cell.
const FULL: char = '\u{2588}';

/// The two halves of a meter, ready to become `TextRun`s in the fill colour
/// and `palette::BAR_TROUGH`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::render) struct Bar {
    pub filled: String,
    pub empty: String,
}

/// A meter `width` cells wide.
///
/// **Rounds down, and that is the rule**: a bar reads full only at max, so
/// 509/510 shows 15 of 16 with the sixteenth in trough colour. Rounding to
/// nearest makes "nearly full" and "full" indistinguishable at a glance,
/// which on an Integrity meter is the difference the player most needs to
/// see.
pub(in crate::render) fn bar(value: f32, max: f32, width: usize) -> Bar {
    // No ceiling and no floor are applied on top of these three arms, and
    // none is needed: the middle arm returns `width` exactly, the last is
    // reached only when `value < max` so its floor is at most `width - 1`,
    // and a negative float saturates to zero on the cast. Both guards were
    // written and then removed after neither could be made to fail —
    // `a_bar_never_exceeds_its_width` pins the property they were defending.
    let filled = if max <= 0.0 {
        // A structure with no durability and a level-0 XP target both reach
        // here; neither is an error and neither has anything to show.
        0
    } else if value >= max {
        width
    } else {
        ((value / max) * width as f32).floor() as usize
    };
    Bar {
        filled: FULL.to_string().repeat(filled),
        empty: FULL.to_string().repeat(width - filled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(b: &Bar) -> (usize, usize) {
        (b.filled.chars().count(), b.empty.chars().count())
    }

    /// The rule. The obvious `round()` implementation fails this, which is
    /// the point of asserting it.
    #[test]
    fn a_bar_reads_full_only_at_max() {
        assert_eq!(cells(&bar(509.0, 510.0, 16)), (15, 1));
        assert_eq!(cells(&bar(510.0, 510.0, 16)), (16, 0));
        // Just under half still reads under half.
        assert_eq!(cells(&bar(7.9, 16.0, 16)), (7, 9));
    }

    /// An unclamped ratio writes a bar wider than its column, which on a
    /// border strip pushes everything after it off the pane in silence.
    #[test]
    fn a_bar_never_exceeds_its_width() {
        let max = 510.0;
        let mut v = -10.0f32;
        while v <= max * 2.0 {
            let b = bar(v, max, 16);
            let (f, e) = cells(&b);
            assert_eq!(f + e, 16, "bar({v}, {max}, 16) is {f}+{e} cells");
            v += 7.3;
        }
    }

    /// Neither a panic nor a full bar: nothing is known, so nothing is shown.
    #[test]
    fn a_zero_max_bar_is_empty_not_a_panic() {
        assert_eq!(cells(&bar(5.0, 0.0, 16)), (0, 16));
        assert_eq!(cells(&bar(0.0, 0.0, 16)), (0, 16));
    }

    /// A zero-width bar is a legitimate outcome of a strip that has run out
    /// of room, and must not panic on the `width - filled` subtraction.
    #[test]
    fn a_zero_width_bar_is_two_empty_strings() {
        assert_eq!(cells(&bar(5.0, 10.0, 0)), (0, 0));
        assert_eq!(cells(&bar(50.0, 10.0, 0)), (0, 0));
    }
}
