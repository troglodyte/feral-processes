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

    /// The key table. Digits are a *quantity* here rather than a row pick,
    /// which is why `selected_index` never appears — `scroll` moves the
    /// cursor on Up/Down and discards everything else it would have
    /// resolved.
    ///
    /// Amounts are edited numerically (`n * 10 + d`, then clamp) rather than
    /// through the `String` buffer the craft and trade quantity pages use.
    /// Those pages have no ceiling to clamp against; this one does, and a
    /// number that cannot exceed what is on the shelf is worth having by
    /// construction rather than by a check at the commit.
    pub(crate) fn handle_collect_key(&mut self, key: GameKey) {
        let len = self.collect_rows.len();
        match key {
            GameKey::Esc => self.leave_collect(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            // Uppercase for the two screen actions, matching the reserved
            // uppercase convention. Nothing here picks a row by letter, so
            // the reservation costs this screen nothing — but it is what
            // makes uppercase read as "acts" everywhere else.
            GameKey::Char('A') => {
                for (row, want) in self.collect_rows.iter().zip(self.collect_basket.iter_mut()) {
                    *want = row.1;
                }
            }
            GameKey::Char('N') => self.collect_basket.iter_mut().for_each(|n| *n = 0),
            GameKey::Char(c) if c.is_ascii_digit() => {
                // `saturating_mul`/`saturating_add`: a held digit key must
                // reach the clamp rather than overflowing `u32` on the way.
                self.edit_row(|n, available| {
                    n.saturating_mul(10)
                        .saturating_add(c.to_digit(10).unwrap_or(0))
                        .min(available)
                });
            }
            GameKey::Backspace => self.edit_row(|n, _| n / 10),
            GameKey::Right => self.edit_row(|n, available| (n + 1).min(available)),
            GameKey::Left => self.edit_row(|n, _| n.saturating_sub(1)),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's amount, handing it that row's
    /// own available so every rule here is per row rather than against a
    /// shared maximum.
    fn edit_row(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let row = self.menu_selected;
        let Some(available) = self.collect_rows.get(row).map(|(_, n)| *n) else {
            return;
        };
        if let Some(want) = self.collect_basket.get_mut(row) {
            *want = f(*want, available);
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
