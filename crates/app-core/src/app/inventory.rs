//! The inventory list, its per-item action page, and erasing a stack.

use crate::*;

impl App {
    /// Opens the inspect page on `copy`, measured for `wearer`, returning to
    /// `from` on Esc.
    ///
    /// **The one way into `Mode::ItemDescribe`.** Seven screens reach it and
    /// each has to hand over all three: a page that inherited the previous
    /// screen's subject, wearer or return mode would describe the wrong copy,
    /// price it for the wrong body, or strand the player a screen out.
    pub(crate) fn open_gear_inspect(&mut self, copy: GearCopy, wearer: Option<Entity>, from: Mode) {
        self.pending_inspect = Some(GearInspect { copy, wearer, from });
        self.mode = Mode::ItemDescribe;
    }

    /// Inspects whatever `wearer` has in `slot`, or says there is nothing
    /// there. An empty slot names no item, and a page about no item is worse
    /// than a line saying so — the same call `open_equip_swap` makes about a
    /// picker with no rows.
    pub(crate) fn inspect_worn(&mut self, wearer: Option<Entity>, slot: EquipmentSlot, from: Mode) {
        let Some(game) = &self.game else { return };
        let entity = wearer.unwrap_or_else(|| game.player_entity());
        match game.worn(entity, slot) {
            Some(worn) => self.open_gear_inspect(worn.copy, wearer, from),
            None => {
                let line = format!("Nothing in the {} slot.", slot.label());
                self.refuse(line);
            }
        }
    }

    /// Equipped slots are numbered 1-3 (Weapon/Armor/Module) and open
    /// `Mode::EquipSwap` for that slot; unequipped inventory items start at
    /// 4 and open `Mode::InventoryItemAction` for the selected item.
    pub(crate) fn handle_inventory_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &self.game else { return };
        let inventory = game.player_status().inventory;
        let total = 3 + inventory.len();

        // Sell one of the highlighted item without the item-action page,
        // the trader picker and the quantity page in between. The three
        // equipment slot rows come first, so only rows past them are items.
        if key == GameKey::Char('S') {
            let row = inventory
                .get(self.menu_selected.saturating_sub(3))
                .filter(|_| self.menu_selected >= 3)
                .map(|row| row.copy.clone());
            if let Some(copy) = row {
                self.quick_sell_from_inventory(copy);
            }
            return;
        }

        // Fuses every matching pair in cargo for one turn, rather than the
        // action page's `[U]` per stack. Uppercase for the reason `S` is:
        // `selected_index` reserves shifted letters for screen actions, so
        // this cannot also pick a row.
        if key == GameKey::Char('U') {
            if let Some(game) = &mut self.game {
                match game.fuse_all_items() {
                    // A confirmation, not a refusal: it says what the key
                    // did, and nothing else visible from behind this popup
                    // says it. Only the `Err` half is history.
                    Ok(msg) => self.status_line = Some(msg),
                    Err(e) => self.refuse(e),
                }
            }
            return;
        }

        // Reads the highlighted row rather than picking one, so it works on
        // a worn slot and a cargo stack alike. Uppercase for `S`'s reason:
        // `selected_index` reserves shifted letters for screen actions, so
        // this cannot also pick a row.
        if key == GameKey::Char('I') {
            match self.menu_selected {
                idx @ 0..=2 => self.inspect_worn(None, EquipmentSlot::ALL[idx], Mode::Inventory),
                idx => {
                    if let Some(row) = inventory.get(idx - 3) {
                        self.open_gear_inspect(row.copy.clone(), None, Mode::Inventory);
                    }
                }
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
            self.open_equip_swap(slot);
            return;
        }
        if let Some(row) = inventory.get(idx - 3) {
            self.pending_inventory_item = Some(row.copy.clone());
            self.mode = Mode::InventoryItemAction;
        }
    }

