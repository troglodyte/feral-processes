//! The perk and research pickers.

use crate::*;

impl App {
    /// Picks a numbered perk to unlock; stays open so multiple can be
    /// unlocked in one visit if there are enough Perk Points.
    pub(crate) fn handle_perks_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let perks = feral_processes_engine::Perk::all();
        if let Some(idx) = self.selected_index(key, perks.len()) {
            let Some(game) = &mut self.game else { return };
            match game.unlock_perk(perks[idx]) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }

    /// Picks a numbered research node to unlock; stays open so several can
    /// be taken in one visit.
    pub(crate) fn handle_research_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
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
        if let Some(idx) = self.selected_index(key, ids.len()) {
            let id = ids[idx].clone();
            let Some(game) = &mut self.game else { return };
            match game.unlock_research(&id) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }
}
