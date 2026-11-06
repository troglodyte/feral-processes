//! Deciding what the map must do so a held bounty has exactly one quarry.

use crate::contracts::ContractId;

/// What `Game::ensure_bounty_quarry` must do, as indices into the marks it
/// gathered.
#[derive(Debug, PartialEq, Eq)]
pub struct Reconcile {
    /// Marks to despawn: duplicates of the held contract, and marks for a
    /// contract that is no longer held.
    pub despawn: Vec<usize>,
    /// Whether the held contract has no quarry standing.
    pub spawn: bool,
}

/// `held` is the id of the bounty the run holds, if any. `marks` are the
/// contracts of every marked creature on the map, in a stable order, so
/// which of two duplicates survives never depends on query order.
pub fn reconcile(held: Option<&ContractId>, marks: &[ContractId]) -> Reconcile {
    let keep = held.and_then(|id| marks.iter().position(|m| m == id));
    Reconcile {
        despawn: (0..marks.len()).filter(|&i| Some(i) != keep).collect(),
        spawn: held.is_some() && keep.is_none(),
    }
}
