//! A settlement's shelf, opened with `[M]` from `Mode::Settlement`.
//!
//! `app/caravan.rs`'s shape, one vendor over: one basket, Left/Right edits
//! the highlighted row, Enter commits both halves in one turn through
//! `Game::commit_settlement_basket` and leaves the screen open. The row
//! resolution is `caravan_row`/`CaravanRow` themselves, reused rather than
//! copied — a settlement's shelf and a caravan's wagon resolve a picked row
//! against the same two-section shape (what's on offer, then what the
//! vendor will take), and a second enum saying the same thing would be the
//! copy that drifts the moment one of them grows a third section.
//!
//! It differs from the caravan screen in two ways. There is no `trader`
//! field to title the popup with — `Game::settlement_report`'s `name` is
//! read instead, since identity lives on the hub page this market is opened
//! from and a settlement never leaves to make the wording "who is this"
//! wrong on the way out. And Esc returns to `Mode::Settlement` rather than
//! the map, because it was reached from a page rather than the map — like
//! `Mode::CompanionEquip`, which returns to `Mode::Companion`, rather than
//! `Mode::Caravan`, which is reached straight off the base menu and returns
//! there through `App::close_screen`.

use crate::app::basket::halve;
use crate::app::caravan::{CaravanRow, caravan_row};
use crate::*;
use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::views::SettlementMarketView;

impl App {
    /// How many of the highlighted **sell** row the player may still put on
    /// the shelf: what the pack holds of it, per row and static —
    /// `App::caravan_sell_available`'s shape exactly.
    pub fn settlement_sell_available(&self, view: &SettlementMarketView, row: usize) -> u32 {
        match caravan_row(row, view.offers.len(), view.sells.len()) {
            Some(CaravanRow::Sell(i)) => view.sells[i].held,
            _ => 0,
        }
    }

