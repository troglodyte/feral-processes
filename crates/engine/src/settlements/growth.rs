//! When a Server becomes a Mainframe, and how well a Mainframe is doing.
//!
//! Two axes and only one of them is stored. **The clock is derived** — a
//! due tick folded out of the world seed and a region, on
//! `placement.rs`'s machinery and under its three prohibitions: no
//! `GameRng` (a draw does not survive a save/load and shifts every later
//! roll), no `StdRng` sequence (not stable across a `rand` upgrade), and
//! **never `%`** (see `derive::index`; a `%` against a small span reads the
//! low bits and anti-correlates neighbouring regions).
//!
//! **The scalar is signed and 0 means "as it was found."** That is the
//! whole reason it is not a counter: an unsigned commerce starting at zero
//! would band every authored Mainframe as Starved in every fresh world,
//! before anyone had traded with it. Signed, zero bands Steady and the
//! player meets a city as its author wrote it.
//!
//! Nothing here reads a `Game`. `game/settlement_growth.rs` is the half
//! that does.

use crate::tuning::{
    SETTLEMENT_COMMERCE_MAX, SETTLEMENT_COMMERCE_MIN, SETTLEMENT_COMMERCE_PULL_TICKS,
    SETTLEMENT_COMMERCE_STARVED, SETTLEMENT_COMMERCE_THRIVING, SETTLEMENT_GROWTH_DUE_MAX,
    SETTLEMENT_GROWTH_DUE_MIN, SETTLEMENT_GROWTH_SALT, SETTLEMENT_MAINFRAME_BONUS_SHARE,
    SETTLEMENT_MAINFRAME_ROWS, SETTLEMENT_SERVER_BONUS_SHARE, SETTLEMENT_SERVER_ROWS,
    SETTLEMENT_STEADY_BONUS_SHARE, SETTLEMENT_STEADY_ROWS,
};

use super::SettlementKey;

/// The tick this region's Server is due to become a Mainframe on the clock
/// alone, before any trade pulls it forward.
///
/// A fifth salted question off `placement::region_seed`, and deliberately
/// not a fifth fold: `salted`'s doc states why one base with per-question
/// salts is the scheme, and a second base here could collide with the four
/// that decide whether a town exists, where, and which.
pub fn due_tick(seed: u32, key: SettlementKey) -> u64 {
    let span = (SETTLEMENT_GROWTH_DUE_MAX - SETTLEMENT_GROWTH_DUE_MIN).max(1) as usize;
    let base = super::placement::region_seed(seed, key);
    let offset = crate::derive::index(super::placement::salted(base, SETTLEMENT_GROWTH_SALT), span);
    SETTLEMENT_GROWTH_DUE_MIN + offset as u64
}

/// How many ticks `commerce` moves a Server's due date, signed.
///
/// Positive pulls it earlier; **negative pushes it later**, which is how a
/// neglected or Hostile Server stalls without a second rule that could
/// disagree with this one. `i64` because the answer is a signed offset
/// against a `u64` clock and the caller does the saturating arithmetic.
pub fn pull_ticks(commerce: i32) -> i64 {
    commerce as i64 * SETTLEMENT_COMMERCE_PULL_TICKS as i64
}

/// The bounds every commerce writer clamps to.
///
/// `relations::clamp`'s rule and its reason: one clamp is enough only
/// because there is one door. If a second writer ever appears, it goes
/// through `Game::adjust_commerce`, not around this.
pub fn clamp_commerce(commerce: i32) -> i32 {
    commerce.clamp(SETTLEMENT_COMMERCE_MIN, SETTLEMENT_COMMERCE_MAX)
}

/// How well a grown city is doing — **derived on every read, never
/// stored**.
///
/// `relations::band`'s rule exactly, and for its reason: a retune of the
/// thresholds re-bands every existing save rather than leaving cities filed
/// under a boundary that has moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Vitality {
    Starved,
    Steady,
    Thriving,
}

/// The one banding. Ordered from the bottom, `relations::band`'s shape.
pub fn vitality(commerce: i32) -> Vitality {
    if commerce <= SETTLEMENT_COMMERCE_STARVED {
        Vitality::Starved
    } else if commerce < SETTLEMENT_COMMERCE_THRIVING {
        Vitality::Steady
    } else {
        Vitality::Thriving
    }
}

