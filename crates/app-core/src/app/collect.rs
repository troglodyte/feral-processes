//! The collect picker's two ends: what it opens on, and where the basket
//! goes. Everything between them — the key table, the cursor, the per-row
//! clamping — is `app/basket.rs`, shared with the deposit screen.

use crate::*;

impl App {
    /// Snapshots what the adjacent machines are offering and opens the
    /// picker.
    ///
    /// `None` room: a shelf gives every row its own ceiling, which is what
    /// that item is sitting on the machine. Nothing is shared across rows
    /// here, unlike the deposit screen's single Depot budget.
    pub(crate) fn open_collect(&mut self, offer: Vec<(ItemId, u32)>) {
        self.open_basket(offer, None, Mode::Collect);
    }

    /// Takes the basket and closes the screen.
    ///
    /// An all-zero basket never reaches the engine. `Game::collect_items`
    /// already makes that request a no-op, so calling through would be
    /// harmless today — but then two places would both have to keep the
    /// no-op true.
    ///
    /// No `status_line`: the engine has already logged the haul, and the log
    /// pane is where a haul is reported.
    pub(crate) fn commit_collect(&mut self) {
        let want = self.basket_request();
        if let (false, Some(game)) = (want.is_empty(), &mut self.game) {
            game.collect_items(&want);
        }
        self.leave_basket();
    }
}
