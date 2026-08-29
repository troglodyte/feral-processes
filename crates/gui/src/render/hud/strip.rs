//! Text mounted **on** a pane's border, breaking the border run behind it.
//!
//! This is the handoff's signature move: the map pane's title, its threat
//! readout and its vitals all ride the frame rather than costing the pane a
//! body row, and the log pane's filters and keybar do the same.
//!
//! **Draw order is the whole of what this module is for.** The caller has
//! already drawn the pane's border *and its interior fill*; this then paints
//! a background quad the measured width of the label and only then the
//! glyphs. Painting the pane's interior after the label cuts the label in
//! half — that is a failure the design handoff recorded against its own HTML
//! reference, and at five call sites it is a bug four of them could have
//! independently. Written once it is a bug the ordering of one function
//! prevents, which is why `border_strip` is a function and not a convention.

use super::palette;
use crate::paint::{Color, Painter, Rect, TextRun};
use crate::text::Metrics;

/// Which border a strip rides, and which end of it the strip starts from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::render) enum Mount {
    TopLeft,
    TopRight,
    BottomLeft,
}

/// Pad either side of the label, inside the background quad. What makes the
/// border read as *broken by* a label rather than overwritten by one.
///
/// `pub(in crate::render)` because `hud::layout` needs it too: a strip's
/// quad reaches `size/2 + pad/2` past its border line on both sides (see
/// `border_strip` below), and `layout::STRIP_CLEARANCE_RATIO` is that same
/// expression rather than a second copy of it — a call, not a copy, per
/// `CLAUDE.md`'s rule on doc comments that claim to mirror another
/// module.
pub(in crate::render) const PAD_RATIO: f32 = 0.5;

/// The separator between two segments of a strip.
const SEP: &str = " · ";

/// One piece of a strip: text, colour, weight.
///
/// A tuple rather than a struct because every producer builds these by the
/// dozen inline and no consumer names a field — `fitting` reads the text and
/// `draw_pieces` turns the three straight into a [`TextRun`].
pub(in crate::render) type Piece = (String, Color, bool);

pub(in crate::render) fn label(text: &str) -> Piece {
    (text.to_string(), palette::FIELD_LABEL, false)
}

pub(in crate::render) fn value(text: String) -> Piece {
    (text, palette::BODY, false)
}

pub(in crate::render) fn sep() -> Piece {
    (SEP.to_string(), palette::FAINT, false)
}

/// Flattens the longest prefix of `segments` that fits `avail`, separators
/// included.
///
/// **This is `stock::fits`' rule applied to segments**: a strip is one row on
/// a border with no wrap and no clip, so what does not fit is dropped from
/// the end, measured, rather than drawn off the pane in silence. Both the
/// vitals strip and the keybar degrade this way, which is why the rule lives
/// here beside [`border_strip`] rather than in either of them.
pub(in crate::render) fn fitting(
    segments: &[Vec<Piece>],
    avail: f32,
    painter: &Painter,
    m: &Metrics,
) -> Vec<Piece> {
    let size = m.small();
    let mut taken: Vec<Piece> = Vec::new();
    for segment in segments {
        let mut next = taken.clone();
        if !next.is_empty() {
            next.push(sep());
        }
        next.extend(segment.iter().cloned());
        let text: String = next.iter().map(|(t, _, _)| t.as_str()).collect();
        if painter.measure_ui_advance(&text, size) > avail {
            break;
        }
        taken = next;
    }
    taken
}

/// Mounts `pieces` on a border and returns the advance consumed.
pub(in crate::render) fn draw_pieces(
    pane: Rect,
    mount: Mount,
    pieces: &[Piece],
    painter: &Painter,
    m: &Metrics,
) -> f32 {
    let runs: Vec<TextRun> = pieces
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    border_strip(pane, mount, &runs, painter, m)
}

/// A baseline this far below the border line puts the text's visual centre
/// on it. Caps run about 0.7em, so their centre sits ~0.35em above the
/// baseline. Expressed as a ratio rather than the handoff's -9px/-11px so it
/// travels with `Metrics` instead of freezing at the reference size.
const BASELINE_RATIO: f32 = 0.35;

