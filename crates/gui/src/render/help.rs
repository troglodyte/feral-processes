//! The manual: the index of topics, and one page of it.
//!
//! Two screens rather than one because they answer to different idioms —
//! the index is an ordinary menu and a page is a document. `App` owns that
//! split and both row counts; this only draws what it is handed.

use super::popup::{colored_item_row, item_row};
use super::*;

/// The topic list. An ordinary numbered menu, so it draws exactly like
/// every other one.
pub(super) fn draw_help_index(app: &App, painter: &Painter, m: &Metrics) {
    let rows = app.help_index_rows();
    let mut out = vec![text_row("The manual. Pick a topic."), text_row("")];
    if rows.is_empty() {
        out.push(text_row("No help pages are installed (assets/help/)."));
    }
    for (i, row) in rows.iter().enumerate() {
        out.push(item_row(
            format!("[{}] {}", menu_shortcut(i), row.title),
            i == app.menu_selected,
        ));
    }
    out.push(text_row(""));
    out.push(text_row(
        "Up/Down + Enter or a row's key to read, Esc to close.",
    ));
    draw_popup("Help", PopupSize::Large, &out, painter, m);
}

/// One page. The prose is the scrollable body — every row an `Item` so
/// `popup_layout` scrolls it, with the highlight standing for the scroll
/// position exactly as the history screen's does. Further reading is pinned
/// below it, typed rather than selected.
pub(super) fn draw_help_page(app: &App, painter: &Painter, m: &Metrics) {
    let Some(view) = app.help_page_view() else {
        // The trail points at a page that is not loaded, which a modder
        // deleting a file mid-session can do. The index is still there.
        draw_help_index(app, painter, m);
        return;
    };
    let mut rows: Vec<Row> = view
        .prose
        .iter()
        .enumerate()
        .map(|(i, line)| colored_item_row(line, i == app.menu_selected, TEXT))
        .collect();
    if !view.links.is_empty() {
        rows.push(text_row(""));
        rows.push(text_row("Read on:"));
        for link in &view.links {
            rows.push(text_row(format!("  [{}] {}", link.shortcut, link.label)));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Up/Down to scroll, a topic's key to read on, Esc to go back.",
    ));
    draw_popup(&view.title, PopupSize::Large, &rows, painter, m);
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
}
