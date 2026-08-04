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
            match game.uninstall_routine(entity, idx) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            return;
        }
        self.mode = Mode::RoutineInstall;
    }

    pub(crate) fn handle_routine_install_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Routines;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        let Some(known) = self.game.as_ref().map(|g| g.installable_routines()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, known.len()) {
            let ability = known[idx].ability.clone();
            let Some(game) = &mut self.game else { return };
            match game.install_routine(entity, &ability) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
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

    /// Enter destroys the program; anything else backs out. Deliberately not
    /// a numbered menu — this is the last stop before an irreversible act.
    pub(crate) fn handle_extract_confirm_key(&mut self, key: GameKey) {
        let (Some(program), Some(index)) =
            (self.pending_extract_program, self.pending_extract_index)
        else {
            self.mode = Mode::Extract;
            return;
        };
        if key != GameKey::Enter {
            self.pending_extract_index = None;
            self.mode = Mode::ExtractPick;
            return;
        }
        self.pending_extract_program = None;
        self.pending_extract_index = None;
        self.mode = Mode::Playing;
        let Some(game) = &mut self.game else { return };
        match game.extract_routine(program, index) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
    }
}