/// Draws `runs` over one of `pane`'s border runs and returns the advance the
/// strip consumed.
///
/// The return value is what lets a caller mount two strips on one border and
/// know whether the second clears the first — the log pane's top border does
/// exactly that.
pub(in crate::render) fn border_strip(
    pane: Rect,
    mount: Mount,
    runs: &[TextRun],
    painter: &Painter,
    m: &Metrics,
) -> f32 {
    let size = m.small();
    let text: String = runs.iter().map(|r| r.text).collect();
    let advance = painter.measure_ui_advance(&text, size);
    if advance <= 0.0 {
        return 0.0;
    }
    let pad = size as f32 * PAD_RATIO;

    let x = match mount {
        Mount::TopLeft | Mount::BottomLeft => pane.x + m.inset,
        // Grows leftward as the text grows, so a long readout runs back into
        // the pane rather than out past its right edge.
        Mount::TopRight => pane.x + pane.w - m.inset - advance,
    };
    let line_y = match mount {
        Mount::TopLeft | Mount::TopRight => pane.y,
        Mount::BottomLeft => pane.y + pane.h,
    };

    // Border, then background, then glyphs. The border is the caller's; the
    // other two are this function's, in this order, and swapping them is the
    // bug the module comment names.
    painter.rect(
        x - pad,
        line_y - size as f32 / 2.0 - pad / 2.0,
        advance + pad * 2.0,
        size as f32 + pad,
        palette::STATUS_BG,
    );
    painter.ui_runs(runs, x, line_y + size as f32 * BASELINE_RATIO, size);
    advance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Painted, paint_order, painted_rects, with_painter};
    use crate::text::ui_metrics;

    fn pane() -> Rect {
        Rect::new(40.0, 60.0, 900.0, 500.0)
    }

    fn runs<'a>(text: &'a str) -> Vec<TextRun<'a>> {
        vec![TextRun {
            text,
            bold: false,
            color: palette::PANE_TITLE,
        }]
    }

    /// The one bug this module exists to prevent. The background must land
    /// *before* the glyphs, or the quad paints over the label it is supposed
    /// to sit behind and the strip is invisible.
    #[test]
    fn a_strip_paints_its_background_before_its_glyphs() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            border_strip(pane(), Mount::TopLeft, &runs("SECTOR MAP"), p, &m);
        });
        let order = paint_order(&shapes);
        let rect = order
            .iter()
            .position(|k| *k == Painted::Rect)
            .expect("a strip paints a background quad");
        let text = order
            .iter()
            .position(|k| *k == Painted::Text)
            .expect("a strip paints glyphs");
        assert!(
            rect < text,
            "background painted at {rect}, glyphs at {text} — the quad covers the label"
        );
    }

    /// What a caller mounting two strips on one border needs in order to know
    /// they do not collide.
    #[test]
    fn a_strip_reports_what_it_consumed() {
        let m = ui_metrics(900.0);
        with_painter(|p| {
            let text = "THREAT  2 hostile · shields holding";
            let got = border_strip(pane(), Mount::TopRight, &runs(text), p, &m);
            let want = p.measure_ui_advance(text, m.small());
            assert!(
                (got - want).abs() < 0.001,
                "reported {got} for a strip measuring {want}"
            );
        });
    }

    /// A right-mounted strip is anchored at the pane's right inset and grows
    /// leftward, so a long readout runs back into the pane instead of out
    /// past its edge.
    ///
    /// Asserted against the painted quad, not against the returned advance:
    /// the obvious form — reconstructing the x from the advance the function
    /// just returned — reduces to a constant and holds however the strip is
    /// placed.
    #[test]
    fn a_right_mounted_strip_ends_at_the_pane_inset() {
        let m = ui_metrics(900.0);
        let pane = pane();
        let right_edge = |text: &str| {
            let (_, shapes) = with_painter(|p| {
                border_strip(pane, Mount::TopRight, &runs(text), p, &m);
            });
            painted_rects(&shapes)
                .first()
                .expect("a strip paints a background quad")
                .max
                .x
        };
        let short = right_edge("THREAT");
        let long = right_edge("THREAT  9 hostile · no defence");
        assert!(
            (short - long).abs() < 0.001,
            "right edge moved from {short} to {long} as the text grew"
        );
        assert!(
            long >= pane.x + pane.w - m.inset,
            "strip ends at {long}, inside the pane's right inset at {}",
            pane.x + pane.w - m.inset
        );
    }

    /// A left-mounted strip is the mirror: its left edge is pinned and it
    /// grows rightward. Without this the test above passes against a strip
    /// that ignores `Mount` entirely.
    #[test]
    fn a_left_mounted_strip_starts_at_the_pane_inset() {
        let m = ui_metrics(900.0);
        let pane = pane();
        let left_edge = |text: &str| {
            let (_, shapes) = with_painter(|p| {
                border_strip(pane, Mount::TopLeft, &runs(text), p, &m);
            });
            painted_rects(&shapes)
                .first()
                .expect("a strip paints a background quad")
                .min
                .x
        };
        let short = left_edge("LOG");
        let long = left_edge("LOG  ALL FIELD BASE COMBAT");
        assert!(
            (short - long).abs() < 0.001,
            "left edge moved from {short} to {long} as the text grew"
        );
        assert!(
            short <= pane.x + m.inset,
            "strip starts at {short}, inside the pane's left inset at {}",
            pane.x + m.inset
        );
    }

    /// An empty strip draws nothing at all, rather than a bare quad sitting
    /// on the border like a gap chewed out of the frame.
    #[test]
    fn an_empty_strip_draws_nothing() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            assert_eq!(border_strip(pane(), Mount::TopLeft, &runs(""), p, &m), 0.0);
        });
        assert!(
            !paint_order(&shapes).contains(&Painted::Rect),
            "an empty strip still painted a quad"
        );
    }
}
