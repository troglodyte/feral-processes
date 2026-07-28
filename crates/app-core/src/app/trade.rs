//! Buying, selling, and the extra confirmation a program sale takes.

use crate::*;

/// Which section of the trade screen a picked row number lands in, and its
/// index within that section.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TradeRow {
    Sell(usize),
    Buy(usize),
    BuyBack(usize),
    Program(usize),
}

/// Resolves a row number against the trade screen's four stacked sections —
/// sell, buy, buyback, programs, in that order — given how many rows each
/// contributed. `None` for a row past the end.
///
/// Pulled out of `handle_trade_action_key` and shared with the renderer that
/// numbers the same rows: the offset arithmetic is where a screen with more
/// than two sections goes wrong, and it is the one part of this flow that
/// can be tested without a trading post to stand in front of.
pub(crate) fn trade_row(
    idx: usize,
    sells: usize,
    buys: usize,
    buybacks: usize,
    programs: usize,
) -> Option<TradeRow> {
    if idx < sells {
        return Some(TradeRow::Sell(idx));
    }
    let idx = idx - sells;
    if idx < buys {
        return Some(TradeRow::Buy(idx));
    }
    let idx = idx - buys;
    if idx < buybacks {
        return Some(TradeRow::BuyBack(idx));
    }
    let idx = idx - buybacks;
    (idx < programs).then_some(TradeRow::Program(idx))
}

impl App {
    /// Picks a nearby trading-post structure to open a trade session with.
    pub(crate) fn handle_trade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_trade)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            self.pending_trade_structure = Some(structures[idx].entity);
            self.mode = Mode::TradeAction;
        }
    }

    /// Picks a sell (from inventory) or buy (from the structure's trade
    /// list) line item — sell offers are numbered first, then buy offers.
    pub(crate) fn handle_trade_action_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_trade_structure = None;
            self.mode = Mode::Trade;
            return;
        }
        let Some(structure) = self.pending_trade_structure else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let Some(trade) = game.trade_options(structure) else {
            self.mode = Mode::Playing;
            return;
        };
        let currency = game.currency();
        let sell_items: Vec<ItemId> = game
            .player_status()
            .inventory
            .iter()
            .map(|(item, _)| item.clone())
            .filter(|item| *item != currency)
            .collect();
        let buy_items: Vec<ItemId> = trade.buy.iter().map(|(item, _)| item.clone()).collect();
        // Empty until the player sells this trader something, so the screen
        // is unchanged at a trader they have never sold to.
        let buybacks = game.buyback_options(structure);
        // Programs come last, and are empty for a trader that deals in items
        // only — so this screen is unchanged at such a trader.
        let programs = game.program_sale_options(structure);
        let total = sell_items.len() + buy_items.len() + buybacks.len() + programs.len();
        if let Some(idx) = self.selected_index(key, total) {
            let choice = match trade_row(
                idx,
                sell_items.len(),
                buy_items.len(),
                buybacks.len(),
                programs.len(),
            ) {
                Some(TradeRow::Sell(i)) => TradeChoice::Sell(sell_items[i].clone()),
                Some(TradeRow::Buy(i)) => TradeChoice::Buy(buy_items[i].clone()),
                Some(TradeRow::BuyBack(i)) => TradeChoice::BuyBack(buybacks[i].item.clone()),
                Some(TradeRow::Program(i)) => {
                    // A program needs no quantity — there is exactly one of it
                    // — so it skips the quantity page and goes to confirmation.
                    self.pending_trade_program = Some(programs[i].clone());
                    self.mode = Mode::TradeProgramConfirm;
                    return;
                }
                None => return,
            };
            self.pending_trade_choice = Some(choice);
            self.trade_quantity_input.clear();
            self.mode = Mode::TradeQuantity;
        }
    }

    /// Confirms or abandons the program sale picked in `Mode::TradeAction`.
    /// `y` sells; Esc and `n` both back out, because a mis-hit on a screen
    /// that permanently destroys a levelled program must not be a sale.
    pub(crate) fn handle_trade_program_confirm_key(&mut self, key: GameKey) {
        let confirmed = match key {
            GameKey::Char('y') | GameKey::Char('Y') => true,
            GameKey::Esc | GameKey::Char('n') | GameKey::Char('N') => false,
            _ => return,
        };
        let Some(option) = self.pending_trade_program.take() else {
            self.mode = Mode::Trade;
            return;
        };
        if !confirmed {
            self.mode = Mode::TradeAction;
            return;
        }
        let Some(structure) = self.pending_trade_structure else {
            self.mode = Mode::Playing;
            return;
        };
        if let Some(game) = &mut self.game {
            match game.sell_companion(structure, option.entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        // Matches an item sale: the visit ends rather than looping back into
        // a list whose rows have just shifted under the player.
        self.pending_trade_structure = None;
        self.mode = Mode::Playing;
    }

    /// Types a quantity for the pending sell/buy line item; Enter commits it.
    pub(crate) fn handle_trade_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_trade_choice = None;
                self.trade_quantity_input.clear();
                self.mode = Mode::TradeAction;
            }
            GameKey::Backspace => {
                self.trade_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.trade_quantity_input.len() < 4 => {
                self.trade_quantity_input.push(c);
            }
            GameKey::Enter => {
                let Some(choice) = self.pending_trade_choice.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                let Some(structure) = self.pending_trade_structure else {
                    self.mode = Mode::Playing;
                    return;
                };
                let quantity: u32 = if self.trade_quantity_input.is_empty() {
                    1
                } else {
                    self.trade_quantity_input.parse().unwrap_or(0)
                };
                self.trade_quantity_input.clear();
                if quantity == 0 {
                    self.pending_trade_structure = None;
                    self.mode = Mode::Playing;
                    return;
                }
                if let Some(game) = &mut self.game {
                    let result = match choice {
                        TradeChoice::Sell(item) => game.sell_item(structure, item, quantity),
                        TradeChoice::Buy(item) => game.buy_item(structure, item, quantity),
                        TradeChoice::BuyBack(item) => game.buy_back(structure, item, quantity),
                    };
                    match result {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                }
                self.pending_trade_structure = None;
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }
}
