//! The stock block: what the base is holding, drawn down the map's top-left
//! corner.
//!
//! **A block inside the pane, for `compass_block`'s reason.** It overlays
//! tiles that are still drawn, so the layout does not move and the viewport
//! never re-lays. It starts `layout::strip_inset` below the pane's top edge
//! because the THREAT readout rides that border and its quad hangs down
//! into the pane.
//!
//! `Game::base_stock` decides what the rows are and what order they come
//! in; everything here is about fitting them into a corner. The block has
//! two budgets and both are measured, never estimated from a character
//! count — the UI font is proportional, and `Painter` clips vertically and
//! never horizontally, so an over-wide name is drawn across the map in
//! silence. A name wider than its column is cut with an ellipsis, and the
//! rows past the height budget are **counted**, not dropped.
//!
//! Drawn over the surface map and base space alone. The Stack's frame map
//! owns this corner underground, and a tactical board is the fight.

use feral_processes_engine::StockRow;

use super::layout::strip_inset;
use super::palette;
use crate::paint::{Painter, Rect};
use crate::render::PANEL_BG;
use crate::text::Metrics;

/// Breathing space inside the block's own border, as a fraction of the UI
/// line height — `compass_block`'s figure, so the two corners match.
const PAD_RATIO: f32 = 0.35;
/// The widest a name may be drawn, in advances of `M` at the block's size.
const NAME_COLUMN_CHARS: f32 = 15.0;
/// The share of the pane's height the block may cover.
const HEIGHT_FRACTION: f32 = 0.5;
const HEADER: &str = "BASE STOCK";
const ELLIPSIS: char = '…';

/// `name` as it fits in `max_w`: whole when it does, otherwise cut back to
/// the longest prefix that still fits with an ellipsis on the end.
fn fit_name(name: &str, max_w: f32, size: u16, painter: &Painter) -> String {
    if painter.measure_ui_advance(name, size) <= max_w {
        return name.to_string();
    }
    let mut cut: String = name.to_string();
    while cut.pop().is_some() {
        let candidate = format!("{}{ELLIPSIS}", cut.trim_end());
        if painter.measure_ui_advance(&candidate, size) <= max_w {
            return candidate;
        }
    }
    ELLIPSIS.to_string()
}

