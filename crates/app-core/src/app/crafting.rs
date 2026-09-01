//! The recipe picker and its quantity prompt.

use crate::app::basket::halve;
use crate::*;

/// The most a compile may be asked for, matching the four-digit ceiling the
/// typed buffer already has. An arrow that walked past what the player can
/// type would leave the page in a state Backspace could not get back out of.
const CRAFT_QUANTITY_MAX: u32 = 9999;

impl App {
    pub(crate) fn handle_craft_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let recipes = game.craft_recipes();
        if let Some(idx) = self.selected_index(key, recipes.len()) {
            self.pending_craft = Some(recipes[idx].result.clone());
            self.craft_quantity_input.clear();
            self.careful_craft = false;
            self.mode = Mode::CraftQuantity;
        }
    }

    /// The quantity this page is offering: what it draws, what an arrow
    /// steps, and what Enter compiles. An empty buffer is **1**, not 0 — the
    /// page opens on one, and an arrow that stepped the parsed zero would
    /// print the number already on screen and read as a dropped keypress.
    ///
    /// `pub` for `App::take_available`'s reason: the screen prints this same
    /// figure, and its own empty-buffer branch was a second copy of the rule
    /// rather than a call to the one the keys are answering.
    pub fn craft_quantity(&self) -> u32 {
        if self.craft_quantity_input.is_empty() {
            1
        } else {
            self.craft_quantity_input.parse().unwrap_or(0)
        }
    }

    /// Second page of the compile flow: asks how many units of
    /// `pending_craft` to make before actually calling `Game::craft`. `[F]`
    /// is a shortcut for 5 at once, `[M]` for the most affordable right now
    /// (see `Game::max_craftable`) — both bypass typing digits and Enter.
    ///
    /// `[C]` toggles a careful compile, which is why `[M]` reads the
    /// maximum *at the price the batch will actually be charged*: a careful
    /// max quoted off the plain price is a batch the player cannot afford.
    ///
    /// **Right increases and Left decreases**, `app/caravan.rs`' rule and
    /// for its reason: the quantity is unsigned and there is no container
    /// column for an arrow to point at, so the transfer picker's inversion
    /// does not apply. Shift is a *target* and Ctrl a *step*, the split
    /// `app/basket.rs` made, and the end they head for is `craft_ceiling`.
    pub(crate) fn handle_craft_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_craft = None;
                self.craft_quantity_input.clear();
                self.mode = Mode::Craft;
            }
            GameKey::Backspace => {
                self.craft_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.craft_quantity_input.len() < 4 => {
                self.craft_quantity_input.push(c);
            }
            GameKey::Left => self.step_craft_quantity(|n, _| n.saturating_sub(1)),
            GameKey::Right => self.step_craft_quantity(|n, _| n.saturating_add(1)),
            GameKey::ShiftLeft => self.step_craft_quantity(|_, _| 0),
            GameKey::ShiftRight => self.step_craft_quantity(|_, max| max),
            GameKey::CtrlLeft => self.step_craft_quantity(|n, _| halve(n, 0)),
            GameKey::CtrlRight => self.step_craft_quantity(halve),
            GameKey::Char('c') | GameKey::Char('C') => {
                self.careful_craft = !self.careful_craft;
            }
            GameKey::Char('f') | GameKey::Char('F') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, 5);
            }
            GameKey::Char('m') | GameKey::Char('M') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                let Some(game) = &self.game else {
                    self.mode = Mode::Playing;
                    return;
                };
                let max = game.max_craftable(&result, self.careful_craft);
                if max == 0 {
                    let name = game.item_name(&result).to_string();
                    self.refuse(format!("Not enough resources to compile any {name}."));
                    self.mode = Mode::Playing;
                    return;
                }
                self.commit_craft(result, max);
            }
            GameKey::Enter => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                let quantity = self.craft_quantity();
                self.craft_quantity_input.clear();
                self.commit_craft(result, quantity);
            }
            _ => {}
        }
    }

    /// What `[M]` would compile: the most this batch can afford, priced with
    /// the careful flag the page is currently holding. The end the two
    /// right-hand modifiers head for, so the number an arrow reaches and the
    /// number the page prints are one call rather than two.
    fn craft_ceiling(&self) -> u32 {
        match (&self.game, &self.pending_craft) {
            (Some(game), Some(result)) => game.max_craftable(result, self.careful_craft),
            _ => 0,
        }
    }

    /// Applies `f` to the quantity and writes it back, clamped to the page's
    /// own four-digit ceiling.
    ///
    /// The max affordable is handed in as the *target* the modifiers aim at
    /// rather than as a clamp on every arrow. `[F]` already offers a batch
    /// the purse may not cover and the engine refuses it by name, so a plain
    /// arrow that snapped a typed 50 down to the ten you can afford would be
    /// the one place on this screen where a number the player set moved on
    /// its own.
    fn step_craft_quantity(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let max = self.craft_ceiling();
        let n = f(self.craft_quantity(), max).min(CRAFT_QUANTITY_MAX);
        self.craft_quantity_input = n.to_string();
    }

    /// Calls `Game::craft(result, quantity)` and returns to normal play,
    /// shared by the craft-quantity page's Enter, `[F]` (5), and `[M]` (max)
    /// paths. A quantity of 0 (e.g. Enter on an explicitly typed "0") is a
    /// silent no-op rather than a round-trip to the engine for an error.
    fn commit_craft(&mut self, result: ItemId, quantity: u32) {
        if quantity == 0 {
            self.mode = Mode::Playing;
            return;
        }
        if let Some(game) = &mut self.game {
            let outcome = game.craft(&result, quantity, self.careful_craft);
            self.report(outcome);
        }
        self.mode = Mode::Playing;
    }
}
