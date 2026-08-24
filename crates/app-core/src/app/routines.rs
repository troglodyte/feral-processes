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

    /// Spends one etched disk out of cargo on the slot chosen in
    /// `Mode::Routines`.
    ///
    /// `e` opens the etch screen instead, because this is exactly where a
    /// player finds out they hold no disk of the routine they came for —
    /// making them back out two screens to go and make one would be a menu
    /// asking them to remember what it already knows.
    pub(crate) fn handle_routine_install_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Routines;
            return;
        }
        if key == GameKey::Char('e') {
            self.menu_selected = 0;
            self.etch_return = Some(Mode::RoutineInstall);
            self.mode = Mode::RoutineEtch;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        let Some(disks) = self.game.as_ref().map(|g| g.etched_disks_held()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, disks.len()) {
            let ability = disks[idx].ability.clone();
            let Some(game) = &mut self.game else { return };
            let outcome = game.install_disk(entity, &ability);
            self.report(outcome);
            self.mode = Mode::Routines;
        }
    }

    /// Burns a blank Routine Disk with a routine the player knows.
    ///
    /// Stays open after each etch rather than dropping back, the way
    /// `Mode::Research` does: a player who came here to make one disk
    /// usually came to make three, and the blanks are the only thing
    /// stopping them.
    ///
    /// Esc follows `App::etch_return` — back to the install screen for the
    /// `[e]` detour, out to the party menu for the row that opens this
    /// screen in its own right.
    pub(crate) fn handle_routine_etch_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.menu_selected = 0;
            match self.etch_return.take() {
                Some(mode) => self.mode = mode,
                None => self.close_screen(),
            }
            return;
        }
        let Some(known) = self.game.as_ref().map(|g| g.etchable_routines()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, known.len()) {
            let ability = known[idx].ability.clone();
            let Some(game) = &mut self.game else { return };
            let outcome = game.etch_disk(&ability);
            self.report(outcome);
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
        let outcome = game.extract_routine(program, index);
        self.report(outcome);
    }
}
