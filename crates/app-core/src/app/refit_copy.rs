//! The Refit Bench screen: paying to restore one worn copy.

use crate::*;

impl App {
    /// Enter pays the bill and mends the copy; Esc leaves. A refusal (no
    /// bench, a short purse, nothing worn) is the engine's sentence on the
    /// status line, and the screen stays on the same copy.
    pub(crate) fn handle_refit_copy_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.refit_copy = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(copy) = self.refit_copy.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        if key != GameKey::Enter {
            return;
        }
        let Some(game) = &mut self.game else { return };
        match game.refit_copy(&copy) {
            // The engine has logged the sentence; the line only ever draws
            // as a refusal, so success clears it, as `finish_mod` does.
            Ok((mended, _)) => {
                self.refit_copy = Some(mended);
                self.status_line = None;
            }
            Err(e) => self.refuse(e),
        }
    }
}
