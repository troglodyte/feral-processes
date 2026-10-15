//! Hub navigation. Each `Hub`'s `pages()` is its only list of pages;
//! everything here is read off it.

use crate::*;

impl App {
    /// Runs ahead of every hub screen's own handler and answers whether it
    /// took the key. The hub has one cursor, on the rail or in the screen
    /// (`hub_on_rail`). On the rail, Up/Down walk the pages (wrapping),
    /// Right or Enter moves the cursor into the screen, Esc leaves to
    /// wherever the hub was opened from, and every other key is swallowed —
    /// a row jump or a screen action with no cursor on the screen would act
    /// on a row the player cannot see is chosen. In the screen, a page's
    /// screens lie in a row to the right of the rail: Right steps to the next
    /// (stopping at the last), Left to the previous and off the first back to
    /// the rail, and Esc goes straight to the rail. Everything else falls
    /// through. `[`/`]` turn pages from either half.
    ///
    /// `Esc` on the rail leaves the hub rather than stepping back a page:
    /// the rail is a set of siblings, not a history.
    pub(crate) fn handle_hub_nav(&mut self, key: GameKey) -> bool {
        let Some((hub, page, screen)) = hub_of(self.mode) else {
            return false;
        };
        let table = hub.pages();
        let pages = table.len();
        let turn = |by: usize| table[(page + by) % pages].screens[0];
        let screens = table[page].screens;
        match key {
            GameKey::Char(']') => self.mode = turn(1),
            GameKey::Char('[') => self.mode = turn(pages - 1),
            _ if self.hub_on_rail => match key {
                GameKey::Down => self.mode = turn(1),
                GameKey::Up => self.mode = turn(pages - 1),
                GameKey::Right | GameKey::Enter => self.hub_on_rail = false,
                GameKey::Esc => self.close_screen(),
                _ => {}
            },
            // With a program picked Esc steps back to the list, which the
            // downed-program handler owns; Left would strand that pick on
            // the rail.
            GameKey::Esc | GameKey::Left if self.in_downed_drill_down() => return false,
            GameKey::Esc => self.hub_on_rail = true,
            // The duty table spends Left and Right on its column cursor, so
            // it only leaves off its first and last columns.
            GameKey::Left if !(self.mode == Mode::BaseStaff && self.work_column > 0) => {
                match screen.checked_sub(1) {
                    Some(prev) => self.mode = screens[prev],
                    None => self.hub_on_rail = true,
                }
            }
            GameKey::Right if screen + 1 < screens.len() && !self.on_inner_staff_column() => {
                self.mode = screens[screen + 1];
            }
            _ => return false,
        }
        true
    }

    fn in_downed_drill_down(&self) -> bool {
        self.mode == Mode::DownedPrograms && self.pending_downed_program_index.is_some()
    }

    /// Whether Right on the duty table still has a column to move to.
    fn on_inner_staff_column(&mut self) -> bool {
        self.mode == Mode::BaseStaff && self.work_column + 1 < self.base_staff_table().columns.len()
    }

    /// Opens the hub where the player left it, or on Overview the first
    /// time, with the cursor on the rail. `b` opens it from the map
    /// with no `menu_origin`, so `Esc` from the rail returns there.
    pub(crate) fn open_base_report(&mut self) {
        self.mode = self.hub_last[Hub::Base.slot()].unwrap_or(Mode::BaseOverview);
        self.hub_on_rail = true;
    }

    /// Opens the inventory where the player left it, or on Items the first
    /// time, with the cursor on the rail — `open_base_report`'s rule.
    pub(crate) fn open_inventory_hub(&mut self) {
        self.mode = self.hub_last[Hub::Inventory.slot()].unwrap_or(Mode::Inventory);
        self.hub_on_rail = true;
    }

    /// Overview: a cursor over the pages after itself. `Enter`, or a row's
    /// own shortcut, jumps to that page's first screen.
    pub(crate) fn handle_base_overview_key(&mut self, key: GameKey) {
        if let Some(row) = self.selected_index(key, BASE_REPORT_PAGES.len() - 1) {
            self.mode = BASE_REPORT_PAGES[row + 1].screens[0];
        }
    }

    /// A command page: a cursor over its rows, `selected_index` style. The
    /// hub nav has already let Up/Down/Enter and row letters through because
    /// the cursor is in the page. The row's index is parked so Esc from its
    /// screen lands back on it.
    pub(crate) fn handle_command_page_key(&mut self, key: GameKey) {
        let Some(rows) = self.command_rows(self.mode) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, rows.len()) {
            self.menu_origin = Some(self.mode);
            self.command_return_row = Some(idx);
            self.open_row_target(rows[idx].target);
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
            Mode::FloorOrders => game.floor_marks().len(),
            _ => unreachable!("guarded by handle_key's list-screen pattern"),
        };
        self.scroll(key, rows);
    }
}
