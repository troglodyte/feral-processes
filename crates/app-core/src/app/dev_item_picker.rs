//! `Mode::DevItemPicker` — the dev keypad's `Give item` screen.
//!
//! Rows are `Game::dev_plain_items`, so app-core never filters `ItemDb`
//! itself. `menu_selected` is the highlighted row, as in every list; Enter
//! gives the highlighted item.

use crate::app::list_filter::filter_matches;
use crate::{App, GameKey};
use feral_processes_engine::Game;

/// What one press of a row adds.
const DEV_GIVE_QTY: u32 = 10;

/// Indices into `dev_plain_items()` that `filter` leaves, by name. The
/// handler and `App::dev_item_rows` both go through it.
pub fn dev_item_visible(game: &Game, filter: &str) -> Vec<usize> {
    let items = game.dev_plain_items();
    filter_matches(items.iter().map(|(_, name, _)| name.as_str()), filter)
}

impl App {
    /// The rows the filter leaves: (item name, held count). The popup
    /// windows itself around `menu_selected`.
    pub fn dev_item_rows(&self) -> Vec<(String, u32)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let items = game.dev_plain_items();
        dev_item_visible(game, self.filter_text())
            .into_iter()
            .map(|i| (items[i].1.clone(), items[i].2))
            .collect()
    }

    pub(crate) fn handle_dev_item_picker_key(&mut self, key: GameKey) {
        if self.take_filter_key(key) {
            return;
        }
        if key == GameKey::Esc {
            self.close_dev_picker();
            return;
        }
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let visible = dev_item_visible(game, self.filter_text());
        let Some(pos) = self.selected_index(key, visible.len()) else {
            return;
        };
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let id = game.dev_plain_items()[visible[pos]].0.clone();
        match game.dev_give_item(&id, DEV_GIVE_QTY) {
            Ok(line) => self.status_line = Some(line),
            Err(e) => self.refuse(e),
        }
    }
}
