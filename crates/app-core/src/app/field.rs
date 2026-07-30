//! Casting a field routine outside battle — see `Game::field_routines`.

use crate::*;

impl App {
    /// Who a `OneAlly` field routine can be aimed at. Reuses
    /// `Game::routine_holders` — the same "you, then every program you own"
    /// walk `Mode::RoutineTarget` already trusts — rather than a second one,
    /// because `Game::cast_field_routine` checks only that a target is
    /// alive, not that the player owns it. This is what stands between the
    /// picker and handing the engine a hostile or unowned entity.
    pub(crate) fn field_ally_options(&mut self) -> Vec<RoutineHolderView> {
        self.game
            .as_mut()
            .map(|g| g.routine_holders())
            .unwrap_or_default()
    }

    /// Picks which installed field routine to run. A row that needs no ally
    /// target casts on the spot; one that does hands off to
    /// `Mode::FieldCastAlly` for who it lands on — the routine and its
    /// target are separate choices, same split `Mode::BattleSpecial` makes
    /// before `Mode::BattleAlly`.
    pub(crate) fn handle_field_cast_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let routines = game.field_routines();
        let Some(idx) = self.selected_index(key, routines.len()) else {
            return;
        };
        let row = &routines[idx];
        if !row.affordable {
            self.status_line = Some(format!("Not enough Power to run {}.", row.name));
            return;
        }
        if row.needs_ally_target {
            self.pending_field_routine = Some(idx);
            self.menu_selected = 0;
            self.mode = Mode::FieldCastAlly;
            return;
        }
        let Some(game) = &mut self.game else { return };
        match game.cast_field_routine(idx, None) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }

    /// Picks who the routine chosen in `Mode::FieldCast` lands on, then
    /// casts it.
    pub(crate) fn handle_field_cast_ally_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_field_routine = None;
            self.menu_selected = 0;
            self.mode = Mode::FieldCast;
            return;
        }
        let Some(index) = self.pending_field_routine else {
            self.mode = Mode::FieldCast;
            return;
        };
        let targets = self.field_ally_options();
        let Some(idx) = self.selected_index(key, targets.len()) else {
            return;
        };
        let target = targets[idx].entity;
        self.pending_field_routine = None;
        let Some(game) = &mut self.game else { return };
        match game.cast_field_routine(index, Some(target)) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }
}
