//! Casting a field routine outside battle — see `Game::field_routines`.

use crate::*;

impl App {
    /// Who a `OneAlly` field routine can be aimed at: you, then your active
    /// `Party` — `Game::field_cast_targets`, not the wider
    /// `Game::routine_holders` `Mode::RoutineTarget` uses. That wider list is
    /// every program you own regardless of location, but only the player and
    /// the party are ever ticked (`tick_field_buffs`); offering a benched
    /// program here used to let a cast pay Power for a buff that ticked
    /// nowhere. `Game::cast_field_routine` checks the same narrower set
    /// again on its own, so this is a UX narrowing, not the only guard.
    ///
    /// `routine` is the pending `field_routines` index, which is what lets a
    /// row say the cast would displace a buff already running on that target.
    pub(crate) fn field_ally_options(&mut self, routine: usize) -> Vec<FieldCastTargetView> {
        self.game
            .as_mut()
            .map(|g| g.field_cast_targets(routine))
            .unwrap_or_default()
    }

    /// Picks which installed field routine to run. A row that needs no
    /// second pick casts on the spot; one that does hands off to whichever
    /// picker its `FieldCastPick` names — the routine and its target are
    /// separate choices, same split `Mode::BattleSpecial` makes before
    /// `Mode::BattleAlly`.
    pub(crate) fn handle_field_cast_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let routines = game.field_routines();
        let Some(idx) = self.selected_index(key, routines.len()) else {
            return;
        };
        let row = &routines[idx];
        // The engine wrote the sentence, so this never has to guess which
        // need a routine spends or whether it needs open grid — see
        // `FieldRoutineView::unavailable`.
        if let Some(reason) = &row.unavailable {
            self.status_line = Some(format!("Can't run {} — {reason}.", row.name));
            return;
        }
        match row.second_pick {
            FieldCastPick::Ally => {
                self.pending_field_routine = Some(idx);
                self.menu_selected = 0;
                self.mode = Mode::FieldCastAlly;
                return;
            }
            FieldCastPick::Cell => {
                self.pending_field_routine = Some(idx);
                // The cursor opens on the party's own cell, which is the one
                // coordinate a player already knows the meaning of.
                self.field_cursor = self.game.as_ref().and_then(|g| g.stack_pos_xy());
                self.mode = Mode::FieldCastCell;
                return;
            }
            FieldCastPick::None => {}
        }
        let Some(game) = &mut self.game else { return };
        match game.cast_field_routine(idx, FieldCastTarget::None) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }

    /// Moves the cell cursor and commits the jump it is aimed at.
    ///
    /// The cursor walks with the same keys the player already walks with,
    /// read north-up like the map it is drawn over rather than relative to
    /// the party's facing — a cursor that rotated with the party would make
    /// the one screen the player bets their run on the hardest to read.
    ///
    /// Clamped to the frame's bounds, so an out-of-bounds coordinate is
    /// unreachable rather than lethal. Inside the grid, solid is solid.
    pub(crate) fn handle_field_cast_cell_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_field_routine = None;
            self.field_cursor = None;
            self.mode = Mode::FieldCast;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let Some((w, h)) = game.frame_bounds() else {
            self.mode = Mode::Playing;
            return;
        };
        let Some((cx, cy)) = self.field_cursor else {
            self.mode = Mode::FieldCast;
            return;
        };
        let (dx, dy) = match key {
            GameKey::Up | GameKey::Char('k') => (0, -1),
            GameKey::Down | GameKey::Char('j') => (0, 1),
            GameKey::Left | GameKey::Char('h') => (-1, 0),
            GameKey::Right | GameKey::Char('l') => (1, 0),
            GameKey::Enter => {
                let Some(index) = self.pending_field_routine.take() else {
                    self.mode = Mode::FieldCast;
                    return;
                };
                self.field_cursor = None;
                match game.cast_field_routine(index, FieldCastTarget::Cell(cx, cy)) {
                    Ok(()) => self.status_line = None,
                    Err(e) => self.status_line = Some(e),
                }
                self.mode = Mode::Playing;
                return;
            }
            _ => return,
        };
        self.field_cursor = Some(((cx + dx).clamp(0, w - 1), (cy + dy).clamp(0, h - 1)));
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
        let targets = self.field_ally_options(index);
        let Some(idx) = self.selected_index(key, targets.len()) else {
            return;
        };
        let target = targets[idx].entity;
        self.pending_field_routine = None;
        let Some(game) = &mut self.game else { return };
        match game.cast_field_routine(index, FieldCastTarget::Ally(target)) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }
}
