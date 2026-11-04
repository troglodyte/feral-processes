//! The Refit Bench view: one copy's condition and the bench's reserve. It
//! only reads; the bench mends on its own while the player stands by it.

use crate::*;

impl App {
    /// Esc leaves; every other key is ignored.
    pub(crate) fn handle_refit_copy_key(&mut self, key: GameKey) {
        if key == GameKey::Esc || self.refit_copy.is_none() {
            self.refit_copy = None;
            self.mode = Mode::Inventory;
        }
    }
}
