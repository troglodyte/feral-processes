//! The counter a visiting caravan sets out, and the quantity page for
//! selling into it.
//!
//! Modelled on `app/stack_market.rs`, and its header states why a second
//! counterparty gets its own screen rather than a third section bolted into
//! `Mode::Trade`: the two share a concept and nothing else. There is no
//! structure `Entity` here, no buyback section, and the things for sale are
//! not all items. What they *do* share is the engine call that decides a
//! price, which is where sharing belongs.
//!
//! It differs from the Stack market's screen in one way: a caravan's cargo
//! rows can be deep — a wagon carries a stack, not a curio — so selling into
//! one a unit at a time is a keypress per Core Fragment. That used to be a
//! per-item quantity page; it is now a **basket**, modelled on
//! `app/basket.rs`. Every row carries an amount, Enter commits them all
//! through `Game::commit_caravan_basket`, and the visit costs one turn
//! however many lines it holds.

use crate::*;
use feral_processes_engine::views::CaravanView;

/// Which section of the caravan screen a picked row lands in, and its index
/// within that section.
///
/// The offer index carried here is the row's position in the **drawn list**,
/// not its shelf index — `handle_caravan_key` resolves one to the other
/// through the view, because a bought row leaves the list and the two stop
/// agreeing the moment anything is bought. `MarketRow` carries the same
/// warning for the same reason.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CaravanRow {
    Offer(usize),
    Sell(usize),
}

/// Resolves a row number against the caravan screen's two stacked sections —
/// what is on the wagon, then what the wagon will take — given how many rows
/// each contributed. `None` for a row past the end.
///
/// Pulled out for the reason `market_row` and `trade_row` were: the offset
/// arithmetic is where a screen with more than one section goes wrong, and it
/// is the part of this flow that can be tested without a trader standing in
/// front of you.
pub(crate) fn caravan_row(idx: usize, offers: usize, sells: usize) -> Option<CaravanRow> {
    if idx < offers {
        return Some(CaravanRow::Offer(idx));
    }
    (idx - offers < sells).then(|| CaravanRow::Sell(idx - offers))
}

/// `basket::half_way_to` in this screen's currency. A *call*, not a copy:
/// its `div_ceil` on the magnitude of the gap is what makes the step
/// terminate, and rounded down a gap of one gives a step of zero and the key
/// goes dead with the row neither full nor empty.
fn halve(n: u32, target: u32) -> u32 {
    crate::app::basket::half_way_to(n as i64, target as i64).max(0) as u32
}

impl App {
    /// How many of the highlighted **sell** row the player may still put on
    /// the wagon: what the pack holds of it, per row and static.
    ///
    /// `pub`, not `pub(crate)`, for `App::take_available`'s reason: the
    /// screen draws this same figure, and recomputing it in `gui` would be a
    /// second copy of the rule rather than a call to the one governing the
    /// keys. Zero for an offer row, which has no shelf of the player's to
    /// count.
    pub fn caravan_sell_available(&self, view: &CaravanView, row: usize) -> u32 {
        match caravan_row(row, view.offers.len(), view.sells.len()) {
            Some(CaravanRow::Sell(i)) => view.sells[i].held,
            _ => 0,
        }
    }

