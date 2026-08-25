//! The party's own map of the Stack frame they are standing in.
//!
//! Drawn north-up as a plain grid, which is the one place the Stack layer
//! deliberately breaks from the first-person view: a map you have to rotate
//! in your head is a second puzzle on top of the maze. The renderer knows
//! nothing about what has or hasn't been explored — `Game::frame_map`
//! hands over `FrameMapCell::Unknown` for anything never seen, and this
//! file only has to draw it.

use super::*;
use feral_processes_app_core::STACK_MAP_MIN_ZOOM;
use feral_processes_engine::{FrameMapCell, FrameMapMark, FrameMapView};

/// Fraction of the shorter pane axis the map grid fills, leaving room for
/// the heading above it and the legend below.
const GRID_FILL: f32 = 0.74;

/// The same, for the always-on inset, which carries no text of its own —
/// so the grid fills its rect but for the border it is drawn inside.
const INSET_FILL: f32 = 0.94;

/// Side of the corner inset, as a fraction of the first-person pane's width.
///
/// Sized by the *smallest* window's legibility floor rather than by taste:
/// a 21-wide frame has to leave map glyphs readable at 1280x720, which is
/// what pins this at 0.30 rather than the tidier quarter-pane. Run
/// `inset_glyphs_stay_legible_at_the_smallest_window` for the figures — a
/// doc comment restating them is a copy that goes stale, which this repo
/// has already paid for three times in `manifest_layout`.
///
/// Presentation, so it lives here and not in `tuning.rs` — how much of the
/// corridor the map covers is not difficulty.
const INSET_FRACTION: f32 = 0.30;

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

/// The Wild Jump cursor's outline. Deliberately not `CYAN` — the party's own
/// `@` is cyan, and on the one screen where "where I am" and "where I am
/// aiming" have to stay distinguishable, sharing a colour is how a player
/// jumps into a wall.
const CURSOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);

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
        | FrameMapCell::Orphan
        | FrameMapCell::Market => WALKED,
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
        FrameMapCell::Market => Some(('$', YELLOW)),
        // Corruption deliberately has none: it is the one kind carrying its
        // meaning in `tile_color` instead. See `CORRUPT`.
        //
        // Exhaustive rather than a `_ => None` tail, and for the reason
        // `render/stack.rs::cell_mark` is: under a wildcard a new
        // `FrameMapCell` compiles clean and draws as bare floor, so a cell
        // worth walking to is one the map never mentions. Deciding what it
        // looks like is now a compile error rather than an omission.
        FrameMapCell::Corruption
        | FrameMapCell::Unknown
        | FrameMapCell::Rock
        | FrameMapCell::Floor => None,
    }
}

fn mark_glyph(mark: FrameMapMark) -> (char, Color) {
    match mark {
        FrameMapMark::Party => ('@', CYAN),
        FrameMapMark::Fight => ('x', RED),
    }
}

/// Side length of one map cell in pixels, and the top-left corner the grid
/// starts at, for a `view` drawn into a `w` by `h` rect filling `fill` of it.
///
/// Square cells — the frame is square and a stretched map would misreport
/// distances the player is trying to count.
fn layout(view: &FrameMapView, w: f32, h: f32, fill: f32) -> (f32, f32, f32) {
    if view.width <= 0 || view.height <= 0 {
        return (0.0, 0.0, 0.0);
    }
    let cell = (w * fill / view.width as f32).min(h * fill / view.height as f32);
    let grid_w = cell * view.width as f32;
    let grid_h = cell * view.height as f32;
    ((w - grid_w) / 2.0, (h - grid_h) / 2.0, cell)
}

/// Map glyphs are sized off the cell they sit in, so both maps shrink their
/// lettering with their grid rather than one of them clipping.
fn glyph_px(cell: f32) -> u16 {
    (cell * 0.8) as u16
}

