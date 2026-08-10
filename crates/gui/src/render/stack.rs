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
//!
//! Each cross-section is drawn across its whole row, not just down the middle
//! of it. Only the middle used to be drawn, with the cells either side
//! consulted for one thing — whether to put a wall there — so an open flank
//! had nothing claiming it and a room came out as lit corridor between two
//! slabs of background. Every column is the same slice offset by whole cells
//! (`column_slice`), which is both the true projection and what makes the
//! columns tile edge to edge.

use super::popup::*;
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

/// The fog the cell marks fade under, which is deliberately gentler than
/// the one the geometry fades under.
///
/// `FOG` is a depth cue: it exists so the corridor reads as receding rather
/// than as concentric rectangles, and dimming a surface to a quarter of
/// itself four cells out is the whole point. A mark is the opposite kind of
/// thing — it is there to be spotted from the far end of the view, and the
/// same curve applied to it puts the thing you are meant to notice at 24%
/// brightness against geometry that is also near-black.
const MARK_FOG: f32 = 0.85;

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

/// What the corridor recedes into where no cell of the cone claims the
/// pixel — the far end of a view that has not run out of corridor, and the
/// outer edges of a room wider than three cells.
///
/// Both used to be `VOID`, and `VOID` is a hole: hard-edged near-black
/// butted against lit geometry reads as a gap in the world rather than as
/// somewhere the light does not reach.
///
/// Bounded from both sides, and the upper bound is the one that is easy to
/// get wrong — it sits behind the *far* end of the corridor as well as
/// beside the near end, so anything brighter than the dimmest wall the fog
/// produces turns the vanishing point into the brightest thing on screen.
/// `the_unlit_fill_is_lighter_than_void_and_darker_than_the_far_wall` holds
/// both.
const UNLIT: Color = Color::new(0.03, 0.07, 0.085, 1.0);

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

/// The same cross-section for the cell `lateral` columns to the party's
/// right, negative for their left.
///
/// One whole cell of horizontal offset per column, and no change of size:
/// cells at one depth are all the same distance away, so they differ only
/// in where they sit across the view. That is what makes the columns tile
/// — the right edge of one is the left edge of the next, at every depth —
/// which in turn is what lets the floor and ceiling of neighbouring cells
/// meet along a shared edge instead of overlapping or leaving a seam.
fn column_slice(depth: usize, lateral: i32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let (l, t, r, b) = slice(depth, w, h);
    let dx = (r - l) * lateral as f32;
    (l + dx, t, r + dx, b)
}

/// How bright a surface `depth` cells away is drawn.
fn shade(depth: usize) -> f32 {
    NEAR_SHADE * FOG.powi(depth as i32)
}

