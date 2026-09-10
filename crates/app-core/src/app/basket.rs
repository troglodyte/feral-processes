//! The transfer picker: a row per item, a **signed** amount per row, and one
//! commit that moves exactly that basket in both directions at once.
//!
//! Negative puts into an adjacent Depot, positive takes off an adjacent
//! `Stock`, and the two ends of a row are the two things the arrows walk
//! between. One key table rather than the two screens this replaces, because
//! an item can be on both sides at once and a mirrored table could never say
//! so.
//!
//! The two ceilings are what a row is clamped against, and they are not the
//! same shape. A take is **per row and static** — what that item is sitting
//! on the machine. A put is **one budget shared across every row**, the
//! Depot's remaining room, so filling one row lowers all the others.
//!
//! The table below is deliberately subtle: a `div_ceil` that is what makes
//! the Ctrl step terminate, and a saturating digit accumulation that lets a
//! held key reach the clamp rather than overflow.

use crate::*;
use feral_processes_engine::tuning::MAX_DOWNED_PROGRAMS;

/// Closes half the gap between `n` and `target`, landing exactly on the
/// target rather than stalling one short.
///
/// `div_ceil` on the **magnitude** of the gap is what makes it terminate:
/// rounded down, a gap of one gives a step of zero and the key goes dead
/// with the row neither full nor empty. Generalised over the sign so each
/// modifier pair points at the end its unmodified arrow heads for.
pub(crate) fn half_way_to(n: i64, target: i64) -> i64 {
    let gap = target - n;
    n + gap.signum() * gap.unsigned_abs().div_ceil(2) as i64
}

/// `half_way_to` in the currency the unsigned pickers count in — the
/// caravan basket and the compile quantity page both step a `u32` toward a
/// ceiling, and a second `div_ceil` written out by hand is the copy that
/// goes dead at a gap of one.
pub(crate) fn halve(n: u32, target: u32) -> u32 {
    half_way_to(n as i64, target as i64).max(0) as u32
}

impl App {
    /// How much of the highlighted row the player may still **take**: what
    /// that item is sitting on the adjacent shelves, per row and static.
    pub(crate) fn take_available(&self, row: usize) -> u32 {
        match self.basket_rows.get(row) {
            Some(TransferEntry::Item(r)) => r.on_shelves,
            // A carrier already in the pack has nothing to take; a racked
            // one is one unit, and only while the pack has somewhere to put
            // it. `MAX_DOWNED_PROGRAMS` is a store-wide ceiling rather than
            // a per-row one, so it is the *other* rows that spend it — the
            // same asymmetry `put_available` documents below, and for the
            // same reason: counting this row would make it unlowerable.
            Some(TransferEntry::Carrier(c)) if c.racked => {
                u32::from(self.carrier_pack_room(row) > 0)
            }
            _ => 0,
        }
    }

    /// Room left in the player's own store for another carrier, ignoring
    /// `row`'s own pending amount.
    ///
    /// What is already held is derived from the rows themselves — a carrier
    /// row that is not racked is one sitting in the pack — rather than
    /// carried as a second figure that could disagree with them.
    fn carrier_pack_room(&self, row: usize) -> u32 {
        let held = self
            .basket_rows
            .iter()
            .filter(|e| e.carrier().is_some_and(|c| !c.racked))
            .count();
        let (mut taking, mut giving) = (0usize, 0usize);
        for (i, (entry, n)) in self
            .basket_rows
            .iter()
            .zip(self.basket_amounts.iter())
            .enumerate()
        {
            if i == row || entry.carrier().is_none() {
                continue;
            }
            match (*n).cmp(&0) {
                std::cmp::Ordering::Greater => taking += 1,
                std::cmp::Ordering::Less => giving += 1,
                std::cmp::Ordering::Equal => {}
            }
        }
        let after = held + taking - giving.min(held + taking);
        (MAX_DOWNED_PROGRAMS.saturating_sub(after)) as u32
    }

