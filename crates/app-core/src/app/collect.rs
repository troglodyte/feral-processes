//! The collect picker: a row per item on offer, a quantity per row, and one
//! commit that takes exactly that basket.

use crate::*;

impl App {
    /// Snapshots what is on offer and opens the picker. The basket is
    /// written in the same breath as the rows so the two lengths cannot
    /// disagree.
    pub(crate) fn open_collect(&mut self, offer: Vec<(ItemId, u32)>) {
        self.collect_basket = vec![0; offer.len()];
        self.collect_rows = offer;
        self.menu_selected = 0;
        self.mode = Mode::Collect;
    }

    pub(crate) fn handle_collect_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_collect();
        }
    }

    /// The one teardown both exits use. Clearing the two fields is what
    /// stops a reopened screen showing a stale shelf.
    fn leave_collect(&mut self) {
        self.collect_rows.clear();
        self.collect_basket.clear();
        self.mode = Mode::Playing;
    }
}