/// The rect the corner inset occupies inside a `w` by `h` first-person pane.
///
/// Top-left, one heading line down: the pane's top-left corner already
/// carries `draw_stack`'s `Facing / Depth / Trace` line, and the map covering
/// the Trace band would trade the readout this layer's escalation is
/// explained by for the map that explains the maze.
fn inset_rect(pane: Rect, m: &Metrics) -> Rect {
    let top = m.inset + m.font_size as f32 + m.gap;
    let side = (pane.w * INSET_FRACTION)
        .min(pane.w - m.inset * 2.0)
        .min(pane.h - top - m.inset)
        .max(0.0);
    Rect::new(pane.x + m.inset, pane.y + top, side, side)
}

/// How many cells either side of the party a zoom level shows, or `None`
/// for the whole frame.
///
/// Odd side lengths so the party sits in the middle cell rather than on a
/// seam. The steps are 15, 11 and 7 cells across a 21-wide frame: the first
/// is most of it, the last is the junction you are standing in.
fn window_radius(zoom: u16) -> Option<i32> {
    match zoom {
        STACK_MAP_MIN_ZOOM => None,
        2 => Some(7),
        3 => Some(5),
        _ => Some(3),
    }
}

/// The view a zoom level draws: the whole frame, or a window of it centred
/// on the party.
///
/// A cropped `FrameMapView` rather than a window passed down into
/// `draw_grid`, so the one drawing path stays the one drawing path — the
/// grid loop never learns that zoom exists, and every glyph and colour
/// decision keeps being made in exactly one place.
///
/// The centre comes off the party's own mark, which is the only thing in the
/// view that knows where they are. No party mark, a frame smaller than the
/// window, or the widest zoom all mean "draw the lot" rather than a special
/// case downstream.
fn crop(view: &FrameMapView, zoom: u16) -> FrameMapView {
    let Some(radius) = window_radius(zoom) else {
        return view.clone();
    };
    let side = radius * 2 + 1;
    if view.width <= side || view.height <= side {
        return view.clone();
    }
    let Some((cx, cy)) = view
        .marks
        .iter()
        .find(|(_, mark)| *mark == FrameMapMark::Party)
        .map(|&(cell, _)| cell)
    else {
        return view.clone();
    };

    // Clamped rather than centred at the edges: a window hanging half off
    // the frame wastes half the inset, and would index past the rows.
    let x0 = (cx - radius).clamp(0, view.width - side);
    let y0 = (cy - radius).clamp(0, view.height - side);

    FrameMapView {
        width: side,
        height: side,
        cells: (y0..y0 + side)
            .map(|y| {
                (x0..x0 + side)
                    .map(|x| view.cells[y as usize][x as usize])
                    .collect()
            })
            .collect(),
        // Dropped rather than clamped: a fight marker pinned to the window's
        // edge would report a corridor that never had one.
        marks: view
            .marks
            .iter()
            .filter_map(|&((x, y), mark)| {
                let (lx, ly) = (x - x0, y - y0);
                (lx >= 0 && lx < side && ly >= 0 && ly < side).then_some(((lx, ly), mark))
            })
            .collect(),
        ..view.clone()
    }
}

/// The grid itself: tiles, their glyphs, and the marks over the top.
///
/// The one drawing path both maps share. The full screen and the inset differ
/// in where they put it and what they write around it, never in what a cell
/// looks like — a second glyph table is the drift that would let the inset
/// quietly teach the wrong vocabulary.
fn draw_grid(view: &FrameMapView, painter: &Painter, ox: f32, oy: f32, cell: f32) {
    let inset = cell * (1.0 - TILE_FILL) / 2.0;
    let glyph_px = glyph_px(cell);

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
}

/// The frame map as a permanent inset in the corner of the first-person
/// pane, drawn over the corridor after it.
///
/// Answers "which way am I facing, where have I been" at a glance; the `g`
/// screen still answers "where is the wing I haven't walked", with three
/// times the cell size and the legend that teaches the glyphs.
/// `zoom` is `App::stack_zoom`: the lowest level draws the whole frame,
/// higher ones a window around the party.
pub(super) fn draw_map_inset(
    view: &FrameMapView,
    zoom: u16,
    painter: &Painter,
    pane: Rect,
    m: &Metrics,
) {
    let r = inset_rect(pane, m);
    if r.w <= 0.0 || r.h <= 0.0 {
        return;
    }
    painter.rect(r.x, r.y, r.w, r.h, PANEL_BG);
    painter.rect_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);

    let view = crop(view, zoom);
    let (ox, oy, cell) = layout(&view, r.w, r.h, INSET_FILL);
    if cell <= 0.0 {
        return;
    }
    draw_grid(&view, painter, r.x + ox, r.y + oy, cell);
}

