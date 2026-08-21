//! The base stock strip: one row across the top of every screen that draws
//! the world behind it, saying what the base's machines and depots hold.
//!
//! `Game::base_stock` decides what the piles are and what order they come
//! in; everything here is about fitting them on one row. The row is the
//! whole design constraint — a base with a dozen chains running holds more
//! kinds of thing than any single line can name, so the piles that fit are
//! drawn and the rest are counted.

use feral_processes_engine::StockRow;

use super::{BORDER, PANEL_BG, TEXT, TEXT_DIM};
use crate::paint::{Painter, TextRun};
use crate::text::Metrics;

/// Between one pile and the next.
const PAIR_GAP: &str = "   ";

/// The height the strip claims off the top of the window.
///
/// One row plus the inset above and below it, so the panes below start
/// clear of it. Kept below the band `draw_popup` leaves free at the top of
/// the screen — a popup is capped at 85% of the window height and centred,
/// so the strip stays readable with a menu open, which is most of why it
/// sits at the top rather than in the log pane.
pub(super) fn strip_height(m: &Metrics) -> f32 {
    m.line_height + m.inset
}

/// One pile as it is written: the tag, then the quantity.
///
/// Two pieces rather than one string because they are drawn in different
/// colours — the tag is chrome the eye skips once it has learnt the row,
/// and the number is the thing being read.
fn pieces(rows: &[StockRow]) -> Vec<(String, String)> {
    rows.iter()
        .map(|r| (format!("[{}]", r.tag), format!(" {}", r.qty)))
        .collect()
}

/// The plain text of the first `shown` piles, with a `+N` tail when the
/// rest did not fit. What gets measured, and what the runs below spell.
fn line(pieces: &[(String, String)], shown: usize) -> String {
    let mut out = String::new();
    for (tag, qty) in pieces.iter().take(shown) {
        if !out.is_empty() {
            out.push_str(PAIR_GAP);
        }
        out.push_str(tag);
        out.push_str(qty);
    }
    let dropped = pieces.len() - shown;
    if dropped > 0 {
        if !out.is_empty() {
            out.push_str(PAIR_GAP);
        }
        out.push_str(&format!("+{dropped}"));
    }
    out
}

/// How many piles fit in `avail`, counting the `+N` tail the ones left over
/// will need.
///
/// Measured rather than estimated from a character count: the UI font is
/// proportional, so a row of narrow tags fits piles a row of wide ones does
/// not. The status column seam is the warning here — a row too wide is not
/// clipped, it is simply drawn off the end of the panel in silence.
fn fits(pieces: &[(String, String)], avail: f32, painter: &Painter, m: &Metrics) -> usize {
    let mut shown = 0;
    for take in 1..=pieces.len() {
        if painter.measure_ui_advance(line(pieces, take), m.font_size) > avail {
            break;
        }
        shown = take;
    }
    shown
}

