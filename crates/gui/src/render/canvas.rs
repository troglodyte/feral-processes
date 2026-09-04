//! `draw_canvas`: the pixel grid, its cursor, and a row of palette swatches
//! — the mechanics `render/icon_editor.rs` used to own outright, extracted
//! so the dev-only sprite editor (a later task) can draw the same thing
//! against a wider grid and a different palette. Consumes app-core's
//! `CanvasEditor`/`CanvasView` the same way `icon_editor.rs` already did.
//!
//! **Two mutually exclusive modes, chosen by `view.edge`.** A grid to draw
//! (`edge > 0`) means "draw the cells, the grid lines and the brush-sized
//! cursor"; no grid (`edge == 0`, `cells` empty) means "draw the swatch
//! row instead". This is what lets one call reproduce the icon editor's
//! two independently-bordered panels pixel-for-pixel: it calls this twice,
//! once per panel, rather than once for a combined region neither panel's
//! chrome matches. A single call that always drew both would either
//! duplicate the swatch row (task 6's brief names the trap: a painted
//! cell's colour would show up on the grid, on the icon editor's palette
//! panel, *and* on this function's own swatch row — a count of 3 where the
//! screen's own test wants exactly 2) or force this function to reproduce
//! the *other* panel's independent border/background/label, which belongs
//! to the caller.
//!
//! **The swatch size comes from `rect.h`, not from the grid's cell.** The
//! two panels are independently sized in the icon editor (the palette
//! strip is narrower than the canvas — see `icon_editor.rs`'s own
//! `SWATCH_LINES` comment) and a swatch derived from the grid's own cell
//! size could not reproduce that. The caller passes the exact box the
//! swatch row must fill; `SWATCH_GAP_RATIO` is the one thing this function
//! decides — the gap between swatches as a fraction of a swatch's own
//! side — shared by every caller so a wider palette isn't a second tuned
//! constant.

use super::*;
use feral_processes_app_core::CanvasView;

const GRID_LINE: Color = Color::new(0.18, 0.20, 0.24, 1.0);
pub(crate) const CURSOR_COLOR: Color = WHITE;
const CURSOR_THICKNESS: f32 = 2.0;
pub(crate) const SELECTED_SWATCH_COLOR: Color = WHITE;
const SELECTED_SWATCH_THICKNESS: f32 = 2.0;

/// A swatch's gap to its neighbour, as a fraction of the swatch's own
/// side — the icon editor's shipped `SWATCH_GAP_LINES / SWATCH_LINES`
/// (0.3 / 0.9).
const SWATCH_GAP_RATIO: f32 = 1.0 / 3.0;

/// Draws the cell grid, the grid lines and the brush-sized cursor (when
/// `view.edge > 0`), or the swatch row (when it is 0), and nothing else —
/// no background, no border, no label. The caller owns every other pixel
/// on the screen; see this module's doc comment for why one call cannot
/// draw both onto the icon editor's two separately-bordered panels.
pub(crate) fn draw_canvas(p: &Painter, rect: Rect, view: &CanvasView, palette: &[(u8, u8, u8)]) {
    if view.edge == 0 {
        draw_swatch_row(p, rect, view.selected, palette);
        return;
    }
    draw_grid(p, rect, view, palette);
}

fn draw_grid(p: &Painter, rect: Rect, view: &CanvasView, palette: &[(u8, u8, u8)]) {
    let edge = view.edge as usize;
    let cell = rect.w / edge as f32;
    let side = edge as f32 * cell;

    for y in 0..edge {
        for x in 0..edge {
            let idx = view.cells[y * edge + x];
            let (px, py) = (rect.x + x as f32 * cell, rect.y + y as f32 * cell);
            p.rect(px, py, cell, cell, cell_color(idx, palette));
        }
    }
    for i in 0..=edge {
        let x = rect.x + i as f32 * cell;
        p.line(x, rect.y, x, rect.y + side, 1.0, GRID_LINE);
        let y = rect.y + i as f32 * cell;
        p.line(rect.x, y, rect.x + side, y, 1.0, GRID_LINE);
    }

    let brush = view.brush.max(1) as f32;
    let (cx, cy) = (view.cursor.0 as f32, view.cursor.1 as f32);
    p.rect_lines(
        rect.x + cx * cell,
        rect.y + cy * cell,
        cell * brush,
        cell * brush,
        CURSOR_THICKNESS,
        CURSOR_COLOR,
    );
}

