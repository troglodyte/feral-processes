//! The base report hub's navigation. `BASE_REPORT_PAGES` is the only list of
//! pages; everything here is read off it.

use crate::*;

impl App {
    /// Runs ahead of every hub screen's own handler and answers whether it
    /// took the key. `[`/`]` walk the pages (wrapping), `Tab` walks the
    /// current page's screens (wrapping), `Esc` leaves to wherever the hub
    /// was opened from. Everything else — digits, lowercase, Enter — falls
    /// through, because Alerts and the later list screens already spend
    /// those on row jumps.
    ///
    /// `Esc` leaves the hub rather than stepping back a page: the rail is a
    /// set of siblings, not a history.
    pub(crate) fn handle_base_report_nav(&mut self, key: GameKey) -> bool {
        let Some((page, screen)) = base_page_of(self.mode) else {
            return false;
        };
        let pages = BASE_REPORT_PAGES.len();
        match key {
            GameKey::Char(']') => {
                self.mode = BASE_REPORT_PAGES[(page + 1) % pages].screens[0];
            }
            GameKey::Char('[') => {
                self.mode = BASE_REPORT_PAGES[(page + pages - 1) % pages].screens[0];
            }
            GameKey::Tab => {
                let screens = BASE_REPORT_PAGES[page].screens;
                self.mode = screens[(screen + 1) % screens.len()];
            }
            GameKey::Esc => self.close_screen(),
            _ => return false,
        }
        true
    }

    /// Opens the hub where the player left it, or on Overview the first
    /// time. `menu_origin` is whatever the caller already set (a group menu)
    /// or `None` from the map, so `Esc` goes back the way it came.
    pub(crate) fn open_base_report(&mut self) {
        self.mode = self.base_report_last.unwrap_or(Mode::BaseOverview);
    }

    /// Overview: a cursor over the pages after itself. `Enter`, or a row's
    /// own shortcut, jumps to that page's first screen.
    pub(crate) fn handle_base_overview_key(&mut self, key: GameKey) {
        if let Some(row) = self.selected_index(key, BASE_REPORT_PAGES.len() - 1) {
            self.mode = BASE_REPORT_PAGES[row + 1].screens[0];
        }
    }

    /// The three Staff screens: a cursor over the screen's own rows, which
    /// is the scroll position. The row counts are the engine reports the
    /// renderer draws, so the cursor cannot run past what is shown.
    pub(crate) fn handle_staff_key(&mut self, key: GameKey) {
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let rows = match self.mode {
            Mode::StaffWellbeing => game.staff_wellbeing().len(),
            Mode::StaffCoverage => game.need_coverage().len(),
            _ => game.discontent().len(),
        };
        self.scroll(key, rows);
    }

    /// The Machines, Power, Stock and Build-order screens: a cursor over the
    /// rows the renderer draws from the same engine report, read-only.
    pub(crate) fn handle_base_list_key(&mut self, key: GameKey) {
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let rows = match self.mode {
            Mode::BaseMachines => game.machine_report().len(),
            Mode::BasePower => game.power_report().lines.len(),
            Mode::BaseStock => game.stock_report().len(),
            Mode::BuildOrders => game.build_order_report().len(),
            Mode::DigOrders => game.dig_marks().len(),
            _ => game.floor_marks().len(),
        };
        self.scroll(key, rows);
    }
}