/// Aiming a Wild Jump: the same map, with a cursor on the cell the jump
/// would resolve at — `Mode::FieldRoutineCell`.
///
/// The **third caller** of this file, and deliberately not a fourth copy of
/// the glyph table: it reuses `layout`, `draw_grid`, `tile_color` and
/// `cell_glyph` and adds only the cursor on top. This is the one screen
/// where drift would silently misinform rather than merely look wrong — the
/// player is about to bet their run on what it says, and an unknown cell
/// drawn as anything but unknown is the difference between a gamble and an
/// ambush.
///
/// Drawn whole rather than cropped to `App::stack_zoom`: the point of the
/// screen is picking somewhere you have *not* been, and a window around the
/// party hides exactly that.
pub(super) fn draw_frame_map_cursor(
    view: &FrameMapView,
    cursor: (i32, i32),
    painter: &Painter,
    w: f32,
    h: f32,
    m: &Metrics,
) {
    painter.rect(0.0, 0.0, w, h, PANEL_BG);
    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    let heading = format!(
        "WILD JUMP   target {},{}   {}   Enter to commit, Esc to back out",
        cursor.0,
        cursor.1,
        cursor_reading(view, cursor),
    );
    painter.ui(
        &heading,
        m.inset,
        m.inset + m.font_size as f32,
        m.font_size,
        CYAN,
    );

    let (ox, oy, cell) = layout(view, w, h, GRID_FILL);
    if cell <= 0.0 {
        return;
    }
    draw_grid(view, painter, ox, oy, cell);

    // After the grid and its marks, so the cursor is never painted over by
    // the cell it is sitting on.
    let (px, py) = (ox + cursor.0 as f32 * cell, oy + cursor.1 as f32 * cell);
    painter.rect_lines(px, py, cell, cell, 2.0, CURSOR);

    let legend = "unlit = unmapped, and unmapped is the gamble — solid substrate kills you";
    let dims = painter.measure_ui(legend, m.font_size);
    painter.ui(
        legend,
        (w - dims.width) / 2.0,
        h - m.inset,
        m.font_size,
        TEXT_DIM,
    );
}

/// What the map already knows about the cell under the cursor, in a word.
///
/// Read straight off `FrameMapView` rather than off the frame, so it can
/// never say more than the map is showing — the whole risk of this screen is
/// a readout that quietly launders unknown ground into known ground.
fn cursor_reading(view: &FrameMapView, (x, y): (i32, i32)) -> &'static str {
    let Some(cell) = view
        .cells
        .get(y as usize)
        .and_then(|row| row.get(x as usize))
    else {
        return "off the frame";
    };
    match cell {
        FrameMapCell::Unknown => "UNMAPPED",
        FrameMapCell::Rock => "solid — this kills you",
        _ => "open ground",
    }
}

