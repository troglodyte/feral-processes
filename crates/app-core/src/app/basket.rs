//! The basket picker, shared by both screens that build one: a row per item,
//! a quantity per row, and one commit that moves exactly that basket.
//!
//! Two screens ride this. `Mode::Collect` takes goods off the adjacent
//! machines; `Mode::Deposit` puts the pack's cargo into an adjacent Depot.
//! Everything a player presses is the same on both — the cursor, `[A]`/`[N]`,
//! the digits, the four modifier verbs — and **the only thing that differs is
//! how high a row may go**, which is carried as a value on `App::basket_room`
//! rather than as a second copy of the key table.
//!
//! That is deliberate and load-bearing. The table below is sixty lines of
//! deliberately subtle semantics: an inverted Left/Right that is *specified*
//! to be inverted, a `div_ceil` that is what makes the Ctrl step terminate,
//! and a saturating digit accumulation that lets a held key reach the clamp
//! rather than overflow. Two copies of it would drift, and the inversion is
//! precisely the thing a later reader "restores" — only one copy would be
//! under the test that says so in as many words.

use crate::*;

impl App {
    /// Snapshots the offer and opens whichever picker asked for it. The
    /// amounts are written in the same breath as the rows so the two lengths
    /// cannot disagree.
    ///
    /// `room` is the axis: `None` for a shelf, where each row's ceiling is
    /// its own, and `Some(r)` for one budget shared across every row. See
    /// `basket_available`.
    pub(crate) fn open_basket(&mut self, rows: Vec<(ItemId, u32)>, room: Option<u32>, mode: Mode) {
        self.basket_amounts = vec![0; rows.len()];
        self.basket_rows = rows;
        self.basket_room = room;
        self.menu_selected = 0;
        self.mode = mode;
    }

    /// How high the highlighted row may go, and the whole of what separates
    /// the two screens.
    ///
    /// A shelf gives every row an independent ceiling — what that item is
    /// sitting on the machine. A Depot has **one** `output_room()` across
    /// every row, so filling one row lowers all the others.
    ///
    /// Subtracting only the *other* rows is what lets the highlighted row
    /// keep its own amount while it is being edited. Counting itself would
    /// make every key a no-op the moment the basket reached the budget — the
    /// row could never be lowered and then raised again, because its own
    /// units would already be spending the budget it was asking against.
    pub(crate) fn basket_available(&self, row: usize) -> u32 {
        let taken: u32 = self.basket_amounts.iter().sum();
        let others = taken.saturating_sub(self.basket_amounts.get(row).copied().unwrap_or(0));
        let budget = self
            .basket_room
            .map_or(u32::MAX, |r| r.saturating_sub(others));
        self.basket_rows
            .get(row)
            .map_or(0, |(_, qty)| *qty)
            .min(budget)
    }

    /// The key table. Digits are a *quantity* here rather than a row pick,
    /// which is why `selected_index` never appears — `scroll` moves the
    /// cursor on Up/Down and discards everything else it would have
    /// resolved.
    ///
    /// Amounts are edited numerically (`n * 10 + d`, then clamp) rather than
    /// through the `String` buffer the craft and trade quantity pages use.
    /// Those pages have no ceiling to clamp against; these ones do, and a
    /// number that cannot exceed what is available is worth having by
    /// construction rather than by a check at the commit.
    ///
    /// **Left adds and Right removes**, inverted against every other
    /// Left/Right in the game — the manifest pager, the arena row editor and
    /// all four movement handlers step `Right` positive. It is inverted here
    /// by request, so the inconsistency is the specification rather than a
    /// slip: anything "restoring" it fails
    /// `left_and_right_step_by_one_and_saturate`, which says so in as many
    /// words.
    pub(crate) fn handle_basket_key(&mut self, key: GameKey) {
        let len = self.basket_rows.len();
        match key {
            GameKey::Esc => self.leave_basket(),
            // Which screen this is decides where the basket goes, and that is
            // the one place the two diverge after the keys.
            GameKey::Enter => self.commit_collect(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            // Uppercase for the two screen actions, matching the reserved
            // uppercase convention. Nothing here picks a row by letter, so
            // the reservation costs these screens nothing — but it is what
            // makes uppercase read as "acts" everywhere else.
            //
            // Filled one row at a time through `edit_row` rather than zipped
            // straight across the rows: under a shared budget each row's
            // ceiling depends on what the rows before it just took, so a zip
            // would hand every row the full budget and blow past it.
            GameKey::Char('A') => {
                for row in 0..len {
                    let available = self.basket_available(row);
                    if let Some(want) = self.basket_amounts.get_mut(row) {
                        *want = available;
                    }
                }
            }
            GameKey::Char('N') => self.basket_amounts.iter_mut().for_each(|n| *n = 0),
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
            GameKey::Left => self.edit_row(|n, available| (n + 1).min(available)),
            GameKey::Right => self.edit_row(|n, _| n.saturating_sub(1)),
            // The two modifiers are different verbs. Shift is a *target* —
            // an end of the range, idempotent under the key repeat driving
            // these arrows. Ctrl is a *step*: it closes half the gap to the
            // end it is heading for, so pressing it again halves what is
            // left rather than landing on the same number twice.
            //
            // `div_ceil` on the step is what makes it terminate. Rounded
            // down, a gap of one gives a step of zero and the key goes dead
            // with the row neither full nor empty.
            GameKey::ShiftLeft => self.edit_row(|_, available| available),
            GameKey::ShiftRight => self.edit_row(|_, _| 0),
            GameKey::CtrlLeft => {
                self.edit_row(|n, available| n + available.saturating_sub(n).div_ceil(2))
            }
            GameKey::CtrlRight => self.edit_row(|n, _| n - n.div_ceil(2)),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's amount, handing it that row's own
    /// available so every rule here is per row rather than against a shared
    /// maximum — even when the ceiling those availables come from is itself
    /// shared.
    fn edit_row(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let row = self.menu_selected;
        if row >= self.basket_rows.len() {
            return;
        }
        let available = self.basket_available(row);
        if let Some(want) = self.basket_amounts.get_mut(row) {
            *want = f(*want, available);
        }
    }

    /// The basket as the engine wants it: the rows actually asked for, with
    /// the untouched ones dropped.
    pub(crate) fn basket_request(&self) -> Vec<(ItemId, u32)> {
        self.basket_rows
            .iter()
            .zip(self.basket_amounts.iter())
            .filter(|(_, n)| **n > 0)
            .map(|((item, _), n)| (item.clone(), *n))
            .collect()
    }

    /// The one teardown every exit uses. Clearing the three fields is what
    /// stops a reopened screen showing a stale shelf or a stale pack.
    pub(crate) fn leave_basket(&mut self) {
        self.basket_rows.clear();
        self.basket_amounts.clear();
        self.basket_room = None;
        self.mode = Mode::Playing;
    }
}
