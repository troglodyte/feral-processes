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
    pub(crate) fn open_transfer(
        &mut self,
        rows: Vec<TransferRow>,
        carriers: Vec<TransferCarrier>,
        room: Option<u32>,
        rack_room: u32,
        source: TransferSource,
    ) {
        // Items first, carriers after: a carrier row's position less the
        // item count is its index into `Game::rack_offer()`, which is what
        // `basket_request` hands the commit door.
        let entries: Vec<TransferEntry> = rows
            .into_iter()
            .map(TransferEntry::Item)
            .chain(carriers.into_iter().map(TransferEntry::Carrier))
            .collect();
        self.basket_amounts = vec![0; entries.len()];
        self.basket_rows = entries;
        self.basket_room = room;
        self.basket_rack_room = rack_room;
        self.transfer_source = source;
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
    /// and `Game::take_from_outpost` already make that request a no-op, so
    /// calling through would be harmless today — but then two places would
    /// both have to keep the no-op true.
    ///
    /// No `status_line`: the engine has already logged what moved, and the
    /// log pane is where a haul is reported.
    ///
    /// **`TransferSource` decides which door the basket spends through, and
    /// nothing else about the screen changes.** An outpost's rows are
    /// `can_put: 0` (`Game::outpost_transfer_offer`), so `basket.give` is
    /// always empty there — `Game::take_from_outpost` only ever sees a
    /// take.
    pub(crate) fn commit_transfer(&mut self) {
        let basket = self.basket_request();
        if !basket.is_empty()
            && let Some(game) = &mut self.game
        {
            match self.transfer_source {
                TransferSource::Base => {
                    game.transfer_items(&basket);
                }
                TransferSource::Outpost(tile) => {
                    self.status_line = game.take_from_outpost(tile, &basket).err();
                }
            }
        }
        self.leave_basket();
    }
}
