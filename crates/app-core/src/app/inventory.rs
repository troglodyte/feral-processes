//! The inventory list, its per-item action page, and erasing a stack.

use crate::*;

impl App {
    /// Equipped slots are numbered 1-3 (Weapon/Armor/Module) and unequip
    /// immediately when pressed; unequipped inventory items start at 4 and
    /// open `Mode::InventoryItemAction` for the selected item.
    pub(crate) fn handle_inventory_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let inventory = game.player_status().inventory;
        let total = 3 + inventory.len();

        // Sell one of the highlighted item without the item-action page,
        // the trader picker and the quantity page in between. The three
        // equipment slot rows come first, so only rows past them are items.
        if key == GameKey::Char('S') {
            let item = inventory
                .get(self.menu_selected.saturating_sub(3))
                .filter(|_| self.menu_selected >= 3)
                .map(|(item, _)| item.clone());
            if let Some(item) = item {
                self.quick_sell_from_inventory(item);
            }
            return;
        }

        let Some(idx) = self.selected_index(key, total) else {
            return;
        };
        let slot = match idx {
            0 => Some(EquipmentSlot::Weapon),
            1 => Some(EquipmentSlot::Armor),
            2 => Some(EquipmentSlot::Module),
            _ => None,
        };
        if let Some(slot) = slot {
            let Some(game) = &mut self.game else { return };
            match game.unequip(slot) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            return;
        }
        if let Some((item, _)) = inventory.get(idx - 3) {
            self.pending_inventory_item = Some(item.clone());
            self.mode = Mode::InventoryItemAction;
        }
    }

    /// Sells one `item` to the trader in range, or asks which one.
    ///
    /// Two traders is a question this key cannot answer — their buyback
    /// shelves are separate, so which shop you sell to decides where the
    /// goods can be bought back. Rather than pick a shop on the player's
    /// behalf it opens the existing picker with the item already decided,
    /// which is the `TradeOrigin::Inventory` path.
    fn quick_sell_from_inventory(&mut self, item: ItemId) {
        let Some(game) = &mut self.game else { return };
        let traders = traders_in_range(game);
        match traders.as_slice() {
            [] => {
                self.status_line =
                    Some("Nothing in range buys anything. Find a trading post.".to_string())
            }
            [only] => {
                let structure = *only;
                self.execute_trade(structure, TradeChoice::Sell(item), 1);
            }
            _ => {
                self.pending_trade_choice = Some(TradeChoice::Sell(item));
                self.trade_origin = TradeOrigin::Inventory;
                self.mode = Mode::Trade;
            }
        }
    }

    pub(crate) fn handle_inventory_item_action_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_inventory_item = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(item) = self.pending_inventory_item.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let actions: Vec<char> = {
            let Some(game) = &mut self.game else {
                self.mode = Mode::Inventory;
                return;
            };
            inventory_item_actions(game, &item)
                .into_iter()
                .map(|(k, _)| k)
                .collect()
        };
        let idx = self
            .selected_index(key, actions.len())
            .or_else(|| match key {
                GameKey::Char(c) => actions.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        if idx.map(|i| actions[i]) == Some('x') {
            self.pending_erase = Some(item);
            self.erase_quantity_input.clear();
            self.mode = Mode::EraseQuantity;
            self.pending_inventory_item = None;
            return;
        }
        if idx.map(|i| actions[i]) == Some('s') {
            self.begin_sale_from_inventory(item);
            return;
        }
        if idx.map(|i| actions[i]) == Some('d') {
            // `pending_inventory_item` deliberately survives: the describe
            // page is about that item, and Esc comes back here.
            self.mode = Mode::ItemDescribe;
            return;
        }
        if idx.map(|i| actions[i]) == Some('c') {
            let Some(game) = &mut self.game else { return };
            game.use_item(&item);
            self.status_line = None;
            self.pending_inventory_item = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(game) = &mut self.game else { return };
        // Equipping clears the status line (its result shows in the equipment
        // panel); a fuse hands back a confirmation to surface, since it
        // changes nothing else visible from behind the inventory popup. Both
        // report a refusal on the status line.
        let outcome = match idx.map(|i| actions[i]) {
            Some('e') => Some(game.equip(&item).err()),
            Some('u') => Some(match game.fuse_item(&item) {
                Ok(msg) => Some(msg),
                Err(e) => Some(e),
            }),
            _ => None,
        };
        let Some(outcome) = outcome else { return };
        self.status_line = outcome;
        self.pending_inventory_item = None;
        self.mode = Mode::Inventory;
    }

    /// Starts a sale of `item` from the inventory rather than from the
    /// trader's list, reusing the same quantity page the trader flow ends
    /// on. With exactly one trading post in range there is nothing to
    /// choose, so the picker is skipped; with several the player still has
    /// to say which, and `Mode::Trade` already asks that question.
    ///
    /// `[S]ell` is only listed when a post is in range at all
    /// (`inventory_item_actions`), so the empty case is unreachable from
    /// the menu and falls back to the inventory rather than inventing an
    /// error for it.
    fn begin_sale_from_inventory(&mut self, item: ItemId) {
        let Some(game) = &mut self.game else { return };
        let posts: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_trade)
            .collect();
        self.trade_origin = TradeOrigin::Inventory;
        self.pending_trade_choice = Some(TradeChoice::Sell(item));
        self.trade_quantity_input.clear();
        match posts.as_slice() {
            [] => {
                self.pending_trade_choice = None;
                self.mode = Mode::Inventory;
            }
            [only] => {
                self.pending_trade_structure = Some(only.entity);
                self.mode = Mode::TradeQuantity;
            }
            _ => self.mode = Mode::Trade,
        }
    }

    /// The describe page is read-only: any key steps back to the actions.
    pub(crate) fn handle_item_describe_key(&mut self, _key: GameKey) {
        self.mode = Mode::InventoryItemAction;
    }

    /// Second page of the erase flow: how many units of `pending_erase` to
    /// destroy. `[A]` erases the whole stack, matching the pre-cap
    /// behavior. An empty input on Enter means 1.
    pub(crate) fn handle_erase_quantity_key(&mut self, key: GameKey) {
        let Some(item) = self.pending_erase.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let stack_qty = self
            .game
            .as_ref()
            .map(|g| {
                g.player_status()
                    .inventory
                    .iter()
                    .find(|(i, _)| *i == item)
                    .map(|(_, q)| *q)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        match key {
            GameKey::Esc => {
                self.pending_erase = None;
                self.erase_quantity_input.clear();
                self.mode = Mode::Inventory;
            }
            GameKey::Backspace => {
                self.erase_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.erase_quantity_input.len() < 4 => {
                self.erase_quantity_input.push(c);
            }
            GameKey::Char('a') | GameKey::Char('A') => {
                self.commit_erase(item, stack_qty);
            }
            GameKey::Enter => {
                let quantity: u32 = if self.erase_quantity_input.is_empty() {
                    1
                } else {
                    self.erase_quantity_input.parse().unwrap_or(0)
                };
                self.commit_erase(item, quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::erase_item` and returns to the inventory screen. A
    /// quantity of 0 is a silent no-op rather than a round-trip to the
    /// engine for an error, matching `commit_craft`.
    fn commit_erase(&mut self, item: ItemId, quantity: u32) {
        self.pending_erase = None;
        self.erase_quantity_input.clear();
        self.mode = Mode::Inventory;
        if quantity == 0 {
            return;
        }
        if let Some(game) = &mut self.game {
            match game.erase_item(&item, quantity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }
}
