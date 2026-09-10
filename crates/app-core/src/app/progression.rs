//! The perk and research pickers.

use crate::*;
use feral_processes_engine::GraphDir;

impl App {
    /// Picks a numbered perk to unlock; stays open so multiple can be
    /// unlocked in one visit if there are enough Perk Points.
    pub(crate) fn handle_perks_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        // Through `perk_defs` rather than `Perk::all()` so the numbering the
        // player types against is the one the picker drew — a perk whose
        // `.ron` file is missing isn't listed and can't be bought by index.
        // Uppercase, and checked before `selected_index`: lowercase letters
        // are row labels past the digits (`DIGIT_ROWS + (c - 'a')`) and this
        // picker has eighteen rows, so a lowercase key would pick a perk
        // *and* open the wipe on one keypress.
        if key == GameKey::Char('X') {
            self.mode = Mode::RespecPerksConfirm;
            return;
        }
        let Some(perks) = self
            .game
            .as_ref()
            .map(|g| g.perk_defs().into_iter().map(|d| d.id).collect::<Vec<_>>())
        else {
            return;
        };
        if let Some(idx) = self.selected_index(key, perks.len()) {
            let Some(game) = &mut self.game else { return };
            let outcome = game.unlock_perk(perks[idx]);
            self.report(outcome);
        }
    }

    /// Confirms or backs out of a full perk refund.
    pub(crate) fn handle_respec_perks_confirm_key(&mut self, key: GameKey) {
        match key {
            GameKey::Char('y') | GameKey::Char('Y') => {
                if let Some(game) = &mut self.game {
                    let outcome = game.respec_perks();
                    self.report(outcome);
                }
                self.mode = Mode::Perks;
            }
            GameKey::Esc | GameKey::Char('n') | GameKey::Char('N') => self.mode = Mode::Perks,
            _ => {}
        }
    }

    /// Picks a numbered research node to unlock; stays open so several can
    /// be taken in one visit.
    pub(crate) fn handle_research_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        // Checked before `selected_index`, for the reason `handle_perks_key`
        // checks its own `X` first: uppercase is not a row selector today,
        // but putting the check after would be a live hazard the day
        // somebody widens `selected_index`.
        if key == GameKey::Char('G') {
            self.research_graph_view = !self.research_graph_view;
            return;
        }
        // Collecting the ids through `as_ref().map` (rather than a
        // `let Some(game) = &self.game` binding) ends the borrow here —
        // `selected_index` needs `&mut self`.
        let Some(ids) = self.game.as_ref().map(|g| {
            g.research_nodes()
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        if !self.research_graph_view {
            if let Some(idx) = self.selected_index(key, ids.len()) {
                let id = ids[idx].clone();
                let Some(game) = &mut self.game else { return };
                let outcome = game.unlock_research(&id);
                self.report(outcome);
            }
            return;
        }
        // The graph view. Digits and lowercase letters select nothing here —
        // a row number labels nothing the player can see on a flow chart.
        let Some(from) = ids.get(self.menu_selected.min(ids.len().saturating_sub(1))) else {
            return;
        };
        let dir = match key {
            GameKey::Up => Some(GraphDir::Up),
            GameKey::Down => Some(GraphDir::Down),
            GameKey::Left => Some(GraphDir::Left),
            GameKey::Right => Some(GraphDir::Right),
            _ => None,
        };
        if let Some(dir) = dir {
            let Some(game) = self.game.as_ref() else {
                return;
            };
            let landed = game.research_graph().step(from, dir);
            // A scan of 34 entries per keypress. An index map would be a
            // second cursor to keep in step with `research_nodes()`, which
            // re-sorts by state.
            if let Some(idx) = ids.iter().position(|id| *id == landed) {
                self.menu_selected = idx;
            }
            return;
        }
        if key == GameKey::Enter {
            let id = from.clone();
            let Some(game) = &mut self.game else { return };
            let outcome = game.unlock_research(&id);
            self.report(outcome);
        }
    }
}
