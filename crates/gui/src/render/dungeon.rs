//! The first-person dungeon view.
//!
//! Draws `DungeonView` as a receding corridor. The engine has already
//! rotated the cells into view space (`cells[ahead][lateral]`, middle column
//! straight ahead), so nothing here knows which way north is — this file
//! only ever draws forward.
//!
//! The projection is the classic blobber one: a stack of nested "frames",
//! each the cross-section of the corridor at one cell of distance, shrinking
//! toward a vanishing point at the centre of the pane. Walls between two
//! frames are trapezoids, which is what `Painter::poly` exists for.

use super::*;
use feral_processes_engine::{DungeonCellView, DungeonView};

/// How much narrower each successive frame is. Tuned by eye: much above this
/// and a four-deep corridor barely converges, much below and depth 2 is
/// already a dot.
const SHRINK: f32 = 0.58;

/// Brightness of the nearest wall, and the factor each cell of distance
/// multiplies it by. The fog is what makes distance legible at all — with
/// flat shading every frame is the same colour and the corridor reads as
/// concentric rectangles rather than as depth.
const NEAR_SHADE: f32 = 1.0;
const FOG: f32 = 0.62;

/// Fraction of the pane's half-height the corridor occupies at depth 0.
/// Below 1.0 so floor and ceiling are visible bands rather than meeting the
/// pane edge exactly.
const CORRIDOR_HEIGHT: f32 = 0.82;

const WALL: Color = Color::new(0.22, 0.62, 0.62, 1.0);
const FLOOR: Color = Color::new(0.10, 0.26, 0.30, 1.0);
const CEILING: Color = Color::new(0.06, 0.14, 0.20, 1.0);
const VOID: Color = Color::new(0.02, 0.03, 0.05, 1.0);

/// The corridor's cross-section `depth` cells away, in pane-local pixels.
///
/// Returned as (left, top, right, bottom). Every frame is centred on the
/// vanishing point at the pane's middle, so the walls converge there.
fn frame(depth: usize, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let scale = SHRINK.powi(depth as i32);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let half_w = w / 2.0 * scale;
    let half_h = h / 2.0 * CORRIDOR_HEIGHT * scale;
    (cx - half_w, cy - half_h, cx + half_w, cy + half_h)
}

/// How bright a surface `depth` cells away is drawn.
fn shade(depth: usize) -> f32 {
    NEAR_SHADE * FOG.powi(depth as i32)
}

