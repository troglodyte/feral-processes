//! The recipe picker and its quantity prompt.

use crate::*;

impl App {
    pub(crate) fn handle_craft_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let recipes = game.craft_recipes();
        if let Some(idx) = self.selected_index(key, recipes.len()) {
            self.pending_craft = Some(recipes[idx].result.clone());
            self.craft_quantity_input.clear();
            self.mode = Mode::CraftQuantity;
        }
    }

    /// Second page of the compile flow: asks how many units of
    /// `pending_craft` to make before actually calling `Game::craft`. `[F]`
    /// is a shortcut for 5 at once, `[M]` for the most affordable right now
    /// (see `Game::max_craftable`) — both bypass typing digits and Enter.
    pub(crate) fn handle_craft_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_craft = None;
                self.craft_quantity_input.clear();
                self.mode = Mode::Craft;
            }
            GameKey::Backspace => {
                self.craft_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.craft_quantity_input.len() < 4 => {
                self.craft_quantity_input.push(c);
            }
            GameKey::Char('f') | GameKey::Char('F') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, 5);
            }
            GameKey::Char('m') | GameKey::Char('M') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                let Some(game) = &self.game else {
                    self.mode = Mode::Playing;
                    return;
                };
                let max = game.max_craftable(&result, false);
                if max == 0 {
                    let name = game.item_name(&result).to_string();
                    self.status_line = Some(format!("Not enough resources to compile any {name}."));
                    self.mode = Mode::Playing;
                    return;
                }
                self.commit_craft(result, max);
            }
            GameKey::Enter => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                let quantity: u32 = if self.craft_quantity_input.is_empty() {
                    1
                } else {
                    self.craft_quantity_input.parse().unwrap_or(0)
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::craft(result, quantity)` and returns to normal play,
    /// shared by the craft-quantity page's Enter, `[F]` (5), and `[M]` (max)
    /// paths. A quantity of 0 (e.g. Enter on an explicitly typed "0") is a
    /// silent no-op rather than a round-trip to the engine for an error.
    fn commit_craft(&mut self, result: ItemId, quantity: u32) {
        if quantity == 0 {
            self.mode = Mode::Playing;
            return;
        }
        if let Some(game) = &mut self.game {
            match game.craft(&result, quantity, false) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }
}