    /// What the purse can still reach for the highlighted **offer** row:
    /// Credits, plus what the basket's pending sales will pay, minus what
    /// its *other* pending buys have already claimed.
    ///
    /// **One budget across every offer row**, unlike the per-row sell
    /// ceiling — the mirror of the transfer picker's put side, and for the
    /// same reason. Subtracting only the other rows is what lets the
    /// highlighted row be lowered and raised while it is being edited;
    /// counting itself makes every key a no-op the moment the basket reaches
    /// the budget.
    ///
    /// Sales are counted **in**, because `Game::commit_caravan_basket` sells
    /// before it buys. That is the whole reason the two sections are one
    /// basket.
    pub fn caravan_budget(&self, view: &CaravanView, row: usize) -> u32 {
        let offers = view.offers.len();
        let mut proceeds = 0u32;
        let mut claimed = 0u32;
        for (i, n) in self.caravan_amounts.iter().enumerate() {
            match caravan_row(i, offers, view.sells.len()) {
                Some(CaravanRow::Sell(s)) => {
                    proceeds = proceeds.saturating_add(view.sells[s].unit_price * n);
                }
                Some(CaravanRow::Offer(o)) if i != row => {
                    claimed =
                        claimed.saturating_add(view.offers[o].unit_cost * view.offers[o].qty * n);
                }
                _ => {}
            }
        }
        view.credits
            .saturating_add(proceeds)
            .saturating_sub(claimed)
    }

    /// What the highlighted row's amount may reach: one whole shelf slot for
    /// an offer the purse can cover, or the held stack for a sell row.
    pub fn caravan_ceiling(&self, view: &CaravanView, row: usize) -> u32 {
        match caravan_row(row, view.offers.len(), view.sells.len()) {
            // `0..=1`, never a count: what the wagon has of something *is*
            // the offer, and `CaravanOffer::qty` is part of the price the
            // player was quoted.
            Some(CaravanRow::Offer(i)) => {
                let price = view.offers[i].unit_cost * view.offers[i].qty;
                u32::from(self.caravan_budget(view, row) >= price)
            }
            Some(CaravanRow::Sell(_)) => self.caravan_sell_available(view, row),
            None => 0,
        }
    }

