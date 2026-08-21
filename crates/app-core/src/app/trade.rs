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
        let sell_rows: Vec<GearCopy> = game
            .player_status()
            .inventory
            .iter()
            .map(|row| row.copy.clone())
            .filter(|copy| copy.item != currency)
            .collect();
        let buy_items: Vec<ItemId> = trade.buy.iter().map(|(item, _)| item.clone()).collect();
        // Empty until the player sells this trader something, so the screen
        // is unchanged at a trader they have never sold to.
        let buybacks = game.buyback_options(structure);
        // Programs come last, and are empty for a trader that deals in items
        // only — so this screen is unchanged at such a trader.
        let programs = game.program_sale_options(structure);
        let total = sell_rows.len() + buy_items.len() + buybacks.len() + programs.len();

        // One unit off the highlighted row, no quantity page — a visit is
        // normally a run of trades, so the four-screen round trip is paid
        // per item. Uppercase because `selected_index` reserves it: a key
        // that both moved the selection and spent money would be a bug
        // waiting for a full pack.
        // Every shelf but the program one names an item, so `[I]` answers
        // on three of the four sections. A buy row names an id rather than a
        // copy the player owns — a trader's stock is plain, which is what
        // `GearCopy::plain` says.
        if key == GameKey::Char('I') {
            let row = trade_row(
                self.menu_selected.min(total.saturating_sub(1)),
                sell_rows.len(),
                buy_items.len(),
                buybacks.len(),
                programs.len(),
            );
            let copy = match row {
                Some(TradeRow::Sell(i)) => Some(sell_rows[i].clone()),
                Some(TradeRow::Buy(i)) => Some(GearCopy::plain(buy_items[i].clone())),
                Some(TradeRow::BuyBack(i)) => Some(buybacks[i].copy.clone()),
                Some(TradeRow::Program(_)) | None => None,
            };
            if let Some(copy) = copy {
                self.open_gear_inspect(copy, None, Mode::TradeAction);
            }
            return;
        }

        if let GameKey::Char(dir @ ('S' | 'B')) = key {
            let selling = dir == 'S';
            let row = trade_row(
                self.menu_selected.min(total.saturating_sub(1)),
                sell_rows.len(),
                buy_items.len(),
                buybacks.len(),
                programs.len(),
            );
            match row {
                Some(TradeRow::Sell(i)) if selling => {
                    let copy = sell_rows[i].clone();
                    self.execute_trade(structure, TradeChoice::Sell(copy), 1)
                }
                Some(TradeRow::Buy(i)) if !selling => {
                    self.execute_trade(structure, TradeChoice::Buy(buy_items[i].clone()), 1)
                }
                Some(TradeRow::BuyBack(i)) if !selling => {
                    let row = &buybacks[i];
                    self.execute_trade(structure, TradeChoice::BuyBack(row.copy.clone()), 1)
                }
                // Deliberately not a quick sale. Selling a levelled program
                // is permanent and a quick key is exactly a mis-hit, so this
                // row goes the slow way however it was reached — see
                // `handle_trade_program_confirm_key`.
                Some(TradeRow::Program(i)) if selling => {
                    self.pending_trade_program = Some(programs[i].clone());
                    self.mode = Mode::TradeProgramConfirm;
                }
                // A row is a sell or a buy, never both, so the other key is
                // a mis-hit. Say which one this row wants rather than
                // guessing at a transaction.
                Some(_) => {
                    self.status_line = Some(
                        if selling {
                            "That's the trader's — [B] buys one."
                        } else {
                            "That's yours — [S] sells one."
                        }
                        .to_string(),
                    )
                }
                None => {}
            }
            return;
        }

        if let Some(idx) = self.selected_index(key, total) {
            let choice = match trade_row(
                idx,
                sell_rows.len(),
                buy_items.len(),
                buybacks.len(),
                programs.len(),
            ) {
                Some(TradeRow::Sell(i)) => TradeChoice::Sell(sell_rows[i].clone()),
                Some(TradeRow::Buy(i)) => TradeChoice::Buy(buy_items[i].clone()),
                Some(TradeRow::BuyBack(i)) => TradeChoice::BuyBack(buybacks[i].copy.clone()),
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

    /// Runs one transaction and reports it, without deciding where the
    /// player ends up — the quantity page returns to wherever the trade
    /// began, while a quick key stays on the list.
    ///
    /// The single place either path reaches the engine, so what may be
    /// traded is decided once, by `Game`. A quick key that re-checked the
    /// rules itself would be a second copy of them, and the copy that
    /// drifts is the one nobody runs.
    pub(crate) fn execute_trade(&mut self, structure: Entity, choice: TradeChoice, qty: u32) {
        let Some(game) = &mut self.game else { return };
        let result = match choice {
            TradeChoice::Sell(copy) => game.sell_item(structure, copy, qty),
            TradeChoice::Buy(item) => game.buy_item(structure, item, qty),
            TradeChoice::BuyBack(copy) => game.buy_back(structure, copy, qty),
        };
        match result {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
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
                self.execute_trade(structure, choice, quantity);
                self.return_to_trade_list();
            }
            _ => {}
        }
    }
}