impl Vitality {
    pub fn label(self) -> &'static str {
        match self {
            Vitality::Starved => "Starved",
            Vitality::Steady => "Steady",
            Vitality::Thriving => "Thriving",
        }
    }

    /// How many shelf rows a grown city at this band draws.
    ///
    /// Exhaustive on purpose, `Temperament::buy_mult`'s reason: a fourth
    /// band with no row count is one that reads as broken rather than as
    /// neutral. **A Server never asks this** — it has nowhere to fall to,
    /// and its commerce is spoken for by `due_tick`.
    pub(crate) fn rows(self) -> u32 {
        match self {
            Vitality::Starved => SETTLEMENT_SERVER_ROWS,
            Vitality::Steady => SETTLEMENT_STEADY_ROWS,
            Vitality::Thriving => SETTLEMENT_MAINFRAME_ROWS,
        }
    }

    /// This band's standout share. Exhaustive, `rows`' reason.
    pub(crate) fn bonus_share(self) -> u32 {
        match self {
            Vitality::Starved => SETTLEMENT_SERVER_BONUS_SHARE,
            Vitality::Steady => SETTLEMENT_STEADY_BONUS_SHARE,
            Vitality::Thriving => SETTLEMENT_MAINFRAME_BONUS_SHARE,
        }
    }
}

/// Zero must band Steady, at both ends.
///
/// **A compile-time assertion and not a test**, `SETTLEMENT_GARRISON_MAX`'s
/// precedent: the module doc's whole argument for a signed scalar is that an
/// untouched authored Mainframe reads as its author wrote it, and closing
/// that by retune must fail the *build* rather than one test in a suite
/// somebody could mark ignored.
const _: () = assert!(SETTLEMENT_COMMERCE_STARVED < 0);
const _: () = assert!(0 < SETTLEMENT_COMMERCE_THRIVING);

/// The dwindle never falls through the Server floor, and never rises past
/// the Mainframe ceiling. An `M` drawing fewer rows than an `s` would make
/// the label a lie. A `const _` for the reason above.
const _: () = assert!(SETTLEMENT_SERVER_ROWS <= SETTLEMENT_STEADY_ROWS);
const _: () = assert!(SETTLEMENT_STEADY_ROWS <= SETTLEMENT_MAINFRAME_ROWS);
const _: () = assert!(SETTLEMENT_SERVER_BONUS_SHARE <= SETTLEMENT_STEADY_BONUS_SHARE);
const _: () = assert!(SETTLEMENT_STEADY_BONUS_SHARE <= SETTLEMENT_MAINFRAME_BONUS_SHARE);

/// The span `derive::index` reduces into must be non-empty, or the fold
/// divides into nothing.
const _: () = assert!(SETTLEMENT_GROWTH_DUE_MIN < SETTLEMENT_GROWTH_DUE_MAX);

