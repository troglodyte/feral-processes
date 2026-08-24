//! Aiming the inspector at a tile, and the manifest screen it opens.

use crate::*;
use feral_processes_engine::tuning::EXAMINE_RANGE_TILES;
use feral_processes_engine::{ExamineDir, InspectTarget};

impl App {
    /// Picks a direction (arrows/hjkl) and inspects the first creature the
    /// engine finds stepping that way from the player, rather than picking
    /// from a numbered list of grid coordinates.
    pub(crate) fn handle_inspect_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        // Underground the four keys are read in view space and describe a
        // cell, because `Position` is pinned to the surface entrance tile
        // down there and a scan of it would report the base as lying that
        // way. `Game::find_target_in_direction` refuses underground for the
        // same reason.
        if game.is_underground() {
            let dir = match key {
                GameKey::Up | GameKey::Char('k') => ExamineDir::Ahead,
                GameKey::Down | GameKey::Char('j') => ExamineDir::Underfoot,
                GameKey::Left | GameKey::Char('h') => ExamineDir::Left,
                GameKey::Right | GameKey::Char('l') => ExamineDir::Right,
                _ => return,
            };
            self.pending_description = game.describe_view_direction(dir);
            self.status_line = None;
            self.mode = Mode::CellDescribe;
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
        match game.find_target_in_direction(dx, dy, EXAMINE_RANGE_TILES) {
            Some(InspectTarget::Creature(entity)) => {
                self.pending_manifest = Some(entity);
                self.manifest_origin = ManifestOrigin::Map;
                self.status_line = None;
                self.mode = Mode::Manifest;
            }
            Some(InspectTarget::Structure(entity)) => {
                self.pending_structure_manifest = Some(entity);
                self.status_line = None;
                self.mode = Mode::StructureManifest;
            }
            // A trader gets its own line rather than a manifest: it has no
            // `Stats` for a sheet to draw and nothing may target one as a
            // combat participant. `Mode::CellDescribe` is already the screen
            // for "here is what that is", which is why this needs no mode of
            // its own.
            Some(InspectTarget::Caravan(entity)) => {
                self.pending_description = game.caravan_blurb(entity);
                self.status_line = None;
                self.mode = Mode::CellDescribe;
            }
            // Nothing standing there — but in base space the ray may still
            // have run into a wall, and a wall is now something with a name.
            // Asked only after the creature and structure arms, because a
            // program standing in front of a seam is the more interesting
            // answer.
            // Bound before the match rather than matched on directly:
            // `App::refuse` wants the whole of `self`, so the `game` borrow
            // has to have ended by the time the empty arm reports.
            None => {
                let described = game.describe_base_rock(dx, dy, EXAMINE_RANGE_TILES);
                match described {
                    Some(text) => {
                        self.pending_description = Some(text);
                        self.status_line = None;
                        self.mode = Mode::CellDescribe;
                    }
                    None => {
                        self.refuse("Nothing in that direction.");
                        self.mode = Mode::Playing;
                    }
                }
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

    /// The cell description is read-only and reached only from the corridor
    /// view, so there is nothing to page through and no origin to return to.
    /// Any key leaves, the way a plain popup does.
    pub(crate) fn handle_cell_describe_key(&mut self, _key: GameKey) {
        self.pending_description = None;
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
            self.manifest_origin = ManifestOrigin::Picker;
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

    /// Esc from the manifest: back to the list it was opened from, or to the
    /// map if it was opened from there.
    ///
    /// Either list re-highlights whoever the sheet was *showing* rather than
    /// the row originally picked — after paging with ←/→ those differ, and
    /// the list should agree with the sheet you just left.
    ///
    /// The two lists differ in what they can be asked for. The picker holds
    /// every subject the manifest can page to, so the lookup always lands;
    /// the roster holds programs only, so a sheet paged onto the *player* has
    /// no row there, and the highlight is left standing where it was rather
    /// than snapped to the top — which is what `keeps_highlight` parks it for
    /// across the side trip.
    fn leave_manifest(&mut self) {
        match self.manifest_origin {
            ManifestOrigin::Map => {
                self.pending_manifest = None;
                self.mode = Mode::Playing;
            }
            ManifestOrigin::Picker => {
                let subjects = self.manifest_subjects();
                self.menu_selected = self
                    .pending_manifest
                    .and_then(|e| subjects.iter().position(|&s| s == e))
                    .unwrap_or(0);
                self.mode = Mode::ManifestPick;
            }
            ManifestOrigin::Roster => {
                if let Some(row) = self.roster_row_of(self.pending_manifest) {
                    self.menu_selected = row;
                }
                self.mode = Mode::Companion;
            }
        }
    }

    /// Where `entity` sits in the roster `Mode::Companion` lists, if it is in
    /// it at all. Read through `owned_pets` rather than `manifest_subjects`
    /// because those are two different lists — the roster has no player row,
    /// so their indices agree only by luck.
    fn roster_row_of(&mut self, entity: Option<Entity>) -> Option<usize> {
        let entity = entity?;
        self.game
            .as_mut()?
            .owned_pets()
            .iter()
            .position(|p| p.entity == entity)
    }
}
