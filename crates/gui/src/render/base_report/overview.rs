//! The landing page: one line per page, its worst gap or "ok".

use super::*;
use feral_processes_app_core::menu_shortcut;
use feral_processes_engine::BaseReportFlags;

/// The Overview is not a row in its own list: it is where you already are.
pub(super) fn overview_rows(flags: &BaseReportFlags, selected: usize) -> Vec<Row> {
    let name_w = BASE_REPORT_PAGES
        .iter()
        .map(|p| p.title.chars().count())
        .max()
        .unwrap_or(0);
    let mut rows: Vec<Row> = (1..BASE_REPORT_PAGES.len())
        .map(|page| {
            let status = page_flag(flags, page);
            let text = format!(
                "[{}] {:<name_w$}  {}",
                menu_shortcut(page - 1),
                BASE_REPORT_PAGES[page].title,
                status.unwrap_or("ok"),
            );
            let color = if status.is_some() { YELLOW } else { TEXT_DIM };
            colored_item_row(text, page - 1 == selected, color)
        })
        .collect();
    rows.insert(0, text_row(""));
    rows.insert(
        0,
        Row::TextColored("Where the base is short".to_string(), CYAN),
    );
    rows
}
