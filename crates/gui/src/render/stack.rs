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
use feral_processes_engine::components::POWER_MAX;
use feral_processes_engine::{StackCellView, StackView};

/// How much narrower each successive slice is. Tuned by eye: much above this
/// and a four-deep corridor barely converges, much below and depth 2 is
/// already a dot.
const SHRINK: f32 = 0.58;

/// Brightness of the nearest wall, and the factor each cell of distance
/// multiplies it by. The fog is what makes distance legible at all — with
/// flat shading every slice is the same colour and the corridor reads as
/// concentric rectangles rather than as depth.
///
/// The fog thickens as the player's Power reserve drains, `FOG_FULL` down to
/// `FOG_EMPTY` — this view's half of the same statement `render/base.rs`'s
/// `vignette_floor` makes on the surface map, turned down the axis this view
/// actually has. On a grid "away from the player" is radial; down a corridor
/// it is depth, so the corridor's edge-darkening *is* its fog.
///
/// It thickens the fog rather than dimming the corridor outright for the
/// reason the surface map leaves its centre alone: the nearest cell is
/// `powi(0)` and so is identical at every reserve, which keeps a drained
/// reserve reading as sight closing in rather than as the renderer having
/// faulted. And it moves no engine number — `view_cone` and `visible_rows`
/// still hand over exactly the cells they always did, so this stays a
/// **look** and never becomes a sight-range nerf wearing one.
const NEAR_SHADE: f32 = 1.0;
const FOG_FULL: f32 = 0.62;
const FOG_EMPTY: f32 = 0.50;

/// The fog the cell marks fade under, which is deliberately gentler than
/// the one the geometry fades under.
///
/// `FOG_FULL` is a depth cue: it exists so the corridor reads as receding rather
/// than as concentric rectangles, and dimming a surface to a quarter of
/// itself four cells out is the whole point. A mark is the opposite kind of
/// thing — it is there to be spotted from the far end of the view, and the
/// same curve applied to it puts the thing you are meant to notice at 24%
/// brightness against geometry that is also near-black.
///
/// **Which is also why it takes no reserve term.** A thickening fog is
/// allowed to swallow the walls; the glyph telling the player there is a
/// door down there is the one thing a drained reserve must not take, exactly
/// as `VIGNETTE_FLOOR_EMPTY` is floored so the surface map keeps a hostile
/// at its edge visible.
const MARK_FOG: f32 = 0.85;

/// Fraction of the pane's half-height the corridor occupies at depth 0.
/// Below 1.0 so floor and ceiling are visible bands rather than meeting the
/// pane edge exactly.
const CORRIDOR_HEIGHT: f32 = 0.82;

