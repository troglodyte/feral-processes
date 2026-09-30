//! The Mod Bench screens: stripping and fitting affixes on one copy.

use crate::*;

impl App {
    /// Slot rows: lowercase or a digit highlights one, `R` strips the
    /// highlighted affix, Enter on an empty slot opens the picker. Stripping
    /// is an uppercase action beside the row selectors, the rule every
    /// lettered list follows.
    pub(crate) fn handle_mod_copy_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mod_copy = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(copy) = self.mod_copy.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let Some(rows) = self.game.as_ref().map(|g| mod_slot_rows(g, &copy)) else {
            return;
        };
        // Enter is the one key `selected_index` resolves to a row, and here
        // it is an action on the highlighted row, not a selection.
        if key != GameKey::Enter {
            if let Some(i) = self.selected_index(key, rows.len()) {
                self.menu_selected = i;
                return;
            }
        }
        let row = rows.get(self.menu_selected).cloned().flatten();
        match (key, row) {
            (GameKey::Char('R'), Some(affix)) => {
                let Some(game) = &mut self.game else { return };
                let outcome = game.remove_affix(&copy, &affix);
                self.finish_mod(outcome, Mode::ModCopy);
            }
            (GameKey::Char('R'), None) => self.refuse("That slot is empty."),
            (GameKey::Enter, None) => {
                self.status_line = None;
                self.mode = Mode::ModPickAffix;
            }
            (GameKey::Enter, Some(_)) => self.refuse("That slot is full — press R to strip it."),
            _ => {}
        }
    }

    /// Picker rows apply on select; a refusal (no free slot, can't pay)
    /// stays on the picker's status line so the player can pick differently.
    pub(crate) fn handle_mod_pick_affix_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ModCopy;
            return;
        }
        let Some(copy) = self.mod_copy.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let Some(affixes) = self.game.as_ref().map(|g| g.appliable_affixes(&copy)) else {
            return;
        };
        let Some(affix) = self
            .selected_index(key, affixes.len())
            .map(|i| affixes[i].clone())
        else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let outcome = game.apply_affix(&copy, &affix);
        self.finish_mod(outcome, Mode::ModPickAffix);
    }

    /// Follows the re-keyed copy on success and lands on the slot list; a
    /// refusal goes to the status line and stays on `on_refusal`.
    fn finish_mod(&mut self, outcome: Result<(GearCopy, String), String>, on_refusal: Mode) {
        match outcome {
            Ok((copy, msg)) => {
                self.mod_copy = Some(copy);
                self.status_line = Some(msg);
                self.mode = Mode::ModCopy;
            }
            Err(e) => {
                self.refuse(e);
                self.mode = on_refusal;
            }
        }
    }
}
