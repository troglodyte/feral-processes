use super::*;

/// The in-cover mark: a horizon line and a sun's half-disc standing on it,
/// flat side down.
pub(super) struct Sunrise {
    pub(super) horizon: [(f32, f32); 2],
    pub(super) sun: Vec<(f32, f32)>,
}

/// The in-cover sunrise for a body whose tile is `cell_px` wide at
/// `(px, py)`.
///
/// A free function for `turn_arrow`'s reason — the geometry is the thing
/// worth holding, and holding it needs no `Painter`.
pub(super) fn sunrise(px: f32, py: f32, cell_px: f32) -> Sunrise {
    let r = cell_px * COVER_SUN_RADIUS;
    let right = px + cell_px - 2.0;
    let left = right - cell_px * COVER_MARK_WIDTH;
    let horizon = py + RARITY_BAR_PX + 1.0 + r;
    let cx = (left + right) / 2.0;
    let sun = (0..=COVER_SUN_SEGMENTS)
        .map(|i| {
            let a = std::f32::consts::PI * f32::from(i) / f32::from(COVER_SUN_SEGMENTS);
            (cx + r * a.cos(), horizon - r * a.sin())
        })
        .collect();
    Sunrise {
        horizon: [(left, horizon), (right, horizon)],
        sun,
    }
}

/// The three points of that arrow, given the top-left of the acting body's
/// tile and how far this frame's bob has lifted it.
///
/// A free function for `marks::nemesis_mark_rect`'s reason — the geometry is
/// the thing worth holding, and holding it needs no `Painter`.
pub(super) fn turn_arrow(px: f32, py: f32, tile_px: f32, lift: f32) -> [(f32, f32); 3] {
    let cx = px + tile_px / 2.0;
    let half = tile_px * TURN_ARROW_WIDTH / 2.0;
    let point = py - TURN_ARROW_GAP - lift;
    let base = point - tile_px * TURN_ARROW_HEIGHT;
    [(cx - half, base), (cx + half, base), (cx, point)]
}

/// Washes and outlines one cell of a field — the reach field and the
/// placeable field are the same shape drawn in different colours, so this is
/// the one place that shape is written.
///
/// Draws nothing when `cell` is not itself in `field`. The wash is a flat
/// tint at `REACH_WASH_ALPHA`; the boundary is drawn a side at a time and
/// only where the neighbour on that side is *not* in `field` — a body and a
/// `Blocked` cell are both walls in `reach::movement_field`, so a gap in the
/// field is a cell that genuinely cannot be reached, and outlining it is the
/// same answer the field's outer edge gives.
///
/// **`field` may be a footprint-shaped region and this needs no change for
/// it**: the boundary check above already asks only whether a *neighbouring
/// cell* is also a member, so a solid 2x2 block passed in reads as one
/// region with no seam drawn between its own four cells — the same rule
/// that already suppressed a line between two adjacent single-cell anchors.
/// `expand_to_footprint` is what turns a candidate anchor into that block
/// before this ever sees it.
pub(super) fn draw_cell_field(
    painter: &Painter,
    field: &[(i32, i32)],
    edge: &[(i32, i32)],
    cell: (i32, i32),
    (px, py): (f32, f32),
    tile_px: f32,
    color: Color,
) {
    if field.contains(&cell) {
        painter.rect(
            px,
            py,
            tile_px - 1.0,
            tile_px - 1.0,
            Color::new(color.r, color.g, color.b, REACH_WASH_ALPHA),
        );
    }
    if !edge.contains(&cell) {
        return;
    }
    let far = tile_px - 1.0;
    for (dx, dy, from, to) in [
        (0, -1, (0.0, 0.0), (far, 0.0)),
        (0, 1, (0.0, far), (far, far)),
        (-1, 0, (0.0, 0.0), (0.0, far)),
        (1, 0, (far, 0.0), (far, far)),
    ] {
        if edge.contains(&(cell.0 + dx, cell.1 + dy)) {
            continue;
        }
        painter.line(
            px + from.0,
            py + from.1,
            px + to.0,
            py + to.1,
            REACH_EDGE_PX,
            color,
        );
    }
}

/// Expands a set of candidate anchors into every cell the acting body's own
/// footprint would cover standing at each one — `1` cell per anchor without
/// a `Squad`, so an ordinary body's wash is unchanged.
///
/// `view.reachable`/`covered`/`provoking` are all anchors for the body
/// whose turn it is (`TacticalView`'s own doc on each), never another
/// body's, so one `footprint` — the actor's — is right for every caller.
/// Anchored top-left, `tactical::footprint_cells_at`'s own convention.
///
/// Duplicate cells from two overlapping footprints are left in rather than
/// deduped: `draw_cell_field`'s `field.contains` reads a `Vec` as a set
/// already, and the caller iterates real board cells once each regardless
/// of how many times one appears here.
pub(super) fn expand_to_footprint(anchors: &[(i32, i32)], footprint: u8) -> Vec<(i32, i32)> {
    let side = i32::from(footprint.max(1));
    anchors
        .iter()
        .flat_map(|&(x, y)| (0..side).flat_map(move |dy| (0..side).map(move |dx| (x + dx, y + dy))))
        .collect()
}
