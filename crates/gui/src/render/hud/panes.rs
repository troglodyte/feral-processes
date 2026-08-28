//! What the info column's open pane draws: the BASE, CREW and PACK bodies.
//!
//! The column **does not scroll**, the same as the gear inspect and memories
//! pages, so the body rect's height is a layout constraint and not a starting
//! point. That is the whole reason this module builds rows before it draws
//! any: [`fitting_rows`] is `strip::fitting`'s rule turned ninety degrees —
//! what does not fit is **counted**, never drawn past the bottom edge in
//! silence — and it is written once here rather than three times, once per
//! pane, where it would be three sites agreeing rather than one fact.
//!
//! The three builders are pure functions of view data. They take no
//! `Painter`, which is what lets the census assert on the rows a pane *would*
//! draw at a given window size without standing a renderer up — the property
//! `hud::layout` already relies on one scale up.
//!
//! See `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md`.

use super::palette;
use super::strip::Piece;
use crate::paint::{Painter, Rect, TextRun};
use crate::text::Metrics;

/// One line of a pane body.
///
/// Exhaustive rather than a struct with an `is_rule` flag, `cell_mark`'s
/// rule: [`draw_rows`] matches on it, so a fourth kind fails to compile
/// instead of drawing as a blank line nobody notices.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::render) enum Row {
    /// Text, with an optional right-aligned tail. **The tail is where a
    /// keycap chip goes** — the handoff's rule is that a row the player can
    /// act on carries its key at the right end of that same row, never on a
    /// line of its own, so the key is never separated from the thing it acts
    /// on.
    Text { left: Vec<Piece>, right: Vec<Piece> },
    /// A hairline across the body, between two blocks.
    Rule,
}

pub(in crate::render) fn text(left: Vec<Piece>) -> Row {
    Row::Text {
        left,
        right: Vec::new(),
    }
}

pub(in crate::render) fn with_tail(left: Vec<Piece>, right: Vec<Piece>) -> Row {
    Row::Text { left, right }
}

/// A block's sub-head — `PRODUCTION`, `DEFENCE`. Its own row so the blocks
/// read as blocks and not as one run of rows.
pub(in crate::render) fn subhead(name: &str) -> Row {
    text(vec![(name.to_string(), palette::PANE_TITLE, true)])
}

/// A keycap chip: the key, then what it opens.
pub(in crate::render) fn chip(key: char, verb: &str) -> Vec<Piece> {
    vec![
        (format!("{key}"), palette::EMPHASIS, true),
        (format!(" {verb}"), palette::LABEL, false),
    ]
}

/// How tall one row draws.
fn row_height(row: &Row, m: &Metrics) -> f32 {
    match row {
        Row::Text { .. } => m.line_height,
        Row::Rule => m.gap,
    }
}

/// The longest prefix of `rows` that fits `avail`, and how many were cut.
///
/// **`stock::fits`' rule on the vertical axis.** A pane has no scrollbar to
/// defer a row to, so the overflow is reported rather than dropped — the
/// caller spends a row saying how many did not fit, which is what stops the
/// column lying about what the base is doing. A trailing [`Row::Rule`] is
/// dropped rather than counted: a divider with nothing under it is not
/// information the player lost.
///
/// Reserves room for the overflow row itself whenever it will be needed, or
/// the count would be the thing that overflows.
pub(in crate::render) fn fitting_rows(rows: &[Row], avail: f32, m: &Metrics) -> (Vec<Row>, usize) {
    let total: f32 = rows.iter().map(|r| row_height(r, m)).sum();
    if total <= avail {
        return (rows.to_vec(), 0);
    }
    // One row of the budget belongs to the "+N more" line, which only exists
    // in this branch — measured against the full list above, a pane that
    // fits exactly would be cut by the space reserved to say so.
    let budget = avail - m.line_height;
    let mut used = 0.0;
    let mut taken = 0;
    for row in rows {
        let h = row_height(row, m);
        if used + h > budget {
            break;
        }
        used += h;
        taken += 1;
    }
    let mut shown = rows[..taken].to_vec();
    while matches!(shown.last(), Some(Row::Rule)) {
        shown.pop();
    }
    (shown, rows.len() - taken)
}

/// Draws `rows` down `at`, and says so when any were cut.
///
/// The overflow line is drawn in [`palette::ATTENTION`] — it is the player
/// being told the pane is not showing them everything, which is a thing they
/// can act on by opening the full screen the chip names.
pub(in crate::render) fn draw_rows(
    at: Rect,
    rows: &[Row],
    cut: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let size = m.font_size;
    let mut cy = at.y + m.inset + size as f32 / 2.0;
    for row in rows {
        match row {
            Row::Text { left, right } => {
                draw_line(at, cy, left, right, painter, m);
                cy += m.line_height;
            }
            Row::Rule => {
                let y = cy - m.line_height / 2.0 + m.gap / 2.0;
                painter.line(
                    at.x + m.inset,
                    y,
                    at.x + at.w - m.inset,
                    y,
                    1.0,
                    palette::DIVIDER,
                );
                cy += m.gap;
            }
        }
    }
    if cut > 0 {
        let more = vec![(format!("+{cut} more"), palette::ATTENTION, false)];
        draw_line(at, cy, &more, &[], painter, m);
    }
}