    /// Which row each equipment slot sits on, so Esc can put the highlight
    /// back where the player left it. The inventory leads with the three
    /// slots and `Mode::CompanionEquip` is nothing but them, so one answer
    /// serves both — it is `EquipmentSlot::ALL`'s order, which is also the
    /// order both screens draw.
    pub(crate) fn slot_row(slot: EquipmentSlot) -> usize {
        EquipmentSlot::ALL
            .iter()
            .position(|s| *s == slot)
            .unwrap_or(0)
    }

    /// Opens the replacement picker for `slot`, unless there is nothing to
    /// pick. An occupied slot always offers to be emptied, so the empty case
    /// only arises for a bare slot with no gear in cargo that fits it —
    /// where opening a picker with no rows would just be a dead end.
    fn open_equip_swap(&mut self, slot: EquipmentSlot) {
        let Some(game) = &self.game else { return };
        let wearer = game.player_entity();
        if equip_swap_rows(game, wearer, slot).is_empty() {
            let line = format!("Nothing in cargo fits your {} slot.", slot.label());
            self.refuse(line);
            return;
        }
        self.pending_swap_slot = Some(slot);
        // The inventory's highlight indexes a different list from this one,
        // and can sit well past its end.
        self.menu_selected = 0;
        self.mode = Mode::EquipSwap;
    }

    /// Picks a replacement for `pending_swap_slot`, or empties it.
    ///
    /// Both outcomes are one engine call that already handles the exchange:
    /// `Game::equip` returns the outgoing item to cargo itself, so a swap is
    /// never an unequip followed by an equip — which could strand the player
    /// bare-handed if the second half were refused.
    ///
    /// `pending_swap_target` decides both the wearer and where Esc goes: the
    /// screen a picker returns to is the one that opened it. It is cleared on
    /// *every* exit, the commit path included, so a later swap opened from
    /// the inventory can never inherit a program from an earlier one.
    pub(crate) fn handle_equip_swap_key(&mut self, key: GameKey) {
        let Some(slot) = self.pending_swap_slot else {
            self.mode = Mode::Inventory;
            return;
        };
        let target = self.pending_swap_target;
        let done = |app: &mut Self| {
            app.pending_swap_slot = None;
            app.pending_swap_target = None;
            app.menu_selected = Self::slot_row(slot);
            match target {
                Some(_) => app.mode = Mode::CompanionEquip,
                None => app.mode = Mode::Inventory,
            }
        };
        if key == GameKey::Esc {
            done(self);
            return;
        }
        let Some(game) = &self.game else { return };
        let wearer = target.unwrap_or_else(|| game.player_entity());
        // The picker is where the question is actually asked — its rows name
        // candidates the player does not own the consequences of yet. The
        // unequip row names no item and so opens nothing.
        if key == GameKey::Char('I') {
            let row = equip_swap_rows(game, wearer, slot)
                .into_iter()
                .nth(self.menu_selected);
            if let Some(SwapRow {
                choice: SwapChoice::Equip(copy),
                ..
            }) = row
            {
                self.open_gear_inspect(copy, target, Mode::EquipSwap);
            }
            return;
        }
        let choices: Vec<SwapChoice> = equip_swap_rows(game, wearer, slot)
            .into_iter()
            .map(|r| r.choice)
            .collect();
        let Some(idx) = self.selected_index(key, choices.len()) else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let outcome = match &choices[idx] {
            SwapChoice::Equip(copy) => game.equip(wearer, copy),
            SwapChoice::Unequip => game.unequip(wearer, slot),
        };
        self.report(outcome);
        done(self);
    }

