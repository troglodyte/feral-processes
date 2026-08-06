//! Posted programs walking: taking a post, carrying a clogged machine's
//! output to a depot, and coming back.
//!
//! `components::Carrying` is the only state this feature stores. Where a
//! worker is headed and whether it has arrived are both read off `Position`,
//! so the two cannot disagree with each other the way a hand-maintained
//! `HaulState` enum would.

use crate::*;

/// Takes up to `HAUL_CARRY_CAPACITY` units of one item out of `stock`'s
/// output, or `None` if there is nothing to take.
///
/// The item is the first key in `BTreeMap` order — `Stock` keys by `ItemId`
/// in a `BTreeMap` precisely so choices like this are stable run to run, and
/// picking deterministically is what lets a load be a single `(item, qty)`
/// pair rather than a map.
pub(crate) fn take_haul_load(stock: &mut Stock) -> Option<Carrying> {
    // Cloned out before the map is touched: the borrow behind `.keys()` is
    // still live otherwise.
    let item = stock.output.keys().next().cloned()?;
    let held = stock.output.get(&item).copied().unwrap_or(0);
    let qty = held.min(tuning::HAUL_CARRY_CAPACITY);
    if qty == 0 {
        return None;
    }
    if held == qty {
        stock.output.remove(&item);
    } else {
        stock.output.insert(item.clone(), held - qty);
    }
    Some(Carrying { item, qty })
}
