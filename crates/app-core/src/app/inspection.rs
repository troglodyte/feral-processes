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
            // A build site takes the caravan's shape and for the caravan's
            // reason: there is no structure standing there yet, so a
            // structure sheet would draw a machine with no stock, no status
            // and no tier. What the player wants is what is still to be
            // carried to it, which is one line.
            Some(InspectTarget::BuildSite(entity)) => {
                self.pending_description = game.build_site_blurb(entity);
                self.status_line = None;
                self.mode = Mode::CellDescribe;
            }
            // A settlement gets the shape `Caravan` and `BuildSite` above
            // don't: it has a page of its own, `Mode::Settlement`, the same
            // one a bump opens — `Game::settlement_key` is the one bridge
            // from the `Entity` `InspectTarget` carries to the key both
            // doors render through, so the bump and `x` cannot drift into
            // two derivations of the same town.
            Some(InspectTarget::Settlement(entity)) => {
                self.pending_settlement = game.settlement_key(entity);
                self.status_line = None;
                self.mode = Mode::Settlement;
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

    /// The settlement page: `Mode::CompanionMemories`'s shape, Esc only,
    /// rather than `Mode::CellDescribe`'s "any key leaves" just above. A
    /// settlement page is reached by walking into the tile as often as by
    /// examining it, and a bump opens it on the same keypress that moved the
    /// player — a direction key still held down must not double as the
    /// dismissal the way it would if any key closed the screen.
    pub(crate) fn handle_settlement_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_settlement = None;
            self.status_line = None;
            self.mode = Mode::Playing;
        }
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
        if key == GameKey::Char('w') {
            self.start_watching();
            return;
        }
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

    /// `w` on a manifest: put the map's camera on this program and go watch
    /// it.
    ///
    /// **Drops the whole sheet rather than backing into the list it was
    /// opened from**, which is where the roster route reaches this. Watching
    /// happens on the map, so `leave_manifest`'s return-to-your-list rule is
    /// the wrong one here — there would be nothing to see behind the list.
    ///
    /// `Game::watch_position` is the one gate. The footer offers `w` exactly
    /// when it answers `Some`, so this refusal is only ever reached by a
    /// player pressing a key the screen did not advertise — but it is a
    /// sentence rather than a swallowed press, because a key that does
    /// nothing at all reads as the feature being broken.
    fn start_watching(&mut self) {
        let watchable = self
            .pending_manifest
            .zip(self.game.as_ref())
            .is_some_and(|(e, g)| g.watch_position(e).is_some());
        if !watchable {
            self.refuse(
                "You can only watch a program the base is working — one in \
                 the party, in your hand, away on a sortie or standing guard \
                 isn't somewhere you can look.",
            );
            return;
        }
        self.watching = self.pending_manifest;
        self.pending_manifest = None;
        self.status_line = None;
        self.mode = Mode::Playing;
    }

    /// Where the map's camera is centred, or `None` when it is on the party.
    ///
    /// **The read is also the release.** Asking `Game::watch_position` every
    /// frame and dropping `App::watching` the moment it answers `None` is
    /// what makes "the program was dissolved / dispatched / taken into the
    /// party / the party left base space" one rule instead of a list of
    /// endings that grows a case short. The engine's door already knows all
    /// of them; nothing here restates any.
    pub fn watch_center(&mut self) -> Option<(i32, i32)> {
        let entity = self.watching?;
        let at = self.game.as_ref()?.watch_position(entity);
        if at.is_none() {
            self.watching = None;
        }
        at
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