    /// Sells one `item` to the trader in range, or asks which one.
    ///
    /// Two traders is a question this key cannot answer — their buyback
    /// shelves are separate, so which shop you sell to decides where the
    /// goods can be bought back. Rather than pick a shop on the player's
    /// behalf it opens the existing picker with the item already decided,
    /// which is the `TradeOrigin::Inventory` path.
    fn quick_sell_from_inventory(&mut self, copy: GearCopy) {
        let Some(game) = &mut self.game else { return };
        let traders = traders_in_range(game);
        match traders.as_slice() {
            [] => {
                self.status_line =
                    Some("Nothing in range buys anything. Find a trading post.".to_string())
            }
            [only] => {
                let structure = *only;
                self.execute_trade(structure, TradeChoice::Sell(copy), 1);
            }
            _ => {
                self.pending_trade_choice = Some(TradeChoice::Sell(copy));
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
        let Some(copy) = self.pending_inventory_item.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let actions: Vec<char> = {
            let Some(game) = &mut self.game else {
                self.mode = Mode::Inventory;
                return;
            };
            inventory_item_actions(game, &copy.item)
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
            self.pending_erase = Some(copy);
            self.erase_quantity_input.clear();
            self.mode = Mode::EraseQuantity;
            self.pending_inventory_item = None;
            return;
        }
        if idx.map(|i| actions[i]) == Some('s') {
            self.begin_sale_from_inventory(copy);
            return;
        }
        if idx.map(|i| actions[i]) == Some('d') || key == GameKey::Char('I') {
            // `pending_inventory_item` deliberately survives: Esc comes back
            // here, to the action list for this same copy.
            self.open_gear_inspect(copy, None, Mode::InventoryItemAction);
            return;
        }
        if idx.map(|i| actions[i]) == Some('c') {
            let Some(game) = &mut self.game else { return };
            game.use_item(&copy.item);
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
        let wearer = game.player_entity();
        // `Ok(None)` clears the line, `Ok(Some)` confirms, `Err` refuses.
        // Taken as one value while the `game` borrow is live and acted on
        // once it has ended, because `refuse` wants the whole of `self`.
        let outcome = match idx.map(|i| actions[i]) {
            Some('e') => game.equip(wearer, &copy).map(|()| None),
            Some('u') => game.fuse_item(&copy).map(Some),
            _ => return,
        };
        match outcome {
            Ok(msg) => self.status_line = msg,
            Err(e) => self.refuse(e),
        }
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
    fn begin_sale_from_inventory(&mut self, copy: GearCopy) {
        let Some(game) = &mut self.game else { return };
        let posts: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_trade)
            .collect();
        self.trade_origin = TradeOrigin::Inventory;
        self.pending_trade_choice = Some(TradeChoice::Sell(copy));
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
        // Back to whichever screen opened it. `pending_inspect` is taken
        // rather than read, so a later page cannot inherit this one's return
        // mode the way `pending_swap_target` must not outlive its picker.
        self.mode = self
            .pending_inspect
            .take()
            .map(|inspect| inspect.from)
            .unwrap_or(Mode::InventoryItemAction);
    }

    /// Second page of the erase flow: how many units of `pending_erase` to
    /// destroy. `[A]` erases the whole stack, matching the pre-cap
    /// behavior. An empty input on Enter means 1.
    pub(crate) fn handle_erase_quantity_key(&mut self, key: GameKey) {
        let Some(copy) = self.pending_erase.clone() else {
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
                    .find(|r| r.copy == copy)
                    .map(|r| r.qty)
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
                self.commit_erase(copy, stack_qty);
            }
            GameKey::Enter => {
                let quantity: u32 = if self.erase_quantity_input.is_empty() {
                    1
                } else {
                    self.erase_quantity_input.parse().unwrap_or(0)
                };
                self.commit_erase(copy.clone(), quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::erase_item` and returns to the inventory screen. A
    /// quantity of 0 is a silent no-op rather than a round-trip to the
    /// engine for an error, matching `commit_craft`.
    fn commit_erase(&mut self, copy: GearCopy, quantity: u32) {
        self.pending_erase = None;
        self.erase_quantity_input.clear();
        self.mode = Mode::Inventory;
        if quantity == 0 {
            return;
        }
        if let Some(game) = &mut self.game {
            let outcome = game.erase_item(&copy, quantity);
            self.report(outcome);
        }
    }
}
