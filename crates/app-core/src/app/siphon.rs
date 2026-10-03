//! The Power Siphon's screen: load a staff program into the machine beside
//! the party, or let the one in it go.
//!
//! Opened with `[P]` beside a siphon, `open_rig_tool`'s shape — an action on
//! one machine, so the key lives on the map and never in a group menu. The
//! rows inside use lowercase selectors like every other list.

use crate::*;

impl App {
    /// Opens the screen for the first adjacent siphon, or refuses.
    ///
    /// **Answers whether anything happened**, `open_rig_tool`'s rule: a
    /// refusal answers `false`, because `after_world_action` clears the
    /// status line of a key that acted and would wipe the sentence just
    /// written through `App::refuse`.
    pub(crate) fn open_siphon(&mut self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        let Some(siphon) = game.adjacent_siphons().first().copied() else {
            self.refuse("There is no Power Siphon here.");
            return false;
        };
        self.siphon = Some(siphon);
        self.menu_selected = 0;
        self.mode = Mode::Siphon;
        true
    }

    pub(crate) fn leave_siphon(&mut self) {
        self.siphon = None;
        self.mode = Mode::Playing;
    }

    /// Confirms `Mode::Siphon`'s rows. **`Game::siphon_holder` decides which
    /// of two screens this is**, the same call `render::draw_siphon` reads:
    /// an occupied siphon shows one row that releases its program, an empty
    /// one is the `Game::base_staff` picker.
    pub(crate) fn handle_siphon_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_siphon();
            return;
        }
        let Some(siphon) = self.siphon else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &self.game else { return };
        // Walked away from, or demolished, while the screen was open.
        if !game.adjacent_siphons().contains(&siphon) {
            self.leave_siphon();
            return;
        }
        let holder = game.siphon_holder(siphon);
        let staff = if holder.is_none() {
            game.base_staff()
        } else {
            Vec::new()
        };
        let rows = if holder.is_some() { 1 } else { staff.len() };
        let Some(idx) = self.selected_index(key, rows) else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let outcome = match holder {
            Some(program) => game.release_siphoned(program),
            None => game.siphon_program(staff[idx], siphon),
        };
        self.report(outcome);
        self.leave_siphon();
    }
}