    /// How much of the highlighted row the player may still **put**:
    /// `TransferRow::can_put` — what the pack holds of it *and is allowed to
    /// put* — capped by the Depot room the other rows have not already
    /// spent. `carried` is deliberately not the figure clamped against: it
    /// is the holding the screen draws, and it stays honest where no Depot
    /// or a bank makes the put impossible.
    ///
    /// Subtracting only the *other* rows is what lets the highlighted row
    /// keep its own amount while it is being edited. Counting itself would
    /// make every key a no-op the moment the basket reached the budget — the
    /// row could never be lowered and then raised again, because its own
    /// units would already be spending the budget it was asking against.
    ///
    /// A pending *take* deliberately does not credit the budget: a take may
    /// come off a machine that is not a Depot, so crediting it would offer
    /// room that never appears. Under-offering is safe — `transfer_items`
    /// takes before it gives, so the real room at the commit is never
    /// smaller than this.
    pub(crate) fn put_available(&self, row: usize) -> u32 {
        // `(-n).max(0)`: a giving row is negative, so this is its magnitude
        // and a taking row contributes nothing. Folded with `saturating_add`
        // because nothing bounds a modded Depot's capacity.
        let given = self
            .basket_amounts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != row)
            .fold(0u32, |acc, (_, n)| acc.saturating_add((-*n).max(0) as u32));
        match self.basket_rows.get(row) {
            Some(TransferEntry::Item(r)) => {
                let budget = self.basket_room.unwrap_or(0).saturating_sub(given);
                r.can_put.min(budget)
            }
            // A pack-side carrier is one unit, and the shared budget it
            // spends is rack slots rather than Depot room — the other
            // carrier rows' pending puts, exactly as above.
            Some(TransferEntry::Carrier(c)) if !c.racked => {
                let spent = self
                    .basket_rows
                    .iter()
                    .zip(self.basket_amounts.iter())
                    .enumerate()
                    .filter(|(i, (e, n))| *i != row && e.carrier().is_some() && **n < 0)
                    .count() as u32;
                u32::from(self.basket_rack_room.saturating_sub(spent) > 0)
            }
            _ => 0,
        }
    }

    /// The key table. Digits are a *quantity* here rather than a row pick,
    /// which is why `selected_index` never appears — `scroll` moves the
    /// cursor on Up/Down and discards everything else it would have
    /// resolved.
    ///
    /// Amounts are edited numerically (`n * 10 + d`, then clamp) rather than
    /// through the `String` buffer the craft and trade quantity pages use.
    /// Those pages have no ceiling to clamp against; this one does, and a
    /// number that cannot exceed what is available is worth having by
    /// construction rather than by a check at the commit.
    ///
    /// **Left takes out and Right puts in**, which is the arrow moving stock
    /// in the direction of the column it heads for. The screen is a table
    /// reading `you | container`, so the container is the rightmost
    /// column and you are the one to its left: Left pulls units off the
    /// container toward you (a take, positive) and Right pushes them from you
    /// into it (a put, negative). `left_takes_out_and_right_puts_in` is the
    /// pin, and the sign convention is untouched by it — only which arrow
    /// reaches which end.
    pub(crate) fn handle_basket_key(&mut self, key: GameKey) {
        let len = self.basket_rows.len();
        match key {
            GameKey::Esc => self.leave_basket(),
            GameKey::Enter => self.commit_transfer(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            // Uppercase for the two screen actions, matching the reserved
            // uppercase convention. Nothing here picks a row by letter, so
            // the reservation costs these screens nothing — but it is what
            // makes uppercase read as "acts" everywhere else.
            //
            // `[A]` writes the take ceiling over **every** row, clearing a
            // give the player had set on a row with nothing on the shelf.
            // That is what "take everything" means on one axis, and it is a
            // decision rather than an oversight.
            //
            // Filled one row at a time rather than zipped straight across:
            // `[A]` no longer touches the shared budget, but the loop is
            // what stops the next person reintroducing a zip that would.
            GameKey::Char('A') => {
                for row in 0..len {
                    let want = self.take_available(row) as i64;
                    if let Some(n) = self.basket_amounts.get_mut(row) {
                        *n = want;
                    }
                }
            }
            GameKey::Char('N') => self.basket_amounts.iter_mut().for_each(|n| *n = 0),
            // The third screen action, and the only one that leaves: what a
            // Depot beside you will take in at all. It is offered from here
            // because this is the screen that already knows the player is
            // standing next to one — and it discards the basket on the way
            // out, since a filter is exactly what `can_put` is derived
            // from. See `App::open_depot_filter`.
            GameKey::Char('F') => self.open_depot_filter(),
            // The magnitude accumulates in the row's **current sign**, and a
            // row sitting at zero types a take. `saturating_*` because a
            // held digit key must reach the clamp rather than overflow.
            GameKey::Char(c) if c.is_ascii_digit() => {
                let d = c.to_digit(10).unwrap_or(0) as u64;
                self.edit_row(|n, _, _| {
                    let sign = if n < 0 { -1 } else { 1 };
                    let magnitude = n.unsigned_abs().saturating_mul(10).saturating_add(d);
                    sign * magnitude.min(i64::MAX as u64) as i64
                });
            }
            GameKey::Backspace => self.edit_row(|n, _, _| n / 10),
            GameKey::Left => self.edit_row(|n, _, _| n + 1),
            GameKey::Right => self.edit_row(|n, _, _| n - 1),
            // The two modifiers are different verbs. Shift is a *target* —
            // an end of the range, idempotent under the key repeat driving
            // these arrows. Ctrl is a *step*: it closes half the gap to the
            // end it is heading for, so pressing it again halves what is
            // left rather than landing on the same number twice.
            GameKey::ShiftLeft => self.edit_row(|_, take, _| take),
            GameKey::ShiftRight => self.edit_row(|_, _, put| -put),
            GameKey::CtrlLeft => self.edit_row(|n, take, _| half_way_to(n, take)),
            GameKey::CtrlRight => self.edit_row(|n, _, put| half_way_to(n, -put)),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's amount and clamps the result to
    /// that row's two ends, so every rule above can be written as the number
    /// it wants rather than as the number it is allowed.
    fn edit_row(&mut self, f: impl FnOnce(i64, i64, i64) -> i64) {
        let row = self.menu_selected;
        if row >= self.basket_rows.len() {
            return;
        }
        let take = self.take_available(row) as i64;
        let put = self.put_available(row) as i64;
        if let Some(n) = self.basket_amounts.get_mut(row) {
            *n = f(*n, take, put).clamp(-put, take);
        }
    }

    /// The basket as the engine wants it: what to take, what to give and
    /// which carriers cross, each with the untouched rows dropped.
    pub(crate) fn basket_request(&self) -> TransferBasket {
        let mut basket = TransferBasket::default();
        let items = self
            .basket_rows
            .iter()
            .filter(|e| e.item().is_some())
            .count();
        for (i, (entry, n)) in self
            .basket_rows
            .iter()
            .zip(self.basket_amounts.iter())
            .enumerate()
        {
            if *n == 0 {
                continue;
            }
            match entry {
                TransferEntry::Item(row) if *n > 0 => {
                    basket.take.push((row.item.clone(), *n as u32))
                }
                TransferEntry::Item(row) => basket
                    .give
                    .push((row.item.clone(), n.unsigned_abs() as u32)),
                // The row's position less the item count — `open_transfer`'s
                // ordering is what makes that the `rack_offer` index.
                TransferEntry::Carrier(_) => basket.carriers.push(i - items),
            }
        }
        basket
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
