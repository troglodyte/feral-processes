//! The stock block: what the base is researching and holding, drawn down
//! the map's top-left corner.
//!
//! **A block inside the pane, for `compass_block`'s reason.** It overlays
//! tiles that are still drawn, so the layout does not move and the viewport
//! never re-lays. It starts `layout::strip_inset` below the pane's top edge
//! because the THREAT readout rides that border and its quad hangs down
//! into the pane.
//!
//! `Game::research_readout`, `Game::base_stock` and `Game::downed_store`
//! decide what the lines say; everything here is about fitting them into a
//! corner. **One box, three sections**, research over stock over the downed
//! store, sharing one border, one width and one height budget — three boxes
//! would each need their own. The block has two
//! budgets and both are measured, never estimated from a character count —
//! the UI font is proportional, and `Painter` clips vertically and never
//! horizontally, so an over-wide name is drawn across the map in silence. A
//! name wider than its column is cut with an ellipsis, and the stock rows
//! past the height budget are **counted**, not dropped. The research and
//! downed sections are never cut short: each is at most three lines, and
//! their height comes out of the budget **before** the stock rows are
//! allotted what is left. Appended last and left to compete, the downed
//! readout is the section a well-stocked base silently deletes.
//!
//! **Every section says nothing when it has nothing to say**, which is the
//! rule that keeps the block honest about a corner it is covering map with:
//! no stock, no `BASE STOCK` header; nothing held, no downed section; and
//! all three empty at once is the `None` that draws no box at all. A store
//! reading `0/10` is not news — the section appears the moment the first
//! body goes into it, which is also when the cap starts to matter.
//!
//! Drawn over the surface map and base space alone. The Stack's frame map
//! owns this corner underground, and a tactical board is the fight.

use feral_processes_engine::{ResearchReadout, StockRow};

use super::layout::strip_inset;
use super::palette;
use crate::paint::{Color, Painter, Rect};
use crate::render::PANEL_BG;
use crate::text::Metrics;

/// Breathing space inside the block's own border, as a fraction of the UI
/// line height — `compass_block`'s figure, so the two corners match.
const PAD_RATIO: f32 = 0.35;
/// The widest a name may be drawn, in advances of `M` at the block's size.
const NAME_COLUMN_CHARS: f32 = 15.0;
/// The widest a line with no figure beside it may be drawn: a name column
/// plus the gap and figure it does not have. "needs " spends six of a
/// name column's cells before the material is named at all.
const NOTE_COLUMN_CHARS: f32 = 22.0;
/// The share of the pane's height the block may cover.
const HEIGHT_FRACTION: f32 = 0.5;
const HEADER: &str = "BASE STOCK";
const RESEARCH_HEADER: &str = "RESEARCHING";
const DOWNED_HEADER: &str = "DOWNED PROGRAMS";
const DOWNED_LABEL: &str = "held";
const IDLE: &str = "none";
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

/// One line of the block. A `Pair` is a name with its figure right-aligned
/// on the block's edge; everything else is one piece of text on its own.
///
/// A `Pair` carries its figure's ink because one of the three sections has
/// a state the figure itself is the only evidence of — a full downed store
/// refuses the next body, and `10/10` drawn in the same colour as `3/10`
/// is the readout hiding the one number worth reading.
enum Line {
    Text(String, Color),
    Pair(String, String, Color),
}

/// The research section's lines, names already cut to their column.
fn research_lines(readout: &ResearchReadout, max_name_w: f32, size: u16, p: &Painter) -> Vec<Line> {
    let header = Line::Text(RESEARCH_HEADER.to_string(), palette::FIELD_LABEL);
    let fit = |name: &str| fit_name(name, max_name_w, size, p);
    let note_w = p.measure_ui_advance("M", size) * NOTE_COLUMN_CHARS;
    match readout {
        ResearchReadout::Idle => vec![header, Line::Text(IDLE.to_string(), palette::FAINT)],
        ResearchReadout::Earning { name, earned, cost } => {
            vec![
                header,
                Line::Pair(fit(name), format!("{earned}/{cost}"), palette::EMPHASIS),
            ]
        }
        // `ATTENTION`, the machine-stall rule: waiting will not pay the bill.
        ResearchReadout::Stalled { name, short_of } => vec![
            header,
            Line::Text(fit(name), palette::BODY),
            Line::Text(
                fit_name(&format!("needs {short_of}"), note_w, size, p),
                palette::ATTENTION,
            ),
        ],
    }
}

