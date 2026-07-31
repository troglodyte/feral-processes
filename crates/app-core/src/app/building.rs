//! Placing, staffing, demolishing and upgrading base structures.

use crate::*;

impl App {
    pub(crate) fn handle_build_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let defs = game.buildable_structure_defs();
        if let Some(idx) = self.selected_index(key, defs.len()) {
            self.pending_structure = Some(defs[idx].id.clone());
            self.mode = Mode::BuildDirection;
        }
    }

    pub(crate) fn handle_build_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_structure = None;
            self.mode = Mode::Playing;
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => Some((0, -1)),
            GameKey::Down | GameKey::Char('j') => Some((0, 1)),
            GameKey::Left | GameKey::Char('h') => Some((-1, 0)),
            GameKey::Right | GameKey::Char('l') => Some((1, 0)),
            _ => None,
        };
        let Some((dx, dy)) = dir else { return };
        let Some(id) = self.pending_structure.take() else {
            self.mode = Mode::Playing;
            return;
        };
        if let Some(game) = &mut self.game {
            match game.place_structure(&id, dx, dy) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }

    pub(crate) fn handle_cronjob_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::CronjobStructure;
        }
    }

    pub(crate) fn handle_cronjob_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_work)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_cronjob(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    /// Picks a nearby workable structure for the player to work themselves —
    /// the same `can_work` list `Mode::CronjobStructure` offers, since it is
    /// the same job either way (see `Game::work_structure`).
    pub(crate) fn handle_work_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_work)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.work_structure(structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_guard_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::GuardStructure;
        }
    }

    pub(crate) fn handle_guard_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_guard(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_remove_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked_entity = structures[idx].entity;
            let picked_is_home = structures[idx].is_home;
            if picked_is_home {
                self.pending_remove_structure = Some(picked_entity);
                self.mode = Mode::RemoveConfirm;
                return;
            }
            let Some(game) = &mut self.game else { return };
            match game.remove_structure(picked_entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_upgrade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        // Filtered on `tier`, not just `is_structure`: offering an
        // un-upgradeable structure and then refusing it would be a worse
        // menu than not listing it.
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure && e.tier.is_some())
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked = structures[idx].entity;
            let Some(game) = &mut self.game else { return };
            match game.upgrade_structure(picked) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_remove_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_remove_structure = None;
            self.mode = Mode::Playing;
            return;
        }
        let options = ['y', 'n'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('y') => {
                if let Some(structure) = self.pending_remove_structure.take() {
                    let Some(game) = &mut self.game else { return };
                    match game.remove_structure(structure) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                }
                self.mode = Mode::Playing;
            }
            Some('n') => {
                self.pending_remove_structure = None;
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }

    /// Lists every deployed symlink-capable structure (e.g. Home) anywhere
    /// on the map — not scan-radius-limited like the build/cronjob
    /// menus — and teleports the player to the picked one.
    pub(crate) fn handle_symlink_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let targets = game.symlink_targets();
        if let Some(idx) = self.selected_index(key, targets.len()) {
            let Some(game) = &mut self.game else { return };
            match game.use_symlink(targets[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }
}
