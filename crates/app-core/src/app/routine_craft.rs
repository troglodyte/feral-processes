//! The Workbench's three routine-crafting pages (ADR 0029): Build routine,
//! Subroutines and Built routines.
//!
//! The build page keeps a draft (`RoutineDraft`) on `App`; every figure it
//! shows comes from `Game::build_rows` / `build_part_options` /
//! `build_preview`, and Compile calls `Game::build_routine`, so the screen
//! cannot promise what the engine would refuse.

use crate::app::list_filter::filter_matches;
use crate::*;
use feral_processes_engine::routine_craft::Subroutine;

/// Longest routine name the name field takes.
pub use feral_processes_engine::tuning::CRAFT_NAME_MAX as ROUTINE_NAME_MAX;

/// The build page's cursor rows after the six part slots.
pub const NAME_ROW: usize = 6;
pub const COMPILE_ROW: usize = 7;
/// How many rows the build page has.
pub const BUILD_PAGE_ROWS: usize = 8;

/// The routine being designed on the build page. Session state: closing the
/// Workbench discards it, and nothing is spent until Compile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoutineDraft {
    pub parts: Vec<Subroutine>,
    pub name: String,
    /// The slot whose list `Mode::RoutineBuildPick` is showing.
    pub picking: Option<&'static str>,
    /// The build row to put the highlight back on after a pick or a name.
    pub row: usize,
}

/// Indices into `Game::subroutine_rows` that `filter` leaves, by label.
pub fn subroutines_visible(game: &Game, filter: &str) -> Vec<usize> {
    let labels: Vec<String> = game
        .subroutine_rows()
        .into_iter()
        .map(|r| r.label)
        .collect();
    filter_matches(labels.iter().map(String::as_str), filter)
}

impl App {
    pub(crate) fn drop_routine_craft_state(&mut self) {
        self.routine_draft = RoutineDraft::default();
    }

    pub(crate) fn handle_routine_build_key(&mut self, key: GameKey) {
        let Some(game) = &self.game else { return };
        let rows = game.build_rows(&self.routine_draft.parts);
        let Some(idx) = self.selected_index(key, BUILD_PAGE_ROWS) else {
            return;
        };
        self.routine_draft.row = idx;
        match idx {
            NAME_ROW => self.mode = Mode::RoutineBuildName,
            COMPILE_ROW => self.compile_draft(),
            _ => {
                let row = &rows[idx];
                if let Some(why) = row.unavailable {
                    self.refuse(why);
                    return;
                }
                self.routine_draft.picking = Some(row.slot);
                self.menu_selected = 0;
                self.mode = Mode::RoutineBuildPick;
            }
        }
    }

    fn compile_draft(&mut self) {
        let draft = std::mem::take(&mut self.routine_draft);
        let Some(game) = &mut self.game else { return };
        match game.build_routine(&draft.name, &draft.parts) {
            Ok(_) => {
                self.routine_draft = RoutineDraft {
                    row: COMPILE_ROW,
                    ..RoutineDraft::default()
                };
                self.status_line = None;
            }
            Err(e) => {
                self.routine_draft = draft;
                self.refuse(e);
            }
        }
    }

    pub(crate) fn handle_routine_build_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_pick();
            return;
        }
        let Some(slot) = self.routine_draft.picking else {
            self.leave_pick();
            return;
        };
        let Some(game) = &self.game else { return };
        let options = game.build_part_options(slot, &self.routine_draft.parts);
        let cap = game.part_cap();
        let Some(idx) = self.selected_index(key, options.len() + 1) else {
            return;
        };
        let parts = &mut self.routine_draft.parts;
        if idx == options.len() {
            parts.retain(|p| p.slot() != slot);
        } else {
            let part = options[idx].part;
            // A Trickle adds a round, so it stacks; every other slot holds
            // one part and a pick replaces it.
            if slot == "Timing" {
                if parts.len() >= cap {
                    self.refuse(format!("A routine holds at most {cap} parts."));
                    return;
                }
            } else {
                parts.retain(|p| p.slot() != slot);
            }
            if slot == "Core" {
                parts.insert(0, part);
            } else {
                parts.push(part);
            }
        }
        self.prune_draft();
        self.leave_pick();
    }

    /// Drops every part the Core no longer takes, so changing the Core never
    /// leaves a build the engine would refuse.
    fn prune_draft(&mut self) {
        let Some(game) = &self.game else { return };
        let parts = &self.routine_draft.parts;
        let core: Vec<Subroutine> = parts
            .iter()
            .copied()
            .filter(|p| p.slot() == "Core")
            .collect();
        let kept: Vec<Subroutine> = parts
            .iter()
            .copied()
            .filter(|part| {
                part.slot() == "Core"
                    || game
                        .build_part_options(part.slot(), &core)
                        .iter()
                        .any(|o| o.part == *part)
            })
            .collect();
        self.routine_draft.parts = kept;
    }

    fn leave_pick(&mut self) {
        self.routine_draft.picking = None;
        self.menu_selected = self.routine_draft.row;
        self.mode = Mode::RoutineBuild;
    }

    pub(crate) fn handle_routine_build_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc | GameKey::Enter => {
                self.menu_selected = self.routine_draft.row;
                self.mode = Mode::RoutineBuild;
            }
            GameKey::Backspace => {
                self.routine_draft.name.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.routine_draft.name.chars().count() < ROUTINE_NAME_MAX =>
            {
                self.routine_draft.name.push(c);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_subroutines_key(&mut self, key: GameKey) {
        if self.take_filter_key(key) {
            return;
        }
        let Some(game) = &self.game else { return };
        let visible = subroutines_visible(game, self.filter_text());
        let Some(pos) = self.selected_index(key, visible.len()) else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let part = game.subroutine_rows()[visible[pos]].part;
        let outcome = game.craft_subroutine(part);
        self.report(outcome);
    }

    pub(crate) fn handle_built_routines_key(&mut self, key: GameKey) {
        let Some(game) = &self.game else { return };
        let len = game.built_routine_rows().len();
        if self.selected_index(key, len).is_some() {
            self.refuse("Install it from the routines screen (m).");
        }
    }
}
