//! The shelf a Stack market puts in front of the party, and the one extra
//! pick the narrowest routine rung takes.
//!
//! Deliberately not `Mode::Trade`'s flow with a second kind of counterparty
//! bolted in. The two screens share a key and nothing else: there is no
//! structure `Entity` here, no buyback section, no quantity page, and the
//! things for sale are not items. What they *do* share is the engine call
//! that decides a price, which is where sharing belongs.

use crate::*;

/// Which section of the market screen a picked row lands in, and its index
/// within that section.
///
/// The offer index carried here is the row's position in the drawn list,
/// not its shelf index — `handle_stack_market_key` resolves one to the
/// other through the view, because a bought row leaves the list and the two
/// stop agreeing the moment anything is bought.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MarketRow {
    Offer(usize),
    Sell(usize),
}

/// Resolves a row number against the market screen's two stacked sections —
/// what is for sale, then what the stall will take — given how many rows
/// each contributed. `None` for a row past the end.
///
/// Pulled out for the reason `trade_row` was: the offset arithmetic is
/// where a screen with more than one section goes wrong, and it is the part
/// of this flow that can be tested without a stall to stand at.
pub(crate) fn market_row(idx: usize, offers: usize, sells: usize) -> Option<MarketRow> {
    if idx < offers {
        return Some(MarketRow::Offer(idx));
    }
    (idx - offers < sells).then(|| MarketRow::Sell(idx - offers))
}

impl App {
    /// Buys a row, or sells one stack of cargo a unit at a time.
    ///
    /// `[S]` sells the whole of the highlighted cargo row instead of one
    /// unit — uppercase for the reason the trade screen reserves it, that a
    /// key which both moved the selection and completed a transaction is a
    /// bug waiting for a full pack. Bulk is on the *shifted* key rather than
    /// the plain one because a Stack trader has no buyback: emptying a stack
    /// by accident here cannot be undone by buying it back.
    pub(crate) fn handle_stack_market_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(view) = self.game.as_mut().and_then(|g| g.stack_market()) else {
            // The stall was bought out by the purchase that just landed.
            self.mode = Mode::Playing;
            return;
        };
        let total = view.offers.len() + view.sells.len();

        if key == GameKey::Char('S') {
            let row = market_row(
                self.menu_selected.min(total.saturating_sub(1)),
                view.offers.len(),
                view.sells.len(),
            );
            match row {
                Some(MarketRow::Sell(i)) => {
                    let row = &view.sells[i];
                    self.sell_to_market(row.item.clone(), row.tier, row.qty);
                }
                Some(MarketRow::Offer(_)) => {
                    self.status_line = Some("That's theirs — pick it to buy it.".to_string())
                }
                None => {}
            }
            return;
        }

        let Some(idx) = self.selected_index(key, total) else {
            return;
        };
        match market_row(idx, view.offers.len(), view.sells.len()) {
            Some(MarketRow::Offer(i)) => {
                let offer = &view.offers[i];
                // The one rung with a question left. The other two know
                // their own recipients, so sending them through a picker
                // would be asking the player to confirm a choice they do
                // not have.
                if matches!(
                    offer.kind,
                    MarketOfferKind::Routine {
                        scope: RoutineScope::One,
                        ..
                    }
                ) {
                    self.pending_market_offer = Some(offer.index);
                    self.menu_selected = 0;
                    self.mode = Mode::StackMarketTarget;
                    return;
                }
                self.buy_market_offer(offer.index, None);
            }
            Some(MarketRow::Sell(i)) => {
                let row = &view.sells[i];
                self.sell_to_market(row.item.clone(), row.tier, 1);
            }
            None => {}
        }
    }

    /// Picks who the routine row held in `pending_market_offer` is written
    /// to. The same holder list the routine panel offers, so "who can run a
    /// routine" is answered in one place.
    pub(crate) fn handle_stack_market_target_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_market_offer = None;
            self.mode = Mode::StackMarket;
            return;
        }
        let Some(index) = self.pending_market_offer else {
            self.mode = Mode::StackMarket;
            return;
        };
        let Some(holders) = self.game.as_mut().map(|g| g.routine_holders()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, holders.len()) {
            let holder = holders[idx].entity;
            self.pending_market_offer = None;
            self.buy_market_offer(index, Some(holder));
            self.menu_selected = 0;
            self.mode = Mode::StackMarket;
        }
    }

    /// Runs one purchase and reports it, and drops back to the map if that
    /// was the last row on the shelf — a screen left open on a stall with
    /// nothing on it is a screen the player has to press Esc to escape for
    /// no reason.
    ///
    /// Both entry points come through here so what may be bought is decided
    /// once, by `Game`, exactly as `execute_trade` does for the surface.
    fn buy_market_offer(&mut self, index: usize, target: Option<Entity>) {
        let Some(game) = &mut self.game else { return };
        match game.buy_market_offer(index, target) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.close_if_bought_out();
    }

    fn sell_to_market(&mut self, item: ItemId, tier: u32, qty: u32) {
        let Some(game) = &mut self.game else { return };
        match game.sell_to_market(item, tier, qty) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.close_if_bought_out();
    }

    /// A trade costs a tick (`Game::buy_market_offer` and `sell_to_market`
    /// both call it), and a tick can be the one that starves the player —
    /// the same reason the surface trade flow checks after every
    /// transaction.
    fn close_if_bought_out(&mut self) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_market_offer = None;
            return;
        }
        if self
            .game
            .as_mut()
            .is_some_and(|g| g.stack_market().is_none())
        {
            self.pending_market_offer = None;
            self.mode = Mode::Playing;
        }
    }
}
