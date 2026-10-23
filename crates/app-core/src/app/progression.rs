//! The perk and research pickers.

use crate::*;
use feral_processes_engine::StatOwner;
use feral_processes_engine::{GraphDir, ResearchState, ResearchTree};

impl App {
    /// Picks a numbered perk to unlock; stays open so multiple can be
    /// unlocked in one visit if there are enough Perk Points.
    pub(crate) fn handle_perks_key(&mut self, key: GameKey) {
        // Through `perk_defs` rather than `Perk::all()` so the numbering the
        // player types against is the one the picker drew — a perk whose
        // `.ron` file is missing isn't listed and can't be bought by index.
        // Uppercase, so a stray lowercase `x` can never open a wipe of
        // eighteen rows' worth of spent perks.
        if key == GameKey::Char('X') {
            self.mode = Mode::RespecPerksConfirm;
            return;
        }
        // Uppercase for the same reason as `X`.
        if key == GameKey::Char('S') {
            self.open_stat_allocation(StatOwner::Player, AllocationOrigin::Perks);
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
            match game.buy_perk(perks[idx]) {
                Ok(report) => {
                    self.status_line = None;
                    self.pending_perk_report = Some(report);
                    self.mode = Mode::PerkBought;
                }
                Err(why) => self.refuse(why),
            }
        }
    }

    /// `Enter` or `Esc` leaves the purchase page for the menu it came from.
    pub(crate) fn handle_perk_bought_key(&mut self, key: GameKey) {
        if matches!(key, GameKey::Enter | GameKey::Esc) {
            self.pending_perk_report = None;
            self.mode = Mode::Perks;
        }
    }

    /// The **one** writer of `perk_previews`, run by `handle_key` on every
    /// entry to `Mode::Perks`.
    pub(crate) fn refresh_perk_previews(&mut self) {
        let Some(game) = &mut self.game else {
            self.perk_previews.clear();
            return;
        };
        self.perk_previews = game
            .perk_defs()
            .into_iter()
            .map(|def| game.preview_perk(def.id))
            .collect();
    }

    /// Confirms or backs out of a full perk refund.
    pub(crate) fn handle_respec_perks_confirm_key(&mut self, key: GameKey) {
        let confirmed = match key {
            GameKey::Esc => Some(false),
            _ => self.yes_no(key),
        };
        match confirmed {
            Some(true) => {
                if let Some(game) = &mut self.game {
                    let outcome = game.respec_perks();
                    self.report(outcome);
                }
                self.mode = Mode::Perks;
            }
            Some(false) => self.mode = Mode::Perks,
            None => {}
        }
    }

    /// Picks a numbered research node to unlock; stays open so several can
    /// be taken in one visit. Shared by `Mode::Research` and
    /// `Mode::RoutineResearch` — one handler taking the tree rather than a
    /// copied one, since the two screens differ only in which tree
    /// `research_nodes`/`research_graph` are asked for.
    pub(crate) fn handle_research_key(&mut self, key: GameKey, tree: ResearchTree) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        // Uppercase, as `handle_perks_key`'s `X` is: a destructive or
        // mode-flipping key stays off the lowercase space.
        if key == GameKey::Char('G') {
            self.research_graph_view = !self.research_graph_view;
            return;
        }
        // Uppercase, so a stray lowercase `a` cannot abandon a project.
        if key == GameKey::Char('A') {
            let Some(game) = &mut self.game else { return };
            let outcome = game.abandon_research();
            self.report(outcome);
            return;
        }
        // Dev-only and unbound otherwise. Uppercase for `G`'s and `A`'s
        // reason; it needs the id list, so it waits for it below.
        let dev_unlock = key == GameKey::Char('U') && self.dev_console_enabled();
        // Collecting the ids through `as_ref().map` (rather than a
        // `let Some(game) = &self.game` binding) ends the borrow here —
        // `selected_index` needs `&mut self`.
        let Some(ids) = self.game.as_ref().map(|g| {
            g.research_nodes(tree)
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        if dev_unlock {
            self.dev_unlock_research(tree, &ids);
            return;
        }
        if !self.research_graph_view {
            if let Some(idx) = self.selected_index(key, ids.len()) {
                let id = ids[idx].clone();
                let Some(game) = &mut self.game else { return };
                let outcome = game.select_research(&id);
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
            let landed = game.research_graph(tree).step(from, dir);
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
            let outcome = game.select_research(&id);
            self.report(outcome);
        }
    }

    /// The dev console's `U`: the graph's cursor node, or in the list view
    /// (which has no cursor) the active project.
    fn dev_unlock_research(&mut self, tree: ResearchTree, ids: &[String]) {
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let target = if self.research_graph_view {
            ids.get(self.menu_selected.min(ids.len().saturating_sub(1)))
                .cloned()
        } else {
            game.research_nodes(tree)
                .into_iter()
                .find(|n| n.state == ResearchState::Active)
                .map(|n| n.id)
        };
        let Some(id) = target else {
            self.refuse("Press G and put the cursor on a node.");
            return;
        };
        let Some(game) = self.game.as_mut() else {
            return;
        };
        match game.dev_unlock_research(&id) {
            Ok(line) => self.status_line = Some(line),
            Err(e) => self.refuse(e),
        }
    }
}
