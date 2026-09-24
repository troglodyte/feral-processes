//! `Mode::LevelUp`'s one handler.

use crate::*;

impl App {
    /// Only two keys act; everything else is ignored, `Mode::Notification`'s
    /// own shape.
    ///
    /// `Esc` closes to `Mode::Playing` and immediately lets the next queued
    /// notification through, `handle_notification_key`'s pattern. Uppercase
    /// `P` — lowercase letters are row selectors, and this page has none —
    /// closes the page and opens `Mode::Perks` directly. `menu_origin` is
    /// left untouched (this page never sets it), so `handle_perks_key`'s own
    /// `Esc` — `App::close_screen` — falls back to `Mode::Playing` rather
    /// than back to the page it can no longer answer for.
    pub(crate) fn handle_level_up_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_level_up = None;
                self.mode = Mode::Playing;
                self.show_next_notification();
            }
            GameKey::Char('P') => {
                self.pending_level_up = None;
                self.mode = Mode::Perks;
            }
            _ => {}
        }
    }
}