    /// What the purse can still reach for the highlighted **offer** row —
    /// `App::caravan_budget`'s shape exactly: one budget across every offer
    /// row, counting the basket's own pending sales in and every *other*
    /// pending buy out, because `Game::commit_settlement_basket` sells
    /// before it buys.
    pub fn settlement_budget(&self, view: &SettlementMarketView, row: usize) -> u32 {
        let offers = view.offers.len();
        let mut proceeds = 0u32;
        let mut claimed = 0u32;
        for (i, n) in self.settlement_amounts.iter().enumerate() {
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

    /// What the purse will hold once this basket commits —
    /// `App::caravan_purse_after`'s shape: `settlement_budget` with no row
    /// held back.
    pub fn settlement_purse_after(&self, view: &SettlementMarketView) -> u32 {
        self.settlement_budget(view, usize::MAX)
    }

    /// What the highlighted row's amount may reach: one whole shelf slot for
    /// an offer the purse can cover, or the held stack for a sell row —
    /// `App::caravan_ceiling`'s shape.
    pub fn settlement_ceiling(&self, view: &SettlementMarketView, row: usize) -> u32 {
        match caravan_row(row, view.offers.len(), view.sells.len()) {
            Some(CaravanRow::Offer(i)) => {
                let price = view.offers[i].unit_cost * view.offers[i].qty;
                u32::from(self.settlement_budget(view, row) >= price)
            }
            Some(CaravanRow::Sell(_)) => self.settlement_sell_available(view, row),
            None => 0,
        }
    }

    /// The key table — `App::handle_caravan_key`'s shape, including its
    /// **Right increases and Left decreases** rule and `[A]` filling the
    /// sell rows only, for the same reasons stated there.
    pub(crate) fn handle_settlement_market_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.settlement_amounts.clear();
            self.mode = Mode::Settlement;
            return;
        }
        let Some(settlement_key) = self.pending_settlement else {
            self.settlement_amounts.clear();
            self.mode = Mode::Playing;
            return;
        };
        let Some(view) = self
            .game
            .as_mut()
            .and_then(|g| g.settlement_view(settlement_key))
        else {
            // The party stepped off the settlement's tile, or it never
            // materialized in the first place. The key is dropped with the
            // basket, not left behind: three arms drop out of this screen
            // and all three must leave `pending_settlement` in the same
            // state, or which one you left by decides what a later reader
            // sees.
            self.settlement_amounts.clear();
            self.pending_settlement = None;
            self.mode = Mode::Playing;
            return;
        };
        let total = view.offers.len() + view.sells.len();
        // Resized rather than cleared, `handle_caravan_key`'s reason: a
        // basket survives every keypress that does not tick, which is all
        // of them but Enter, and the two lists cannot move without one.
        if self.settlement_amounts.len() != total {
            self.settlement_amounts = vec![0; total];
        }
        self.menu_selected = self.menu_selected.min(total.saturating_sub(1));

        match key {
            GameKey::Up | GameKey::Down => self.scroll(key, total),
            GameKey::Enter => self.commit_settlement_market_basket(settlement_key, &view),
            GameKey::Char('N') => self.settlement_amounts.iter_mut().for_each(|n| *n = 0),
            GameKey::Char('A') => {
                for row in 0..total {
                    let want = self.settlement_sell_available(&view, row);
                    if let Some(n) = self.settlement_amounts.get_mut(row) {
                        *n = want;
                    }
                }
            }
            // Only the sell rows name a copy the player is holding —
            // `handle_caravan_key`'s reason for the same guard.
            GameKey::Char('I') => {
                if let Some(CaravanRow::Sell(i)) =
                    caravan_row(self.menu_selected, view.offers.len(), view.sells.len())
                {
                    let copy = view.sells[i].copy.clone();
                    self.open_gear_inspect(copy, None, Mode::SettlementMarket);
                }
            }
            GameKey::Left => self.edit_settlement_market_row(&view, |n, _| n.saturating_sub(1)),
            GameKey::Right => self.edit_settlement_market_row(&view, |n, _| n.saturating_add(1)),
            GameKey::ShiftLeft => self.edit_settlement_market_row(&view, |_, _| 0),
            GameKey::ShiftRight => self.edit_settlement_market_row(&view, |_, ceiling| ceiling),
            GameKey::CtrlLeft => self.edit_settlement_market_row(&view, |n, _| halve(n, 0)),
            GameKey::CtrlRight => self.edit_settlement_market_row(&view, halve),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's amount and clamps it to that
    /// row's ceiling — `App::edit_caravan_row`'s shape.
    fn edit_settlement_market_row(
        &mut self,
        view: &SettlementMarketView,
        f: impl FnOnce(u32, u32) -> u32,
    ) {
        let row = self.menu_selected;
        if row >= self.settlement_amounts.len() {
            return;
        }
        let ceiling = self.settlement_ceiling(view, row);
        if let Some(n) = self.settlement_amounts.get_mut(row) {
            *n = f(*n, ceiling).min(ceiling);
        }
    }

    /// The basket as the engine wants it, and the one commit —
    /// `App::commit_caravan_basket`'s shape, at the settlement `key`.
    fn commit_settlement_market_basket(&mut self, key: SettlementKey, view: &SettlementMarketView) {
        let mut sells = Vec::new();
        let mut buys = Vec::new();
        for (i, n) in self.settlement_amounts.iter().enumerate() {
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
        let outcome = game.commit_settlement_basket(key, sells, buys);
        let committed = outcome.is_ok();
        match outcome {
            Ok(summary) => self.status_line = Some(summary),
            Err(e) => self.refuse(e),
        }
        if committed {
            self.settlement_amounts.clear();
        }
        self.close_if_settlement_gone(key);
    }

    /// A trade costs a tick, and a tick can be the one that starves the
    /// player — `App::close_if_gone`'s reason, one vendor over. Unlike a
    /// caravan a settlement never rolls away, so the only way
    /// `Game::settlement_view` goes `None` here is the party having been
    /// moved off its tile inside that tick; either way `pending_settlement`
    /// is cleared with it, `handle_settlement_key`'s Esc arm's own rule.
    fn close_if_settlement_gone(&mut self, key: SettlementKey) {
        self.check_game_over();
        if self.mode == Mode::GameOver {
            return;
        }
        if self
            .game
            .as_mut()
            .is_some_and(|g| g.settlement_view(key).is_none())
        {
            self.settlement_amounts.clear();
            self.pending_settlement = None;
            self.mode = Mode::Playing;
        }
    }
}