const WALL: Color = Color::new(0.22, 0.62, 0.62, 1.0);
const FLOOR: Color = Color::new(0.10, 0.26, 0.30, 1.0);
const CEILING: Color = Color::new(0.06, 0.14, 0.20, 1.0);
pub(super) const VOID: Color = Color::new(0.02, 0.03, 0.05, 1.0);
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
fn slice(depth: usize, pane: Rect) -> (f32, f32, f32, f32) {
    let scale = SHRINK.powi(depth as i32);
    let (cx, cy) = (pane.x + pane.w / 2.0, pane.y + pane.h / 2.0);
    let half_w = pane.w / 2.0 * scale;
    let half_h = pane.h / 2.0 * CORRIDOR_HEIGHT * scale;
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
fn column_slice(depth: usize, lateral: i32, pane: Rect) -> (f32, f32, f32, f32) {
    let (l, t, r, b) = slice(depth, pane);
    let dx = (r - l) * lateral as f32;
    (l + dx, t, r + dx, b)
}

/// How thick the fog is at a given Power reserve. See `FOG_FULL`.
fn fog(power: f32) -> f32 {
    let fraction = (power / POWER_MAX).clamp(0.0, 1.0);
    FOG_EMPTY + (FOG_FULL - FOG_EMPTY) * fraction
}

/// How bright a surface `depth` cells away is drawn, at a given reserve.
fn shade(depth: usize, power: f32) -> f32 {
    NEAR_SHADE * fog(power).powi(depth as i32)
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

/// The colours of the two side walls of an open cell: the surface each solid
/// neighbour presents edge-on, or `None` where the cone has no neighbour that
/// way or the neighbour opens into more corridor.
///
/// It reads `face_color` rather than painting a flat `WALL`, and that is the
/// whole point of the function existing. `solid` has always included both
/// doors, so a door beside the party did stop the corridor — but the wall it
/// stopped it with was drawn the same cyan as rock, and a door one step to
/// your left was indistinguishable from the wall it is set into. Dead ahead
/// the same door draws brown with a `+` over it, because the *face* branch
/// asks `face_color` and this one did not.
///
/// The colour is all a door beside the party has. At depth 0 the neighbour's
/// own column is entirely off the pane
/// (`the_column_beside_the_party_swings_into_the_pane`), so it never draws
/// its own face and its `+` is clipped away — this shared boundary is the
/// only pixel the door owns.
fn flank_colors(row: &[StackCellView], i: usize) -> (Option<Color>, Option<Color>) {
    let side = |cell: Option<&StackCellView>| cell.copied().filter(|&c| solid(c)).map(face_color);
    (
        side(i.checked_sub(1).and_then(|j| row.get(j))),
        side(row.get(i + 1)),
    )
}

/// Draws the corridor into `pane`.
///
/// The pane takes its origin from the caller rather than sitting at the
/// window's, because the base stock strip claims a row above it. Every
/// piece of the projection derives from `slice`, so that origin is stated
/// once here and the whole corridor follows it.
pub(super) fn draw_stack(view: &StackView, painter: &Painter, pane: Rect, m: &Metrics, power: f32) {
    // Floored before any geometry, so nothing inside the corridor's own band
    // can come out as hard `VOID`. Two places need it and neither is a bug in
    // the projection: the view runs out of cells before the corridor runs out
    // of floor, and a ray that leaves a three-wide cone sideways as it
    // recedes has left it for good — which is simply what standing in a room
    // wider than the cone looks like. The band only, so the letterbox above
    // and below the corridor stays void.
    let (bl, bt, br, bb) = slice(0, pane);
    painter.rect(bl, bt, br - bl, bb - bt, UNLIT);

    // The columns either side of the party's line of sight overhang the pane
    // — at depth 0 they are entirely off it — so the pane cuts them.
    painter.clipped(pane.x, pane.y, pane.w, pane.h, |painter| {
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
                draw_cell(painter, row, i, depth, pane, m, power);
            }
        }
    });

    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, BORDER);

    let heading = format!(
        "Facing {}   Depth {} / {}   ({}, {})   Trace: {}",
        view.facing, view.depth, view.frames, view.position.0, view.position.1, view.trace
    );
    painter.ui(
        &heading,
        pane.x + m.inset,
        pane.y + m.inset + m.font_size as f32,
        m.font_size,
        CYAN,
    );

    if let Some(standing) = &view.standing_on {
        let dims = painter.measure_ui(standing, m.font_size);
        painter.ui(
            standing,
            pane.x + (pane.w - dims.width) / 2.0,
            pane.y + pane.h - m.inset,
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
    pane: Rect,
    m: &Metrics,
    power: f32,
) {
    let cell = row[i];
    let lateral = i as i32 - (row.len() / 2) as i32;
    let (nl, nt, nr, nb) = column_slice(depth, lateral, pane);
    let (fl, ft, fr, fb) = column_slice(depth + 1, lateral, pane);
    let s = shade(depth, power);

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
        // `flank_colors` is what makes a door beside the party read as a
        // door rather than as the rock it is set into.
        let (left, right) = flank_colors(row, i);
        if let Some(color) = left {
            painter.poly(&[(nl, nt), (fl, ft), (fl, fb), (nl, nb)], dim(color, s));
        }
        if let Some(color) = right {
            painter.poly(&[(nr, nt), (fr, ft), (fr, fb), (nr, nb)], dim(color, s));
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
        // `$` for a market, the same glyph the surface trader draws with on
        // the zone map — one vocabulary for "somebody is selling here",
        // wherever the party meets it.
        StackCellView::Market => Some(('$', YELLOW)),
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

/// `draw_cell_describe`'s rows, built without touching a `Painter` — the
/// same split `building::build_direction_rows` uses for its prompt, so the
/// wrapping and the "nothing to say" fallback are each directly assertable
/// instead of only reachable through a paint call nothing can inspect.
fn cell_describe_rows(text: Option<&str>) -> Vec<Row> {
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
    rows
}

/// The environment paragraph reached with `x` + a direction underground.
///
/// The same shape as `inventory::draw_gear_inspect` — the repo's one
/// prose-on-screen pattern, and `wrap_text` its only wrap helper.
pub(super) fn draw_cell_describe(
    text: Option<&str>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "You look",
        PopupSize::Large,
        &cell_describe_rows(text),
        refusal,
        painter,
        m,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_shrink_monotonically_with_distance() {
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        let mut last_width = f32::MAX;
        for depth in 0..6 {
            let (l, _, r, _) = slice(depth, pane);
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
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (w, h) = (pane.w, pane.h);
        for depth in 0..6 {
            let (l, t, r, b) = slice(depth, pane);
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
        let (l, _, r, _) = slice(0, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert!((l - 0.0).abs() < 0.001);
        assert!((r - 800.0).abs() < 0.001);
    }

    #[test]
    fn slices_nest_strictly_inside_one_another() {
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        for depth in 0..5 {
            let (nl, nt, nr, nb) = slice(depth, pane);
            let (fl, ft, fr, fb) = slice(depth + 1, pane);
            assert!(fl > nl && fr < nr, "depth {depth} walls cross over");
            assert!(ft > nt && fb < nb, "depth {depth} floor and ceiling cross");
        }
    }

    /// The middle column is the projection's origin, not a special case.
    #[test]
    fn the_middle_column_is_the_plain_slice() {
        for depth in 0..5 {
            assert_eq!(
                column_slice(depth, 0, Rect::new(0.0, 0.0, 800.0, 600.0)),
                slice(depth, Rect::new(0.0, 0.0, 800.0, 600.0))
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
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        for depth in 0..5 {
            for lateral in -2..2 {
                let (_, _, r, _) = column_slice(depth, lateral, pane);
                let (l, _, _, _) = column_slice(depth, lateral + 1, pane);
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
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (l, _, r, _) = column_slice(0, -1, pane);
        assert!(r <= 0.0, "the cell alongside the party is not in frame");
        assert!(l < 0.0);

        for depth in 1..4 {
            let (_, _, r, _) = column_slice(depth, -1, pane);
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
        let pane = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (bl, bt, br, bb) = slice(0, pane);
        assert!(
            bl <= pane.x && br >= pane.x + pane.w,
            "the fill does not span the pane"
        );
        for depth in 0..6 {
            for lateral in -3..=3 {
                let (_, t, _, b) = column_slice(depth, lateral, pane);
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
        let far_wall = dim(WALL, shade(3, POWER_MAX));
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
            let s = shade(depth, POWER_MAX);
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

    /// A door beside the party has to read as a door. The corridor stops at
    /// one either way — `solid` has always included both doors — but the
    /// wall it stopped at was painted the same cyan as rock, so the player
    /// found out a door was there by walking into it. Dead ahead the same
    /// door draws brown, which is what made this legible as a bug rather
    /// than as the view simply not modelling doors.
    #[test]
    fn a_door_beside_the_party_is_not_drawn_as_rock() {
        let row = [
            StackCellView::Door,
            StackCellView::Floor,
            StackCellView::Rock,
        ];
        let (left, right) = flank_colors(&row, 1);
        assert_eq!(
            left,
            Some(DOOR),
            "a door to the left draws as the rock it is set into"
        );
        assert_eq!(right, Some(WALL), "rock to the right must stay rock");
    }

    /// The sealed/plain split the face branch already draws is the same one
    /// seen edge-on: a seal is a locked way on, and mistaking it for a door
    /// you can walk through sends the party to find a key they don't need.
    #[test]
    fn a_sealed_door_beside_the_party_keeps_its_own_colour() {
        let row = [
            StackCellView::Floor,
            StackCellView::Floor,
            StackCellView::SealedDoor,
        ];
        assert_eq!(flank_colors(&row, 1).1, Some(SEALED));
    }

    /// Open corridor either side claims no wall, and neither does the edge
    /// of the cone — the outermost column's outer side is left to the unlit
    /// fill rather than guessed at.
    #[test]
    fn an_open_flank_and_the_edge_of_the_cone_draw_no_wall() {
        let open = [
            StackCellView::Floor,
            StackCellView::Floor,
            StackCellView::Cache,
        ];
        assert_eq!(flank_colors(&open, 1), (None, None));

        let walled = [
            StackCellView::Floor,
            StackCellView::Rock,
            StackCellView::Floor,
        ];
        assert_eq!(
            flank_colors(&walled, 0),
            (None, Some(WALL)),
            "the cone's edge invented a wall it cannot see"
        );
        assert_eq!(flank_colors(&walled, 2), (Some(WALL), None));
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
                draw_stack(case, p, Rect::new(0.0, 0.0, 1000.0, 640.0), &m, POWER_MAX);
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
                s >= shade(depth, POWER_MAX),
                "depth {depth} marks fade faster than the geometry"
            );
            last = s;
        }
        assert!(
            mark_shade(3) > shade(3, POWER_MAX) * 2.0,
            "the far end of the view is where the fog was eating the marks"
        );
    }

    /// The corridor's half of the Power vignette. A drained reserve is meant
    /// to be read as the dark closing in from the far end, which is where
    /// this view's "edge" is.
    #[test]
    fn a_drained_reserve_thickens_the_fog() {
        assert_eq!(fog(POWER_MAX), FOG_FULL);
        assert_eq!(fog(0.0), FOG_EMPTY);

        let mut previous = f32::MAX;
        for i in 0..=10 {
            let f = fog(POWER_MAX * (10 - i) as f32 / 10.0);
            assert!(
                f <= previous,
                "the fog thinned at step {i}: {f} after {previous}"
            );
            previous = f;
        }
        assert!(
            shade(3, 0.0) < shade(3, POWER_MAX),
            "the far end must darken"
        );
    }

    /// What keeps it a *vignette* rather than a wash: `powi(0)` is 1 whatever
    /// the fog, so the cell the party is standing in front of is pixel-for-
    /// pixel identical at every reserve. A drained corridor that dimmed
    /// uniformly would read as the renderer having faulted.
    #[test]
    fn the_nearest_cell_is_untouched_by_the_reserve() {
        assert_eq!(shade(0, 0.0), shade(0, POWER_MAX));
        assert!(
            shade(1, 0.0) < shade(1, POWER_MAX),
            "and the next one is not"
        );
    }

    /// `MARK_FOG` carries no reserve term, and this is the property that
    /// buys: the fog may swallow the walls, but the `+` that says there is a
    /// door down there survives an empty reserve — the corridor's version of
    /// `VIGNETTE_FLOOR_EMPTY` being floored well short of illegible.
    #[test]
    fn a_mark_still_outshines_the_geometry_at_an_empty_reserve() {
        for depth in 0..6 {
            assert!(
                mark_shade(depth) >= shade(depth, 0.0),
                "depth {depth}: a drained reserve ate the mark"
            );
        }
        assert!(
            mark_shade(3) > shade(3, 0.0) * 2.0,
            "the far end is where a thickening fog would eat the marks first"
        );
    }

    /// `PowerReserve` clamps itself, but this reads a `PlayerStatus` field
    /// rather than the type, and a fog past either end would either flatten
    /// the depth cue or drive the corridor to black.
    #[test]
    fn the_fog_is_clamped_at_both_ends() {
        assert_eq!(fog(POWER_MAX * 4.0), FOG_FULL);
        assert_eq!(fog(-40.0), FOG_EMPTY);
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
            draw_stack(&empty, p, Rect::new(0.0, 0.0, 800.0, 600.0), &m, POWER_MAX);
            draw_stack(&single, p, Rect::new(0.0, 0.0, 800.0, 600.0), &m, POWER_MAX);
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
        let longest = "M".repeat(feral_processes_engine::MAX_UNDERFOOT_LINE);
        crate::paint::with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane_w = super::super::hud::layout::regions(
                NARROWEST_WINDOW.0,
                NARROWEST_WINDOW.1,
                char_w,
                &m,
                false,
            )
            .map_pane
            .w;
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

    /// Pulls the text out of a row the way `building.rs`'s own `row_text`
    /// does, for the same reason: asserting on the rows a screen builds
    /// rather than on anything a `Painter` would have to record (nothing
    /// here records).
    fn row_text(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    /// A cache paragraph longer than `DESCRIBE_WRAP_COLUMNS` must actually
    /// wrap at that width, using `wrap_text`, and still end with the
    /// "go back" footer every plain popup carries. This is the test that
    /// fails if `draw_cell_describe`'s content logic is ever gutted to
    /// nothing: an empty or unwrapped `rows` would show up here directly,
    /// where no earlier test in this file touched `cell_describe_rows` or
    /// `DESCRIBE_WRAP_COLUMNS` at all.
    #[test]
    fn the_cell_description_wraps_its_own_text_and_keeps_the_footer() {
        let paragraph = "A stretch of corridor unspools ahead, longer than the \
            wrap column allows on one line, so it has to break onto more \
            than a single row of the popup before the footer appears.";
        assert!(
            paragraph.chars().count() > DESCRIBE_WRAP_COLUMNS,
            "fixture must actually need wrapping"
        );

        let rows = cell_describe_rows(Some(paragraph));
        let text: Vec<&str> = rows.iter().map(row_text).collect();
        let expected: Vec<String> = wrap_text(paragraph, DESCRIBE_WRAP_COLUMNS);

        assert!(
            expected.len() > 1,
            "fixture did not actually wrap: {expected:?}"
        );
        assert_eq!(
            &text[..expected.len()],
            expected.iter().map(String::as_str).collect::<Vec<_>>(),
            "the popup's rows are not wrap_text's output"
        );
        assert_eq!(text.last(), Some(&"Any key to go back"));
    }

    /// `describe_view_direction` always answers on the engine side, but the
    /// popup still has to survive `None` gracefully rather than showing a
    /// blank box.
    #[test]
    fn a_missing_cell_description_still_says_something() {
        let rows = cell_describe_rows(None);
        let text: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(text.contains(&"Nothing to say about that."));
        assert_eq!(text.last(), Some(&"Any key to go back"));
    }

    /// The full paint call still has to survive both shapes without
    /// panicking — `cell_describe_rows`'s two tests above pin content;
    /// this is `drawing_every_shape_of_corridor_does_not_panic`'s sibling,
    /// pinning that the popup call built from those rows actually paints.
    #[test]
    fn drawing_the_cell_description_does_not_panic() {
        let m = crate::text::ui_metrics(900.0);
        crate::paint::with_painter(|p| {
            draw_cell_describe(Some("A doorway, still framed."), None, p, &m);
            draw_cell_describe(None, None, p, &m);
        });
    }

    /// `with_painter` reports what it actually recorded, so `draw_
    /// cell_describe` — the full paint call, not just `cell_describe_rows`'s
    /// content — is directly assertable: this is the test that goes red if
    /// `draw_cell_describe` itself were ever gutted to a no-op, which
    /// nothing above it in this module can catch (`cell_describe_rows`'s
    /// tests are pinned to the pure content-building helper, and
    /// `drawing_the_cell_description_does_not_panic` above stays green for
    /// an empty draw exactly as readily as a real one).
    ///
    /// Pins three things `cell_describe_rows`'s own tests cannot see at
    /// all: the title actually reaches the screen, the body text actually
    /// reaches the screen (not merely gets built into a `Row`), and the
    /// popup draws at `PopupSize::Large` rather than `Small` — the two
    /// sizes leave no other trace a test outside `popup.rs` can read,
    /// which is why this checks the widest painted rect's width against
    /// `screen_w * 0.88` (`Large`'s fraction) rather than `* 0.5`
    /// (`Small`'s).
    #[test]
    fn drawing_the_cell_description_paints_the_title_the_body_and_a_large_popup() {
        const SCREEN_W: f32 = 1440.0; // the fixed geometry `with_painter` sets up
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_cell_describe(Some("A doorway, still framed."), None, p, &m);
        });

        let text = crate::paint::painted_text(&shapes);
        assert!(
            text.iter().any(|t| t == "You look"),
            "the title never painted: {text:?}"
        );
        assert!(
            text.iter().any(|t| t.contains("A doorway, still framed.")),
            "the body text never painted: {text:?}"
        );

        let widest = crate::paint::painted_rect_widths(&shapes)
            .into_iter()
            .fold(0.0_f32, f32::max);
        let large_w = SCREEN_W * 0.88;
        assert!(
            (widest - large_w).abs() < 1.0,
            "expected a PopupSize::Large panel ({large_w}px wide), the widest painted rect was {widest}px"
        );
    }
}
