//! What the base's machines and depots hold, reduced to piles that fit on
//! one row.
//!
//! `Game::base_stock` decides what the piles are and what order they come
//! in; everything here is about fitting them on one row. The row is the
//! whole design constraint — a base with a dozen chains running holds more
//! kinds of thing than any single line can name, so the piles that fit are
//! drawn and the rest are counted. `hud::status_bar` is the one caller now
//! — it used to be a strip of its own across the top of the window, before
//! the status bar absorbed it as its centre zone.

use feral_processes_engine::StockRow;

use crate::paint::Painter;
use crate::text::Metrics;

/// Between one pile and the next.
const PAIR_GAP: &str = "   ";

/// One pile as it is written: the tag, then the quantity.
///
/// Two pieces rather than one string because they are drawn in different
/// colours — the tag is chrome the eye skips once it has learnt the row,
/// and the number is the thing being read.
pub(super) fn pieces(rows: &[StockRow]) -> Vec<(String, String)> {
    rows.iter()
        .map(|r| (format!("[{}]", r.tag), format!(" {}", r.qty)))
        .collect()
}

/// The plain text of the first `shown` piles, with a `+N` tail when the
/// rest did not fit. What gets measured, and what the runs below spell.
pub(super) fn line(pieces: &[(String, String)], shown: usize) -> String {
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
pub(super) fn fits(
    pieces: &[(String, String)],
    avail: f32,
    painter: &Painter,
    m: &Metrics,
) -> usize {
    let mut shown = 0;
    for take in 1..=pieces.len() {
        if painter.measure_ui_advance(line(pieces, take), m.font_size) > avail {
            break;
        }
        shown = take;
    }
    shown
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
}
