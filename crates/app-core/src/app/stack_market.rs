//! The shelf a Stack market puts in front of the party, and the one extra
//! pick the narrowest routine rung takes.
//!
//! Deliberately not `Mode::Trade`'s flow with a second kind of counterparty
//! bolted in. The two screens share a key and nothing else: there is no
//! structure `Entity` here, no buyback section, no quantity page, and the
//! things for sale are not items. What they *do* share is the engine call
//! that decides a price, which is where sharing belongs.

use crate::app::list_filter::filter_matches;
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

/// Source indices into the market's two sections that the filter leaves.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MarketVisible {
    pub offers: Vec<usize>,
    pub sells: Vec<usize>,
}

impl MarketVisible {
    pub fn total(&self) -> usize {
        self.offers.len() + self.sells.len()
    }

    /// The section and *view* index a highlight position lands on; an offer
    /// still has to go through `view.offers[i].index` to reach its shelf slot.
    fn resolve(&self, pos: usize) -> Option<MarketRow> {
        Some(
            match market_row(pos, self.offers.len(), self.sells.len())? {
                MarketRow::Offer(k) => MarketRow::Offer(self.offers[k]),
                MarketRow::Sell(k) => MarketRow::Sell(self.sells[k]),
            },
        )
    }
}

/// The rows of `view` that `filter` leaves, by displayed name.
pub fn market_visible(
    view: &feral_processes_engine::StackMarketView,
    filter: &str,
) -> MarketVisible {
    MarketVisible {
        offers: filter_matches(view.offers.iter().map(|o| o.name.as_str()), filter),
        sells: filter_matches(view.sells.iter().map(|r| r.name.as_str()), filter),
    }
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
        if self.take_filter_key(key) {
            return;
        }
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(view) = self.game.as_mut().and_then(|g| g.stack_market()) else {
            // The stall was bought out by the purchase that just landed.
            self.mode = Mode::Playing;
            return;
        };
        let vis = market_visible(&view, self.filter_text());
        let total = vis.total();

        if key == GameKey::Char('S') {
            let row = vis.resolve(self.menu_selected.min(total.saturating_sub(1)));
            match row {
                Some(MarketRow::Sell(i)) => {
                    let row = &view.sells[i];
                    self.sell_to_market(row.copy.clone(), row.qty);
                }
                Some(MarketRow::Offer(_)) => self.refuse("That's theirs — pick it to buy it."),
                None => {}
            }
            return;
        }

        // Only the sell rows name gear — an offer is a routine disk or a
        // program, and its own `detail` line already carries what it is.
        if key == GameKey::Char('I') {
            let row = vis.resolve(self.menu_selected.min(total.saturating_sub(1)));
            if let Some(MarketRow::Sell(i)) = row {
                let copy = view.sells[i].copy.clone();
                self.open_gear_inspect(copy, None, Mode::StackMarket);
            }
            return;
        }

        let Some(idx) = self.selected_index(key, total) else {
            return;
        };
        match vis.resolve(idx) {
            Some(MarketRow::Offer(i)) => {
                // No target picker on any row any more: every routine row
                // sells disks into cargo, and who runs them is a question
                // the routine panel asks later. That is what deleted
                // `Mode::StackMarketTarget`.
                self.buy_market_offer(view.offers[i].index);
            }
            Some(MarketRow::Sell(i)) => {
                let row = &view.sells[i];
                self.sell_to_market(row.copy.clone(), 1);
            }
            None => {}
        }
    }

    /// Runs one purchase and reports it, and drops back to the map if that
    /// was the last row on the shelf — a screen left open on a stall with
    /// nothing on it is a screen the player has to press Esc to escape for
    /// no reason.
    ///
    /// What may be bought is decided by `Game`, exactly as `execute_trade`
    /// does for the surface — this only reports the answer.
    fn buy_market_offer(&mut self, index: usize) {
        let Some(game) = &mut self.game else { return };
        let outcome = game.buy_market_offer(index);
        self.report(outcome);
        self.close_if_bought_out();
    }

    fn sell_to_market(&mut self, copy: GearCopy, qty: u32) {
        let Some(game) = &mut self.game else { return };
        let outcome = game.sell_to_market(copy, qty);
        self.report(outcome);
        self.close_if_bought_out();
    }

    /// A trade costs a tick (`Game::buy_market_offer` and `sell_to_market`
    /// both call it), and a tick can be the one that starves the player —
    /// the same reason the surface trade flow checks after every
    /// transaction.
    fn close_if_bought_out(&mut self) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            return;
        }
        if self
            .game
            .as_mut()
            .is_some_and(|g| g.stack_market().is_none())
        {
            self.mode = Mode::Playing;
        }
    }
}