    /// The key table.
    ///
    /// **Right increases and Left decreases** — *not* the transfer picker's
    /// inversion. That one is specified for a single row spanning both
    /// directions, so its amount is signed and an arrow picks an end. Here
    /// the sign is fixed by which section the row is in, so inverting would
    /// read as a slip rather than as a specification.
    /// `left_puts_in_and_right_takes_out` is about `Mode::Transfer` and stays
    /// untouched.
    ///
    /// `[A]` fills the **sell** rows only. On the picker it writes the take
    /// ceiling over every row, and the take side is the one with a per-row
    /// ceiling; here that is the sell side. Filling the offer side would
    /// spend the whole purse on one keypress, on a screen with no buyback.
    ///
    /// Enter commits, clears the basket and **leaves the screen open**: a
    /// wagon is a place you shop at, not a form you submit.
    pub(crate) fn handle_caravan_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.caravan_amounts.clear();
            self.close_screen();
            return;
        }
        let Some(view) = self.game.as_mut().and_then(|g| g.caravan_view()) else {
            // The wagon was bought out, or the trader has rolled away.
            self.caravan_amounts.clear();
            self.mode = Mode::Playing;
            return;
        };
        let total = view.offers.len() + view.sells.len();
        // Resized rather than cleared, so a basket survives every keypress
        // that does not tick — which is all of them but Enter. The two lists
        // cannot move without a tick, so index alignment holds by
        // construction rather than by a check.
        if self.caravan_amounts.len() != total {
            self.caravan_amounts = vec![0; total];
        }
        // A cursor past the end must resolve to a row rather than index off
        // it, and it gets there on its own: a committed basket drops bought
        // rows out of the offers section while `menu_selected` stays where it
        // was. Clamped once, here, so every key below can index straight in —
        // `edit_caravan_row` returning early on an out-of-range row is a dead
        // key that looks exactly like a row already at its ceiling.
        self.menu_selected = self.menu_selected.min(total.saturating_sub(1));

        match key {
            GameKey::Up | GameKey::Down => self.scroll(key, total),
            GameKey::Enter => self.commit_caravan_basket(&view),
            GameKey::Char('N') => self.caravan_amounts.iter_mut().for_each(|n| *n = 0),
            GameKey::Char('A') => {
                for row in 0..total {
                    let want = self.caravan_sell_available(&view, row);
                    if let Some(n) = self.caravan_amounts.get_mut(row) {
                        *n = want;
                    }
                }
            }
            // Only the sell rows name a copy the player is holding. An
            // offer's gear row names one that is still on the wagon, and
            // `[I]` scales a piece to its *wearer* — a figure quoted for a
            // copy nobody owns would be scaled to nobody.
            GameKey::Char('I') => {
                if let Some(CaravanRow::Sell(i)) =
                    caravan_row(self.menu_selected, view.offers.len(), view.sells.len())
                {
                    let copy = view.sells[i].copy.clone();
                    self.open_gear_inspect(copy, None, Mode::Caravan);
                }
            }
            GameKey::Left => self.edit_caravan_row(&view, |n, _| n.saturating_sub(1)),
            GameKey::Right => self.edit_caravan_row(&view, |n, _| n.saturating_add(1)),
            // The two modifiers are different verbs, `handle_basket_key`'s
            // split: Shift is a *target* (an end of the row, idempotent
            // under key repeat), Ctrl is a *step* that halves the gap to
            // that end.
            GameKey::ShiftLeft => self.edit_caravan_row(&view, |_, _| 0),
            GameKey::ShiftRight => self.edit_caravan_row(&view, |_, ceiling| ceiling),
            GameKey::CtrlLeft => self.edit_caravan_row(&view, |n, _| halve(n, 0)),
            GameKey::CtrlRight => self.edit_caravan_row(&view, halve),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's amount and clamps it to that
    /// row's ceiling, so every rule above is written as the number it wants
    /// rather than as the number it is allowed.
    fn edit_caravan_row(&mut self, view: &CaravanView, f: impl FnOnce(u32, u32) -> u32) {
        let row = self.menu_selected;
        if row >= self.caravan_amounts.len() {
            return;
        }
        let ceiling = self.caravan_ceiling(view, row);
        if let Some(n) = self.caravan_amounts.get_mut(row) {
            *n = f(*n, ceiling).min(ceiling);
        }
    }

    /// The basket as the engine wants it, and the one commit.
    fn commit_caravan_basket(&mut self, view: &CaravanView) {
        let mut sells = Vec::new();
        let mut buys = Vec::new();
        for (i, n) in self.caravan_amounts.iter().enumerate() {
            if *n == 0 {
                continue;
            }
            match caravan_row(i, view.offers.len(), view.sells.len()) {
                Some(CaravanRow::Offer(o)) => buys.push(view.offers[o].index),
                Some(CaravanRow::Sell(s)) => sells.push((view.sells[s].copy.clone(), *n)),
                None => {}
            }
        }
        let Some(game) = &mut self.game else { return };
        let outcome = game.commit_caravan_basket(sells, buys);
        let committed = outcome.is_ok();
        match outcome {
            // A *confirmation*, so it is written straight to the banner
            // rather than through `refuse` — the log the player scrolls back
            // through is a record of refusals, and the engine has already
            // logged every line this basket moved. It is worth saying at all
            // because the screen stays open and the amounts vanish, which on
            // its own reads exactly like `[N]`.
            Ok(summary) => self.status_line = Some(summary),
            Err(e) => self.refuse(e),
        }
        // Cleared only on a commit that landed: a refused basket is what the
        // player has to fix, and clearing it would delete the thing the
        // refusal is about.
        if committed {
            self.caravan_amounts.clear();
        }
        self.close_if_gone();
    }

    /// A trade costs a tick, and a tick can be the one the trader leaves on
    /// — or the one that starves the player. `handle_stack_market_key`'s
    /// `close_if_bought_out`, and for both of its reasons.
    fn close_if_gone(&mut self) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            return;
        }
        if self
            .game
            .as_mut()
            .is_some_and(|g| g.caravan_view().is_none())
        {
            self.caravan_amounts.clear();
            self.mode = Mode::Playing;
        }
    }
}
