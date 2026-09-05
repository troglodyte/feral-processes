//! The one door a basket of sells and buys is spent through — shared by
//! every vendor a trip to town can offer. `Game::commit_caravan_basket`
//! is the caravan's caller; a settlement market is the second, and calls
//! the same door without touching a line of caravan code.
//!
//! **What stays with the vendor and what moves here follows one line: a
//! vendor validates its own rows, because what may be refused differs from
//! one to the next** (a caravan checks `roster_room` and its own shelf; a
//! town checks its own) **— this door owns only what is true of every
//! basket, whichever vendor built it.** That is the funding comparison,
//! that sells land before buys, and that the whole commit costs one tick.

use crate::*;

/// A basket already reduced to its two totals and its two apply steps.
///
/// Nothing in here can refuse. By the time a caller builds one of these,
/// every question the vendor can ask has already been asked — `settle_basket`
/// only checks the one thing no *single* row's validation can: whether the
/// sum of what the basket earns covers the sum of what it costs.
///
/// `apply_sells`/`apply_buys` are `FnOnce` closures rather than data this
/// module walks itself, because what a "sell" or a "buy" *is* — a
/// `GearCopy` off a caravan's wagon today, a settlement's own offer kind
/// tomorrow — is exactly the part this door must never know about. Two
/// generic parameters rather than a boxed `dyn Fn`: there are two call
/// sites, no dynamic dispatch is needed, and each closure captures its own
/// already-resolved rows by value with nothing borrowed across the call —
/// which is what let `commit_caravan_basket` build them ahead of a
/// `self.settle_basket(..)` call without fighting the borrow checker over
/// two live mutable borrows of `Game`.
pub(crate) struct BasketPlan<Sells, Buys>
where
    Sells: FnOnce(&mut Game),
    Buys: FnOnce(&mut Game),
{
    /// Sum of every sell line's payout, priced by the vendor.
    pub proceeds: u32,
    /// Sum of every buy line's price, priced the same way.
    pub cost: u32,
    /// How many sell/buy lines survived validation. Carried as counts
    /// rather than re-derived from the closures after the fact, since a
    /// closure that has already run cannot be asked how much it did.
    pub sold: usize,
    pub bought: usize,
    /// Applies every planned sell — raises the money, moves the goods,
    /// logs the line. Infallible: any refusal was already raised while the
    /// vendor built the plan.
    pub apply_sells: Sells,
    /// Applies every planned buy, in the same sense.
    pub apply_buys: Buys,
}

impl Game {
    /// Spends a validated `BasketPlan`.
    ///
    /// **The two ordering rules `commit_caravan_basket` used to state in
    /// its own doc comment now live here, and this is the one place either
    /// can be checked.** The funding comparison counts a basket's own
    /// sales *in* — `held + proceeds < cost`, never `held < cost` — which
    /// is only true because sells are applied before buys: a buy applied
    /// first would spend money the vendor has not paid out yet. One
    /// `tick()` for the whole basket, not one per line, because the basket
    /// is the visit.
    pub(crate) fn settle_basket<Sells, Buys>(
        &mut self,
        plan: BasketPlan<Sells, Buys>,
    ) -> Result<String, String>
    where
        Sells: FnOnce(&mut Game),
        Buys: FnOnce(&mut Game),
    {
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let held = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        let cost = plan.cost;
        if held + plan.proceeds < cost {
            return Err(format!("Not enough {money} (need {cost})."));
        }

        // ---- nothing below may refuse ----
        (plan.apply_sells)(self);
        (plan.apply_buys)(self);
        self.tick();

        Ok(match (plan.sold, plan.bought) {
            (0, b) => format!("Bought {b}."),
            (s, 0) => format!("Sold {s}."),
            (s, b) => format!("Sold {s}, bought {b}."),
        })
    }
}
