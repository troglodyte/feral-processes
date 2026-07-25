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
            let Some(game) = &self.game else {
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
