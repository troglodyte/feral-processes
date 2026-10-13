//! The base report hub: a page rail down the left and the current screen's
//! rows in the content rect beside it. The pages and their screens come from
//! `app_core::BASE_REPORT_PAGES`; this module only draws what that table
//! says, so a page cannot be on the rail and missing from the nav.
//!
//! Each absorbed screen is a row builder drawn by `draw_rows_in`, not a
//! popup: the frame owns the chrome, the screen owns its rows.

mod frame;
#[cfg(test)]
pub(in crate::render) use frame::content_rect_at;
mod overview;
mod staff;

use super::popup::*;
use super::*;
use feral_processes_app_core::{BASE_REPORT_PAGES, WorkOrderRow, base_page_of, base_popup_parent};
use feral_processes_engine::WorkTable;

/// What the screens that read `App` state rather than the `Game` are drawn
/// from, taken off the app before `app.game` is borrowed — the same
/// handler-and-renderer-share-one-list rule `render_screen` follows.
pub(super) struct AppScreens<'a> {
    pub(super) work_orders: &'a [WorkOrderRow],
    pub(super) base_staff: Option<&'a WorkTable>,
    pub(super) work_column: usize,
}

/// Draws the hub at `mode`, which `base_page_of` has already placed on a
/// page. `selected` is the screen's own cursor (`App::menu_selected`).
pub(super) fn draw_base_report(
    game: &mut Game,
    mode: Mode,
    selected: usize,
    app_screens: &AppScreens,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some((page, screen)) = base_page_of(mode) else {
        return;
    };
    let flags = game.base_report_flags();
    let rows = screen_rows(game, mode, selected, app_screens, &flags);
    let content = frame::paint_frame(page, screen, mode, &flags, refusal, painter, m);
    draw_rows_in(content, &rows, painter, m);
}

/// A popup child's parent screen, drawn as the frame behind it. The child
/// owns the cursor and the refusal line, so the parent shows neither.
pub(super) fn draw_popup_parent(
    game: &mut Game,
    child: Mode,
    app_screens: &AppScreens,
    painter: &Painter,
    m: &Metrics,
) {
    if let Some(parent) = base_popup_parent(child) {
        draw_base_report(game, parent, usize::MAX, app_screens, None, painter, m);
    }
}

/// The rows of one hub screen. Every figure comes from the engine; this only
/// formats.
fn screen_rows(
    game: &mut Game,
    mode: Mode,
    selected: usize,
    app_screens: &AppScreens,
    flags: &feral_processes_engine::BaseReportFlags,
) -> Vec<Row> {
    match mode {
        Mode::BaseOverview => overview::overview_rows(flags, selected),
        Mode::StaffWellbeing => staff::wellbeing_rows(&game.staff_wellbeing(), selected),
        Mode::StaffCoverage => staff::coverage_rows(&game.need_coverage(), selected),
        Mode::StaffDiscontent => staff::discontent_rows(&game.discontent(), selected),
        Mode::BaseOutput => building::base_output_rows(&game.base_output_report()),
        Mode::WorkOrders => building::work_order_screen_rows(
            app_screens.work_orders,
            game.labour_demand(),
            selected,
        ),
        Mode::BaseStaff => match app_screens.base_staff {
            Some(table) => building::base_staff_menu_rows(
                table,
                &game.owned_pets(),
                selected,
                app_screens.work_column,
            ),
            None => Vec::new(),
        },
        Mode::Structures => building::structures_rows(game, selected),
        Mode::Alerts => alerts::alert_rows(&game.alerts(), selected),
        other => vec![text_row(format!("No report for {other:?}."))],
    }
}

/// Why `page` is flagged, if it is: the engine's reason for the page's
/// first screen. A page the engine has no flag for is never flagged.
pub(super) fn page_flag(
    flags: &feral_processes_engine::BaseReportFlags,
    page: usize,
) -> Option<&str> {
    match BASE_REPORT_PAGES[page].screens[0] {
        Mode::StaffWellbeing => flags.staff.as_deref(),
        Mode::BaseOutput => flags.production.as_deref(),
        Mode::WorkOrders => flags.work_orders.as_deref(),
        Mode::BaseStaff => flags.staffing.as_deref(),
        Mode::Alerts => flags.alerts.as_deref(),
        _ => None,
    }
}
