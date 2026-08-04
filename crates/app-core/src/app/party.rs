//! Companion inspection and the three-page program fusion flow.

use crate::*;

impl App {
    /// Lists every tamed program you own, wherever it is — pressing a party
    /// member's number stands it down, pressing any other program's number
    /// adds it to the party (up to `MAX_PARTY_SIZE` at once). `<` and `>`
    /// shift the highlighted member along the battle line instead of
    /// changing who's on it.
    pub(crate) fn handle_companion_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let shift = match key {
            GameKey::Char('<') | GameKey::Char(',') => Some(SlotShift::Forward),
            GameKey::Char('>') | GameKey::Char('.') => Some(SlotShift::Back),
            _ => None,
        };
        if let Some(shift) = shift {
            self.shift_party_slot(shift);
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates = game.owned_pets();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            let candidate = &candidates[idx];
            let Some(game) = &mut self.game else { return };
            if candidate.party_slot.is_some() {
                game.remove_companion(candidate.entity);
                self.status_line = None;
            } else {
                match game.add_companion(candidate.entity) {
                    Ok(()) => self.status_line = None,
                    Err(e) => self.status_line = Some(e),
                }
            }
        }
    }

    /// Shifts the highlighted program one slot along the battle line, and
    /// moves the highlight with it — so repeated presses walk one member to
    /// the front rather than swapping the same pair back and forth. Safe to
    /// compute the new row arithmetically: `owned_pets` leads with the party
    /// in slot order, and the swap stays inside that prefix.
    fn shift_party_slot(&mut self, shift: SlotShift) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(entity) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        match game.move_party_member(entity, shift) {
            Ok(()) => {
                self.status_line = None;
                self.menu_selected = match shift {
                    SlotShift::Forward => row.saturating_sub(1),
                    SlotShift::Back => row + 1,
                };
            }
            Err(e) => self.status_line = Some(e),
        }
    }

    /// Picks the first of two tamed programs to fuse together. Draws from
    /// the whole roster rather than what's within `MENU_SCAN_RADIUS`:
    /// `Game::fuse_companions` has no distance requirement, so scanning by
    /// distance only hid programs left working across the map.
    pub(crate) fn handle_fuse_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates = game.owned_pets();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_first = Some(candidates[idx].entity);
            self.mode = Mode::FuseSecond;
        }
    }

    /// Picks the second program to fuse with the one from `handle_fuse_key`,
    /// then actually runs the fusion.
    pub(crate) fn handle_fuse_second_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_fuse_first = None;
            self.close_screen();
            return;
        }
        let Some(first) = self.pending_fuse_first else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let candidates: Vec<_> = game
            .owned_pets()
            .into_iter()
            .filter(|p| p.entity != first)
            .collect();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_second = Some(candidates[idx].entity);
            self.fuse_name_input.clear();
            self.mode = Mode::FuseName;
        }
    }

    /// Types a name (up to `feral_processes_engine::MAX_CUSTOM_NAME_LEN`
    /// characters) for the program that'll result from fusing
    /// `pending_fuse_first`/`pending_fuse_second`; Enter runs the fusion
    /// (blank keeps the default species name). Esc backs up one step to
    /// re-pick the second program, rather than aborting the whole fusion —
    /// the first pick is still good.
    pub(crate) fn handle_fuse_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_fuse_second = None;
                self.fuse_name_input.clear();
                self.mode = Mode::FuseSecond;
            }
            GameKey::Backspace => {
                self.fuse_name_input.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.fuse_name_input.chars().count()
                        < feral_processes_engine::MAX_CUSTOM_NAME_LEN =>
            {
                self.fuse_name_input.push(c);
            }
            GameKey::Enter => {
                let (Some(first), Some(second)) = (
                    self.pending_fuse_first.take(),
                    self.pending_fuse_second.take(),
                ) else {
                    self.mode = Mode::Playing;
                    return;
                };
                let name = (!self.fuse_name_input.is_empty()).then(|| self.fuse_name_input.clone());
                self.fuse_name_input.clear();
                let Some(game) = &mut self.game else { return };
                match game.fuse_companions(first, second, name) {
                    Ok(()) => self.status_line = None,
                    Err(e) => self.status_line = Some(e),
                }
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }
}
