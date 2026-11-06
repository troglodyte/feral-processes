//! The one tick-to-words helper for the Sites screen. The game never shows
//! the player a tick, so every age ("seen recently") and wait ("arrives
//! soon") on the Sites pages goes through here.

use crate::tuning::{SITE_TIME_A_WHILE_TICKS, SITE_TIME_NOW_TICKS, SITE_TIME_SOON_TICKS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeBand {
    Now,
    Soon,
    AWhile,
    Long,
}

pub fn time_band(ticks: u64) -> TimeBand {
    if ticks <= SITE_TIME_NOW_TICKS {
        TimeBand::Now
    } else if ticks <= SITE_TIME_SOON_TICKS {
        TimeBand::Soon
    } else if ticks <= SITE_TIME_A_WHILE_TICKS {
        TimeBand::AWhile
    } else {
        TimeBand::Long
    }
}

impl TimeBand {
    /// How long past an event reads.
    pub fn ago(self) -> &'static str {
        match self {
            TimeBand::Now => "just now",
            TimeBand::Soon => "recently",
            TimeBand::AWhile => "a while ago",
            TimeBand::Long => "long ago",
        }
    }

    /// How long until a coming event reads.
    pub fn until(self) -> &'static str {
        match self {
            TimeBand::Now => "any moment",
            TimeBand::Soon => "soon",
            TimeBand::AWhile => "a while yet",
            TimeBand::Long => "a long way off",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_meet_at_their_edges() {
        assert_eq!(time_band(0), TimeBand::Now);
        assert_eq!(time_band(30), TimeBand::Now);
        assert_eq!(time_band(31), TimeBand::Soon);
        assert_eq!(time_band(300), TimeBand::Soon);
        assert_eq!(time_band(301), TimeBand::AWhile);
        assert_eq!(time_band(1800), TimeBand::AWhile);
        assert_eq!(time_band(1801), TimeBand::Long);
        assert_eq!(time_band(u64::MAX), TimeBand::Long);
    }

    #[test]
    fn no_band_word_carries_a_digit() {
        for band in [
            TimeBand::Now,
            TimeBand::Soon,
            TimeBand::AWhile,
            TimeBand::Long,
        ] {
            for word in [band.ago(), band.until()] {
                assert!(!word.chars().any(|c| c.is_ascii_digit()), "{word}");
                assert!(!word.to_lowercase().contains("tick"), "{word}");
            }
        }
    }
}
