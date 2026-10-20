//! `Mode::DevZonePicker` — the dev keypad's `Warp to zone` screen.
//!
//! Rows are `Game::dev_warp_targets`. A pick runs the real breach forward
//! (`Game::dev_warp_to_zone`) and closes to the map.

use crate::{App, GameKey, Mode};

impl App {
    /// The visible page: (selector, zone number).
    pub fn dev_zone_rows(&self) -> Vec<(char, u32)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        self.dev_picker_page(&game.dev_warp_targets())
    }

    pub(crate) fn handle_dev_zone_picker_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(zones) = self.game.as_ref().map(|g| g.dev_warp_targets()) else {
            return;
        };
        if self.scroll_dev_picker(key, zones.len()) {
            return;
        }
        let Some(idx) = self.dev_picker_pick(key, zones.len()) else {
            return;
        };
        let Some(game) = self.game.as_mut() else {
            return;
        };
        match game.dev_warp_to_zone(zones[idx]) {
            Ok(line) => {
                self.menu_origin = None;
                self.mode = Mode::Playing;
                self.after_world_action(true, false, 0);
                self.status_line = Some(line);
            }
            Err(e) => self.refuse(e),
        }
    }
}