/// How bright the mark on a cell `depth` cells away is drawn — see
/// `MARK_FOG` for why this is not `shade`.
fn mark_shade(depth: usize) -> f32 {
    NEAR_SHADE * MARK_FOG.powi(depth as i32)
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

    // Floored before any geometry, so nothing inside the corridor's own band
    // can come out as hard `VOID`. Two places need it and neither is a bug in
    // the projection: the view runs out of cells before the corridor runs out
    // of floor, and a ray that leaves a three-wide cone sideways as it
    // recedes has left it for good — which is simply what standing in a room
    // wider than the cone looks like. The band only, so the letterbox above
    // and below the corridor stays void.
    let (bl, bt, br, bb) = slice(0, w, h);
    painter.rect(bl, bt, br - bl, bb - bt, UNLIT);

    // The columns either side of the party's line of sight overhang the pane
    // — at depth 0 they are entirely off it — so the pane cuts them.
    painter.clipped(0.0, 0.0, w, h, |painter| {
        // Back to front, so nearer geometry paints over further geometry and
        // no depth sorting is needed. That is the whole occlusion rule, and
        // it stays correct now that the whole row is drawn rather than only
        // its middle: every cell at one depth is the same distance away, so
        // a nearer cell's face covers exactly the pixels it hides. A side
        // passage running past the rock ahead is drawn and stays visible; one
        // that runs behind it is drawn and is painted over.
        for depth in (0..view.cells.len()).rev() {
            let row = &view.cells[depth];
            for i in 0..row.len() {
                draw_cell(painter, row, i, depth, w, h, m);
            }
        }
    });

    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    let heading = format!(
        "Facing {}   Depth {} / {}   ({}, {})   Trace: {}",
        view.facing, view.depth, view.frames, view.position.0, view.position.1, view.trace
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

/// Draws `row[i]` — one cell of the cross-section `depth` ahead — into its
/// own slice of the pane.
///
/// The middle column is the one whose lateral offset works out to zero and
/// has no special case: what used to be the whole of `draw_stack`'s loop
/// body is this, run once per cell. The row comes in whole rather than just
/// the cell, because a wall is a property of the boundary between two cells
/// rather than of either one — and because the offset is measured from the
/// row's own middle, so a row shorter than the rest is drawn centred rather
/// than indexed off the end of.
fn draw_cell(
    painter: &Painter,
    row: &[StackCellView],
    i: usize,
    depth: usize,
    w: f32,
    h: f32,
    m: &Metrics,
) {
    let cell = row[i];
    let lateral = i as i32 - (row.len() / 2) as i32;
    let (nl, nt, nr, nb) = column_slice(depth, lateral, w, h);
    let (fl, ft, fr, fb) = column_slice(depth + 1, lateral, w, h);
    let s = shade(depth);

    let face = draws_as_face(depth, cell);
    if face {
        // This cell's face toward us fills its slice. Anything behind it was
        // drawn already and is now covered, which is exactly right.
        painter.rect(nl, nt, nr - nl, nb - nt, dim(face_color(cell), s));
    } else {
        // An open cell: floor and ceiling recede from this slice to the next.
        painter.poly(&[(nl, nb), (nr, nb), (fr, fb), (fl, fb)], dim(FLOOR, s));
        painter.poly(&[(nl, nt), (nr, nt), (fr, ft), (fl, ft)], dim(CEILING, s));

        // A solid neighbour is seen edge-on from here, which is the wall this
        // draws — the neighbour's own face is its business, and it draws
        // that itself when its turn comes. The outermost column has no
        // neighbour in the cone, so its outer side is left to the unlit fill
        // rather than guessed at: the cone is what the party can see, and a
        // wall invented past its edge would be the view claiming to know.
        if i > 0 && solid(row[i - 1]) {
            painter.poly(&[(nl, nt), (fl, ft), (fl, fb), (nl, nb)], dim(WALL, s));
        }
        if i + 1 < row.len() && solid(row[i + 1]) {
            painter.poly(&[(nr, nt), (fr, ft), (fr, fb), (nr, nb)], dim(WALL, s));
        }
    }

    // Anything worth noticing reads as a glyph rather than as geometry or as
    // a shade — the party needs to spot it down a corridor, and neither a
    // subtle change in floor shape nor one in wall colour carries that far.
    // Drawn last so the surface it names is under it.
    if let Some((mark, color)) = cell_mark(cell) {
        let glyph = mark.to_string();
        let dims = painter.measure_map(&glyph, m.font_size * 2);
        // A face fills its slice, so its mark goes in the middle of it; an
        // open cell's lies on the floor at the far end of it. That is the
        // only thing the two placements disagree about.
        let (cx, baseline) = if face {
            ((nl + nr) / 2.0, (nt + nb) / 2.0 + dims.height / 2.0)
        } else {
            ((fl + fr) / 2.0, fb - (fb - ft) * 0.15)
        };
        painter.map(
            &glyph,
            cx - dims.width / 2.0,
            baseline,
            m.font_size * 2,
            dim(color, mark_shade(depth)),
        );
    }
}

/// The glyph a cell is marked with, and the colour to draw it in. Where it
/// lands is the caller's business: on the floor of an open cell, in the
/// middle of the face of one that fills its slice.
///
/// Carries its own colour rather than being painted a single yellow by the
/// caller, because phase 3's three kinds are not all the same kind of news:
/// a fault and a breakpoint are places to go, corruption is a place not to.
/// The four original marks keep the yellow they have always had.
fn cell_mark(cell: StackCellView) -> Option<(char, Color)> {
    match cell {
        StackCellView::LinkDown => Some(('>', YELLOW)),
        StackCellView::LinkUp => Some(('<', YELLOW)),
        StackCellView::Cache => Some(('!', YELLOW)),
        StackCellView::Lair => Some(('&', YELLOW)),
        // Matched to `frame_map.rs`'s glyphs so the corridor and the map
        // teach one vocabulary. Corruption is the exception in shape only —
        // the map carries it as a tile colour, since it is an area rather
        // than a point — but both are purple.
        StackCellView::Breakpoint => Some(('*', BLUE)),
        StackCellView::Fault => Some(('v', ORANGE)),
        StackCellView::Corruption => Some(('~', MAGENTA)),
        StackCellView::Orphan => Some(('o', GREEN)),
        // The two that are drawn as a face rather than as corridor, and the
        // reason this table is no longer only about floors: `face_color`
        // alone left a door indistinguishable from the rock it is set into
        // once the fog had been through both. Same `+` the map uses, and the
        // same split between an open door and a sealed one.
        StackCellView::Door => Some(('+', ORANGE)),
        StackCellView::SealedDoor => Some(('+', RED)),
        StackCellView::Rock | StackCellView::Floor => None,
    }
}

/// How wide the cell description lets prose run before wrapping. Matches
/// `inventory::DESCRIBE_WRAP_COLUMNS` and for the same reason — a fixed
/// column count rather than a pixel width derived from the window, which
/// varies per machine.
const DESCRIBE_WRAP_COLUMNS: usize = 72;

/// The environment paragraph reached with `x` + a direction underground.
///
/// The same shape as `inventory::draw_item_describe` — the repo's one
/// prose-on-screen pattern, and `wrap_text` its only wrap helper.
pub(super) fn draw_cell_describe(text: Option<&str>, painter: &Painter, m: &Metrics) {
    let mut rows = Vec::new();
    match text {
        Some(text) => rows.extend(
            wrap_text(text, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        ),
        None => rows.push(text_row("Nothing to say about that.")),
    }
    rows.push(text_row(""));
    rows.push(text_row("Any key to go back"));
    draw_popup("You look", PopupSize::Large, &rows, painter, m);
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

    /// The middle column is the projection's origin, not a special case.
    #[test]
    fn the_middle_column_is_the_plain_slice() {
        for depth in 0..5 {
            assert_eq!(
                column_slice(depth, 0, 800.0, 600.0),
                slice(depth, 800.0, 600.0)
            );
        }
    }

    /// Columns tile: one cell's right edge is its neighbour's left edge, at
    /// every depth. That is what makes the floor and ceiling of adjacent
    /// cells meet along a shared edge — a column placed by anything other
    /// than a whole cell width would leave a seam of background between two
    /// lit surfaces, which is the exact fault this change exists to remove.
    #[test]
    fn neighbouring_columns_share_an_edge() {
        let (w, h) = (800.0, 600.0);
        for depth in 0..5 {
            for lateral in -2..2 {
                let (_, _, r, _) = column_slice(depth, lateral, w, h);
                let (l, _, _, _) = column_slice(depth, lateral + 1, w, h);
                assert!(
                    (r - l).abs() < 0.001,
                    "depth {depth}: column {lateral} does not meet {}",
                    lateral + 1
                );
            }
        }
    }

    /// The cell beside the party is off the edge of the view at depth 0 and
    /// swings into it as it recedes. Both halves matter: the first is why
    /// the pane has to clip, and the second is why drawing the flanks fills
    /// the region that used to be void.
    #[test]
    fn the_column_beside_the_party_swings_into_the_pane() {
        let (w, h) = (800.0, 600.0);
        let (l, _, r, _) = column_slice(0, -1, w, h);
        assert!(r <= 0.0, "the cell alongside the party is not in frame");
        assert!(l < 0.0);

        for depth in 1..4 {
            let (_, _, r, _) = column_slice(depth, -1, w, h);
            assert!(
                r > 0.0,
                "depth {depth}'s left-hand cell claims nothing inside the pane"
            );
        }
    }

    /// No pixel inside the corridor's band can be left at hard `VOID`: the
    /// band is floored with `UNLIT` before anything else is drawn, and every
    /// column at every depth sits vertically inside it. A cell that reached
    /// outside the band would be drawn over the letterbox instead, which is
    /// the frame rather than the world.
    #[test]
    fn every_column_stays_inside_the_band_the_unlit_fill_covers() {
        let (w, h) = (800.0, 600.0);
        let (bl, bt, br, bb) = slice(0, w, h);
        assert!(bl <= 0.0 && br >= w, "the fill does not span the pane");
        for depth in 0..6 {
            for lateral in -3..=3 {
                let (_, t, _, b) = column_slice(depth, lateral, w, h);
                assert!(
                    t >= bt - 0.001 && b <= bb + 0.001,
                    "depth {depth} column {lateral} reaches outside the fill"
                );
            }
        }
    }

    /// The fill has to clear the void it replaced without reaching the
    /// dimmest wall the fog produces. Below the floor it is the hole again;
    /// above the far wall the vanishing point — which the fill also sits
    /// behind — becomes the brightest thing on screen.
    ///
    /// The far end is the last row the engine sends, which this file is
    /// never told; `STACK_VIEW_DEPTH` ships at 4, so the deepest surface
    /// drawn is three cells out.
    #[test]
    fn the_unlit_fill_is_lighter_than_void_and_darker_than_the_far_wall() {
        let far_wall = dim(WALL, shade(3));
        for (fill, void, wall) in [
            (UNLIT.r, VOID.r, far_wall.r),
            (UNLIT.g, VOID.g, far_wall.g),
            (UNLIT.b, VOID.b, far_wall.b),
        ] {
            assert!(fill > void, "the fill is still the hole it replaced");
            assert!(fill < wall, "the fill outshines the end of the corridor");
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
    fn plain_corridor_gets_no_marker() {
        assert_eq!(
            cell_mark(StackCellView::LinkDown).map(|(c, _)| c),
            Some('>')
        );
        assert_eq!(cell_mark(StackCellView::LinkUp).map(|(c, _)| c), Some('<'));
        assert_eq!(cell_mark(StackCellView::Floor), None);
        assert_eq!(cell_mark(StackCellView::Rock), None);
    }

    /// Every cell kind that is not plain corridor has to be visible from
    /// down a corridor. The trap this guards is specific: `cell_mark` used
    /// to end in `_ => None`, so a new `StackCellView` variant compiled
    /// perfectly and drew as bare floor — the party would have walked into
    /// corruption with nothing on screen to warn them. The match is now
    /// exhaustive, and this pins the three kinds it exists for.
    #[test]
    fn the_new_cell_kinds_are_marked_in_the_corridor() {
        for cell in [
            StackCellView::Breakpoint,
            StackCellView::Fault,
            StackCellView::Corruption,
        ] {
            assert!(
                cell_mark(cell).is_some(),
                "{cell:?} draws as bare corridor — invisible until stepped on"
            );
        }
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
            trace: "Traced",
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

    /// A door's only cue used to be the colour of its face, and a colour is
    /// exactly what the fog eats: measured, a door three cells off drew as
    /// rgb(33, 26, 11) against rock's rgb(13, 38, 38) — two near-black
    /// blobs. The player learned a corridor ended in a door by walking into
    /// it. A door now carries the same `+` the map marks it with, so what
    /// says "door" is a shape rather than a shade.
    #[test]
    fn a_door_carries_the_maps_glyph_onto_its_face() {
        assert_eq!(cell_mark(StackCellView::Door), Some(('+', ORANGE)));
        assert_eq!(cell_mark(StackCellView::SealedDoor), Some(('+', RED)));
    }

    /// Marks are the informational layer and the fog is a depth cue for
    /// geometry, so they fade on their own gentler curve — otherwise the
    /// glyph that exists to be spotted down a corridor is dimmed to 24% of
    /// itself at the far end of the one it is spotted from.
    #[test]
    fn a_mark_fades_more_slowly_than_the_surface_it_sits_on() {
        let mut last = f32::MAX;
        for depth in 0..6 {
            let s = mark_shade(depth);
            assert!(s <= last, "depth {depth} marks brighten with distance");
            assert!(
                s >= shade(depth),
                "depth {depth} marks fade faster than the geometry"
            );
            last = s;
        }
        assert!(
            mark_shade(3) > shade(3) * 2.0,
            "the far end of the view is where the fog was eating the marks"
        );
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
            trace: "Quiet",
            position: (0, 0),
            cells: Vec::new(),
            standing_on: None,
        };
        let single = StackView {
            depth: 1,
            frames: 1,
            facing: "S",
            trace: "Hunted",
            position: (0, 0),
            cells: vec![vec![StackCellView::Floor]],
            standing_on: None,
        };
        crate::paint::with_painter(|p| {
            draw_stack(&empty, p, 800.0, 600.0, &m);
            draw_stack(&single, p, 800.0, 600.0, &m);
        });
    }

    /// `engine::MAX_UNDERFOOT_LINE` is a character budget; this is what makes
    /// it a real one. The UI font is DejaVu Sans Mono, so the widest possible
    /// line of that many characters is that many of any glyph — measured
    /// against the corridor pane at the narrowest window the UI supports.
    #[test]
    fn the_longest_underfoot_line_fits_the_stack_pane() {
        const NARROWEST_WINDOW: (f32, f32) = (1280.0, 720.0);
        let m = crate::text::ui_metrics(NARROWEST_WINDOW.1);
        let pane_w = NARROWEST_WINDOW.0 * super::super::base::PANE_W;
        let longest = "M".repeat(feral_processes_engine::MAX_UNDERFOOT_LINE);
        crate::paint::with_painter(|p| {
            let dims = p.measure_ui(&longest, m.font_size);
            assert!(
                dims.width <= pane_w,
                "{} chars measured {:.1}px against a {:.1}px pane",
                feral_processes_engine::MAX_UNDERFOOT_LINE,
                dims.width,
                pane_w
            );
        });
    }
}
