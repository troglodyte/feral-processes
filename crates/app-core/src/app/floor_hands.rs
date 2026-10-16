//! `Mode::PutDown` — the picker behind `y`, which sets one thing from the
//! pack on the floor.

use crate::*;
use feral_processes_engine::PutDownOption;

impl App {
    /// The one list the keys and the renderer both read, so a row's shortcut
    /// is the row the player sees.
    pub fn put_down_rows(&self) -> Vec<PutDownOption> {
        self.game
            .as_ref()
            .map(|g| g.put_down_options())
            .unwrap_or_default()
    }

    /// Refused here, with app-core's own sentences, where the engine has no
    /// refusal to hand over: an empty picker is not an action it can fail.
    pub(crate) fn open_put_down(&mut self) {
        let Some(game) = &self.game else { return };
        if !game.in_base() {
            self.refuse("Nothing to put down out here — the floor is through the anchor.");
            return;
        }
        if game.put_down_options().is_empty() {
            self.refuse("You're carrying nothing to put down.");
            return;
        }
        self.mode = Mode::PutDown;
        self.menu_selected = 0;
    }

    pub(crate) fn handle_put_down_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let mut rows = self.put_down_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let choice = rows.swap_remove(idx).choice;
        let Some(game) = &mut self.game else { return };
        match game.put_down(&choice) {
            Ok(()) => {
                self.mode = Mode::Playing;
                self.after_world_action(true, false, 0);
            }
            Err(reason) => self.refuse(reason),
        }
    }
}
