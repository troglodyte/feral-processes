//! Placing, staffing, demolishing and upgrading base structures.

use crate::*;

impl App {
    /// Every tamed program the player owns — the candidates for a cronjob or
    /// a guard posting.
    ///
    /// The whole roster rather than what is within `MENU_SCAN_RADIUS`, for
    /// the reason `handle_fuse_key` draws from `owned_pets`: a companion's
    /// `Position` is the tile it was captured on and is never written again,
    /// so a distance filter hides programs by where they were beaten. It hid
    /// them from the *row* too — `base_menu_rows` drops a row whose screen
    /// would be empty, so a player whose only program was tamed 40 tiles ago
    /// lost the Cronjob row entirely and never learned posting exists.
    /// Neither `assign_cronjob` (which now starts the program from the
    /// player's own tile) nor `assign_guard` asks anything about where the
    /// program is standing.
    ///
    /// This and the three lists below exist because each was written twice:
    /// once in the handler that picks from it, once in the renderer that
    /// draws it. The base menu's row-availability check (see
    /// `App::base_menu_rows`) would have been a third copy, and a menu that
    /// offers a row leading to an empty screen is exactly the drift that
    /// invites.
    pub fn nearby_programs(&mut self) -> Vec<EntityView> {
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        game.owned_program_views()
    }

    /// Nearby structures that accept a posted program. The same list whether
    /// the work is done by a program (`Mode::CronjobStructure`) or by the
    /// player themselves (`Mode::WorkStructure`) — it is the same job either
    /// way, see `Game::work_structure`.
    pub fn workable_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.can_work)
    }

    /// Every nearby structure, whatever it is: a guard posts to any of them
    /// and demolition takes any of them.
    pub fn nearby_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.is_structure)
    }

    /// Nearby structures that declare an upgrade path. Filtered on `tier`
    /// rather than just `is_structure`: offering an un-upgradeable structure
    /// and then refusing it would be a worse menu than not listing it.
    pub fn upgradeable_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.is_structure && e.tier.is_some())
    }

    fn scanned(&mut self, keep: impl Fn(&EntityView) -> bool) -> Vec<EntityView> {
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| keep(e))
            .collect()
    }

    pub(crate) fn handle_build_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
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
            self.close_screen();
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
            self.close_screen();
            return;
        }
        let workers = self.nearby_programs();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::CronjobStructure;
        }
    }

    pub(crate) fn handle_cronjob_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.close_screen();
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let structures = self.workable_structures();
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
    /// `App::workable_structures`, the same list `Mode::CronjobStructure`
    /// offers, since it is the same job either way (see
    /// `Game::work_structure`).
    pub(crate) fn handle_work_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let structures = self.workable_structures();
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
            self.close_screen();
            return;
        }
        let workers = self.nearby_programs();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::GuardStructure;
        }
    }

    pub(crate) fn handle_guard_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.close_screen();
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let structures = self.nearby_structures();
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
            self.close_screen();
            return;
        }
        let structures = self.nearby_structures();
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

    /// `d` + a direction: demolish whatever stands on that neighbouring
    /// tile.
    ///
    /// The adjacency is `Game::adjacent_structure`'s, not a cone like `x`'s —
    /// this key destroys what it finds, so it has to be something you are
    /// standing next to. Home routes into the same warning the menu's picker
    /// uses, read off the same `is_home` field, so the two ways in cannot
    /// disagree about what cascades.
    pub(crate) fn handle_remove_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => (0, -1),
            GameKey::Down | GameKey::Char('j') => (0, 1),
            GameKey::Left | GameKey::Char('h') => (-1, 0),
            GameKey::Right | GameKey::Char('l') => (1, 0),
            _ => return,
        };
        let Some(game) = &mut self.game else { return };
        let Some(found) = game.adjacent_structure(dir.0, dir.1) else {
            self.status_line = Some("Nothing to demolish that way.".to_string());
            self.mode = Mode::Playing;
            return;
        };
        if found.is_home {
            self.pending_remove_structure = Some(found.entity);
            self.mode = Mode::RemoveConfirm;
            return;
        }
        match game.remove_structure(found.entity) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }

    pub(crate) fn handle_upgrade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let structures = self.upgradeable_structures();
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
            self.close_screen();
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
            self.close_screen();
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