fn draw_line(at: Rect, cy: f32, left: &[Piece], right: &[Piece], painter: &Painter, m: &Metrics) {
    let size = m.font_size;
    if !left.is_empty() {
        let runs: Vec<TextRun> = left
            .iter()
            .map(|(t, c, b)| TextRun {
                text: t,
                bold: *b,
                color: *c,
            })
            .collect();
        painter.ui_runs(&runs, at.x + m.inset, cy, size);
    }
    if !right.is_empty() {
        let tail: String = right.iter().map(|(t, _, _)| t.as_str()).collect();
        let w = painter.measure_ui_advance(&tail, size);
        let runs: Vec<TextRun> = right
            .iter()
            .map(|(t, c, b)| TextRun {
                text: t,
                bold: *b,
                color: *c,
            })
            .collect();
        painter.ui_runs(&runs, at.x + at.w - m.inset - w, cy, size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;

    fn line(n: usize) -> Row {
        text(vec![(format!("row {n}"), palette::BODY, false)])
    }

    fn lines(n: usize) -> Vec<Row> {
        (0..n).map(line).collect()
    }

    /// A pane inside its budget is drawn whole and reports nothing.
    #[test]
    fn a_pane_that_fits_is_not_cut() {
        let m = ui_metrics(720.0);
        let rows = lines(5);
        let (shown, cut) = fitting_rows(&rows, m.line_height * 5.0, &m);
        assert_eq!(cut, 0, "a pane that fits exactly was cut");
        assert_eq!(shown.len(), 5);
    }

    /// The column has no scrollbar, so the overflow is a number the player is
    /// told and never a row that silently is not there.
    #[test]
    fn an_overflowing_pane_counts_what_it_dropped() {
        let m = ui_metrics(720.0);
        let rows = lines(20);
        let (shown, cut) = fitting_rows(&rows, m.line_height * 10.0, &m);
        assert!(cut > 0, "twenty rows in ten rows of room were not counted");
        assert_eq!(shown.len() + cut, rows.len(), "rows vanished uncounted");
    }

    /// **The trap this reserve exists for.** The `+N more` line is itself a
    /// row, so a fitter that fills the budget to the brim draws its own
    /// overflow notice past the bottom edge — the exact silence the count was
    /// added to break. Delete the `- m.line_height` in `fitting_rows` and
    /// this fails.
    #[test]
    fn the_overflow_line_has_room_to_be_drawn() {
        let m = ui_metrics(720.0);
        for budget in 2..14 {
            let avail = m.line_height * budget as f32;
            let rows = lines(40);
            let (shown, cut) = fitting_rows(&rows, avail, &m);
            assert!(cut > 0, "the fixture must overflow at {budget}");
            let drawn: f32 = shown.iter().map(|r| row_height(r, &m)).sum();
            assert!(
                drawn + m.line_height <= avail,
                "at {budget} rows the pane drew {drawn:.1} and the +N more line \
                 needs {:.1} more of {avail:.1}",
                m.line_height
            );
        }
    }

    /// A divider with nothing under it is not information the player lost, so
    /// it comes off the end rather than being reported as a dropped row.
    #[test]
    fn a_trailing_rule_is_dropped_not_counted() {
        let m = ui_metrics(720.0);
        let mut rows = lines(3);
        rows.push(Row::Rule);
        rows.extend(lines(30));
        let (shown, _) = fitting_rows(&rows, m.line_height * 6.0, &m);
        assert!(
            !matches!(shown.last(), Some(Row::Rule)),
            "a divider was left hanging off the end of the pane"
        );
    }

    /// The count is not merely returned — it reaches the screen.
    #[test]
    fn an_overflowing_pane_says_so_on_screen() {
        let m = ui_metrics(720.0);
        let rows = lines(40);
        let at = Rect::new(900.0, 40.0, 400.0, m.line_height * 8.0);
        let (shown, cut) = fitting_rows(&rows, at.h, &m);
        let (_, shapes) = with_painter(|p| draw_rows(at, &shown, cut, p, &m));
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains(&format!("+{cut} more")),
            "the pane dropped {cut} rows without saying so: {text:?}"
        );
    }

    /// A chip rides the right end of its own row, never a line of its own —
    /// the handoff's rule that the key is never separated from the thing it
    /// acts on.
    #[test]
    fn a_tail_is_right_aligned_on_its_own_row() {
        let m = ui_metrics(720.0);
        let at = Rect::new(0.0, 0.0, 400.0, 200.0);
        let rows = [with_tail(
            vec![("4 nodes idle".to_string(), palette::BODY, false)],
            chip('b', "base"),
        )];
        let (_, shapes) = with_painter(|p| draw_rows(at, &rows, 0, p, &m));
        let ys: Vec<f32> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                bevy_egui::egui::epaint::Shape::Text(t) => Some(t.pos.y),
                _ => None,
            })
            .collect();
        assert!(ys.len() >= 2, "the row drew fewer than two pieces: {ys:?}");
        assert!(
            ys.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01),
            "the tail fell onto its own line: {ys:?}"
        );
    }
}
