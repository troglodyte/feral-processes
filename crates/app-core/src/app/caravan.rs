//! The counter a visiting caravan sets out, and the quantity page for
//! selling into it.
//!
//! Modelled on `app/stack_market.rs`, and its header states why a second
//! counterparty gets its own screen rather than a third section bolted into
//! `Mode::Trade`: the two share a concept and nothing else. There is no
//! structure `Entity` here, no buyback section, and the things for sale are
//! not all items. What they *do* share is the engine call that decides a
//! price, which is where sharing belongs.
//!
//! It differs from the Stack market's screen in one way, and that is why the
//! quantity page exists: a caravan's cargo rows can be deep — a wagon carries
//! a stack, not a curio — so selling into one a unit at a time is a keypress
//! per Core Fragment.

use crate::*;

/// Which section of the caravan screen a picked row lands in, and its index
/// within that section.
///
/// The offer index carried here is the row's position in the **drawn list**,
/// not its shelf index — `handle_caravan_key` resolves one to the other
/// through the view, because a bought row leaves the list and the two stop
/// agreeing the moment anything is bought. `MarketRow` carries the same
/// warning for the same reason.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CaravanRow {
    Offer(usize),
    Sell(usize),
}

/// Resolves a row number against the caravan screen's two stacked sections —
/// what is on the wagon, then what the wagon will take — given how many rows
/// each contributed. `None` for a row past the end.
///
/// Pulled out for the reason `market_row` and `trade_row` were: the offset
/// arithmetic is where a screen with more than one section goes wrong, and it
/// is the part of this flow that can be tested without a trader standing in
/// front of you.
pub(crate) fn caravan_row(idx: usize, offers: usize, sells: usize) -> Option<CaravanRow> {
    if idx < offers {
        return Some(CaravanRow::Offer(idx));
    }
    (idx - offers < sells).then(|| CaravanRow::Sell(idx - offers))
}

impl App {
    /// Buys a row, or opens the quantity page for one stack of cargo.
    ///
    /// `[S]` sells the whole of the highlighted cargo row without asking.
    /// Uppercase for the reason the trade screen reserves it — a key that
    /// both moved the selection and completed a transaction is a bug waiting
    /// for a full pack — and bulk is on the *shifted* key rather than the
    /// plain one because a caravan has no buyback: emptying a stack by
    /// accident here cannot be undone by buying it back.
    pub(crate) fn handle_caravan_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(view) = self.game.as_mut().and_then(|g| g.caravan_view()) else {
            // The wagon was bought out, or the trader has rolled away.
            self.mode = Mode::Playing;
            return;
        };
        let total = view.offers.len() + view.sells.len();
        let highlighted = |app: &App| {
            caravan_row(
                app.menu_selected.min(total.saturating_sub(1)),
                view.offers.len(),
                view.sells.len(),
            )
        };

        if key == GameKey::Char('S') {
            match highlighted(self) {
                Some(CaravanRow::Sell(i)) => {
                    let row = &view.sells[i];
                    self.sell_to_caravan(row.copy.clone(), row.held);
                }
                Some(CaravanRow::Offer(_)) => self.refuse("That's theirs — pick it to buy it."),
                None => {}
            }
            return;
        }

        // Only the sell rows name a copy the player is holding. An offer's
        // gear row names one that is still on the wagon, and `[I]` scales a
        // piece to its *wearer* — a figure quoted for a copy nobody owns
        // would be scaled to nobody.
        if key == GameKey::Char('I') {
            if let Some(CaravanRow::Sell(i)) = highlighted(self) {
                let copy = view.sells[i].copy.clone();
                self.open_gear_inspect(copy, None, Mode::Caravan);
            }
            return;
        }

        let Some(idx) = self.selected_index(key, total) else {
            return;
        };
        match caravan_row(idx, view.offers.len(), view.sells.len()) {
            Some(CaravanRow::Offer(i)) => {
                // A whole row at once, never a unit at a time: what the
                // wagon has of something is the offer, and `CaravanOffer::qty`
                // is part of the price the player was quoted.
                let index = view.offers[i].index;
                let Some(game) = &mut self.game else { return };
                let outcome = game.buy_caravan_offer(index);
                self.report(outcome);
                self.close_if_gone();
            }
            Some(CaravanRow::Sell(i)) => {
                self.pending_caravan_sale = Some(view.sells[i].copy.clone());
                self.trade_quantity_input.clear();
                self.mode = Mode::CaravanQuantity;
            }
            None => {}
        }
    }

    /// How many of the picked stack to sell. Digits and Enter, like
    /// `Mode::TradeQuantity`, and it shares that screen's input buffer
    /// because only one quantity page can be open at a time.
    pub(crate) fn handle_caravan_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_caravan_sale = None;
                self.trade_quantity_input.clear();
                self.mode = Mode::Caravan;
            }
            GameKey::Backspace => {
                self.trade_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.trade_quantity_input.len() < 4 => {
                self.trade_quantity_input.push(c);
            }
            GameKey::Enter => {
                let Some(copy) = self.pending_caravan_sale.take() else {
                    self.mode = Mode::Caravan;
                    return;
                };
                let quantity: u32 = if self.trade_quantity_input.is_empty() {
                    1
                } else {
                    self.trade_quantity_input.parse().unwrap_or(0)
                };
                self.trade_quantity_input.clear();
                // Nothing sold and no tick spent, so this is the step back
                // Esc takes — `handle_trade_quantity_key`'s rule.
                if quantity == 0 {
                    self.mode = Mode::Caravan;
                    return;
                }
                self.sell_to_caravan(copy, quantity);
                if self.mode == Mode::CaravanQuantity {
                    self.mode = Mode::Caravan;
                }
            }
            _ => {}
        }
    }

    fn sell_to_caravan(&mut self, copy: GearCopy, qty: u32) {
        let Some(game) = &mut self.game else { return };
        let outcome = game.sell_to_caravan(copy, qty);
        self.report(outcome);
        self.close_if_gone();
    }

    /// A trade costs a tick, and a tick can be the one the trader leaves on
    /// — or the one that starves the player. `handle_stack_market_key`'s
    /// `close_if_bought_out`, and for both of its reasons.
    fn close_if_gone(&mut self) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            return;
        }
        if self
            .game
            .as_mut()
            .is_some_and(|g| g.caravan_view().is_none())
        {
            self.mode = Mode::Playing;
        }
    }
}
