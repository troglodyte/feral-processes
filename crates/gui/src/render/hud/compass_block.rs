//! The compass block: where the party said they were going, drawn in the
//! map's top-right corner.
//!
//! **Not a border strip, and that is the decision worth keeping.** The
//! compass rode `map_pane`'s bottom border for one release. A strip centres
//! its background quad *on* a border line, so it reaches into the pane as
//! far as out of it — which made the map buy a band it could never draw
//! tiles in (`layout::map_body`, now gone) and made the gap between the map
//! and log panes hold two clearances instead of one, because the vitals
//! reach up into that same space. A block floating *inside* the pane costs
//! none of that: it overlays tiles that are still drawn, so the layout does
//! not move and the viewport never re-lays.
//!
//! It starts `layout::strip_inset` below the pane's top edge for the rule
//! that survived the move — the THREAT readout rides that top border, and
//! its quad reaches `strip_clearance` down into the pane, so anything the
//! pane's body draws up there starts below it or is cut in half.

use super::layout::strip_inset;
use super::palette;
use crate::paint::{Painter, Rect, TextRun};
use crate::render::PANEL_BG;
use crate::text::Metrics;
use feral_processes_engine::CompassRow;

/// Breathing space inside the block's own border, as a fraction of the UI
/// line height. A ratio rather than a literal so it travels with `Metrics`
/// instead of freezing at one window size.
const PAD_RATIO: f32 = 0.35;

/// How much wider than the arrow's own advance its column is. The arrow is
/// the one thing in the block that changes shape as the party walks, and a
/// column sized to whichever arrow is showing would make the name beside it
/// jitter left and right on every step.
const ARROW_COLUMN_RATIO: f32 = 1.8;

/// The two lines of text, top to bottom. The name leads because it is what
/// the player chose; the figure is underneath it because that is the half
/// that changes as they walk.
fn lines(row: &CompassRow) -> [String; 2] {
    [row.label.clone(), format!("{} tiles", row.distance)]
}

/// Draws the block into `pane`'s top-right corner and returns the box it
/// filled, so a test can ask what it covered without re-deriving the
/// geometry.
///
/// Returns `None` when there is not room for it — a pane narrower than the
/// block draws nothing rather than a box hanging off the edge, which is
/// `strip::fitting`'s rule in the one form a fixed-size box can take it.
pub(in crate::render) fn draw_compass_block(
    pane: Rect,
    row: &CompassRow,
    painter: &Painter,
    m: &Metrics,
) -> Option<Rect> {
    let pad = m.line_height * PAD_RATIO;
    let size = m.small();
    let [name, figure] = lines(row);

    let arrow = row.arrow.to_string();
    let arrow_w = painter.measure_ui_advance(&arrow, size) * ARROW_COLUMN_RATIO;
    let text_w = painter
        .measure_ui_advance(&name, size)
        .max(painter.measure_ui_advance(&figure, size));
    let w = pad * 2.0 + arrow_w + text_w;
    let h = pad * 2.0 + m.line_height * 2.0;

    // Inset from the right edge by the same `m.inset` every other mount on
    // this frame measures from, and from the top by `strip_inset` — see the
    // module doc: the THREAT strip's quad hangs into the pane above this.
    let x = pane.x + pane.w - m.inset - w;
    let y = pane.y + strip_inset(m);
    if x < pane.x + m.inset || y + h > pane.y + pane.h {
        return None;
    }

    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);

    // Both text lines share the arrow column's right edge, so the block
    // reads as two columns rather than as three left-aligned rows.
    let text_x = x + pad + arrow_w;
    let baseline = |line: f32| y + pad + m.line_height * line + size as f32 * 0.8;

    painter.ui_runs(
        &[TextRun {
            text: &arrow,
            bold: true,
            color: palette::EMPHASIS,
        }],
        x + pad,
        baseline(0.0),
        size,
    );
    painter.ui_runs(
        &[TextRun {
            text: &name,
            bold: false,
            color: palette::EMPHASIS,
        }],
        text_x,
        baseline(0.0),
        size,
    );
    painter.ui_runs(
        &[TextRun {
            text: &figure,
            bold: false,
            color: palette::BODY,
        }],
        text_x,
        baseline(1.0),
        size,
    );
    Some(Rect::new(x, y, w, h))
}