/// **Trade can never found a city at tick zero.** The maximum pull is
/// strictly less than the earliest possible due date, so a player with
/// unlimited Credits still cannot have a Mainframe before
/// `SETTLEMENT_GROWTH_DUE_MIN - (MAX x PULL)` — the clock is always doing
/// some of the work. A `const _` because a retune that closed this would
/// make the ambient half decorative, which is exactly the failure the whole
/// design is arranged against.
const _: () = assert!(
    SETTLEMENT_COMMERCE_MAX as u64 * SETTLEMENT_COMMERCE_PULL_TICKS < SETTLEMENT_GROWTH_DUE_MIN
);

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: i32) -> impl Iterator<Item = SettlementKey> {
        (-n..n).flat_map(move |ry| (-n..n).map(move |rx| SettlementKey { rx, ry }))
    }

    /// The whole point of a derived clock: ask twice, get the same answer.
    /// A due tick that drifted between calls would make a town grow and
    /// un-grow as the player walked in and out of range.
    #[test]
    fn a_region_is_due_at_the_same_tick_every_time() {
        for key in keys(6) {
            assert_eq!(due_tick(4242, key), due_tick(4242, key), "{key:?} drifted");
        }
    }

    /// Two worlds are two different schedules. Without this the seed is not
    /// reaching the fold and every run would grow the same towns at the
    /// same moments.
    #[test]
    fn a_different_seed_schedules_the_towns_differently() {
        let schedule = |seed| keys(8).map(|k| due_tick(seed, k)).collect::<Vec<_>>();
        assert_ne!(schedule(4242), schedule(99));
    }

    /// Every date is inside the authored span. A fold that escaped the span
    /// would put a town's date at tick 0 (grown from the first frame, in
    /// every world) or past `u64` sanity.
    #[test]
    fn every_due_tick_lands_inside_the_authored_span() {
        for key in keys(10) {
            let due = due_tick(4242, key);
            assert!(
                (SETTLEMENT_GROWTH_DUE_MIN..SETTLEMENT_GROWTH_DUE_MAX).contains(&due),
                "{key:?} is due at {due}, outside \
                 {SETTLEMENT_GROWTH_DUE_MIN}..{SETTLEMENT_GROWTH_DUE_MAX}"
            );
        }
    }

    /// The `%` trap, `placement.rs`'s own test one axis over. Reduced with
    /// `%` against a span, neighbouring regions read one low bit of a
    /// coordinate that differs by one and anti-correlate — which would show
    /// up as the map growing in stripes. Asserted as span coverage: a
    /// stripe cannot fill every tenth of the range.
    #[test]
    fn due_ticks_spread_across_the_span_rather_than_clustering() {
        let span = SETTLEMENT_GROWTH_DUE_MAX - SETTLEMENT_GROWTH_DUE_MIN;
        let mut buckets = [0usize; 10];
        for key in keys(16) {
            let offset = due_tick(4242, key) - SETTLEMENT_GROWTH_DUE_MIN;
            buckets[((offset * 10 / span) as usize).min(9)] += 1;
        }
        for (i, count) in buckets.iter().enumerate() {
            assert!(*count > 0, "no town is ever due in tenth {i}: {buckets:?}");
        }
    }

    /// Zero is Steady, and it is the assertion the whole signed design
    /// rests on: zero is where every authored Mainframe starts.
    #[test]
    fn an_untouched_town_is_steady() {
        assert_eq!(vitality(0), Vitality::Steady);
    }

    #[test]
    fn the_bands_answer_at_their_own_thresholds() {
        assert_eq!(vitality(SETTLEMENT_COMMERCE_THRIVING), Vitality::Thriving);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_THRIVING - 1), Vitality::Steady);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_STARVED), Vitality::Starved);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_STARVED + 1), Vitality::Steady);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_MAX), Vitality::Thriving);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_MIN), Vitality::Starved);
    }

    /// A starved city is never worse than a town. The floor is the label's
    /// warrant: an `M` that drew fewer rows than an `s` would be a lie the
    /// page keeps telling.
    #[test]
    fn a_starved_city_never_draws_fewer_rows_than_a_server() {
        for commerce in [SETTLEMENT_COMMERCE_MIN, -1000, SETTLEMENT_COMMERCE_STARVED] {
            assert!(vitality(clamp_commerce(commerce)).rows() >= SETTLEMENT_SERVER_ROWS);
            assert!(
                vitality(clamp_commerce(commerce)).bonus_share() >= SETTLEMENT_SERVER_BONUS_SHARE
            );
        }
    }

    /// And never better than the ceiling the shipped constant already sets.
    #[test]
    fn a_thriving_city_never_draws_more_rows_than_a_mainframe() {
        for commerce in [SETTLEMENT_COMMERCE_MAX, 1000] {
            assert_eq!(
                vitality(clamp_commerce(commerce)).rows(),
                SETTLEMENT_MAINFRAME_ROWS
            );
            assert_eq!(
                vitality(clamp_commerce(commerce)).bonus_share(),
                SETTLEMENT_MAINFRAME_BONUS_SHARE
            );
        }
    }

    /// The clamp is the one every writer goes through, so it has to hold at
    /// both ends and be idempotent.
    #[test]
    fn commerce_clamps_at_both_ends() {
        assert_eq!(clamp_commerce(i32::MAX), SETTLEMENT_COMMERCE_MAX);
        assert_eq!(clamp_commerce(i32::MIN), SETTLEMENT_COMMERCE_MIN);
        assert_eq!(clamp_commerce(0), 0);
        assert_eq!(clamp_commerce(clamp_commerce(999)), SETTLEMENT_COMMERCE_MAX);
    }

    /// Negative commerce pushes the date *later*, which is how a neglected
    /// or Hostile Server stalls without a second rule for stalling.
    #[test]
    fn negative_commerce_pushes_the_date_later() {
        assert!(pull_ticks(-10) < 0);
        assert_eq!(pull_ticks(0), 0);
        assert!(pull_ticks(10) > 0);
    }
}