/// Draws the strip across the top of the window.
pub(super) fn draw_stock_strip(rows: &[StockRow], painter: &Painter, m: &Metrics) {
    let h = strip_height(m);
    let w = painter.screen_w();
    painter.rect(0.0, 0.0, w, h, PANEL_BG);
    painter.line(0.0, h, w, h, 2.0, BORDER);

    let baseline = m.inset + m.font_size as f32 / 2.0;
    if rows.is_empty() {
        painter.ui("Base stock: none", m.inset, baseline, m.font_size, TEXT_DIM);
        return;
    }

    let pieces = pieces(rows);
    let avail = w - m.inset * 2.0;
    let shown = fits(&pieces, avail, painter, m);

    let mut runs: Vec<TextRun> = Vec::new();
    let gap = PAIR_GAP.to_string();
    let tail;
    for (i, (tag, qty)) in pieces.iter().take(shown).enumerate() {
        if i > 0 {
            runs.push(TextRun {
                text: &gap,
                bold: false,
                color: TEXT_DIM,
            });
        }
        runs.push(TextRun {
            text: tag,
            bold: false,
            color: TEXT_DIM,
        });
        runs.push(TextRun {
            text: qty,
            bold: false,
            color: TEXT,
        });
    }
    let dropped = pieces.len() - shown;
    if dropped > 0 {
        tail = format!("{PAIR_GAP}+{dropped}");
        runs.push(TextRun {
            text: &tail,
            bold: false,
            color: TEXT_DIM,
        });
    }
    painter.ui_runs(&runs, m.inset, baseline, m.font_size);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::items::ItemId;

    fn stock(piles: &[(&str, u32)]) -> Vec<StockRow> {
        piles
            .iter()
            .map(|(tag, qty)| StockRow {
                item: ItemId::from(*tag),
                tag: tag.to_string(),
                name: tag.to_string(),
                qty: *qty,
            })
            .collect()
    }

    /// The one thing the strip must never do. It is a single row with no
    /// wrap and no clip, so a line wider than the window is not cut off —
    /// it is drawn past the edge, and the piles at the end simply are not
    /// there. Asserted against far more piles than the shipped set holds,
    /// each with a quantity wide enough to be awkward.
    #[test]
    fn the_row_never_draws_wider_than_the_window() {
        let m = ui_metrics(900.0);
        let piles: Vec<(String, u32)> = (0..60)
            .map(|i| (format!("W{i}"), 999_999 - i as u32))
            .collect();
        let rows = stock(
            &piles
                .iter()
                .map(|(t, q)| (t.as_str(), *q))
                .collect::<Vec<_>>(),
        );
        with_painter(|p| {
            let pieces = pieces(&rows);
            let avail = p.screen_w() - m.inset * 2.0;
            let shown = fits(&pieces, avail, p, &m);
            assert!(shown > 0, "something has to fit");
            assert!(shown < rows.len(), "precondition: 60 piles cannot all fit");
            assert!(
                p.measure_ui_advance(line(&pieces, shown), m.font_size) <= avail,
                "the drawn row overflows the window"
            );
        });
    }

    /// Every shipped material at once still leaves the row honest: what is
    /// dropped is counted, so the player can see the readout is partial
    /// rather than believing the base holds only what is named.
    #[test]
    fn the_piles_that_do_not_fit_are_counted_rather_than_dropped_in_silence() {
        let m = ui_metrics(900.0);
        let piles: Vec<(String, u32)> = (0..60).map(|i| (format!("W{i}"), 4321)).collect();
        let rows = stock(
            &piles
                .iter()
                .map(|(t, q)| (t.as_str(), *q))
                .collect::<Vec<_>>(),
        );
        with_painter(|p| {
            let pieces = pieces(&rows);
            let shown = fits(&pieces, p.screen_w() - m.inset * 2.0, p, &m);
            assert!(
                line(&pieces, shown).ends_with(&format!("+{}", rows.len() - shown)),
                "the tail must say how many piles are not named"
            );
        });
    }

    /// A base that holds nothing says so rather than drawing an empty row,
    /// which would read as the strip being broken. The row is present
    /// either way: it is the same height whatever the base holds, so the
    /// map below it does not jump a line as buffers fill and drain.
    #[test]
    fn a_base_holding_nothing_says_so_and_still_claims_its_row() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| draw_stock_strip(&[], p, &m));
        assert_eq!(
            crate::paint::painted_text(&shapes),
            vec!["Base stock: none"],
        );
        assert!(strip_height(&m) > 0.0);
    }

    /// The tag is chrome and the number is the reading, so they are painted
    /// in different colours — the whole point of a glanceable row is that
    /// the eye lands on the quantities.
    #[test]
    fn the_quantities_are_painted_brighter_than_the_tags_they_follow() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("CF", 142), ("BB", 31)]);
        let (_, shapes) = with_painter(|p| draw_stock_strip(&rows, p, &m));
        assert_eq!(
            crate::paint::painted_runs_in(&shapes, TEXT, false),
            vec![" 142", " 31"],
        );
        assert_eq!(
            crate::paint::painted_runs_in(&shapes, TEXT_DIM, false),
            vec!["[CF]", "   [BB]"],
            "adjacent runs of one style are merged before they are painted"
        );
    }

    /// The strip's border is a line at its own bottom edge, so a row that
    /// does not fit inside the band is drawn straight through it — the same
    /// silent overflow the width is measured to avoid, on the other axis.
    /// Measured with the real font rather than assumed off the ratio.
    #[test]
    fn the_row_fits_between_the_strips_top_inset_and_its_border() {
        for window_h in [600.0, 900.0, 1080.0, 1440.0, 2160.0] {
            let m = ui_metrics(window_h);
            with_painter(|p| {
                let line = p.measure_ui("[CF] 142", m.font_size).height;
                assert!(
                    m.inset + line <= strip_height(&m),
                    "at {window_h}px a {line}px row does not fit a {}px strip",
                    strip_height(&m)
                );
            });
        }
    }

    /// The strip has to stay inside the band `draw_popup` leaves clear at
    /// the top of the window, or it is buried by the first menu the player
    /// opens — which is most of the screens it exists to be visible on.
    #[test]
    fn the_strip_clears_the_band_a_popup_leaves_free() {
        for window_h in [600.0, 900.0, 1080.0, 1440.0, 2160.0] {
            let m = ui_metrics(window_h);
            // `draw_popup` caps a panel at 85% of the window and centres it.
            let clear = window_h * (1.0 - 0.85) / 2.0;
            assert!(
                strip_height(&m) <= clear,
                "at {window_h}px the strip is {} tall against {clear}px of clear band",
                strip_height(&m)
            );
        }
    }
}
