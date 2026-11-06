//! How a bounty fight ends the contract. Pure, so the one rule that decides
//! whether the Broker pays is tested without a fight.

/// What a finished bounty fight does to the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BountyOutcome {
    Complete,
    Fail,
}

/// **Reads only whether the quarry fell**, never whether the party won: a
/// party that put the quarry down and then ran from its escort has finished
/// the job, and one that cleared the escort while the quarry slipped off has
/// not. A Forgiving reboot after a loss cannot change it either.
pub fn verdict(quarry_down: bool) -> BountyOutcome {
    if quarry_down {
        BountyOutcome::Complete
    } else {
        BountyOutcome::Fail
    }
}
