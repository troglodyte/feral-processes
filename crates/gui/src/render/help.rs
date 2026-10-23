//! The manual: the index of topics, and one page of it.
//!
//! Two screens rather than one because they answer to different idioms —
//! the index is an ordinary menu and a page is a document. `App` owns that
//! split and both row counts; this only draws what it is handed.

use super::popup::{colored_item_row, item_row};
use super::*;
use feral_processes_app_core::HelpPageView;

/// The topic list. An ordinary menu, so it draws exactly like every other
/// one.
pub(super) fn draw_help_index(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let rows = app.help_index_rows();
    let mut out = vec![text_row("The manual. Pick a topic."), text_row("")];
    if rows.is_empty() {
        out.push(text_row("No help pages are installed (assets/help/)."));
    }
    for (i, row) in rows.iter().enumerate() {
        out.push(item_row(row.title.to_string(), i == app.menu_selected));
    }
    out.push(text_row(""));
    out.push(text_row("Up/Down + Enter to read, Esc to close."));
    draw_popup("Help", PopupSize::Large, &out, refusal, painter, m);
}

const FOOTER_SCROLLING: &str = "Up/Down to scroll, Esc to go back.";
const FOOTER_WITH_LINKS: &str = "Up/Down to scroll, Tab for links, Esc to go back.";
const FOOTER_ON_LINKS: &str = "Up/Down + Enter to read on, Tab to scroll, Esc to go back.";

/// The pinned further-reading block: a heading that names the focus, then a
/// row per link, the highlighted one marked only while the links have focus.
fn link_lines(view: &HelpPageView, focused: bool, selected: usize) -> Vec<String> {
    if view.links.is_empty() {
        return Vec::new();
    }
    let mut out = vec![
        String::new(),
        if focused {
            "Read on (links):"
        } else {
            "Read on:"
        }
        .to_string(),
    ];
    for (i, link) in view.links.iter().enumerate() {
        let mark = if focused && i == selected { "> " } else { "  " };
        out.push(format!("{mark}{}", link.label));
    }
    out
}

fn footer(view: &HelpPageView, focused: bool) -> &'static str {
    match (view.links.is_empty(), focused) {
        (true, _) => FOOTER_SCROLLING,
        (false, false) => FOOTER_WITH_LINKS,
        (false, true) => FOOTER_ON_LINKS,
    }
}

/// One page. The prose is the scrollable body — every row an `Item` so
/// `popup_layout` scrolls it, with the highlight standing for the scroll
/// position exactly as the history screen's does. Further reading is pinned
/// below it and takes Tab to focus.
pub(super) fn draw_help_page(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let Some(view) = app.help_page_view() else {
        // The trail points at a page that is not loaded, which a modder
        // deleting a file mid-session can do. The index is still there.
        draw_help_index(app, refusal, painter, m);
        return;
    };
    let focused = app.help_links_focused;
    let mut rows: Vec<Row> = view
        .prose
        .iter()
        .enumerate()
        .map(|(i, line)| colored_item_row(line, i == app.menu_selected, TEXT))
        .collect();
    rows.extend(
        link_lines(&view, focused, app.help_link_selected)
            .into_iter()
            .map(text_row),
    );
    rows.push(text_row(""));
    rows.push(text_row(footer(&view, focused)));
    draw_popup(&view.title, PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::help::{self, HelpDb};

    fn help_db() -> HelpDb {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/help"));
        let (db, warnings) = HelpDb::load_dir(dir).expect("the shipped manual loads");
        assert!(warnings.is_empty(), "{warnings:#?}");
        db
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so a page row wider than its popup runs off the right
    /// edge in silence.
    ///
    /// This is also what pins `help::WRAP_COLUMNS` to a width the popup can
    /// actually draw — asserted here rather than restated in gui, because a
    /// second copy of the constant is a second thing to keep in step.
    /// `with_painter`'s 1440x900 is the geometry `ui_metrics` is calibrated
    /// against, so the font here is the unscaled body size.
    #[test]
    fn no_row_of_any_shipped_page_overflows_the_popup() {
        let db = help_db();
        let widest = db
            .pages()
            .iter()
            .flat_map(|p| help::page_rows(p, help::WRAP_COLUMNS))
            // `draw_row` prefixes every `Row::Item` with two spaces of its
            // own, which are as much of the drawn line as the text is.
            .map(|line| format!("  {line}"))
            .max_by_key(|line| line.chars().count())
            .expect("the shipped manual has rows");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let popup_w = 1440.0 * 0.88;
            let drawn = p.measure_ui_advance(&widest, m.font_size);
            let room = popup_w - m.pad * 2.0;
            assert!(
                drawn <= room,
                "the widest manual row overflows its popup by {:.0}px \
                 ({drawn:.0} drawn into {room:.0} of room):\n{widest}",
                drawn - room
            );
        });
    }

    /// A row wide enough to prove the assertion above can fail — the shipped
    /// pages all wrap to `WRAP_COLUMNS`, so without this the width test
    /// passes against a `WRAP_COLUMNS` of any size at all.
    #[test]
    fn a_row_past_the_column_budget_would_overflow_the_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let popup_w = 1440.0 * 0.88;
            let room = popup_w - m.pad * 2.0;
            let over = "  ".to_string() + &"W".repeat(help::WRAP_COLUMNS * 2);
            assert!(
                p.measure_ui_advance(&over, m.font_size) > room,
                "twice the column budget still fits, so the width test proves nothing"
            );
        });
    }

    /// Every shipped link row, marked, and the longest footer must fit the
    /// popup: `draw_row` clips vertically only.
    #[test]
    fn the_link_rows_and_footers_fit_the_help_popup() {
        use super::{FOOTER_ON_LINKS, FOOTER_SCROLLING, FOOTER_WITH_LINKS};
        let db = help_db();
        let mut lines: Vec<String> = [FOOTER_ON_LINKS, FOOTER_SCROLLING, FOOTER_WITH_LINKS]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for page in db.pages() {
            lines.extend(page.links.iter().map(|l| format!("> {}", l.label)));
        }
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = crate::render::popup::popup_body_width(
                1440.0,
                crate::render::popup::PopupSize::Large,
                &m,
            );
            for line in &lines {
                let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                assert!(drawn <= room, "overflows by {:.0}px: {line}", drawn - room);
            }
        });
    }
}
