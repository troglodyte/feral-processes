//! `Mode::LevelUp`'s one handler, and the door back onto a page closed
//! with points unspent.

use crate::*;
use feral_processes_engine::StatOwner;

/// Where the level-up page, and the Points and Perks screens it opens, go
/// back to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelUpOrigin {
    /// Shown as a level lands: back to the map, letting the next queued
    /// notification through.
    #[default]
    Map,
    /// Reopened with `L` on your own Manifest: back to the sheet, which is
    /// still `pending_manifest`'s.
    Manifest,
}

impl App {
    /// Only three keys act; everything else is ignored, `Mode::Notification`'s
    /// own shape.
    ///
    /// `Enter` opens the Points screen on the banked stat points - the
    /// report is the page that says growth is now the player's to place.
    /// `Esc` closes to wherever `level_up_origin` says. Uppercase `P` —
    /// lowercase letters are row selectors, and this page has none — closes
    /// the page and opens `Mode::Perks` directly. Every close keeps the
    /// report in `last_level_up` for `reopen_level_up`.
    pub(crate) fn handle_level_up_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.keep_level_up();
                self.leave_level_up_flow();
            }
            GameKey::Enter => {
                self.open_stat_allocation(StatOwner::Player, AllocationOrigin::LevelUp);
                if self.mode == Mode::AllocateStats {
                    self.keep_level_up();
                }
            }
            GameKey::Char('P') => {
                self.keep_level_up();
                self.open_level_up_perks();
            }
            _ => {}
        }
    }

    fn keep_level_up(&mut self) {
        if let Some(report) = self.pending_level_up.take() {
            self.last_level_up = Some(report);
        }
    }

    /// The end of the level-up flow, from the page itself or from the
    /// Points screen it opened.
    pub(crate) fn leave_level_up_flow(&mut self) {
        match self.level_up_origin {
            LevelUpOrigin::Map => {
                self.mode = Mode::Playing;
                self.show_next_notification();
            }
            LevelUpOrigin::Manifest => self.mode = Mode::Manifest,
        }
    }

    /// Perks from the level-up flow. From the map, `menu_origin` is left
    /// untouched (this page never sets it), so Perks' own `Esc` —
    /// `App::close_screen` — falls back to `Mode::Playing` rather than back
    /// to a page it can no longer answer for; from the sheet it goes back to
    /// the sheet.
    pub(crate) fn open_level_up_perks(&mut self) {
        if self.level_up_origin == LevelUpOrigin::Manifest {
            self.menu_origin = Some(Mode::Manifest);
        }
        self.open_perks();
    }

    /// Whether `L` on your own Manifest would reopen the page: one was
    /// shown, and stat or Perk Points are still banked. The footer greys
    /// the key on the same answer.
    pub fn level_up_reopenable(&self) -> bool {
        let Some(game) = &self.game else { return false };
        let status = game.player_status();
        self.last_level_up.is_some() && (status.stat_points > 0 || status.perk_points > 0)
    }

    /// `L` on your own Manifest: the last level-up page again, its unspent
    /// figures read now rather than at the level, since that is what the
    /// page's keys will act on.
    pub(crate) fn reopen_level_up(&mut self) {
        if !self.level_up_reopenable() {
            return;
        }
        let (Some(game), Some(mut report)) = (&self.game, self.last_level_up.clone()) else {
            return;
        };
        let status = game.player_status();
        report.stat_points_unspent = status.stat_points;
        report.perk_points_unspent = status.perk_points;
        self.pending_level_up = Some(report);
        self.level_up_origin = LevelUpOrigin::Manifest;
        self.status_line = None;
        self.mode = Mode::LevelUp;
    }
}
