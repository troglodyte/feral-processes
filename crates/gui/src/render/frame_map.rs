//! The party's own map of the Stack frame they are standing in.
//!
//! Drawn north-up as a plain grid, which is the one place the Stack layer
//! deliberately breaks from the first-person view: a map you have to rotate
//! in your head is a second puzzle on top of the maze. The renderer knows
//! nothing about what has or hasn't been explored — `Game::frame_map`
//! hands over `FrameMapCell::Unknown` for anything never seen, and this
//! file only has to draw it.

use super::*;
use feral_processes_engine::{FrameMapCell, FrameMapMark, FrameMapView};

/// Fraction of the shorter pane axis the map grid fills, leaving room for
/// the heading above it and the legend below.
const GRID_FILL: f32 = 0.74;

/// How much of each cell the tile actually covers. Below 1.0 so the grid
/// reads as cells rather than as a single flood of colour.
const TILE_FILL: f32 = 0.86;

const UNKNOWN: Color = Color::new(0.05, 0.06, 0.09, 1.0);
const ROCK: Color = Color::new(0.13, 0.16, 0.20, 1.0);
const WALKED: Color = Color::new(0.16, 0.38, 0.42, 1.0);
/// Corruption is the one walkable kind with its own tile colour rather than
/// a glyph on `WALKED`. It is an *area* to route around rather than a point
/// to walk to, and at the inset sizes phase 5 will use, a coloured region
/// survives being small in a way a 9px glyph does not.
const CORRUPT: Color = Color::new(0.36, 0.14, 0.30, 1.0);

fn tile_color(cell: FrameMapCell) -> Color {
    match cell {
        FrameMapCell::Unknown => UNKNOWN,
        FrameMapCell::Rock => ROCK,
        FrameMapCell::Corruption => CORRUPT,
        FrameMapCell::Floor
        | FrameMapCell::LinkUp
        | FrameMapCell::LinkDown
        | FrameMapCell::Cache
        | FrameMapCell::Lair
        | FrameMapCell::Door
        | FrameMapCell::SealedDoor
        | FrameMapCell::Breakpoint
        | FrameMapCell::Fault
        | FrameMapCell::Orphan => WALKED,
    }
}

/// The glyph pinned to a cell, if any. Links come off the layout; the
/// party and their fights come off `marks`, which is why marks are drawn
/// second and win.
fn cell_glyph(cell: FrameMapCell) -> Option<(char, Color)> {
    match cell {
        FrameMapCell::LinkDown => Some(('>', YELLOW)),
        FrameMapCell::LinkUp => Some(('<', YELLOW)),
        FrameMapCell::Cache => Some(('!', GREEN)),
        FrameMapCell::Lair => Some(('&', RED)),
        FrameMapCell::Door => Some(('+', ORANGE)),
        FrameMapCell::SealedDoor => Some(('+', RED)),
        FrameMapCell::Breakpoint => Some(('*', BLUE)),
        FrameMapCell::Fault => Some(('v', ORANGE)),
        FrameMapCell::Orphan => Some(('o', GREEN)),
        // Corruption deliberately has none: it is the one kind carrying its
        // meaning in `tile_color` instead. See `CORRUPT`.
        _ => None,
    }
}

fn mark_glyph(mark: FrameMapMark) -> (char, Color) {
    match mark {
        FrameMapMark::Party => ('@', CYAN),
        FrameMapMark::Fight => ('x', RED),
    }
}

/// Side length of one map cell in pixels, and the top-left corner the grid
/// starts at, for a `view` drawn into a `w` by `h` pane.
///
/// Square cells — the frame is square and a stretched map would misreport
/// distances the player is trying to count.
fn layout(view: &FrameMapView, w: f32, h: f32) -> (f32, f32, f32) {
    if view.width <= 0 || view.height <= 0 {
        return (0.0, 0.0, 0.0);
    }
    let cell = (w * GRID_FILL / view.width as f32).min(h * GRID_FILL / view.height as f32);
    let grid_w = cell * view.width as f32;
    let grid_h = cell * view.height as f32;
    ((w - grid_w) / 2.0, (h - grid_h) / 2.0, cell)
}

