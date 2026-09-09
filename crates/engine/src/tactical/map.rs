//! The battle map: a grid generated per encounter and destroyed at teardown.
//!
//! `stack::generate`'s precedent, one space over. A board is **pure in its
//! spec** — the same `BattleSpec` yields the same board, cell for cell — and
//! is never saved, because a fight is never saved. Nothing here draws from
//! `resources::GameRng` (a draw does not survive a save/load and shifts
//! every later roll in the run) and nothing here seeds an `StdRng` either
//! (its sequence is not guaranteed stable across a `rand` upgrade, and the
//! tests below compare whole boards for equality). Every cell is *derived*:
//! folded through `derive::fold` and reduced through `derive::index`, the
//! way `rock::RockDb::kind_at` derives base space.

use crate::tuning::TACTICAL_ROUGH_COST;

/// What a cell of a battle map is made of.
///
/// Four kinds, and the four are the complete 2x2 of "can you cross it"
/// against "can you shoot through it" — which is what keeps them from being
/// an arbitrary list. `Blocked` is a chasm you can see over and cannot
/// cross; `Cover` is a boulder that stops both. Note this is the same
/// asymmetry the Stack already establishes, where `walkable()` and
/// `blocks_sight()` are deliberately not complements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleCell {
    Open,
    Rough,
    Cover,
    Blocked,
}

impl BattleCell {
    /// What crossing this cell costs, or `None` where it cannot be crossed.
    ///
    /// An `Option<u32>` rather than a `u32` with a sentinel because that is
    /// the shape the movement field's step rule takes: `None` is blocked,
    /// `Some(c)` is the cost.
    pub fn movement_cost(self) -> Option<u32> {
        match self {
            BattleCell::Open => Some(1),
            BattleCell::Rough => Some(TACTICAL_ROUGH_COST),
            BattleCell::Cover | BattleCell::Blocked => None,
        }
    }

    /// Derived from `movement_cost` rather than matched separately, so the
    /// two cannot come to disagree about a kind.
    pub fn walkable(self) -> bool {
        self.movement_cost().is_some()
    }

    /// `Cover` is the only kind that stops a line or a cone.
    pub fn blocks_sight(self) -> bool {
        matches!(self, BattleCell::Cover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pair that makes four kinds necessary rather than arbitrary: one
    /// you can see over and cannot cross, one you can do neither with. If
    /// these ever agree, two of the kinds are the same kind.
    #[test]
    fn blocked_is_seen_over_and_cover_is_not() {
        assert!(!BattleCell::Blocked.walkable());
        assert!(!BattleCell::Blocked.blocks_sight());
        assert!(!BattleCell::Cover.walkable());
        assert!(BattleCell::Cover.blocks_sight());
    }

    #[test]
    fn rough_is_crossed_and_costs_more_than_open() {
        assert_eq!(BattleCell::Open.movement_cost(), Some(1));
        assert!(BattleCell::Rough.walkable());
        assert!(
            BattleCell::Rough.movement_cost() > BattleCell::Open.movement_cost(),
            "Rough that costs one is Open with a different name"
        );
    }

    /// `walkable` is derived from `movement_cost` rather than matched
    /// separately, so the two can never come to disagree about a kind.
    #[test]
    fn walkable_is_exactly_having_a_movement_cost() {
        for kind in [
            BattleCell::Open,
            BattleCell::Rough,
            BattleCell::Cover,
            BattleCell::Blocked,
        ] {
            assert_eq!(kind.walkable(), kind.movement_cost().is_some());
        }
    }
}