fn dim(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

fn solid(cell: DungeonCellView) -> bool {
    cell == DungeonCellView::Rock
}

/// Draws the corridor into the pane at the origin, `w` by `h`.
pub(super) fn draw_dungeon(view: &DungeonView, painter: &Painter, w: f32, h: f32, m: &Metrics) {
    painter.rect(0.0, 0.0, w, h, VOID);

    let middle = view.cells.first().map(|row| row.len() / 2).unwrap_or(0);

    // Back to front, so nearer geometry paints over further geometry and no
    // depth sorting is needed.
    for depth in (0..view.cells.len()).rev() {
        let row = &view.cells[depth];
        let (nl, nt, nr, nb) = frame(depth, w, h);
        let (fl, ft, fr, fb) = frame(depth + 1, w, h);
        let s = shade(depth);

        if solid(row[middle]) {
            // The corridor is blocked here: this cell's face toward us fills
            // the whole frame. Anything beyond it was drawn already and is
            // now covered, which is exactly right.
            painter.rect(nl, nt, nr - nl, nb - nt, dim(WALL, s));
            continue;
        }

        // An open cell: floor and ceiling recede from this frame to the next.
        painter.poly(&[(nl, nb), (nr, nb), (fr, fb), (fl, fb)], dim(FLOOR, s));
        painter.poly(&[(nl, nt), (nr, nt), (fr, ft), (fl, ft)], dim(CEILING, s));

        if middle > 0 && solid(row[middle - 1]) {
            painter.poly(&[(nl, nt), (fl, ft), (fl, fb), (nl, nb)], dim(WALL, s));
        }
        if middle + 1 < row.len() && solid(row[middle + 1]) {
            painter.poly(&[(nr, nt), (fr, ft), (fr, fb), (nr, nb)], dim(WALL, s));
        }

        // Stairs read as a marker on the floor of the cell they're in rather
        // than as geometry — the party needs to spot them down a corridor,
        // and a subtle change in floor shape would not carry that far.
        if let Some(mark) = stair_mark(row[middle]) {
            let glyph = mark.to_string();
            let dims = painter.measure_map(&glyph, m.font_size * 2);
            painter.map(
                &glyph,
                (fl + fr) / 2.0 - dims.width / 2.0,
                fb - (fb - ft) * 0.15,
                m.font_size * 2,
                dim(YELLOW, s),
            );
        }
    }

    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    // Compass and depth: the only navigational aid there is, since the
    // dungeon has no auto-map.
    let heading = format!(
        "Facing {}   Depth {}   ({}, {})",
        view.facing, view.depth, view.position.0, view.position.1
    );
    painter.ui(
        &heading,
        m.inset,
        m.inset + m.font_size as f32,
        m.font_size,
        CYAN,
    );

    if let Some(standing) = &view.standing_on {
        let dims = painter.measure_ui(standing, m.font_size);
        painter.ui(
            standing,
            (w - dims.width) / 2.0,
            h - m.inset,
            m.font_size,
            YELLOW,
        );
    }
}

fn stair_mark(cell: DungeonCellView) -> Option<char> {
    match cell {
        DungeonCellView::StairsDown => Some('>'),
        DungeonCellView::StairsUp => Some('<'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_shrink_monotonically_with_distance() {
        let (w, h) = (800.0, 600.0);
        let mut last_width = f32::MAX;
        for depth in 0..6 {
            let (l, _, r, _) = frame(depth, w, h);
            let width = r - l;
            assert!(
                width < last_width,
                "depth {depth} is no narrower than the frame in front of it"
            );
            assert!(width > 0.0);
            last_width = width;
        }
    }

    #[test]
    fn every_frame_is_centred_on_the_vanishing_point() {
        let (w, h) = (800.0, 600.0);
        for depth in 0..6 {
            let (l, t, r, b) = frame(depth, w, h);
            assert!(
                ((l + r) / 2.0 - w / 2.0).abs() < 0.001,
                "depth {depth} is off-centre horizontally"
            );
            assert!(
                ((t + b) / 2.0 - h / 2.0).abs() < 0.001,
                "depth {depth} is off-centre vertically"
            );
        }
    }

    #[test]
    fn the_nearest_frame_spans_the_full_width_of_the_pane() {
        let (l, _, r, _) = frame(0, 800.0, 600.0);
        assert!((l - 0.0).abs() < 0.001);
        assert!((r - 800.0).abs() < 0.001);
    }

    #[test]
    fn frames_nest_strictly_inside_one_another() {
        let (w, h) = (800.0, 600.0);
        for depth in 0..5 {
            let (nl, nt, nr, nb) = frame(depth, w, h);
            let (fl, ft, fr, fb) = frame(depth + 1, w, h);
            assert!(fl > nl && fr < nr, "depth {depth} walls cross over");
            assert!(ft > nt && fb < nb, "depth {depth} floor and ceiling cross");
        }
    }

    #[test]
    fn distance_darkens_and_never_brightens() {
        let mut last = f32::MAX;
        for depth in 0..6 {
            let s = shade(depth);
            assert!(s < last, "depth {depth} is not darker than the one before");
            assert!(s > 0.0, "fog must never reach pure black");
            last = s;
        }
    }

    #[test]
    fn only_stairs_get_a_marker() {
        assert_eq!(stair_mark(DungeonCellView::StairsDown), Some('>'));
        assert_eq!(stair_mark(DungeonCellView::StairsUp), Some('<'));
        assert_eq!(stair_mark(DungeonCellView::Floor), None);
        assert_eq!(stair_mark(DungeonCellView::Rock), None);
    }

    #[test]
    fn only_rock_counts_as_a_wall() {
        assert!(solid(DungeonCellView::Rock));
        assert!(!solid(DungeonCellView::Floor));
        assert!(
            !solid(DungeonCellView::StairsDown),
            "stairs are walkable — treating them as wall would seal the way down"
        );
        assert!(!solid(DungeonCellView::StairsUp));
    }
}
