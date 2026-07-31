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
            self.mode = self.abandoned_trade_mode_from_picker();
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
            // Coming from `[S]ell` in the inventory the line item is already
            // decided, so picking the post is the last question — go
            // straight to the quantity rather than back through a list the
            // player didn't come here for.
            self.mode = match self.trade_origin {
                TradeOrigin::Inventory if self.pending_trade_choice.is_some() => {
                    Mode::TradeQuantity
                }
                _ => Mode::TradeAction,
            };
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
        // The trade currency, not the build salvage — the same filter
        // `render/trade.rs::draw_trade_action_menu` draws with, and it has
        // to be: these two lists are indexed by the same row number, so a
        // different exclusion here would sell the line above or below the
        // one the player is looking at. Salvage is ordinary goods to a
        // trader; Credits are what it won't buy (see `Game::sell_item`).
        let currency = game.trade_currency();
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
        self.return_to_trade_list();
    }

    /// Where a finished transaction leaves the player: back on the trader's
    /// list, since a visit is normally a run of trades — clear the pack a
    /// stack at a time, then spend the proceeds — and dropping to the map
    /// after each one means walking the whole menu path again for the next.
    ///
    /// A trade costs a tick (`Game::sell_item` and friends call it), and a
    /// tick can be the one that starves the player. That is the only thing
    /// that takes the screen away instead.
    /// Backing out of the trader picker. Reached from the map with `t`, in
    /// which case there is nothing behind it but the map, or from `[S]ell`
    /// with several posts in range, in which case the item's action page is.
    fn abandoned_trade_mode_from_picker(&mut self) -> Mode {
        match self.trade_origin {
            TradeOrigin::Inventory => {
                self.pending_trade_choice = None;
                self.trade_origin = TradeOrigin::Trader;
                Mode::InventoryItemAction
            }
            TradeOrigin::Trader => Mode::Playing,
        }
    }

    /// Where backing out of a trade lands, without the tick-and-game-over
    /// handling `return_to_trade_list` does — nothing was bought or sold, so
    /// there is no tick to have starved anyone.
    fn abandoned_trade_mode(&mut self) -> Mode {
        match self.trade_origin {
            TradeOrigin::Inventory => {
                self.pending_trade_structure = None;
                self.trade_origin = TradeOrigin::Trader;
                Mode::InventoryItemAction
            }
            TradeOrigin::Trader => Mode::TradeAction,
        }
    }

    fn return_to_trade_list(&mut self) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_trade_structure = None;
            return;
        }
        self.mode = match self.trade_origin {
            TradeOrigin::Inventory => {
                self.pending_trade_structure = None;
                self.trade_origin = TradeOrigin::Trader;
                Mode::Inventory
            }
            TradeOrigin::Trader => Mode::TradeAction,
        };
    }

    /// Types a quantity for the pending sell/buy line item; Enter commits it.
    pub(crate) fn handle_trade_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_trade_choice = None;
                self.trade_quantity_input.clear();
                self.mode = self.abandoned_trade_mode();
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
                // Nothing traded and no tick spent, so this is the same
                // step back Esc takes.
                if quantity == 0 {
                    self.mode = self.abandoned_trade_mode();
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
                self.return_to_trade_list();
            }
            _ => {}
        }
    }
}
