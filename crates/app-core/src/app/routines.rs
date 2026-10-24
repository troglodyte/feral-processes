//! The routine panel — installing and popping out abilities — and the
//! three-page extraction flow.

use crate::*;

impl App {
    pub(crate) fn handle_routine_target_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let holders = game.routine_holders();
        if let Some(idx) = self.selected_index(key, holders.len()) {
            self.pending_routine_holder = Some(holders[idx].entity);
            self.mode = Mode::Routines;
        }
    }

    pub(crate) fn handle_routines_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_routine_holder = None;
            self.mode = Mode::RoutineTarget;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        // Collecting through `as_ref().map` ends the borrow here —
        // `selected_index` needs `&mut self` (see `handle_research_key`).
        let Some(slots) = self.game.as_ref().map(|g| g.routine_view(entity)) else {
            return;
        };
        let Some(idx) = self.selected_index(key, slots.len()) else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        if slots[idx].ability.is_some() {
            let outcome = game.uninstall_routine(entity, idx);
            self.report(outcome);
            return;
        }
        self.mode = Mode::RoutineInstall;
    }

    /// Writes a built routine into the slot chosen in `Mode::Routines`.
    /// Only unslotted built routines are listed; they are made at the
    /// Workbench, so there is nothing to detour to from here.
    pub(crate) fn handle_routine_install_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Routines;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        let Some(built) = self.game.as_ref().map(|g| g.built_routine_rows()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, built.len()) {
            let id = built[idx].id.clone();
            let Some(game) = &mut self.game else { return };
            let outcome = game.install_built(entity, &id);
            self.report(outcome);
            self.mode = Mode::Routines;
        }
    }

    pub(crate) fn handle_extract_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let programs = game.owned_pets();
        if let Some(idx) = self.selected_index(key, programs.len()) {
            self.pending_extract_program = Some(programs[idx].entity);
            self.mode = Mode::ExtractPick;
        }
    }

    pub(crate) fn handle_extract_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_extract_program = None;
            self.mode = Mode::Extract;
            return;
        }
        let Some(program) = self.pending_extract_program else {
            self.mode = Mode::Extract;
            return;
        };
        let Some(game) = &self.game else { return };
        let offered = game.extractable_routines(program);
        if let Some(idx) = self.selected_index(key, offered.len()) {
            self.pending_extract_index = Some(idx);
            self.mode = Mode::ExtractConfirm;
        }
    }

    /// `y`, or Enter on Yes, destroys the program; `n`, Esc, or Enter on the
    /// default No back out, and any other key leaves the question standing.
    /// The last stop before an irreversible act, so it opens on No.
    pub(crate) fn handle_extract_confirm_key(&mut self, key: GameKey) {
        let (Some(program), Some(index)) =
            (self.pending_extract_program, self.pending_extract_index)
        else {
            self.mode = Mode::Extract;
            return;
        };
        let confirmed = match key {
            GameKey::Esc => Some(false),
            _ => self.yes_no(key),
        };
        match confirmed {
            None => return,
            Some(false) => {
                self.pending_extract_index = None;
                self.mode = Mode::ExtractPick;
                return;
            }
            Some(true) => {}
        }
        self.pending_extract_program = None;
        self.pending_extract_index = None;
        self.mode = Mode::Playing;
        let Some(game) = &mut self.game else { return };
        let outcome = game.extract_routine(program, index);
        self.report(outcome);
    }
}