/// A canvas cell's colour: the caller's own background for index 0
/// (transparent), or `palette`'s entry for a drawn cell. Callers never
/// store an index past their own palette's length — `PlayerIcon::set`'s
/// invariant, unchanged by this extraction — so the non-zero arm cannot go
/// out of bounds.
fn cell_color(index: u8, palette: &[(u8, u8, u8)]) -> Color {
    match index {
        0 => SCREEN_BG,
        n => palette_color(palette[n as usize - 1]),
    }
}

pub(crate) fn palette_color((r, g, b): (u8, u8, u8)) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

/// `rect` is the exact box the row fills — `rect.h` is the swatch side,
/// `rect.w` its total width — so a caller that already computed a narrower
/// strip than its canvas (the icon editor's palette panel) hands over
/// something this function reproduces exactly rather than re-deriving.
fn draw_swatch_row(p: &Painter, rect: Rect, selected: u8, palette: &[(u8, u8, u8)]) {
    let swatch = rect.h;
    let gap = swatch * SWATCH_GAP_RATIO;
    for (i, &rgb) in palette.iter().enumerate() {
        let x = rect.x + i as f32 * (swatch + gap);
        p.rect(x, rect.y, swatch, swatch, palette_color(rgb));
        // `selected` is 1-based — index 0 means transparent and is not a
        // swatch, `app::canvas_editor::FIRST_COLOUR`.
        if selected as usize == i + 1 {
            p.rect_lines(
                x,
                rect.y,
                swatch,
                swatch,
                SELECTED_SWATCH_THICKNESS,
                SELECTED_SWATCH_COLOR,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::CanvasFocus;

    fn view(edge: u8, brush: u8) -> CanvasView {
        CanvasView {
            cells: vec![0; edge as usize * edge as usize],
            edge,
            cursor: (0, 0),
            selected: 1,
            focus: CanvasFocus::Canvas,
            brush,
        }
    }

    /// **The failing test this task starts from.** At brush 2 the cursor
    /// outline is twice the edge of brush 1's — brush 1 is exactly today's
    /// one-cell cursor, so this is the one thing the extraction adds.
    #[test]
    fn the_cursor_scales_with_the_brush() {
        let rect = Rect::new(0.0, 0.0, 40.0, 40.0);
        let (_, shapes1) = crate::paint::with_painter(|p| draw_canvas(p, rect, &view(4, 1), &[]));
        let (_, shapes2) = crate::paint::with_painter(|p| draw_canvas(p, rect, &view(4, 2), &[]));

        let widest = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
            crate::paint::painted_rect_widths(shapes)
                .into_iter()
                .fold(0.0_f32, f32::max)
        };
        let cursor1 = widest(&shapes1);
        let cursor2 = widest(&shapes2);
        assert_eq!(
            cursor2,
            cursor1 * 2.0,
            "a brush-2 cursor must be twice the edge of a brush-1 cursor: \
             {cursor1} vs {cursor2}"
        );
    }

    /// The grid alone: brush 1's cursor is exactly one cell, the untouched
    /// behaviour this extraction must not disturb.
    #[test]
    fn brush_one_cursor_is_exactly_one_cell() {
        let rect = Rect::new(0.0, 0.0, 40.0, 40.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_canvas(p, rect, &view(4, 1), &[]));
        let cell = rect.w / 4.0;
        assert!(
            crate::paint::painted_rect_stroke_count(&shapes, CURSOR_COLOR) >= 1,
            "the cursor must be outlined"
        );
        let widest = crate::paint::painted_rect_widths(&shapes)
            .into_iter()
            .fold(0.0_f32, f32::max);
        assert_eq!(widest, cell, "brush 1's cursor is exactly one cell wide");
    }

    /// `view.edge == 0` switches this function to swatch-row mode: no grid
    /// cell and no cursor are drawn, only the row.
    #[test]
    fn edge_zero_draws_only_the_swatch_row() {
        let palette: [(u8, u8, u8); 3] = [(255, 0, 0), (0, 255, 0), (0, 0, 255)];
        let mut v = view(0, 1);
        v.cells = vec![];
        v.selected = 2;
        let rect = Rect::new(0.0, 0.0, 30.0, 10.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_canvas(p, rect, &v, &palette));

        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, palette_color(palette[1])),
            1,
            "the selected swatch must still be filled once"
        );
        // `CURSOR_COLOR` and `SELECTED_SWATCH_COLOR` are both `WHITE` (as
        // in the pre-extraction code), so a stroke count in either name is
        // the same count — one outline total is only possible if no
        // cursor was drawn alongside the selected swatch.
        assert_eq!(
            crate::paint::painted_rect_stroke_count(&shapes, SELECTED_SWATCH_COLOR),
            1,
            "exactly the selected swatch is outlined, and no cursor besides it"
        );
    }
}
