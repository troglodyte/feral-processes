//! The perk and research pickers.

use crate::*;
use feral_processes_engine::StatOwner;
use feral_processes_engine::{GraphDir, ResearchTree};

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
        // Uppercase for the same reason as `X`: this picker's rows run past
        // `s` in the label alphabet.
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
            self.open_perks();
        }
    }

    /// The one door into `Mode::Perks`: the previews are read here, so every
    /// route in shows figures for the perks as they stand now.
    pub(crate) fn open_perks(&mut self) {
        self.refresh_perk_previews();
        self.mode = Mode::Perks;
    }

    fn refresh_perk_previews(&mut self) {
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
                self.open_perks();
            }
            Some(false) => self.open_perks(),
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
        // Checked before `selected_index`, for the reason `handle_perks_key`
        // checks its own `X` first: uppercase is not a row selector today,
        // but putting the check after would be a live hazard the day
        // somebody widens `selected_index`.
        if key == GameKey::Char('G') {
            self.research_graph_view = !self.research_graph_view;
            return;
        }
        // Uppercase, and checked here beside `G` for its reason: lowercase
        // letters are row selectors past the digits, so a lowercase `a` would
        // both pick a row and abandon the project on one keypress.
        if key == GameKey::Char('A') {
            let Some(game) = &mut self.game else { return };
            let outcome = game.abandon_research();
            self.report(outcome);
            return;
        }
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
}
