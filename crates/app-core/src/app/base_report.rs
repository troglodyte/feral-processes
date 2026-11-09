//! The base report (`b`): its rail — BASE, JOBS, then the site and traffic
//! pages `app::sites_hub` builds — and the handlers of the screens on it.
//! The nav is the shared hub rule (`app/hub.rs`).

use crate::*;
use feral_processes_engine::BaseReportFlags;

/// The base report's one table: the rail, the screen order and "is this mode
/// in the hub" are all read off it, so a page cannot be on the rail and
/// missing from the nav. Rail order is the order here, with Breed slotted in
/// before Alerts once a bay stands; `[` and `]` walk it.
pub const BASE_REPORT_PAGES: &[FixedPage] = &[
    FixedPage {
        title: "Overview",
        tabs: &[HubTab::new("Overview", Mode::BaseOverview)],
    },
    FixedPage {
        title: "Build",
        tabs: &[
            HubTab::new("Deploy", Mode::Build),
            HubTab::new("Build orders", Mode::BuildOrders),
            HubTab::new("Dig", Mode::DigOrders),
            HubTab::new("Floor", Mode::FloorOrders),
        ],
    },
    FixedPage {
        title: "Production",
        tabs: &[
            HubTab::new("Output", Mode::BaseOutput),
            HubTab::new("Machines", Mode::BaseMachines),
            HubTab::new("Power", Mode::BasePower),
            HubTab::new("Recipes", Mode::Recipes),
            HubTab::new("Compile", Mode::Craft),
        ],
    },
    FixedPage {
        title: "Stock",
        tabs: &[
            HubTab::new("Stock", Mode::BaseStock),
            HubTab::new("Work orders", Mode::WorkOrders),
        ],
    },
    FixedPage {
        title: "Staff",
        tabs: &[
            HubTab::new("Wellbeing", Mode::StaffWellbeing),
            HubTab::new("Coverage", Mode::StaffCoverage),
            HubTab::new("Discontent", Mode::StaffDiscontent),
            HubTab::new("Duties", Mode::BaseStaff),
            HubTab::new("Staffing", Mode::Structures),
        ],
    },
    FixedPage {
        title: "Research",
        tabs: &[
            HubTab::new("Projects", Mode::Research),
            HubTab::new("Routines", Mode::RoutineResearch),
            HubTab::new("Affixes", Mode::AffixResearch),
            HubTab::new("Weapons", Mode::WeaponResearch),
            HubTab::new("Study", Mode::PinSubject),
        ],
    },
    FixedPage {
        title: "Alerts",
        tabs: &[HubTab::new("Alerts", Mode::Alerts)],
    },
    // A stopgap until hub-merge phase 3 gives these verbs keys on the
    // structure's Info tab in `i`; delete it then.
    FixedPage {
        title: "Structures",
        tabs: &[HubTab::new("Structures", Mode::BaseStructures)],
    },
];

/// The section header the rail draws above the Overview.
const BASE_SECTION: &str = "BASE";

/// The Breed page, on the rail before Alerts once a bay stands. Not in the
/// fixed table, so every fixed page keeps the index `base_page_flag` and the
/// Overview read it by.
const BREED_PAGE: FixedPage = FixedPage {
    title: "Breed",
    tabs: &[HubTab::new("Breed", Mode::Breed)],
};

/// The JOBS section's one entry: every contract held, then the Broker's
/// board. On the rail once a Broker stands or a contract is held, the
/// question the Contracts page's rail entry used to ask.
pub const CONTRACTS_TITLE: &str = "Contracts";
pub const CONTRACTS_TABS: &[HubTab] = &[
    HubTab::new("Held", Mode::ContractsHeld),
    HubTab::new("Board", Mode::Contracts),
];

/// The base report as a hub: a fixed table, the attention marks off
/// `Game::base_report_flags`.
pub(crate) struct BaseReport;

impl HubKind for BaseReport {
    fn title(&self) -> &'static str {
        "BASE REPORT"
    }

    /// The table, with Breed before Alerts once a bay stands — but not
    /// while the `[c]` hub is open, whose own Breed page is the same mode:
    /// a mode names one hub, so only one may offer it at a time.
    fn pages(&self, app: &App) -> Vec<HubPage> {
        let flags = app.game.as_ref().map(|g| g.base_report_flags());
        let mut pages: Vec<HubPage> = BASE_REPORT_PAGES
            .iter()
            .enumerate()
            .map(|(i, fixed)| HubPage {
                marked: flags
                    .as_ref()
                    .is_some_and(|f| base_page_flag(f, i).is_some()),
                ..fixed.page()
            })
            .collect();
        let bay_stands = app
            .game
            .as_ref()
            .is_some_and(|g| g.base_breeding_bay().is_some());
        if bay_stands {
            let alerts = pages
                .iter()
                .position(|p| p.key == HubPageKey::Named("Alerts"))
                .unwrap_or(pages.len());
            pages.insert(alerts, BREED_PAGE.page());
        }
        pages[0].section = Some(BASE_SECTION);
        // `broker_reach` and not `contract_board`: this runs every frame,
        // and the board rolls every template before it can answer.
        let jobs = app.game.as_ref().is_some_and(|g| {
            g.broker_reach() != BrokerReach::NoBroker || !g.active_contracts().is_empty()
        });
        if jobs {
            pages.push(HubPage {
                section: Some("JOBS"),
                ..FixedPage {
                    title: CONTRACTS_TITLE,
                    tabs: CONTRACTS_TABS,
                }
                .page()
            });
        }
        pages.extend(super::sites_hub::site_pages(app));
        pages
    }
}

/// Why `page` is flagged, if it is: the engine's reason for the first of
/// its tabs that has one, so a page merged from several keeps each of their
/// marks. A page the engine has no flag for is never flagged.
pub fn base_page_flag(flags: &BaseReportFlags, page: usize) -> Option<&str> {
    BASE_REPORT_PAGES[page]
        .tabs
        .iter()
        .find_map(|tab| tab_flag(flags, tab.mode))
}

fn tab_flag(flags: &BaseReportFlags, mode: Mode) -> Option<&str> {
    match mode {
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
            let title = BASE_REPORT_PAGES[row + 1].title;
            self.turn_hub_to(Hub::Base, &HubPageKey::Named(title));
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
