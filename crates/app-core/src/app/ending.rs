//! The Basin Exit's confirmation and the ending pages that follow it.

use crate::*;

impl App {
    /// Takes the engine's "stepped onto the exit" flag and, if it was set,
    /// opens `Mode::BasinExitConfirm`. Returns whether it did, so the caller
    /// skips the rest of its after-step bookkeeping for a step that did not
    /// happen. Drops any queued walk: the player was stopped by a question
    /// and should not march on through it when it is answered.
    pub(crate) fn open_basin_exit_prompt(&mut self) -> bool {
        let asked = self
            .game
            .as_mut()
            .is_some_and(|game| game.take_basin_exit_prompt());
        if asked {
            self.walk = None;
            self.status_line = None;
            self.menu_selected = 0;
            self.mode = Mode::BasinExitConfirm;
        }
        asked
    }

    /// `y` leaves the Basin and opens the ending; `n` and Esc stay. The
    /// row-selector shape of every other yes/no popup.
    pub(crate) fn handle_basin_exit_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        match self.yes_no(key) {
            Some(true) => self.escape_basin(),
            Some(false) => self.mode = Mode::Playing,
            _ => {}
        }
    }

    /// The one writer of `ending_screens`. A refusal (the engine re-checks
    /// the gate) lands on the map's status line.
    fn escape_basin(&mut self) {
        let Some(game) = &mut self.game else { return };
        match game.escape_basin() {
            Ok(()) => {
                self.ending_screens = game.ending_screens();
                self.bank_run_score();
                self.ending_page = 0;
                self.mode = Mode::Ending;
            }
            Err(why) => {
                self.status_line = Some(why);
                self.mode = Mode::Playing;
            }
        }
    }

    /// Enter or Right turns the page forward, Left back; Esc, or Enter on
    /// the last page, resumes play.
    pub(crate) fn handle_ending_key(&mut self, key: GameKey) {
        match key {
            GameKey::Enter | GameKey::Right => {
                if self.ending_page + 1 < self.ending_screens.len() {
                    self.ending_page += 1;
                } else {
                    self.close_ending();
                }
            }
            GameKey::Left => self.ending_page = self.ending_page.saturating_sub(1),
            GameKey::Esc => self.close_ending(),
            _ => {}
        }
    }

    fn close_ending(&mut self) {
        self.ending_screens.clear();
        self.ending_page = 0;
        self.mode = Mode::Escaped;
    }

    /// Enter or Esc leaves the score card for the map.
    pub(crate) fn handle_escaped_key(&mut self, key: GameKey) {
        if matches!(key, GameKey::Enter | GameKey::Esc) {
            self.mode = Mode::Playing;
            // The escape's own achievement is waiting in the queue.
            self.show_next_notification();
        }
    }
}
