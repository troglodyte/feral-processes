//! `Mode::DevItemPicker` — the dev keypad's `Give item` screen.
//!
//! Rows are `Game::dev_plain_items`, so app-core never filters `ItemDb`
//! itself. There are far more items than selectors, so `menu_selected` is a
//! scroll offset and a selector names a row on the visible page.

use crate::{App, GameKey};

/// What one press of a row adds.
const DEV_GIVE_QTY: u32 = 10;

impl App {
    /// The visible page: (selector, item name, held count).
    pub fn dev_item_rows(&self) -> Vec<(char, String, u32)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let rows: Vec<(String, u32)> = game
            .dev_plain_items()
            .into_iter()
            .map(|(_, name, held)| (name, held))
            .collect();
        self.dev_picker_page(&rows)
            .into_iter()
            .map(|(sel, (name, held))| (sel, name, held))
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
        if self.scroll_dev_picker(key, items.len()) {
            return;
        }
        let Some(idx) = self.dev_picker_pick(key, items.len()) else {
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