/// Draws the block into `pane`'s top-left corner and returns the box it
/// filled, so a test can ask what it covered without re-deriving the
/// geometry.
///
/// `None` for an empty base — an empty corner is the readout saying
/// "nothing" — and when the pane has no room for a header and one row.
pub(in crate::render) fn draw_stock_block(
    pane: Rect,
    rows: &[StockRow],
    painter: &Painter,
    m: &Metrics,
) -> Option<Rect> {
    if rows.is_empty() {
        return None;
    }
    let pad = m.line_height * PAD_RATIO;
    let size = m.small();
    let x = pane.x + m.inset;
    let y = pane.y + strip_inset(m);

    // Lines the height budget holds, the header included. Past it the last
    // line is spent on the count, so every row is either named or counted.
    let budget = (pane.h * HEIGHT_FRACTION).min(pane.y + pane.h - y);
    let lines = ((budget - pad * 2.0) / m.line_height).floor().max(0.0) as usize;
    if lines < 2 {
        return None;
    }
    let (shown, tail) = if rows.len() < lines {
        (rows.len(), None)
    } else {
        let shown = lines - 2;
        (shown, Some(format!("+{} more", rows.len() - shown)))
    };

    let max_name_w = painter.measure_ui_advance("M", size) * NAME_COLUMN_CHARS;
    let named: Vec<(String, String)> = rows[..shown]
        .iter()
        .map(|r| {
            (
                fit_name(&r.name, max_name_w, size, painter),
                r.qty.to_string(),
            )
        })
        .collect();
    let widest = |texts: &mut dyn Iterator<Item = &str>| {
        texts
            .map(|t| painter.measure_ui_advance(t, size))
            .fold(0.0, f32::max)
    };
    let name_w = widest(&mut named.iter().map(|(n, _)| n.as_str()));
    let qty_w = widest(&mut named.iter().map(|(_, q)| q.as_str()));
    let gap = painter.measure_ui_advance("  ", size);
    let own_line_w = widest(&mut [HEADER].into_iter().chain(tail.as_deref()));
    let w = pad * 2.0 + (name_w + gap + qty_w).max(own_line_w);
    let h = pad * 2.0 + m.line_height * (1 + shown + usize::from(tail.is_some())) as f32;
    if x + w > pane.x + pane.w - m.inset {
        return None;
    }

    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);

    let baseline = |line: usize| y + pad + m.line_height * line as f32 + size as f32 * 0.8;
    painter.ui(HEADER, x + pad, baseline(0), size, palette::FIELD_LABEL);
    // Amounts right-aligned on one edge, so a column of figures reads down
    // by magnitude the way a ledger does.
    let qty_right = x + w - pad;
    for (i, (name, qty)) in named.iter().enumerate() {
        painter.ui(name, x + pad, baseline(i + 1), size, palette::BODY);
        let qty_x = qty_right - painter.measure_ui_advance(qty, size);
        painter.ui(qty, qty_x, baseline(i + 1), size, palette::EMPHASIS);
    }
    if let Some(tail) = &tail {
        painter.ui(tail, x + pad, baseline(shown + 1), size, palette::FAINT);
    }
    Some(Rect::new(x, y, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, painted_text_boxes, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::items::ItemId;

    fn stock(piles: &[(&str, u32)]) -> Vec<StockRow> {
        piles
            .iter()
            .map(|(name, qty)| StockRow {
                item: ItemId::from(*name),
                tag: "XX".to_string(),
                name: name.to_string(),
                qty: *qty,
            })
            .collect()
    }

    fn pane() -> Rect {
        Rect::new(0.0, 40.0, 1000.0, 600.0)
    }

    #[test]
    fn a_short_name_is_drawn_whole() {
        let m = ui_metrics(900.0);
        with_painter(|p| {
            let max_w = p.measure_ui_advance("M", m.small()) * NAME_COLUMN_CHARS;
            assert_eq!(fit_name("Cache Grain", max_w, m.small(), p), "Cache Grain");
        });
    }

    /// Both halves: the cut name ends in the ellipsis, and it is measured to
    /// fit — a character-count cut passes the first and fails the second on
    /// a row of wide glyphs.
    #[test]
    fn a_long_name_is_cut_to_its_column_with_an_ellipsis() {
        let m = ui_metrics(900.0);
        let long = "An Interminably Named Salvaged Logic Substrate";
        with_painter(|p| {
            let max_w = p.measure_ui_advance("M", m.small()) * NAME_COLUMN_CHARS;
            assert!(
                p.measure_ui_advance(long, m.small()) > max_w,
                "the fixture must overflow the column or it proves nothing"
            );
            let cut = fit_name(long, max_w, m.small(), p);
            assert!(cut.ends_with(ELLIPSIS), "no ellipsis on {cut:?}");
            assert!(
                cut.starts_with("An Intermin"),
                "cut from the wrong end: {cut:?}"
            );
            assert!(
                p.measure_ui_advance(&cut, m.small()) <= max_w,
                "{cut:?} is wider than its column"
            );
        });
    }

    /// Full names and their amounts, one row each, in the engine's order.
    #[test]
    fn each_pile_is_its_full_name_and_its_amount() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("Bytecode Block", 140), ("Cache Grain", 12)]);
        let (_, shapes) = with_painter(|p| draw_stock_block(pane(), &rows, p, &m));
        let text = painted_text(&shapes);
        let at = |s: &str| {
            text.iter()
                .position(|t| t == s)
                .unwrap_or_else(|| panic!("{s:?} not painted: {text:?}"))
        };
        assert!(at(HEADER) < at("Bytecode Block"));
        assert!(at("Bytecode Block") < at("140"));
        assert!(at("140") < at("Cache Grain"));
        assert!(at("Cache Grain") < at("12"));
    }

    #[test]
    fn an_empty_base_draws_nothing() {
        let m = ui_metrics(900.0);
        let (filled, shapes) = with_painter(|p| draw_stock_block(pane(), &[], p, &m));
        assert_eq!(filled, None);
        assert!(
            shapes.is_empty(),
            "an empty base painted {} shapes",
            shapes.len()
        );
    }

    /// Far more piles than the shipped set holds, each with a name wider
    /// than its column and an awkward amount: the block stays inside its
    /// corner and says how many rows it could not name.
    #[test]
    fn a_crowded_base_stays_in_its_corner_and_counts_the_rest() {
        let m = ui_metrics(900.0);
        let names: Vec<String> = (0..60)
            .map(|i| format!("Interminably Named Material Number {i}"))
            .collect();
        let rows = stock(
            &names
                .iter()
                .map(|n| (n.as_str(), 999_999))
                .collect::<Vec<_>>(),
        );
        let (filled, shapes) = with_painter(|p| draw_stock_block(pane(), &rows, p, &m));
        let filled = filled.expect("a tall pane has room for the block");
        let pane = pane();
        assert!(
            filled.x >= pane.x && filled.y >= pane.y,
            "{filled:?} left the pane"
        );
        assert!(
            filled.w <= pane.w / 2.0,
            "the block took {} of a {}px pane",
            filled.w,
            pane.w
        );
        assert!(
            filled.h <= pane.h * HEIGHT_FRACTION + 0.001,
            "the block is {}px tall against a {}px budget",
            filled.h,
            pane.h * HEIGHT_FRACTION
        );
        for (_, text, ink) in painted_text_boxes(&shapes) {
            assert!(
                ink.x >= filled.x
                    && ink.x + ink.w <= filled.x + filled.w
                    && ink.y >= filled.y
                    && ink.y + ink.h <= filled.y + filled.h,
                "{text:?} at {ink:?} drew outside the block {filled:?}"
            );
        }

        let text = painted_text(&shapes);
        let named = text.iter().filter(|t| t.ends_with(ELLIPSIS)).count();
        assert!(named > 0, "no row was drawn at all");
        assert_eq!(
            text.last().map(String::as_str),
            Some(format!("+{} more", rows.len() - named).as_str()),
            "the tail must count exactly the rows that were not named"
        );
    }

    /// A pane too short for a header and one row draws nothing rather than a
    /// box hanging off its bottom edge.
    #[test]
    fn a_pane_with_no_room_draws_nothing() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("Cache Grain", 12)]);
        let squat = Rect::new(0.0, 0.0, 1000.0, m.line_height * 2.0);
        let (filled, _) = with_painter(|p| draw_stock_block(squat, &rows, p, &m));
        assert_eq!(filled, None);
    }
}
