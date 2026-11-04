//! The base report (`b`): its rail, and the handlers of the screens on it.
//! The nav is the shared hub rule (`app/hub.rs`).

use crate::*;
use feral_processes_engine::BaseReportFlags;

/// The base report's one table: the rail, the screen order and "is this mode
/// in the hub" are all read off it, so a page cannot be on the rail and
/// missing from the nav. Rail order is the order here; `[` and `]` walk it.
pub const BASE_REPORT_PAGES: &[HubPage] = &[
    HubPage {
        title: "Overview",
        screens: &[Mode::BaseOverview],
    },
    HubPage {
        title: "Build",
        screens: &[Mode::BaseBuild],
    },
    HubPage {
        title: "Research",
        screens: &[Mode::BaseResearch],
    },
    HubPage {
        title: "Trade",
        screens: &[Mode::BaseTrade],
    },
    HubPage {
        title: "Compile",
        screens: &[Mode::BaseCompile],
    },
    HubPage {
        title: "Recipes",
        screens: &[Mode::Recipes],
    },
    HubPage {
        title: "Staff",
        screens: &[
            Mode::StaffWellbeing,
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
        ],
    },
    HubPage {
        title: "Production",
        screens: &[Mode::BaseOutput, Mode::BaseMachines, Mode::BasePower],
    },
    HubPage {
        title: "Stock",
        screens: &[Mode::BaseStock],
    },
    HubPage {
        title: "Work orders",
        screens: &[Mode::WorkOrders],
    },
    HubPage {
        title: "Staffing",
        screens: &[Mode::BaseStaff, Mode::Structures],
    },
    HubPage {
        title: "Build orders",
        screens: &[Mode::BuildOrders, Mode::DigOrders, Mode::FloorOrders],
    },
    HubPage {
        title: "Alerts",
        screens: &[Mode::Alerts],
    },
];

/// The Breed page, on the rail after the fixed table once a bay stands.
/// Last, so every fixed page keeps the index `base_page_flag` and the marks
/// read it by.
const BREED_PAGE: HubPage = HubPage {
    title: "Breed",
    screens: &[Mode::Breed],
};

/// The base report as a hub: a fixed table, the attention marks off
/// `Game::base_report_flags`.
pub(crate) struct BaseReport;

impl HubKind for BaseReport {
    fn title(&self) -> &'static str {
        "BASE REPORT"
    }

    fn table(&self) -> &'static [HubPage] {
        BASE_REPORT_PAGES
    }

    /// The table, then Breed once a bay stands — but not while the `[c]`
    /// hub is open, whose own Breed page is the same mode: a mode names
    /// one hub, so only one may offer it at a time.
    fn pages(&self, app: &App) -> Vec<&'static HubPage> {
        let mut pages: Vec<&'static HubPage> = BASE_REPORT_PAGES.iter().collect();
        let bay_stands = app
            .game
            .as_ref()
            .is_some_and(|g| g.base_breeding_bay().is_some());
        if bay_stands && app.structure_hub.is_none() {
            pages.push(&BREED_PAGE);
        }
        pages
    }

    fn marks(&self, app: &App) -> Vec<bool> {
        let Some(game) = &app.game else {
            return Vec::new();
        };
        let flags = game.base_report_flags();
        (0..BASE_REPORT_PAGES.len())
            .map(|page| base_page_flag(&flags, page).is_some())
            .collect()
    }
}

/// Why `page` is flagged, if it is: the engine's reason for the page's
/// first screen. A page the engine has no flag for is never flagged.
pub fn base_page_flag(flags: &BaseReportFlags, page: usize) -> Option<&str> {
    match BASE_REPORT_PAGES[page].screens[0] {
        Mode::StaffWellbeing => flags.staff.as_deref(),
        Mode::BaseOutput => flags.production.as_deref(),
        Mode::BaseStock => flags.stock.as_deref(),
        Mode::BuildOrders => flags.build_orders.as_deref(),
        Mode::WorkOrders => flags.work_orders.as_deref(),
        Mode::BaseStaff => flags.staffing.as_deref(),
        Mode::Alerts => flags.alerts.as_deref(),
        _ => None,
    }
}

impl App {
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

    /// The Machines, Power, Stock, Build-order and Recipes screens: a cursor
    /// over the rows the renderer draws from the same engine report,
    /// read-only. Recipes scrolls by entry rather than by drawn line: the
    /// description and steps under a product are sub-rows the renderer draws
    /// unselectable, so app-core owns a count it can derive without knowing
    /// the renderer's line shape.
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
            Mode::Recipes => game.recipe_chains().len(),
            _ => unreachable!("guarded by handle_key's list-screen pattern"),
        };
        self.scroll(key, rows);
    }
}
