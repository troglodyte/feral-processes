//! `Mode::OutpostVisit` and `Mode::OutpostPost` — an outpost's own page and
//! its staff picker. `App::pending_outpost` is set once, from the bump cue
//! `after_world_action` drains (`playing.rs`'s `Visit::Outpost` arm), and
//! every figure on the page comes from `Game::outpost_report`.

use crate::*;
use feral_processes_engine::tuning::OUTPOST_NAME_MAX;

/// One base-staff candidate on `Mode::OutpostPost`'s list — `SortieSquadRow`
/// one field narrower: posting is a single action rather than a squad built
/// up over several keypresses, so there is no `picked` flag to carry.
pub struct OutpostPostRow {
    pub entity: Entity,
    pub name: String,
}

impl App {
    /// `Mode::OutpostVisit`'s one derivation, read off `pending_outpost` —
    /// `None` once the player has walked away and the record can no longer
    /// be found (an outpost cannot move, so this only ever happens if the
    /// tile's own record vanished, which nothing yet does to it).
    pub fn outpost_report(&mut self) -> Option<OutpostReport> {
        let tile = self.pending_outpost?;
        self.game.as_mut()?.outpost_report(tile)
    }

    /// Every base-staff program `Mode::OutpostPost` offers — `Game::
    /// base_staff` already excludes outpost crew, so this list never
    /// includes a program already posted somewhere.
    pub fn outpost_post_candidates(&self) -> Vec<OutpostPostRow> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        game.base_staff()
            .into_iter()
            .map(|entity| OutpostPostRow {
                entity,
                name: game.creature_label(entity),
            })
            .collect()
    }

    pub(crate) fn handle_outpost_visit_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_outpost = None;
            self.status_line = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(tile) = self.pending_outpost else {
            self.mode = Mode::Playing;
            return;
        };
        let crew_len = self.outpost_report().map(|r| r.crew.len()).unwrap_or(0);
        match key {
            // Opens the transfer picker, take side only — the outpost's
            // own door onto `Mode::Transfer` (design correction 11).
            GameKey::Char('C') => {
                let Some(game) = &mut self.game else { return };
                let rows = game.outpost_transfer_offer(tile);
                if rows.is_empty() {
                    self.refuse("There's nothing standing at this outpost to take.");
                    return;
                }
                self.open_transfer(rows, Vec::new(), None, 0, TransferSource::Outpost(tile));
            }
            GameKey::Up | GameKey::Down | GameKey::PageUp | GameKey::PageDown => {
                self.scroll(key, crew_len)
            }
            // `[P]` opens the staff picker. Not gated here on there being
            // any staff to post — an empty picker still tells the player
            // why, and `Game::post_to_outpost` refuses in its own words if
            // they somehow reach it with nobody home.
            GameKey::Char('P') => {
                self.menu_selected = 0;
                self.mode = Mode::OutpostPost;
            }
            // `[U]` recalls the highlighted crew row.
            GameKey::Char('U') => {
                let Some(game) = &mut self.game else { return };
                let crew = game.outpost_crew(tile);
                let Some(&entity) = crew.get(self.menu_selected) else {
                    self.refuse("Select a crew row to recall.");
                    return;
                };
                let outcome = game.recall_from_outpost(entity);
                self.report(outcome);
            }
            GameKey::Char('N') => self.open_outpost_name(tile, false),
            // `[R]` repairs — design spec §8.
            GameKey::Char('R') => {
                let Some(game) = &mut self.game else { return };
                let outcome = game.repair_outpost(tile);
                self.report(outcome);
            }
            _ => {}
        }
    }

    /// Opens the name field holding the outpost's current name. From the
    /// Sites hub the tab is remembered so Enter and Esc come back to it.
    pub(crate) fn open_outpost_name(&mut self, tile: (i32, i32), from_hub: bool) {
        let Some(game) = &self.game else { return };
        let name = game.outpost_destination_name(tile);
        self.outpost_name.set(name);
        self.pending_outpost = Some(tile);
        if from_hub {
            self.launch_from_site(Mode::OutpostName);
        } else {
            self.status_line = None;
            self.mode = Mode::OutpostName;
        }
    }

    pub(crate) fn handle_outpost_name_key(&mut self, key: GameKey) {
        let Some(tile) = self.pending_outpost else {
            self.mode = Mode::Playing;
            return;
        };
        match key {
            GameKey::Esc => {
                self.status_line = None;
                self.leave_site_screen(Mode::OutpostVisit);
            }
            GameKey::Enter => {
                let Some(game) = &mut self.game else { return };
                match game.rename_outpost(tile, self.outpost_name.as_str()) {
                    Ok(()) => {
                        self.status_line = None;
                        self.leave_site_screen(Mode::OutpostVisit);
                    }
                    Err(e) => self.refuse(e),
                }
            }
            key => {
                self.outpost_name.edit(key, OUTPOST_NAME_MAX);
            }
        }
    }

    pub(crate) fn handle_outpost_post_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.status_line = None;
            self.leave_site_screen(Mode::OutpostVisit);
            return;
        }
        let Some(tile) = self.pending_outpost else {
            self.mode = Mode::Playing;
            return;
        };
        let candidates = self.outpost_post_candidates();
        match key {
            GameKey::Enter => {
                let Some(row) = self
                    .selected_index(key, candidates.len())
                    .and_then(|i| candidates.get(i))
                else {
                    return;
                };
                let entity = row.entity;
                let Some(game) = &mut self.game else { return };
                match game.post_to_outpost(tile, entity) {
                    Ok(()) => {
                        self.status_line = None;
                        self.leave_site_screen(Mode::OutpostVisit);
                    }
                    Err(e) => self.refuse(e),
                }
            }
            GameKey::Up | GameKey::Down | GameKey::PageUp | GameKey::PageDown => {
                self.scroll(key, candidates.len())
            }
            _ => {}
        }
    }
}
