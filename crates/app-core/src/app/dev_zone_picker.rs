//! `Mode::DevZonePicker` — the dev keypad's `Warp to zone` screen.
//!
//! Rows are `Game::dev_warp_targets`. A pick runs the real breach forward
//! (`Game::dev_warp_to_zone`) and closes to the map.

use crate::{App, GameKey, Mode};

impl App {
    /// Every row: the zone number. The popup windows itself around
    /// `menu_selected`.
    pub fn dev_zone_rows(&self) -> Vec<u32> {
        self.game
            .as_ref()
            .map(|g| g.dev_warp_targets())
            .unwrap_or_default()
    }

    pub(crate) fn handle_dev_zone_picker_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_dev_picker();
            return;
        }
        let Some(zones) = self.game.as_ref().map(|g| g.dev_warp_targets()) else {
            return;
        };
        let Some(idx) = self.selected_index(key, zones.len()) else {
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