pub(super) fn draw_frame_map(view: &FrameMapView, painter: &Painter, w: f32, h: f32, m: &Metrics) {
    painter.rect(0.0, 0.0, w, h, PANEL_BG);
    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    let heading = format!(
        "DEEP SCAN   depth {} / {}   link {},{}   facing {}   {:.0}% mapped",
        view.depth,
        view.frames,
        view.entrance.0,
        view.entrance.1,
        view.facing,
        view.explored * 100.0,
    );
    painter.ui(
        &heading,
        m.inset,
        m.inset + m.font_size as f32,
        m.font_size,
        CYAN,
    );

    let (ox, oy, cell) = layout(view, w, h);
    if cell <= 0.0 {
        return;
    }
    let inset = cell * (1.0 - TILE_FILL) / 2.0;
    let glyph_px = (cell * 0.8) as u16;

    for (y, row) in view.cells.iter().enumerate() {
        for (x, &kind) in row.iter().enumerate() {
            let (px, py) = (ox + x as f32 * cell, oy + y as f32 * cell);
            painter.rect(
                px + inset,
                py + inset,
                cell - inset * 2.0,
                cell - inset * 2.0,
                tile_color(kind),
            );
            if let Some((ch, color)) = cell_glyph(kind) {
                draw_cell_glyph(painter, ch, color, px, py, cell, glyph_px);
            }
        }
    }

    // After the grid, so a landmark is never painted over by its own cell.
    for &((x, y), mark) in &view.marks {
        if x < 0 || y < 0 || x >= view.width || y >= view.height {
            continue;
        }
        let (ch, color) = mark_glyph(mark);
        let (px, py) = (ox + x as f32 * cell, oy + y as f32 * cell);
        draw_cell_glyph(painter, ch, color, px, py, cell, glyph_px);
    }

    let legend = "@ you  < up  > down  ! cache  + door  & lair  * port  v fault  \
                  purple = corrupt  x fight  unlit = unmapped";
    let dims = painter.measure_ui(legend, m.font_size);
    painter.ui(
        legend,
        (w - dims.width) / 2.0,
        h - m.inset,
        m.font_size,
        TEXT_DIM,
    );
}

/// Centres one map glyph in the cell whose top-left corner is `(px, py)`.
fn draw_cell_glyph(
    painter: &Painter,
    ch: char,
    color: Color,
    px: f32,
    py: f32,
    cell: f32,
    glyph_px: u16,
) {
    if glyph_px == 0 {
        return;
    }
    let text = ch.to_string();
    let dims = painter.measure_map(&text, glyph_px);
    painter.map(
        &text,
        px + (cell - dims.width) / 2.0,
        py + cell - (cell - dims.height) / 2.0,
        glyph_px,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(width: i32, height: i32) -> FrameMapView {
        FrameMapView {
            depth: 2,
            frames: 4,
            width,
            height,
            cells: (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| {
                            if (x + y) % 3 == 0 {
                                FrameMapCell::Unknown
                            } else if x % 2 == 0 {
                                FrameMapCell::Rock
                            } else {
                                FrameMapCell::Floor
                            }
                        })
                        .collect()
                })
                .collect(),
            marks: vec![((1, 1), FrameMapMark::Fight), ((3, 5), FrameMapMark::Party)],
            facing: "N",
            entrance: (12, -40),
            explored: 0.42,
        }
    }

    #[test]
    fn map_cells_are_square() {
        let (_, _, cell) = layout(&view(21, 21), 1000.0, 640.0);
        assert!(cell > 0.0);
        // One cell size drives both axes, so squareness is structural; what
        // needs checking is that it fits the *shorter* axis.
        assert!(cell * 21.0 <= 640.0, "the grid overflows the pane");
    }

    #[test]
    fn the_grid_is_centred_in_the_pane() {
        let (w, h) = (1000.0, 640.0);
        let v = view(21, 21);
        let (ox, oy, cell) = layout(&v, w, h);
        assert!((ox + cell * 21.0 / 2.0 - w / 2.0).abs() < 0.001);
        assert!((oy + cell * 21.0 / 2.0 - h / 2.0).abs() < 0.001);
    }

    /// A non-square frame must still get square cells rather than being
    /// stretched to fill the pane — the player counts corridors off this.
    #[test]
    fn an_oblong_frame_still_gets_square_cells() {
        let (_, _, cell) = layout(&view(31, 11), 1000.0, 640.0);
        assert!(cell * 31.0 <= 1000.0 + 0.001);
        assert!(cell * 11.0 <= 640.0 + 0.001);
    }

    #[test]
    fn unknown_and_seen_rock_are_drawn_differently() {
        assert_ne!(
            tile_color(FrameMapCell::Unknown),
            tile_color(FrameMapCell::Rock),
            "'never been here' and 'nothing here' are what a mapper most needs to tell apart"
        );
    }

    #[test]
    fn links_carry_the_same_glyphs_as_the_first_person_view() {
        assert_eq!(cell_glyph(FrameMapCell::LinkDown).map(|g| g.0), Some('>'));
        assert_eq!(cell_glyph(FrameMapCell::LinkUp).map(|g| g.0), Some('<'));
        assert_eq!(cell_glyph(FrameMapCell::Floor), None);
    }

    #[test]
    fn drawing_a_map_does_not_panic() {
        let m = crate::text::ui_metrics(900.0);
        crate::paint::with_painter(|p| {
            draw_frame_map(&view(21, 21), p, 1000.0, 640.0, &m);
            draw_frame_map(&view(31, 11), p, 1000.0, 640.0, &m);
        });
    }

    /// The renderer must survive whatever the engine hands it, including
    /// shapes it never actually produces.
    #[test]
    fn drawing_a_degenerate_map_does_not_panic() {
        let m = crate::text::ui_metrics(900.0);
        let mut empty = view(0, 0);
        empty.cells = Vec::new();
        // A mark outside the grid: clipped, not indexed with.
        empty.marks = vec![((99, 99), FrameMapMark::Party)];
        crate::paint::with_painter(|p| {
            draw_frame_map(&empty, p, 800.0, 600.0, &m);
        });
    }
}
