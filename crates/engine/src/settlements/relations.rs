//! What a town thinks of you.
//!
//! A signed `standing` per `SettlementKey`, banded for reading. The band is
//! **derived on every read, never stored** — `resources::Standings` holds
//! the number alone, so a retune of the thresholds re-bands every existing
//! save rather than leaving towns filed under a boundary that has moved.
//!
//! **The consequences are named queries, not a table of effects** — the
//! perks module (`crates/engine/src/perks.rs`) is the shape being copied,
//! and for its reason: a consequence is a hook into a particular formula
//! with no shared shape to express as data. `refuses_service` is the one
//! shipped so far; town-sourced raids, hostile patrols and Phase 6's route
//! predation are each a *new query* answered by the same exhaustive match,
//! not a rewrite. `every_standing_band_answers_whether_it_refuses_service`
//! is the census — exhaustive on `Standing`, `cell_mark`'s rule, so a sixth
//! band with no answer fails to compile rather than shipping as neutral.

use serde::{Deserialize, Serialize};

use crate::tuning::{
    SETTLEMENT_ALLIED_STANDING, SETTLEMENT_COLD_STANDING, SETTLEMENT_HOSTILE_STANDING,
    SETTLEMENT_MAX_STANDING, SETTLEMENT_MIN_STANDING, SETTLEMENT_TRADE_CREDITS_PER_POINT,
    SETTLEMENT_WARM_STANDING,
};

/// Everything one town remembers about the party.
///
/// `trade_credits` is a **remainder, not a total**: trade pays standing per
/// `SETTLEMENT_TRADE_CREDITS_PER_POINT` Credits transacted, and without
/// somewhere to keep what is left over, a player who trades in ten small
/// baskets earns nothing while one who trades the same volume in a single
/// basket earns the lot. Dropping it would make the mover a rounding rule
/// rather than a volume rule.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub standing: i32,
    #[serde(default)]
    pub trade_credits: u32,
}

impl Relation {
    /// Folds `credits` of trade in and answers how many standing points it
    /// bought, keeping the remainder for the next basket.
    pub(crate) fn credit_trade(&mut self, credits: u32) -> i32 {
        self.trade_credits += credits;
        let points = self.trade_credits / SETTLEMENT_TRADE_CREDITS_PER_POINT;
        self.trade_credits -= points * SETTLEMENT_TRADE_CREDITS_PER_POINT;
        points as i32
    }
}

/// How a town reads a standing value.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Standing {
    Hostile,
    Cold,
    Neutral,
    Warm,
    Allied,
}

/// The one banding. Ordered from the bottom so the thresholds read as the
/// ladder they are.
pub fn band(standing: i32) -> Standing {
    if standing <= SETTLEMENT_HOSTILE_STANDING {
        Standing::Hostile
    } else if standing <= SETTLEMENT_COLD_STANDING {
        Standing::Cold
    } else if standing < SETTLEMENT_WARM_STANDING {
        Standing::Neutral
    } else if standing < SETTLEMENT_ALLIED_STANDING {
        Standing::Warm
    } else {
        Standing::Allied
    }
}

/// The bounds every writer clamps to — `Game::adjust_standing` is the only
/// one, which is what makes a single clamp enough.
pub fn clamp(standing: i32) -> i32 {
    standing.clamp(SETTLEMENT_MIN_STANDING, SETTLEMENT_MAX_STANDING)
}

impl Standing {
    pub fn label(self) -> &'static str {
        match self {
            Standing::Hostile => "Hostile",
            Standing::Cold => "Cold",
            Standing::Neutral => "Neutral",
            Standing::Warm => "Warm",
            Standing::Allied => "Allied",
        }
    }

    /// Whether the town's market and job board are closed to the party.
    ///
    /// Exhaustive on purpose — see the module doc. The gate is at the
    /// bottom band alone: a town that merely dislikes you still takes your
    /// Credits, which is what keeps the middle of the ladder about price
    /// rather than about access.
    pub fn refuses_service(self) -> bool {
        match self {
            Standing::Hostile => true,
            Standing::Cold | Standing::Neutral | Standing::Warm | Standing::Allied => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The census, `every_perk_has_a_query_that_answers_what_it_is_worth`'s
    /// shape: the match inside `refuses_service` is exhaustive, so a sixth
    /// band fails to compile, and this walks the list to prove the answer is
    /// actually reachable for each one rather than merely written.
    #[test]
    fn every_standing_band_answers_whether_it_refuses_service() {
        for band in [
            Standing::Hostile,
            Standing::Cold,
            Standing::Neutral,
            Standing::Warm,
            Standing::Allied,
        ] {
            let refused = band.refuses_service();
            assert_eq!(
                refused,
                band == Standing::Hostile,
                "{} answers the wrong way",
                band.label()
            );
        }
    }

    #[test]
    fn the_ladder_runs_in_order_and_neutral_holds_zero() {
        assert_eq!(band(0), Standing::Neutral);
        assert_eq!(band(SETTLEMENT_MIN_STANDING), Standing::Hostile);
        assert_eq!(band(SETTLEMENT_MAX_STANDING), Standing::Allied);
        assert_eq!(band(SETTLEMENT_HOSTILE_STANDING), Standing::Hostile);
        assert_eq!(band(SETTLEMENT_HOSTILE_STANDING + 1), Standing::Cold);
        assert_eq!(band(SETTLEMENT_COLD_STANDING), Standing::Cold);
        assert_eq!(band(SETTLEMENT_COLD_STANDING + 1), Standing::Neutral);
        assert_eq!(band(SETTLEMENT_WARM_STANDING), Standing::Warm);
        assert_eq!(band(SETTLEMENT_ALLIED_STANDING), Standing::Allied);
    }

    /// The whole reason `trade_credits` is stored: ten small baskets and one
    /// large one of the same volume must pay the same standing.
    #[test]
    fn trade_volume_keeps_its_remainder_across_baskets() {
        let per = SETTLEMENT_TRADE_CREDITS_PER_POINT;
        let mut split = Relation::default();
        let mut whole = Relation::default();
        let mut split_points = 0;
        for _ in 0..10 {
            split_points += split.credit_trade(per / 10);
        }
        let whole_points = whole.credit_trade(per);
        assert_eq!(split_points, whole_points);
        assert_eq!(whole_points, 1);
    }

    #[test]
    fn a_basket_under_the_threshold_pays_nothing_yet() {
        let mut relation = Relation::default();
        assert_eq!(
            relation.credit_trade(SETTLEMENT_TRADE_CREDITS_PER_POINT - 1),
            0
        );
        assert_eq!(relation.credit_trade(1), 1);
        assert_eq!(relation.trade_credits, 0);
    }
}
