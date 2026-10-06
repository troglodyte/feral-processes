//! The Splice Rig's screen: build an implant into the player, or cut one out.
//!
//! Opened with `[I]` beside a rig, `open_rig_tool`'s shape — an action on a
//! machine, so the key lives on the map. One list, the installed implants
//! first and then the implant items in the pack, so a single lowercase
//! selector picks either; the verbs are uppercase (`[I]` install, `[R]`
//! remove) because a lowercase letter picks a row everywhere else.
//!
//! Splicing past the Load cap is allowed — rejection is its price — so that
//! install asks first, as a state of this screen rather than a `Mode` of its
//! own: it is one question about one row, and a mode would owe a row in every
//! exhaustive list for it.

use crate::*;
use feral_processes_engine::ImplantView;
use feral_processes_engine::implants::ImplantId;
use feral_processes_engine::items::ItemId;

/// The screen's cached read of the player's implants, and the install it is
/// waiting on a yes for.
#[derive(Clone, Debug, PartialEq)]
pub struct SpliceRigScreen {
    pub view: ImplantView,
    /// The overloading install awaiting `[Y]`, if one has been asked for.
    pub confirm: Option<ItemId>,
}

impl SpliceRigScreen {
    /// How many rows the one list has: installed, then installable.
    pub fn rows(&self) -> usize {
        self.view.installed.len() + self.view.installable.len()
    }
}

impl App {
    /// Opens the screen beside a rig, or refuses.
    ///
    /// **Answers whether anything happened**, `open_rig_tool`'s rule: a
    /// refusal answers `false`, because the caller clears the status line of
    /// a key that acted and would wipe the sentence just written.
    pub(crate) fn open_splice_rig(&mut self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        if !game.adjacent_splice_rig() {
            self.refuse("There is no Splice Rig here.");
            return false;
        }
        self.splice_rig = Some(SpliceRigScreen {
            view: game.implant_view(),
            confirm: None,
        });
        self.menu_selected = 0;
        self.mode = Mode::SpliceRig;
        true
    }

    pub(crate) fn leave_splice_rig(&mut self) {
        self.splice_rig = None;
        self.mode = Mode::Playing;
    }

    fn refresh_splice_rig(&mut self) {
        let Some(game) = &self.game else { return };
        if let Some(screen) = &mut self.splice_rig {
            screen.view = game.implant_view();
            screen.confirm = None;
        }
        let rows = self.splice_rig.as_ref().map_or(0, SpliceRigScreen::rows);
        self.menu_selected = self.menu_selected.min(rows.saturating_sub(1));
    }

    pub(crate) fn handle_splice_rig_key(&mut self, key: GameKey) {
        let Some(screen) = &self.splice_rig else {
            self.mode = Mode::Playing;
            return;
        };
        // Walked away from, or the rig demolished, while the screen was open.
        if !self.game.as_ref().is_some_and(|g| g.adjacent_splice_rig()) {
            self.leave_splice_rig();
            return;
        }
        if let Some(item) = screen.confirm.clone() {
            match key {
                GameKey::Char('Y') => self.install_splice_item(&item),
                GameKey::Esc | GameKey::Char('N') => {
                    if let Some(screen) = &mut self.splice_rig {
                        screen.confirm = None;
                    }
                }
                _ => {}
            }
            return;
        }
        let rows = screen.rows();
        match key {
            GameKey::Esc => self.leave_splice_rig(),
            GameKey::Up | GameKey::Down => self.scroll(key, rows),
            GameKey::Enter => match self.selected_splice_row() {
                Some(SpliceRow::Installed(id)) => self.remove_splice_implant(&id),
                Some(SpliceRow::Installable(item)) => self.begin_splice_install(item),
                None => {}
            },
            GameKey::Char('I') => {
                if let Some(SpliceRow::Installable(item)) = self.selected_splice_row() {
                    self.begin_splice_install(item);
                } else {
                    self.refuse("Pick an implant from your pack to install.");
                }
            }
            GameKey::Char('R') => {
                if let Some(SpliceRow::Installed(id)) = self.selected_splice_row() {
                    self.remove_splice_implant(&id);
                } else {
                    self.refuse("Pick an installed implant to remove.");
                }
            }
            key => {
                if let Some(idx) = self.selected_index(key, rows) {
                    self.menu_selected = idx;
                }
            }
        }
    }

    fn selected_splice_row(&self) -> Option<SpliceRow> {
        let view = &self.splice_rig.as_ref()?.view;
        let at = self.menu_selected;
        match view.installed.get(at) {
            Some(row) => Some(SpliceRow::Installed(row.id.clone())),
            None => view
                .installable
                .get(at - view.installed.len())
                .map(|row| SpliceRow::Installable(row.item.clone())),
        }
    }

    /// Installs straight away, or asks first when the Load would pass the
    /// cap. The question is only for the player's own sake: the engine
    /// allows it either way.
    fn begin_splice_install(&mut self, item: ItemId) {
        let Some(screen) = &mut self.splice_rig else {
            return;
        };
        let load = screen
            .view
            .installable
            .iter()
            .find(|row| row.item == item)
            .map_or(0, |row| row.load);
        if screen.view.load + load > screen.view.cap {
            screen.confirm = Some(item);
        } else {
            self.install_splice_item(&item);
        }
    }

    fn install_splice_item(&mut self, item: &ItemId) {
        let Some(game) = &mut self.game else { return };
        let outcome = game.install_implant(item);
        self.report(outcome);
        self.refresh_splice_rig();
        self.after_tick();
    }

    fn remove_splice_implant(&mut self, id: &ImplantId) {
        let Some(game) = &mut self.game else { return };
        let outcome = game.remove_implant(id);
        self.report(outcome);
        self.refresh_splice_rig();
        self.after_tick();
    }
}

enum SpliceRow {
    Installed(ImplantId),
    Installable(ItemId),
}
