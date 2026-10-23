//! `Mode::DevItemPicker` — the dev keypad's `Give item` screen.
//!
//! Rows are `Game::dev_plain_items`, so app-core never filters `ItemDb`
//! itself. `menu_selected` is the highlighted row, as in every list; Enter
//! gives the highlighted item.

use crate::{App, GameKey};

/// What one press of a row adds.
const DEV_GIVE_QTY: u32 = 10;

impl App {
    /// Every row: (item name, held count). The popup windows itself around
    /// `menu_selected`.
    pub fn dev_item_rows(&self) -> Vec<(String, u32)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        game.dev_plain_items()
            .into_iter()
            .map(|(_, name, held)| (name, held))
            .collect()
    }

    pub(crate) fn handle_dev_item_picker_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_dev_picker();
            return;
        }
        let Some(items) = self.game.as_ref().map(|g| g.dev_plain_items()) else {
            return;
        };
        let Some(idx) = self.selected_index(key, items.len()) else {
            return;
        };
        let Some(game) = self.game.as_mut() else {
            return;
        };
        match game.dev_give_item(&items[idx].0, DEV_GIVE_QTY) {
            Ok(line) => self.status_line = Some(line),
            Err(e) => self.refuse(e),
        }
    }
}