pub(super) fn draw_frame_map(view: &FrameMapView, painter: &Painter, w: f32, h: f32, m: &Metrics) {
    painter.rect(0.0, 0.0, w, h, PANEL_BG);
    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    let heading = format!(
        "DEEP SCAN   depth {} / {}   link {},{}   facing {}   {:.0}% mapped{}",
        view.depth,
        view.frames,
        view.entrance.0,
        view.entrance.1,
        view.facing,
        view.explored * 100.0,
        // Said out loud, because a map showing corridors the party has not
        // walked is otherwise indistinguishable from a bug in what the map
        // remembers.
        if view.revealed { "   [DEV REVEAL]" } else { "" },
    );
    painter.ui(
        &heading,
        m.inset,
        m.inset + m.font_size as f32,
        m.font_size,
        CYAN,
    );

    let (ox, oy, cell) = layout(view, w, h, GRID_FILL);
    if cell <= 0.0 {
        return;
    }
    draw_grid(view, painter, ox, oy, cell);

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
    use feral_processes_app_core::STACK_MAP_MAX_ZOOM;

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
            revealed: false,
        }
    }

    /// The pane the inset lives in, at a given window size — built from the
    /// same constants `draw_playing_base` divides the screen with, so a
    /// change to the layout moves these tests rather than leaving them
    /// asserting against a pane that no longer exists.
    fn pane(window_w: f32, window_h: f32) -> (f32, f32) {
        (
            window_w * super::super::base::PANE_W,
            window_h * super::super::base::PANE_H,
        )
    }

    /// The two window sizes the design sized the inset against.
    const WINDOWS: [(f32, f32); 2] = [(1920.0, 1080.0), (1280.0, 720.0)];

    /// Below this, a `!` or `+` on the inset is a smudge rather than a
    /// marker. The floor `INSET_FRACTION` was chosen against, kept here so
    /// the number lives with the assertion that checks it.
    const MIN_INSET_GLYPH_PX: u16 = 9;

    /// The inset shares the pane with the heading above it, the status panel
    /// to its right and the message log below — it may overlap none of them.
    #[test]
    fn the_inset_clears_the_heading_and_stays_inside_the_pane() {
        for (ww, wh) in WINDOWS {
            let m = crate::text::ui_metrics(wh);
            let (w, h) = pane(ww, wh);
            let r = inset_rect(Rect::new(0.0, 0.0, w, h), &m);
            assert!(
                r.y >= m.inset + m.font_size as f32,
                "{ww}x{wh}: the inset covers the Facing/Depth/Trace heading"
            );
            assert!(r.x >= 0.0 && r.y >= 0.0);
            assert!(
                r.x + r.w <= w,
                "{ww}x{wh}: the inset runs into the status panel"
            );
            assert!(
                r.y + r.h <= h,
                "{ww}x{wh}: the inset runs into the message log"
            );
        }
    }

    /// The inset shows the whole frame, so every cell of it has to fit —
    /// a grid clipped by its own border would silently hide a wing.
    #[test]
    fn the_whole_frame_fits_inside_the_inset() {
        for (ww, wh) in WINDOWS {
            let m = crate::text::ui_metrics(wh);
            let (w, h) = pane(ww, wh);
            let r = inset_rect(Rect::new(0.0, 0.0, w, h), &m);
            let v = view(21, 21);
            let (ox, oy, cell) = layout(&v, r.w, r.h, INSET_FILL);
            assert!(cell > 0.0);
            assert!(ox >= -0.001 && oy >= -0.001);
            assert!(
                ox + cell * 21.0 <= r.w + 0.001,
                "{ww}x{wh}: the grid overflows the inset"
            );
            assert!(
                oy + cell * 21.0 <= r.h + 0.001,
                "{ww}x{wh}: the grid overflows the inset"
            );
        }
    }

    /// The inset's whole risk is being too small to read. A `!` cache marker
    /// drawn at four pixels is not a map, it is noise — so the smallest
    /// window the game is played at pins the floor, and a later tweak to
    /// `INSET_FRACTION` that crosses it fails here rather than on screen.
    #[test]
    fn inset_glyphs_stay_legible_at_the_smallest_window() {
        let (ww, wh) = WINDOWS[1];
        let m = crate::text::ui_metrics(wh);
        let (w, h) = pane(ww, wh);
        let r = inset_rect(Rect::new(0.0, 0.0, w, h), &m);
        let (_, _, cell) = layout(&view(21, 21), r.w, r.h, INSET_FILL);
        assert!(
            glyph_px(cell) >= MIN_INSET_GLYPH_PX,
            "{ww}x{wh}: inset glyphs are {}px, below the {MIN_INSET_GLYPH_PX}px floor",
            glyph_px(cell)
        );
    }

    #[test]
    fn drawing_the_inset_does_not_panic() {
        let m = crate::text::ui_metrics(720.0);
        let (w, h) = pane(1280.0, 720.0);
        let mut empty = view(0, 0);
        empty.cells = Vec::new();
        empty.marks = vec![((99, 99), FrameMapMark::Party)];
        crate::paint::with_painter(|p| {
            draw_map_inset(
                &view(21, 21),
                STACK_MAP_MIN_ZOOM,
                p,
                Rect::new(0.0, 0.0, w, h),
                &m,
            );
            draw_map_inset(
                &view(31, 11),
                STACK_MAP_MAX_ZOOM,
                p,
                Rect::new(0.0, 0.0, w, h),
                &m,
            );
            draw_map_inset(&empty, STACK_MAP_MAX_ZOOM, p, Rect::new(0.0, 0.0, w, h), &m);
        });
    }

    /// Level 1 is the whole frame, which is what the inset drew before zoom
    /// existed — a player who never presses `+` sees no change.
    #[test]
    fn the_lowest_zoom_shows_the_whole_frame() {
        let v = view(21, 21);
        let cropped = crop(&v, STACK_MAP_MIN_ZOOM);
        assert_eq!((cropped.width, cropped.height), (21, 21));
        assert_eq!(cropped.cells, v.cells);
        assert_eq!(cropped.marks, v.marks);
    }

    /// Zoomed in, the window follows the party rather than the frame's
    /// middle — the whole point is reading the cells around you.
    #[test]
    fn a_zoomed_window_is_centred_on_the_party() {
        let mut v = view(21, 21);
        v.marks = vec![((10, 10), FrameMapMark::Party)];
        let cropped = crop(&v, STACK_MAP_MAX_ZOOM);
        let side = window_radius(STACK_MAP_MAX_ZOOM).unwrap() * 2 + 1;
        assert_eq!((cropped.width, cropped.height), (side, side));
        assert_eq!(
            cropped.marks,
            vec![((side / 2, side / 2), FrameMapMark::Party)],
            "the party is not in the middle of their own window"
        );
    }

    /// Against a wall the window slides rather than hanging off the frame:
    /// half a map of nothing is half a map wasted, and an out-of-range
    /// index would panic.
    #[test]
    fn a_window_at_the_frames_edge_slides_inside_it() {
        for corner in [(0, 0), (20, 20), (0, 20), (20, 0)] {
            let mut v = view(21, 21);
            v.marks = vec![(corner, FrameMapMark::Party)];
            let cropped = crop(&v, STACK_MAP_MAX_ZOOM);
            let side = window_radius(STACK_MAP_MAX_ZOOM).unwrap() * 2 + 1;
            assert_eq!((cropped.width, cropped.height), (side, side));
            assert_eq!(cropped.cells.len(), side as usize);
            for row in &cropped.cells {
                assert_eq!(row.len(), side as usize);
            }
            let (at, _) = cropped.marks[0];
            assert!(
                at.0 >= 0 && at.0 < side && at.1 >= 0 && at.1 < side,
                "the party fell outside their own window at {corner:?}"
            );
        }
    }

    /// A frame smaller than the window asked for is shown whole rather than
    /// padded — and, more to the point, without indexing past its rows.
    #[test]
    fn a_frame_smaller_than_the_window_is_left_alone() {
        let mut v = view(5, 5);
        v.marks = vec![((2, 2), FrameMapMark::Party)];
        let cropped = crop(&v, STACK_MAP_MAX_ZOOM);
        assert_eq!((cropped.width, cropped.height), (5, 5));
    }

    /// A fight marker three cells away has to still be three cells away
    /// after the crop, or the window is a different map rather than a
    /// closer look at the same one.
    #[test]
    fn marks_move_with_the_window() {
        let mut v = view(21, 21);
        v.marks = vec![
            ((10, 10), FrameMapMark::Party),
            ((10, 7), FrameMapMark::Fight),
        ];
        let cropped = crop(&v, STACK_MAP_MAX_ZOOM);
        let party = cropped
            .marks
            .iter()
            .find(|(_, m)| *m == FrameMapMark::Party)
            .unwrap()
            .0;
        let fight = cropped
            .marks
            .iter()
            .find(|(_, m)| *m == FrameMapMark::Fight)
            .unwrap()
            .0;
        assert_eq!((party.0 - fight.0, party.1 - fight.1), (0, 3));
    }

    /// A mark outside the window is dropped rather than clamped to its edge,
    /// which would draw a fight in a corridor that never had one.
    #[test]
    fn a_mark_outside_the_window_is_dropped() {
        let mut v = view(21, 21);
        v.marks = vec![
            ((10, 10), FrameMapMark::Party),
            ((0, 0), FrameMapMark::Fight),
        ];
        let cropped = crop(&v, STACK_MAP_MAX_ZOOM);
        assert!(cropped.marks.iter().all(|(_, m)| *m != FrameMapMark::Fight));
    }

    /// Zooming in has to actually make the cells bigger, which is the whole
    /// reason to do it.
    #[test]
    fn zooming_in_grows_the_cells() {
        let v = view(21, 21);
        let (_, _, wide) = layout(&crop(&v, STACK_MAP_MIN_ZOOM), 300.0, 300.0, INSET_FILL);
        let (_, _, close) = layout(&crop(&v, STACK_MAP_MAX_ZOOM), 300.0, 300.0, INSET_FILL);
        assert!(
            close > wide,
            "the closest zoom draws no larger than the widest"
        );
    }

    #[test]
    fn map_cells_are_square() {
        let (_, _, cell) = layout(&view(21, 21), 1000.0, 640.0, GRID_FILL);
        assert!(cell > 0.0);
        // One cell size drives both axes, so squareness is structural; what
        // needs checking is that it fits the *shorter* axis.
        assert!(cell * 21.0 <= 640.0, "the grid overflows the pane");
    }

    #[test]
    fn the_grid_is_centred_in_the_pane() {
        let (w, h) = (1000.0, 640.0);
        let v = view(21, 21);
        let (ox, oy, cell) = layout(&v, w, h, GRID_FILL);
        assert!((ox + cell * 21.0 / 2.0 - w / 2.0).abs() < 0.001);
        assert!((oy + cell * 21.0 / 2.0 - h / 2.0).abs() < 0.001);
    }

    /// A non-square frame must still get square cells rather than being
    /// stretched to fill the pane — the player counts corridors off this.
    #[test]
    fn an_oblong_frame_still_gets_square_cells() {
        let (_, _, cell) = layout(&view(31, 11), 1000.0, 640.0, GRID_FILL);
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
    /// The cursor screen reads the *map*, never the frame — so a cell the
    /// party has not seen has to come back as unmapped rather than as
    /// whatever is really there. That is the whole warning the screen gives
    /// before the player bets their run on it.
    #[test]
    fn the_cursor_readout_never_says_more_than_the_map_shows() {
        let mut v = view(21, 21);
        v.cells[3][4] = FrameMapCell::Unknown;
        v.cells[3][5] = FrameMapCell::Rock;
        v.cells[3][6] = FrameMapCell::Floor;
        assert_eq!(cursor_reading(&v, (4, 3)), "UNMAPPED");
        assert!(cursor_reading(&v, (5, 3)).contains("kills"));
        assert_eq!(cursor_reading(&v, (6, 3)), "open ground");
        assert_eq!(cursor_reading(&v, (99, 99)), "off the frame");
        assert_eq!(cursor_reading(&v, (-1, 0)), "off the frame");
    }

    /// The cursor and the party's own mark must never share a colour: on
    /// this screen, "where I am" and "where I am aiming" are the two things
    /// the player is reading, and one colour for both is how someone jumps
    /// into a wall.
    #[test]
    fn the_cursor_is_not_the_colour_of_the_party_mark() {
        assert_ne!(CURSOR, mark_glyph(FrameMapMark::Party).1);
    }

    #[test]
    fn drawing_the_cursor_screen_does_not_panic() {
        let m = crate::text::ui_metrics(900.0);
        let mut empty = view(0, 0);
        empty.cells = Vec::new();
        crate::paint::with_painter(|p| {
            draw_frame_map_cursor(&view(21, 21), (0, 0), p, 1000.0, 640.0, &m);
            draw_frame_map_cursor(&view(31, 11), (30, 10), p, 1000.0, 640.0, &m);
            // A cursor off the grid entirely: clipped by the readout and
            // drawn outside the tiles, never indexed with.
            draw_frame_map_cursor(&empty, (99, 99), p, 800.0, 600.0, &m);
        });
    }

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
