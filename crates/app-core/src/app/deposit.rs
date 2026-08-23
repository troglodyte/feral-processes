//! The deposit picker's two ends: what it opens on, and where the basket
//! goes. Everything between them is `app/basket.rs`, shared with the collect
//! screen.

use crate::*;

impl App {
    /// Snapshots the pack and opens the picker.
    ///
    /// `Some(room)`: a Depot has one `output_room()` across every item, so
    /// unlike the collect screen's shelf the ceiling here is a single budget
    /// the rows spend between them. That is what makes filling one row lower
    /// the rest, and it is enforced in `App::basket_available` rather than at
    /// the commit — a number that cannot exceed what fits is worth having by
    /// construction.
    pub(crate) fn open_deposit(&mut self, offer: Vec<(ItemId, u32)>, room: u32) {
        self.open_basket(offer, Some(room), Mode::Deposit);
    }

    /// Puts the basket into the base's hands and closes the screen.
    ///
    /// An all-zero basket never reaches the engine, for `commit_collect`'s
    /// reason: `Game::deposit_items` already makes that request a no-op, so
    /// calling through would be harmless today — but then two places would
    /// both have to keep the no-op true.
    ///
    /// No `status_line`: the engine has already logged what landed, and the
    /// log pane is where the base reports what it received.
    pub(crate) fn commit_deposit(&mut self) {
        let give = self.basket_request();
        if let (false, Some(game)) = (give.is_empty(), &mut self.game) {
            game.deposit_items(&give);
        }
        self.leave_basket();
    }
}
