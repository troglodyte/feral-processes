//! Aiming the inspector at a tile, and the manifest screen it opens.

use crate::*;
use feral_processes_engine::InspectTarget;

impl App {
    /// Picks a direction (arrows/hjkl) and inspects the first creature the
    /// engine finds stepping that way from the player, rather than picking
    /// from a numbered list of grid coordinates.
    pub(crate) fn handle_inspect_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
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
        let Some(game) = &mut self.game else { return };
        match game.find_target_in_direction(dx, dy, MENU_SCAN_RADIUS) {
            Some(InspectTarget::Creature(entity)) => {
                self.pending_manifest = Some(entity);
                self.manifest_from_picker = false;
                self.status_line = None;
                self.mode = Mode::Manifest;
            }
            Some(InspectTarget::Structure(entity)) => {
                self.pending_structure_manifest = Some(entity);
                self.status_line = None;
                self.mode = Mode::StructureManifest;
            }
            None => {
                self.status_line = Some("Nothing in that direction.".to_string());
                self.mode = Mode::Playing;
            }
        }
    }

    /// The structure sheet is read-only and reached only from the map, so
    /// there is no list to page through and no origin to return to — unlike
    /// `Mode::Manifest`, whose ←/→ exist because it has `manifest_subjects`.
    /// Any key leaves, the way a plain popup does.
    pub(crate) fn handle_structure_manifest_key(&mut self, _key: GameKey) {
        self.pending_structure_manifest = None;
        self.close_screen();
    }

    /// You, then every program you own — everyone the manifest can page
    /// through with ←/→. A wild program reached via `x` is deliberately not
    /// in here: it is not yours to page to, and paging away from it would be
    /// a one-way trip.
    pub fn manifest_subjects(&mut self) -> Vec<Entity> {
        self.game
            .as_mut()
            .map(|game| game.manifest_subjects())
            .unwrap_or_default()
    }

    pub(crate) fn handle_manifest_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_manifest = None;
            self.close_screen();
            return;
        }
        let subjects = self.manifest_subjects();
        if let Some(idx) = self.selected_index(key, subjects.len()) {
            self.pending_manifest = Some(subjects[idx]);
            self.manifest_from_picker = true;
            self.status_line = None;
            self.mode = Mode::Manifest;
        }
    }

    /// Unlike the popup this replaced, the manifest doesn't close on any key
    /// — ←/→ page between subjects, so only Esc leaves.
    pub(crate) fn handle_manifest_key(&mut self, key: GameKey) {
        let step = match key {
            GameKey::Left => -1,
            GameKey::Right => 1,
            GameKey::Esc => {
                self.leave_manifest();
                return;
            }
            _ => return,
        };
        let subjects = self.manifest_subjects();
        let Some(current) = self
            .pending_manifest
            .and_then(|e| subjects.iter().position(|&s| s == e))
        else {
            // A wild program isn't in the list, so there is nothing to cycle
            // to — the footer doesn't offer the keys either.
            return;
        };
        let next = (current as isize + step).rem_euclid(subjects.len() as isize) as usize;
        self.pending_manifest = Some(subjects[next]);
    }

    /// Esc from the manifest: back to the picker it was opened from, or to
    /// the map if it was opened from there.
    ///
    /// Returning to the picker re-highlights whoever the sheet was showing
    /// rather than the row originally picked — after paging with ←/→ those
    /// differ, and the list should agree with the sheet you just left.
    fn leave_manifest(&mut self) {
        if !self.manifest_from_picker {
            self.pending_manifest = None;
            self.mode = Mode::Playing;
            return;
        }
        let subjects = self.manifest_subjects();
        self.menu_selected = self
            .pending_manifest
            .and_then(|e| subjects.iter().position(|&s| s == e))
            .unwrap_or(0);
        self.mode = Mode::ManifestPick;
    }
}
