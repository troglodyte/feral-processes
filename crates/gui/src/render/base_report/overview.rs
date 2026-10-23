//! The landing page: one line per page, its worst gap or "ok".

use super::*;
use feral_processes_engine::BaseReportFlags;

/// The Overview is not a row in its own list: it is where you already are.
pub(super) fn overview_rows(
    flags: &BaseReportFlags,
    command_counts: &[(usize, usize)],
    selected: usize,
) -> Vec<Row> {
    let name_w = BASE_REPORT_PAGES
        .iter()
        .map(|p| p.title.chars().count())
        .max()
        .unwrap_or(0);
    let mut rows: Vec<Row> = (1..BASE_REPORT_PAGES.len())
        .map(|page| {
            let (status, color) = page_status(flags, command_counts, page);
            let text = format!("{:<name_w$}  {status}", BASE_REPORT_PAGES[page].title,);
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

/// A command page says how many things can be done, never yellow: nothing
/// being doable is not a gap in the base.
fn page_status(
    flags: &BaseReportFlags,
    command_counts: &[(usize, usize)],
    page: usize,
) -> (String, Color) {
    if let Some(&(_, n)) = command_counts.iter().find(|(p, _)| *p == page) {
        let text = if n == 0 {
            "none here".to_string()
        } else {
            format!("{n} available")
        };
        return (text, TEXT_DIM);
    }
    match feral_processes_app_core::base_page_flag(flags, page) {
        Some(why) => (why.to_string(), YELLOW),
        None => ("ok".to_string(), TEXT_DIM),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(rows: &[Row], title: &str) -> String {
        rows.iter()
            .map(|r| super::super::fit::text_of(r).to_string())
            .find(|t| t.contains(title))
            .unwrap()
    }

    #[test]
    fn a_command_page_reads_its_count_or_none_here() {
        let flags = BaseReportFlags::default();
        let rows = overview_rows(&flags, &[(1, 4), (2, 0)], 0);
        assert!(status_of(&rows, "Build").ends_with("4 available"));
        assert!(status_of(&rows, "Research").ends_with("none here"));
    }

    #[test]
    fn a_report_page_keeps_its_ok_text() {
        let flags = BaseReportFlags::default();
        let rows = overview_rows(&flags, &[], 0);
        assert!(status_of(&rows, "Stock").ends_with("ok"));
    }

    #[test]
    fn a_command_page_is_never_yellow() {
        let flags = BaseReportFlags::default();
        assert_eq!(page_status(&flags, &[(1, 0)], 1).1, TEXT_DIM);
    }
}
