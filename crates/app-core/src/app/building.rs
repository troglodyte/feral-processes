//! Placing, staffing, demolishing and upgrading base structures.

use crate::*;

/// What `Mode::StructureAssign` is showing: the structure picked on the
/// roster, and everyone who could be put on it.
///
/// The name travels with the rows because this popup covers the roster row it
/// was opened from — a picker headed "Assign Cronjob" with nothing else on it
/// would leave the player guessing which machine they were pointing at.
pub struct Staffing {
    pub target: String,
    pub rows: Vec<StaffRow>,
}

/// One candidate to work the structure highlighted on the roster — see
/// `App::staffing` and `Mode::StructureAssign`.
///
/// A row rather than a bare `Entity` because one of them is not an entity at
/// all: `None` is the player working the machine themselves. Keeping both in
/// one ordered list is what stops the renderer and the handler disagreeing
/// about which index means what, the same rule `App::base_menu_rows` holds
/// for a menu whose rows are hidden dynamically.
#[derive(Clone)]
pub struct StaffRow {
    /// The program to post, or `None` on the row that puts *you* on it.
    /// Carries the whole view so the picker can show a program's level,
    /// health and activity the way `Mode::Cronjob`'s does.
    pub program: Option<EntityView>,
}

impl App {
    /// Who can be put on `pending_post_structure`: yourself if you are
    /// standing beside it, then every program you own.
    ///
    /// The "yourself" row is filtered on `StructureReport::player_adjacent`
    /// rather than offered everywhere and refused, matching
    /// `App::upgradeable_structures` — `Game::work_structure` takes only the
    /// four orthogonal neighbours, and the roster is zone-wide, so on almost
    /// every row that offer would be a dead end. It leads because it is the
    /// answer that needs nothing: a player who owns no programs yet can still
    /// start a machine from here.
    pub fn staffing(&mut self) -> Option<Staffing> {
        let structure = self.pending_post_structure?;
        let row = self
            .game
            .as_mut()
            .map(|g| g.structure_report())
            .unwrap_or_default()
            .into_iter()
            .find(|s| s.entity == structure)?;
        let mut rows: Vec<StaffRow> = row
            .player_adjacent
            .then_some(StaffRow { program: None })
            .into_iter()
            .collect();
        rows.extend(
            self.nearby_programs()
                .into_iter()
                .map(|v| StaffRow { program: Some(v) }),
        );
        Some(Staffing {
            target: row.label,
            rows,
        })
    }

    /// Posts whoever was picked to the structure the roster was showing, and
    /// goes back to that row — see `Mode::StructureAssign`.
    pub(crate) fn handle_structure_assign_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_staffing();
            return;
        }
        let Some(structure) = self.pending_post_structure else {
            self.mode = Mode::Structures;
            return;
        };
        let rows = self.staffing().map(|s| s.rows).unwrap_or_default();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let worker = rows[idx].program.as_ref().map(|v| v.entity);
        let Some(game) = &mut self.game else { return };
        let outcome = match worker {
            Some(worker) => game.assign_cronjob(worker, structure),
            None => game.work_structure(structure),
        };
        self.status_line = outcome.err();
        self.leave_staffing();
    }

    /// Back to the roster, on the structure that was being staffed. Looked up
    /// by entity rather than by a remembered index because the roster is
    /// rebuilt from `Game::structure_report` on the way back in.
    fn leave_staffing(&mut self) {
        let structure = self.pending_post_structure.take();
        self.mode = Mode::Structures;
        let Some(structure) = structure else { return };
        if let Some(row) = self
            .game
            .as_mut()
            .map(|g| g.structure_report())
            .unwrap_or_default()
            .iter()
            .position(|s| s.entity == structure)
        {
            self.menu_selected = row;
        }
    }

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
