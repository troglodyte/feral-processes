//! `draw_canvas_grid` and `draw_swatch_row`: the pixel grid with its
//! brush-sized cursor, and a row of palette swatches — the mechanics
//! `render/icon_editor.rs` used to own outright, extracted so the dev-only
//! sprite editor (a later task) can draw the same thing against a wider
//! grid and a different palette. `draw_canvas_grid` consumes app-core's
//! `CanvasView` the same way `icon_editor.rs` already did; `draw_swatch_row`
//! takes only a selected index and a palette — it was never really an
//! operation on a canvas, just one this file used to perform alongside one.
//!
//! **Two functions, not one, because the icon editor draws them into two
//! independently-bordered panels.** Each takes its own `Rect` — the panes'
//! own convention, "the caller takes the origin" — and the caller decides
//! whether, and where, to call either. Neither function draws a background,
//! a border or a label; that chrome is `icon_editor.rs`'s (and will be
//! whatever the sprite editor's screen wants).
//!
//! **The swatch size comes from `rect.h`, not from the grid's cell.** The
//! two panels are independently sized in the icon editor (the palette
//! strip is narrower than the canvas — see `icon_editor.rs`'s own
//! `SWATCH_LINES` comment), so a swatch derived from the grid's own cell
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
/// The transparent swatch's second checker tone, over `SCREEN_BG` — a
/// checker rather than a plain `SCREEN_BG` square, which would read as a
/// gap in the row rather than a colour you can pick.
const CHECKER_LIGHT: Color = Color::new(0.30, 0.31, 0.34, 1.0);

/// A swatch's gap to its neighbour, as a fraction of the swatch's own
/// side — the icon editor's shipped `SWATCH_GAP_LINES / SWATCH_LINES`
/// (0.3 / 0.9). `pub(crate)` so the sprite editor's pointer resolver
/// (`sprite_forge::swatch_at`) can test a hit against the same spacing this
/// draws, rather than a second hand-copied `/3.0` literal.
pub(crate) const SWATCH_GAP_RATIO: f32 = 1.0 / 3.0;

/// Draws the cell grid, the grid lines and the brush-sized cursor, and
/// nothing else — no background, no border, no label.
pub(crate) fn draw_canvas_grid(
    p: &Painter,
    rect: Rect,
    view: &CanvasView,
    palette: &[(u8, u8, u8)],
) {
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

/// How many swatches a row draws for a palette of `palette_len` colours:
/// the transparent swatch (index 0) and then every entry, so a drawn
/// position is the palette index itself. Every site that sizes or
/// hit-tests a row reads this rather than adding its own `+ 1`.
pub(crate) fn swatch_count(palette_len: usize) -> usize {
    palette_len + 1
}

/// Draws a row of palette swatches and the selected one's outline, and
/// nothing else — no background, no border, no label. `rect` is the exact
/// box the row fills — `rect.h` is the swatch side, `rect.w` its total
/// width — so a caller that already computed a narrower strip than its
/// canvas (the icon editor's palette panel) hands over something this
/// function reproduces exactly rather than re-deriving. `selected` is a
/// palette index and also the drawn position — see `swatch_count`. Takes no
/// `CanvasView`: a swatch row is not a property of a canvas, only of a
/// palette and which entry is selected.
pub(crate) fn draw_swatch_row(p: &Painter, rect: Rect, selected: u8, palette: &[(u8, u8, u8)]) {
    let swatch = rect.h;
    let gap = swatch * SWATCH_GAP_RATIO;
    for i in 0..swatch_count(palette.len()) {
        let x = rect.x + i as f32 * (swatch + gap);
        match i {
            0 => draw_checker(p, x, rect.y, swatch),
            n => p.rect(x, rect.y, swatch, swatch, palette_color(palette[n - 1])),
        }
        if selected as usize == i {
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

/// The transparent swatch: a 2x2 checker, the convention for "no colour".
fn draw_checker(p: &Painter, x: f32, y: f32, side: f32) {
    let half = side / 2.0;
    p.rect(x, y, side, side, SCREEN_BG);
    p.rect(x, y, half, half, CHECKER_LIGHT);
    p.rect(x + half, y + half, half, half, CHECKER_LIGHT);
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
        let (_, shapes1) =
            crate::paint::with_painter(|p| draw_canvas_grid(p, rect, &view(4, 1), &[]));
        let (_, shapes2) =
            crate::paint::with_painter(|p| draw_canvas_grid(p, rect, &view(4, 2), &[]));

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
        let (_, shapes) =
            crate::paint::with_painter(|p| draw_canvas_grid(p, rect, &view(4, 1), &[]));
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

    /// The transparent swatch is drawn first, and selecting index 0
    /// outlines it — the drawn position and the palette index are one
    /// number, so a click and an arrow key cannot disagree about it.
    #[test]
    fn the_transparent_swatch_leads_the_row_and_can_be_selected() {
        let palette: [(u8, u8, u8); 3] = [(255, 0, 0), (0, 255, 0), (0, 0, 255)];
        let rect = Rect::new(0.0, 0.0, 60.0, 10.0);
        let stride = 10.0 * (1.0 + SWATCH_GAP_RATIO);
        for selected in [0u8, 1, 3] {
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_swatch_row(p, rect, selected, &palette));
            let outlined = crate::paint::painted_rect_stroke_boxes(&shapes, SELECTED_SWATCH_COLOR);
            assert_eq!(outlined.len(), 1, "exactly one swatch is outlined");
            let want = selected as f32 * stride;
            assert!(
                (outlined[0].min.x - want).abs() < 0.5,
                "index {selected} must outline drawn position {selected} at x={want}, got {}",
                outlined[0].min.x
            );
        }
        let (_, shapes) = crate::paint::with_painter(|p| draw_swatch_row(p, rect, 1, &palette));
        assert!(
            crate::paint::painted_rect_fill_count(&shapes, CHECKER_LIGHT) >= 1,
            "the transparent swatch is drawn as a checker"
        );
        assert_eq!(swatch_count(palette.len()), 4);
    }

    /// `draw_swatch_row` draws only the row: no grid cell and no cursor,
    /// since it never receives a `CanvasView` to draw either from.
    #[test]
    fn the_swatch_row_draws_only_the_row() {
        let palette: [(u8, u8, u8); 3] = [(255, 0, 0), (0, 255, 0), (0, 0, 255)];
        let rect = Rect::new(0.0, 0.0, 30.0, 10.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_swatch_row(p, rect, 2, &palette));

        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, palette_color(palette[1])),
            1,
            "the selected swatch must still be filled once"
        );
        assert_eq!(
            crate::paint::painted_rect_stroke_count(&shapes, SELECTED_SWATCH_COLOR),
            1,
            "exactly the selected swatch is outlined"
        );
    }
}
