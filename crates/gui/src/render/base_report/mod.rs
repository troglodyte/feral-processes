//! The base report hub: a page rail down the left and the current screen's
//! rows in the content rect beside it. The pages and their screens come from
//! `app_core::BASE_REPORT_PAGES`; this module only draws what that table
//! says, so a page cannot be on the rail and missing from the nav.
//!
//! Each absorbed screen is a row builder drawn by `draw_rows_in`, not a
//! popup: the frame owns the chrome, the screen owns its rows.

mod commands;
mod frame;
pub(in crate::render) use frame::RAIL_CELLS;
#[cfg(test)]
pub(in crate::render) use frame::content_rect_at;
mod orders;
mod overview;
mod production;
mod staff;
mod stock;

use super::popup::*;
use super::*;
use feral_processes_app_core::{
    BASE_REPORT_PAGES, GroupMenuRow, WorkOrderRow, base_page_of, base_popup_parent,
};
use feral_processes_engine::WorkTable;

/// What the screens that read `App` state rather than the `Game` are drawn
/// from, taken off the app before `app.game` is borrowed — the same
/// handler-and-renderer-share-one-list rule `render_screen` follows.
pub(super) struct AppScreens<'a> {
    pub(super) work_orders: &'a [WorkOrderRow],
    pub(super) base_staff: Option<&'a WorkTable>,
    pub(super) work_column: usize,
    /// `App::hub_on_rail`: the rail holds the cursor, so the screen's rows
    /// draw none.
    pub(super) on_rail: bool,
    /// The current screen's rows when it is a command page.
    pub(super) commands: Option<&'a [GroupMenuRow]>,
    /// `(page index, row count)` for each command page, for the Overview.
    pub(super) command_counts: &'a [(usize, usize)],
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
    let content = frame::paint_frame(
        page,
        screen,
        Some(mode),
        app_screens.on_rail,
        &flags,
        refusal,
        painter,
        m,
    );
    draw_rows_in(content, &rows, !app_screens.on_rail, painter, m);
}

/// A popup child's parent page, drawn as the frame behind it: rail and title,
/// but not the screen's rows or its keybar (the child carries its own hints). The popup panel is not quite
/// opaque and lets whatever lies under it through, which over the map is
/// the point and over a screen of text is unreadable, so the rows wait
/// until the popup is gone. The child owns the cursor and the refusal line.
pub(super) fn draw_popup_parent(game: &Game, child: Mode, painter: &Painter, m: &Metrics) {
    let Some(parent) = base_popup_parent(child) else {
        return;
    };
    let Some((page, screen)) = base_page_of(parent) else {
        return;
    };
    frame::paint_frame(
        page,
        screen,
        None,
        false,
        &game.base_report_flags(),
        None,
        painter,
        m,
    );
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
        Mode::BaseOverview => overview::overview_rows(flags, app_screens.command_counts, selected),
        Mode::BaseBuild | Mode::BaseResearch | Mode::BaseTrade | Mode::BaseCompile => {
            commands::command_rows(app_screens.commands.unwrap_or_default(), selected)
        }
        Mode::StaffWellbeing => staff::wellbeing_rows(&game.staff_wellbeing(), selected),
        Mode::StaffCoverage => staff::coverage_rows(&game.need_coverage(), selected),
        Mode::StaffDiscontent => staff::discontent_rows(&game.discontent(), selected),
        Mode::BaseOutput => building::base_output_rows(&game.base_output_report()),
        Mode::BaseMachines => production::machine_rows(&game.machine_report(), selected),
        Mode::BasePower => production::power_rows(&game.power_report(), selected),
        Mode::BaseStock => stock::stock_rows(&game.stock_report(), selected),
        Mode::BuildOrders => orders::build_rows(&game.build_order_report(), selected),
        Mode::DigOrders => orders::dig_rows(&game.dig_marks(), selected),
        Mode::FloorOrders => orders::floor_rows(&game.floor_marks(), selected),
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
                (!app_screens.on_rail).then_some(app_screens.work_column),
            ),
            None => Vec::new(),
        },
        Mode::Structures => building::structures_rows(game, selected),
        Mode::Alerts => alerts::alert_rows(&game.alerts(), selected),
        other => vec![text_row(format!("No report for {other:?}."))],
    }
}

/// Each page whose first screen is a command page, with its index.
pub(super) fn command_pages() -> impl Iterator<Item = (usize, Mode)> {
    BASE_REPORT_PAGES
        .iter()
        .enumerate()
        .map(|(page, entry)| (page, entry.screens[0]))
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
        Mode::BaseStock => flags.stock.as_deref(),
        Mode::BuildOrders => flags.build_orders.as_deref(),
        Mode::WorkOrders => flags.work_orders.as_deref(),
        Mode::BaseStaff => flags.staffing.as_deref(),
        Mode::Alerts => flags.alerts.as_deref(),
        _ => None,
    }
}

/// What every screen's width census shares.
#[cfg(test)]
mod fit {
    use super::*;

    pub(super) fn text_of(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    /// `draw_row` clips a row vertically and never horizontally, so a line
    /// past the right edge is lost. Held at the smallest supported window,
    /// where the content rect is narrowest in characters.
    pub(super) fn assert_rows_fit(rows: &[Row], what: &str) {
        crate::paint::with_painter(|p| {
            for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
                let m = crate::text::ui_metrics(h);
                let char_w = p.measure_ui_advance("M", m.font_size);
                let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
                let room = content.w - m.pad * 2.0;
                for row in rows {
                    // `draw_row` prefixes every `Row::Item` with two spaces.
                    let line = format!("  {}", text_of(row));
                    let drawn = p.measure_ui_advance(&line, m.font_size);
                    assert!(
                        drawn <= room,
                        "a {what} row overflows by {:.0}px at {w}x{h} \
                         ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }
}
