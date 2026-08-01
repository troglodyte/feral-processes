//! The first-person Stack view.
//!
//! Draws `StackView` as a receding corridor. The engine has already
//! rotated the cells into view space (`cells[ahead][lateral]`, middle column
//! straight ahead), so nothing here knows which way north is — this file
//! only ever draws forward.
//!
//! The projection is the classic blobber one: successive cross-sections of
//! the corridor, each one cell further away than the last, shrinking toward
//! a vanishing point at the centre of the pane. Walls between two
//! cross-sections are trapezoids, which is what `Painter::poly` exists for.

use super::*;
use feral_processes_engine::{StackCellView, StackView};

/// How much narrower each successive slice is. Tuned by eye: much above this
/// and a four-deep corridor barely converges, much below and depth 2 is
/// already a dot.
const SHRINK: f32 = 0.58;

/// Brightness of the nearest wall, and the factor each cell of distance
/// multiplies it by. The fog is what makes distance legible at all — with
/// flat shading every slice is the same colour and the corridor reads as
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
const DOOR: Color = Color::new(0.55, 0.42, 0.18, 1.0);
const SEALED: Color = Color::new(0.62, 0.20, 0.24, 1.0);

/// The corridor's cross-section `depth` cells away, in pane-local pixels.
///
/// Returned as (left, top, right, bottom). Every slice is centred on the
/// vanishing point at the pane's middle, so the walls converge there.
fn slice(depth: usize, w: f32, h: f32) -> (f32, f32, f32, f32) {
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

/// Whether this cell is drawn as a face filling its slice rather than as
/// more corridor. A door is not rock, but you cannot see past it, and
/// drawing it as open would be the view lying about what is ahead.
fn solid(cell: StackCellView) -> bool {
    matches!(
        cell,
        StackCellView::Rock | StackCellView::Door | StackCellView::SealedDoor
    )
}

/// Whether the cell `depth` away fills its slice with a face rather than
/// opening into more corridor.
///
/// Never at depth 0. That cell is the one the party is standing in, and a
/// doorway you are inside is open around you — a door is the one cell that
/// both blocks sight and can be walked onto, so this is reachable.
fn draws_as_face(depth: usize, cell: StackCellView) -> bool {
    depth > 0 && solid(cell)
}

/// The colour a solid face is drawn in — doors stand out from the rock they
/// are set into, and a sealed one stands out from a plain one.
fn face_color(cell: StackCellView) -> Color {
    match cell {
        StackCellView::Door => DOOR,
        StackCellView::SealedDoor => SEALED,
        _ => WALL,
    }
}

/// Draws the corridor into the pane at the origin, `w` by `h`.
pub(super) fn draw_stack(view: &StackView, painter: &Painter, w: f32, h: f32, m: &Metrics) {
    painter.rect(0.0, 0.0, w, h, VOID);

    let middle = view.cells.first().map(|row| row.len() / 2).unwrap_or(0);

    // Back to front, so nearer geometry paints over further geometry and no
    // depth sorting is needed.
    for depth in (0..view.cells.len()).rev() {
        let row = &view.cells[depth];
        let (nl, nt, nr, nb) = slice(depth, w, h);
        let (fl, ft, fr, fb) = slice(depth + 1, w, h);
        let s = shade(depth);

        if draws_as_face(depth, row[middle]) {
            // The corridor is blocked here: this cell's face toward us fills
            // the whole slice. Anything beyond it was drawn already and is
            // now covered, which is exactly right.
            painter.rect(nl, nt, nr - nl, nb - nt, dim(face_color(row[middle]), s));
            continue;
        }

        // An open cell: floor and ceiling recede from this slice to the next.
        painter.poly(&[(nl, nb), (nr, nb), (fr, fb), (fl, fb)], dim(FLOOR, s));
        painter.poly(&[(nl, nt), (nr, nt), (fr, ft), (fl, ft)], dim(CEILING, s));

        if middle > 0 && solid(row[middle - 1]) {
            painter.poly(&[(nl, nt), (fl, ft), (fl, fb), (nl, nb)], dim(WALL, s));
        }
        if middle + 1 < row.len() && solid(row[middle + 1]) {
            painter.poly(&[(nr, nt), (fr, ft), (fr, fb), (nr, nb)], dim(WALL, s));
        }

        // Links read as a marker on the floor of the cell they're in rather
        // than as geometry — the party needs to spot them down a corridor,
        // and a subtle change in floor shape would not carry that far.
        if let Some(mark) = link_mark(row[middle]) {
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

    let heading = format!(
        "Facing {}   Depth {} / {}   ({}, {})",
        view.facing, view.depth, view.frames, view.position.0, view.position.1
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

fn link_mark(cell: StackCellView) -> Option<char> {
    match cell {
        StackCellView::LinkDown => Some('>'),
        StackCellView::LinkUp => Some('<'),
        StackCellView::Cache => Some('!'),
        StackCellView::Lair => Some('&'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_shrink_monotonically_with_distance() {
        let (w, h) = (800.0, 600.0);
        let mut last_width = f32::MAX;
        for depth in 0..6 {
            let (l, _, r, _) = slice(depth, w, h);
            let width = r - l;
            assert!(
                width < last_width,
                "depth {depth} is no narrower than the slice in front of it"
            );
            assert!(width > 0.0);
            last_width = width;
        }
    }

    #[test]
    fn every_slice_is_centred_on_the_vanishing_point() {
        let (w, h) = (800.0, 600.0);
        for depth in 0..6 {
            let (l, t, r, b) = slice(depth, w, h);
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
    fn the_nearest_slice_spans_the_full_width_of_the_pane() {
        let (l, _, r, _) = slice(0, 800.0, 600.0);
        assert!((l - 0.0).abs() < 0.001);
        assert!((r - 800.0).abs() < 0.001);
    }

    #[test]
    fn slices_nest_strictly_inside_one_another() {
        let (w, h) = (800.0, 600.0);
        for depth in 0..5 {
            let (nl, nt, nr, nb) = slice(depth, w, h);
            let (fl, ft, fr, fb) = slice(depth + 1, w, h);
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
    fn only_links_get_a_marker() {
        assert_eq!(link_mark(StackCellView::LinkDown), Some('>'));
        assert_eq!(link_mark(StackCellView::LinkUp), Some('<'));
        assert_eq!(link_mark(StackCellView::Floor), None);
        assert_eq!(link_mark(StackCellView::Rock), None);
    }

    /// `slice(0)` spans the whole pane, so a face drawn at depth 0 fills the
    /// view and `continue` skips every corridor surface behind it — the
    /// screen becomes one flat rectangle of door. The party is not stuck,
    /// but nothing on screen tells them which way is out.
    #[test]
    fn the_cell_the_party_is_standing_in_is_never_drawn_as_a_face() {
        assert!(!draws_as_face(0, StackCellView::Door));
        assert!(!draws_as_face(0, StackCellView::SealedDoor));
        assert!(
            draws_as_face(1, StackCellView::Door),
            "a door ahead of the party is still a face — you cannot see past it"
        );
        assert!(draws_as_face(1, StackCellView::Rock));
    }

    #[test]
    fn only_rock_counts_as_a_wall() {
        assert!(solid(StackCellView::Rock));
        assert!(!solid(StackCellView::Floor));
        assert!(
            !solid(StackCellView::LinkDown),
            "links are walkable — treating them as wall would seal the way down"
        );
        assert!(!solid(StackCellView::LinkUp));
    }

    /// A view whose middle column is `ahead`, with `flank` either side.
    fn view(ahead: &[StackCellView], flank: StackCellView) -> StackView {
        StackView {
            depth: 2,
            frames: 4,
            facing: "N",
            position: (3, 4),
            cells: ahead.iter().map(|&c| vec![flank, c, flank]).collect(),
            standing_on: Some("A link leads down".to_string()),
        }
    }

    /// The projection maths above is tested in isolation; this drives the
    /// whole draw through a real `Painter` on a headless egui context, which
    /// is what catches an indexing panic against a shape of view the pure
    /// tests never build. It cannot assert about pixels — nothing without a
    /// display can — so it asserts only that every shape gets drawn.
    #[test]
    fn drawing_every_shape_of_corridor_does_not_panic() {
        use StackCellView::*;
        let m = crate::text::ui_metrics(900.0);
        let cases = [
            view(&[Floor, Floor, Floor, Floor], Rock),
            view(&[Floor, Rock, Rock, Rock], Rock), // blocked one step ahead
            view(&[Rock, Rock, Rock, Rock], Rock),  // sealed in
            view(&[Floor, Floor, Floor, Floor], Floor), // open hall, no walls
            view(&[LinkUp, Floor, LinkDown, Floor], Rock),
            view(&[Door, Floor, Floor, Floor], Rock), // standing in a doorway
            view(&[Floor, Door, Floor, Floor], Rock), // a shut door one step on
        ];
        crate::paint::with_painter(|p| {
            for case in &cases {
                draw_stack(case, p, 1000.0, 640.0, &m);
            }
        });
    }

    /// A renderer must survive whatever the engine hands it, including the
    /// shapes it never actually produces — a view built before the party has
    /// a level, say.
    #[test]
    fn drawing_a_degenerate_view_does_not_panic() {
        let m = crate::text::ui_metrics(900.0);
        let empty = StackView {
            depth: 1,
            frames: 1,
            facing: "N",
            position: (0, 0),
            cells: Vec::new(),
            standing_on: None,
        };
        let single = StackView {
            depth: 1,
            frames: 1,
            facing: "S",
            position: (0, 0),
            cells: vec![vec![StackCellView::Floor]],
            standing_on: None,
        };
        crate::paint::with_painter(|p| {
            draw_stack(&empty, p, 800.0, 600.0, &m);
            draw_stack(&single, p, 800.0, 600.0, &m);
        });
    }
}
