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

use super::popup::*;
use super::*;
use feral_processes_app_core::{BASE_REPORT_PAGES, base_page_of};

/// Draws the hub at `mode`, which `base_page_of` has already placed on a
/// page. `selected` is the screen's own cursor (`App::menu_selected`).
pub(super) fn draw_base_report(
    game: &mut Game,
    mode: Mode,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some((page, screen)) = base_page_of(mode) else {
        return;
    };
    let flags = game.base_report_flags();
    let rows = screen_rows(game, mode, selected, &flags);
    let content = frame::paint_frame(page, screen, mode, &flags, refusal, painter, m);
    draw_rows_in(content, &rows, painter, m);
}

/// The rows of one hub screen. Every figure comes from the engine; this only
/// formats.
fn screen_rows(
    game: &mut Game,
    mode: Mode,
    selected: usize,
    flags: &feral_processes_engine::BaseReportFlags,
) -> Vec<Row> {
    match mode {
        Mode::BaseOverview => overview::overview_rows(flags, selected),
        Mode::BaseOutput => building::base_output_rows(&game.base_output_report()),
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
        Mode::BaseOutput => flags.production.as_deref(),
        Mode::Alerts => flags.alerts.as_deref(),
        _ => None,
    }
}
