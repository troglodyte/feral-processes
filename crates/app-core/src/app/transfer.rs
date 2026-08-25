//! The transfer picker's two ends: what it opens on, and where the basket
//! goes. Everything between them — the key table, the cursor, the two
//! ceilings — is `app/basket.rs`.

use crate::*;

impl App {
    /// Snapshots the offer and opens the picker with every row at zero.
    ///
    /// `room` comes straight from `Game::transfer_room` and is passed
    /// through untouched: `None` is "no Depot beside you", `Some(0)` is "a
    /// Depot with nothing left", and the screen has to be able to tell them
    /// apart.
    pub(crate) fn open_transfer(&mut self, rows: Vec<TransferRow>, room: Option<u32>) {
        self.basket_amounts = vec![0; rows.len()];
        self.basket_rows = rows;
        self.basket_room = room;
        self.menu_selected = 0;
        self.mode = Mode::Transfer;
    }

    /// Moves the basket and closes the screen.
    ///
    /// The two halves are split out of one signed list here rather than kept
    /// as two lists in `App`: the row is the thing the player edits, and a
    /// row has one amount.
    ///
    /// An all-zero basket never reaches the engine. `Game::transfer_items`
    /// already makes that request a no-op, so calling through would be
    /// harmless today — but then two places would both have to keep the
    /// no-op true.
    ///
    /// No `status_line`: the engine has already logged what moved, and the
    /// log pane is where a haul is reported.
    pub(crate) fn commit_transfer(&mut self) {
        let (take, give) = self.basket_request();
        if let (false, Some(game)) = (take.is_empty() && give.is_empty(), &mut self.game) {
            game.transfer_items(&take, &give);
        }
        self.leave_basket();
    }
}
