//! The Power Siphon's screen: load a staff program into the machine beside
//! the party, or let the one in it go.
//!
//! A page of the structure hub (`app/structure_hub.rs`) on a siphon. The rows
//! inside use lowercase selectors like every other list.

use crate::*;

impl App {
    /// Opens the screen on one siphon, the hub's Siphon page.
    pub(crate) fn open_siphon_for(&mut self, siphon: Entity) {
        self.siphon = Some(siphon);
        self.menu_selected = 0;
        self.mode = Mode::Siphon;
    }

    /// Ends the page's work: back to the map, the hub with it.
    pub(crate) fn leave_siphon(&mut self) {
        self.siphon = None;
        self.mode = Mode::Playing;
    }

    /// Confirms `Mode::Siphon`'s rows. **`Game::siphon_holder` decides which
    /// of two screens this is**, the same call `render::draw_siphon` reads:
    /// an occupied siphon shows one row that releases its program, an empty
    /// one is the `Game::base_staff` picker.
    pub(crate) fn handle_siphon_key(&mut self, key: GameKey) {
        let Some(siphon) = self.siphon else {
            return;
        };
        let Some(game) = &self.game else { return };
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