/// The downed store's two lines, or none at all when nothing is held.
///
/// `ATTENTION` on a full store, the machine-stall rule: waiting does not
/// empty it, and what a full store costs is the next body the world tries
/// to hand over — a loss with nothing on screen to show for it unless this
/// figure says so.
fn downed_lines(held: usize, cap: usize) -> Vec<Line> {
    if held == 0 {
        return Vec::new();
    }
    let ink = if held >= cap {
        palette::ATTENTION
    } else {
        palette::EMPHASIS
    };
    vec![
        Line::Text(DOWNED_HEADER.to_string(), palette::FIELD_LABEL),
        Line::Pair(DOWNED_LABEL.to_string(), format!("{held}/{cap}"), ink),
    ]
}

/// Draws the block into `pane`'s top-left corner and returns the box it
/// filled, so a test can ask what it covered without re-deriving the
/// geometry.
///
/// `downed` is `Game::downed_store` — held against the cap.
///
/// `None` when there is nothing to say — no research readout, an empty base
/// and an empty downed store — and when the pane has no room for the
/// sections that cannot be cut plus a stock header and one row.
pub(in crate::render) fn draw_stock_block(
    pane: Rect,
    research: Option<&ResearchReadout>,
    rows: &[StockRow],
    downed: (usize, usize),
    painter: &Painter,
    m: &Metrics,
) -> Option<Rect> {
    let (held, cap) = downed;
    if research.is_none() && rows.is_empty() && held == 0 {
        return None;
    }
    let pad = m.line_height * PAD_RATIO;
    let size = m.small();
    let x = pane.x + m.inset;
    let y = pane.y + strip_inset(m);
    let max_name_w = painter.measure_ui_advance("M", size) * NAME_COLUMN_CHARS;

    let mut lines = research
        .map(|r| research_lines(r, max_name_w, size, painter))
        .unwrap_or_default();

    // Lines the height budget holds. Past it the last line is spent on the
    // count, so every stock row is either named or counted.
    let budget = (pane.h * HEIGHT_FRACTION).min(pane.y + pane.h - y);
    let fits = ((budget - pad * 2.0) / m.line_height).floor().max(0.0) as usize;
    let stock_needs = if rows.is_empty() { 0 } else { 2 };
    // Claimed off the budget before the stock rows are allotted what is
    // left, never appended afterwards: the stock section is the one that
    // spends whatever it is given, so a section added after it is a section
    // a well-stocked base deletes.
    let downed = downed_lines(held, cap);
    if fits < lines.len() + downed.len() + stock_needs {
        return None;
    }
    if !rows.is_empty() {
        lines.push(Line::Text(HEADER.to_string(), palette::FIELD_LABEL));
        let room = fits - lines.len() - downed.len();
        let (shown, tail) = if rows.len() <= room {
            (rows.len(), None)
        } else {
            let shown = room - 1;
            (shown, Some(format!("+{} more", rows.len() - shown)))
        };
        lines.extend(rows[..shown].iter().map(|r| {
            Line::Pair(
                fit_name(&r.name, max_name_w, size, painter),
                r.qty.to_string(),
                palette::EMPHASIS,
            )
        }));
        lines.extend(tail.map(|t| Line::Text(t, palette::FAINT)));
    }
    lines.extend(downed);

    let measure = |t: &str| painter.measure_ui_advance(t, size);
    let pairs = || {
        lines.iter().filter_map(|l| match l {
            Line::Pair(name, figure, _) => Some((name, figure)),
            Line::Text(..) => None,
        })
    };
    let name_w = pairs().map(|(n, _)| measure(n)).fold(0.0, f32::max);
    let figure_w = pairs().map(|(_, f)| measure(f)).fold(0.0, f32::max);
    let gap = measure("  ");
    let text_w = lines
        .iter()
        .filter_map(|l| match l {
            Line::Text(t, _) => Some(measure(t)),
            Line::Pair(..) => None,
        })
        .fold(0.0, f32::max);
    let w = pad * 2.0 + (name_w + gap + figure_w).max(text_w);
    let h = pad * 2.0 + m.line_height * lines.len() as f32;
    if x + w > pane.x + pane.w - m.inset {
        return None;
    }

    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);

    let baseline = |line: usize| y + pad + m.line_height * line as f32 + size as f32 * 0.8;
    // Figures right-aligned on one edge, so a column of them reads down by
    // magnitude the way a ledger does.
    let figure_right = x + w - pad;
    for (i, line) in lines.iter().enumerate() {
        match line {
            Line::Text(text, ink) => painter.ui(text, x + pad, baseline(i), size, *ink),
            Line::Pair(name, figure, ink) => {
                painter.ui(name, x + pad, baseline(i), size, palette::BODY);
                let figure_x = figure_right - measure(figure);
                painter.ui(figure, figure_x, baseline(i), size, *ink);
            }
        }
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
        let (_, shapes) = with_painter(|p| draw_stock_block(pane(), None, &rows, (0, 10), p, &m));
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
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), None, &[], (0, 10), p, &m));
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
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), None, &rows, (0, 10), p, &m));
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
        let (filled, _) = with_painter(|p| draw_stock_block(squat, None, &rows, (0, 10), p, &m));
        assert_eq!(filled, None);
    }

    fn earning() -> ResearchReadout {
        ResearchReadout::Earning {
            name: "Automation".to_string(),
            earned: 412,
            cost: 600,
        }
    }

    /// The project heads the block, its figure on the ledger edge, and the
    /// stock follows under its own header.
    #[test]
    fn a_running_project_is_read_out_above_the_stock() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("Cache Grain", 12)]);
        let research = earning();
        let (_, shapes) =
            with_painter(|p| draw_stock_block(pane(), Some(&research), &rows, (0, 10), p, &m));
        let text = painted_text(&shapes);
        let at = |s: &str| {
            text.iter()
                .position(|t| t == s)
                .unwrap_or_else(|| panic!("{s:?} not painted: {text:?}"))
        };
        assert!(at(RESEARCH_HEADER) < at("Automation"));
        assert!(at("Automation") < at("412/600"));
        assert!(at("412/600") < at(HEADER));
        assert!(at(HEADER) < at("Cache Grain"));
    }

    /// A stalled project names the material instead of a figure that has
    /// stopped moving.
    #[test]
    fn a_stalled_project_names_what_it_needs_and_no_figure() {
        let m = ui_metrics(900.0);
        let research = ResearchReadout::Stalled {
            name: "Automation".to_string(),
            short_of: "Cache Grain".to_string(),
        };
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), Some(&research), &[], (0, 10), p, &m));
        assert!(filled.is_some(), "research alone is worth the corner");
        let text = painted_text(&shapes);
        assert_eq!(text, [RESEARCH_HEADER, "Automation", "needs Cache Grain"]);
    }

    /// An idle lab with an empty base still draws: "none" is the readout.
    #[test]
    fn an_idle_lab_says_none_and_draws_no_stock_header() {
        let m = ui_metrics(900.0);
        let (filled, shapes) = with_painter(|p| {
            draw_stock_block(pane(), Some(&ResearchReadout::Idle), &[], (0, 10), p, &m)
        });
        assert!(filled.is_some());
        assert_eq!(painted_text(&shapes), [RESEARCH_HEADER, IDLE]);
    }

    /// The store's own section, under the stock it is not part of — the
    /// stock is the base's and the downed programs are the player's, which
    /// is why it is a third section sharing the box rather than more rows
    /// under `BASE STOCK`.
    #[test]
    fn a_held_program_reads_out_under_the_stock() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("Cache Grain", 12)]);
        let (_, shapes) = with_painter(|p| draw_stock_block(pane(), None, &rows, (3, 10), p, &m));
        let text = painted_text(&shapes);
        let at = |s: &str| {
            text.iter()
                .position(|t| t == s)
                .unwrap_or_else(|| panic!("{s:?} not painted: {text:?}"))
        };
        assert!(at(HEADER) < at(DOWNED_HEADER), "the stock heads the store");
        assert!(at(DOWNED_HEADER) < at(DOWNED_LABEL));
        assert!(at(DOWNED_LABEL) < at("3/10"));
    }

    /// `0/10` is not news, and the corner is covering map to say it. The
    /// section appears with the first body and not before.
    #[test]
    fn an_empty_store_draws_no_section_at_all() {
        let m = ui_metrics(900.0);
        let rows = stock(&[("Cache Grain", 12)]);
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), None, &rows, (0, 10), p, &m));
        assert!(filled.is_some(), "the stock alone still draws");
        let text = painted_text(&shapes);
        assert!(
            !text.iter().any(|t| t == DOWNED_HEADER || t == "0/10"),
            "an empty store said something: {text:?}"
        );
    }

    /// And a held body alone is worth the corner, the way a stalled project
    /// with an empty base already is.
    #[test]
    fn a_held_program_alone_is_worth_the_corner() {
        let m = ui_metrics(900.0);
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), None, &[], (2, 10), p, &m));
        assert!(filled.is_some(), "nothing but the store still draws a box");
        assert_eq!(painted_text(&shapes), [DOWNED_HEADER, DOWNED_LABEL, "2/10"]);
    }

    /// The one that matters: the stock section spends whatever budget it is
    /// given, so a store appended after it is a store a well-stocked base
    /// deletes. Sixty piles is far past what the corner can hold — the
    /// readout must survive anyway, and the tail must still count honestly
    /// with the two lines it gave up.
    #[test]
    fn a_crowded_base_does_not_push_the_store_out_of_the_block() {
        let m = ui_metrics(900.0);
        let names: Vec<String> = (0..60).map(|i| format!("Material {i}")).collect();
        let rows = stock(&names.iter().map(|n| (n.as_str(), 5)).collect::<Vec<_>>());
        let research = earning();
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), Some(&research), &rows, (4, 10), p, &m));
        let filled = filled.expect("a tall pane has room for the block");
        assert!(
            filled.h <= pane().h * HEIGHT_FRACTION + 0.001,
            "the block is {}px tall against a {}px budget",
            filled.h,
            pane().h * HEIGHT_FRACTION
        );
        let text = painted_text(&shapes);
        assert_eq!(
            &text[text.len() - 3..],
            [DOWNED_HEADER, DOWNED_LABEL, "4/10"],
            "the store must be the last thing in the block: {text:?}"
        );
        let named = text.iter().filter(|t| t.starts_with("Material ")).count();
        assert!(named > 0, "no stock row was drawn at all");
        assert!(
            text.iter()
                .any(|t| t == &format!("+{} more", rows.len() - named)),
            "the tail must count the rows the store's two lines cost it too: {text:?}"
        );
    }

    /// A full store refuses the next body the world tries to hand over, and
    /// this figure is the only thing on screen that says so.
    #[test]
    fn a_full_store_reads_its_figure_in_attention() {
        let m = ui_metrics(900.0);
        let (_, full) = with_painter(|p| draw_stock_block(pane(), None, &[], (10, 10), p, &m));
        assert!(
            crate::paint::painted_runs_in(&full, palette::ATTENTION, false)
                .iter()
                .any(|t| t == "10/10"),
            "a full store drew its figure in the ordinary ink"
        );

        let (_, room) = with_painter(|p| draw_stock_block(pane(), None, &[], (9, 10), p, &m));
        assert!(
            crate::paint::painted_runs_in(&room, palette::ATTENTION, false)
                .iter()
                .all(|t| t != "9/10"),
            "one short of full is not the state this colour is for"
        );
    }

    /// The research section takes its lines off the top of the one budget,
    /// and the tail still counts exactly the stock rows it could not name.
    #[test]
    fn a_crowded_base_counts_its_rows_under_the_research_section() {
        let m = ui_metrics(900.0);
        let names: Vec<String> = (0..60).map(|i| format!("Material {i}")).collect();
        let rows = stock(&names.iter().map(|n| (n.as_str(), 5)).collect::<Vec<_>>());
        let research = earning();
        let (filled, shapes) =
            with_painter(|p| draw_stock_block(pane(), Some(&research), &rows, (0, 10), p, &m));
        let filled = filled.expect("a tall pane has room for the block");
        assert!(filled.h <= pane().h * HEIGHT_FRACTION + 0.001);
        let text = painted_text(&shapes);
        assert_eq!(&text[..3], [RESEARCH_HEADER, "Automation", "412/600"]);
        let named = text.iter().filter(|t| t.starts_with("Material ")).count();
        assert!(named > 0, "no stock row was drawn at all");
        assert_eq!(
            text.last().map(String::as_str),
            Some(format!("+{} more", rows.len() - named).as_str()),
        );
    }
}
