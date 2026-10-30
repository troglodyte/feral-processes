use super::*;

/// What the Excavation plan is drawing over the base map: where the cursor
/// is, and the anchor it has dropped, both in base-space coordinates.
///
/// Read off `App` before the `game` borrow, the same way `status_line` and
/// `stack_zoom` are, and `None` outside `Mode::Excavate`. The *marks* are not
/// in here: a plan the player drew has to stay visible while they walk it, so
/// those are drawn whenever the pane is showing base space.
#[derive(Clone, Copy)]
pub(crate) struct PlanCursor {
    pub cursor: (i32, i32),
    pub anchor: Option<(i32, i32)>,
}

/// A `?` in the top-right quarter of a machine's tile: it has been placed and
/// nothing has been chosen for it to make, so it does no work and draws no
/// power. `ATTENTION`, "the player must act".
///
/// Half the tile's height so it fits its quarter, placed by its measured
/// ink because `Painter::map`'s `y` is a baseline.
pub(super) fn draw_unconfigured_mark(painter: &Painter, px: f32, py: f32, tile_px: f32) {
    const INSET: f32 = 2.0;
    let glyph = "?";
    let size = ((tile_px / 2.0) as u16).max(1);
    let dims = painter.measure_map(glyph, size);
    painter.map(
        glyph,
        px + tile_px - dims.width - INSET,
        py + INSET + dims.height,
        size,
        hud::palette::ATTENTION,
    );
}

pub(super) fn draw_spent_program_mark(painter: &Painter, px: f32, py: f32, tile_px: f32) {
    const INSET: f32 = 2.0;
    let glyph = "x";
    let size = ((tile_px / 2.0) as u16).max(1);
    let dims = painter.measure_map(glyph, size);
    painter.map(
        glyph,
        px + INSET,
        py + INSET + dims.height,
        size,
        hud::palette::ATTENTION,
    );
}

/// A plot tile's outline: quiet, so a field reads as ground set aside and
/// not as a plan or a wall.
pub(super) const PLOT_EDGE: Color = Color::new(0.20, 0.42, 0.22, 1.0);
/// A ripe cell's outline: "ready", the same green a healthy bar uses.
pub(super) const RIPE_EDGE: Color = hud::palette::HEALTHY;
pub(super) const RIPE_EDGE_PX: f32 = 2.0;

/// How large a crop's glyph draws: it grows with its stages, and a ripe one
/// is full size. `None` for a cell with nothing in it.
pub(super) fn crop_glyph_px(full: u16, state: FarmTileState) -> Option<u16> {
    match state {
        FarmTileState::Fallow => None,
        FarmTileState::Growing { stage, stages } => {
            let (stage, stages) = (u32::from(stage), u32::from(stages));
            Some((u32::from(full) * (stage + 1) / (stages + 1)).max(1) as u16)
        }
        FarmTileState::Ripe => Some(full),
    }
}

/// Plots on the ground, drawn over the tiles and under the plan. A crop's
/// glyph shows only where no entity already has the cell's glyph.
pub(super) fn draw_farm_plots(
    painter: &Painter,
    tiles: &[FarmTileView],
    occupied: &std::collections::HashSet<(i32, i32)>,
    dims: &std::collections::HashMap<(i32, i32), f32>,
    at: impl Fn((i32, i32)) -> (f32, f32),
    (tile_px, glyph_px): (f32, u16),
    pane: Rect,
) {
    for tile in tiles {
        let (px, py) = at(tile.pos);
        if px + tile_px < pane.x
            || py + tile_px < pane.y
            || px > pane.x + pane.w
            || py > pane.y + pane.h
        {
            continue;
        }
        // The finish seam's rule: every layer scales by the tile's own
        // `dim`, so a plot dims with the ground under the Power vignette and
        // the fog of a remembered cell rather than glowing through them.
        let dim = dims.get(&tile.pos).copied().unwrap_or(1.0);
        painter.rect_lines(
            px,
            py,
            tile_px - 1.0,
            tile_px - 1.0,
            1.0,
            at_level(PLOT_EDGE, dim),
        );
        if tile.state == FarmTileState::Ripe {
            painter.rect_lines(
                px,
                py,
                tile_px - 1.0,
                tile_px - 1.0,
                RIPE_EDGE_PX,
                at_level(RIPE_EDGE, dim),
            );
        }
        let (Some((glyph, hue)), Some(size)) = (&tile.crop, crop_glyph_px(glyph_px, tile.state))
        else {
            continue;
        };
        if occupied.contains(&tile.pos) {
            continue;
        }
        let dims = painter.measure_map(glyph, size);
        painter.map(
            glyph,
            px + (tile_px - dims.width) / 2.0,
            py + (tile_px + dims.height) / 2.0,
            size,
            at_level(glyph_color(*hue), dim),
        );
    }
}

/// The marks, the box being previewed, and the cursor — one pass over world
/// coordinates after the tile loop, the same shape the spark pass takes.
///
/// A pass of its own rather than three more branches inside the tile loop
/// because none of it is a property of a *tile*: a mark is an entity in base
/// space, and the box is a rectangle that happens to cross tiles. Culling is
/// the pane's, so a box drawn past the edge is simply not painted.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_excavation_plan(
    painter: &Painter,
    stockpiles: &[(i32, i32)],
    marked: &[DigMark],
    plan: Option<PlanCursor>,
    at: impl Fn((i32, i32)) -> (f32, f32),
    tile_px: f32,
    pane: Rect,
) {
    let size = tile_px - 1.0;
    // Answers where the cell landed, or `None` when it was culled — the
    // marks loop below needs the origin to put a cut meter on, and culling
    // is the one thing it must not have to repeat.
    let tile = |world: (i32, i32), fill: Option<Color>, outline: Option<(f32, Color)>| {
        let (px, py) = at(world);
        if px >= pane.x + pane.w
            || py >= pane.y + pane.h
            || px + tile_px <= pane.x
            || py + tile_px <= pane.y
        {
            return None;
        }
        if let Some(fill) = fill {
            painter.rect(px, py, size, size, fill);
        }
        if let Some((thickness, color)) = outline {
            painter.rect_lines(px, py, size, size, thickness, color);
        }
        Some((px, py))
    };
    // Zones first, under the plan: a standing wash that a dig mark or the
    // preview box is drawn over rather than hidden by.
    for &cell in stockpiles {
        tile(cell, Some(STOCKPILE_FILL), None);
    }
    // The plan itself, drawn under the cursor: a wash rather than a glyph,
    // because the cell underneath is already saying whether it is rock, cut
    // or floor and the mark is a second reading on top of that one.
    for mark in marked {
        // The wash says a cell is planned; the bar says how much of the wall
        // is already gone. `palette::PLAN` at full alpha rather than one of
        // the washes, for `CUTTING_OUTLINE`'s reason — this is the job
        // happening, not the plan sitting on the ground. No vignette: the
        // whole pass is drawn flat over the tiles.
        if let Some((px, py)) = tile(mark.pos, Some(MARK_FILL), Some((1.0, MARK_EDGE))) {
            draw_progress_bar(painter, mark.cut, px, py, tile_px, hud::palette::PLAN, 1.0);
        }
    }
    let Some(plan) = plan else { return };
    // The box the anchor is spanning, brighter than a committed mark: this
    // is the thing about to happen, and it has to read over the marks it may
    // be drawn across.
    if let Some(anchor) = plan.anchor {
        let (x0, x1) = (anchor.0.min(plan.cursor.0), anchor.0.max(plan.cursor.0));
        let (y0, y1) = (anchor.1.min(plan.cursor.1), anchor.1.max(plan.cursor.1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                tile((x, y), Some(PREVIEW_FILL), None);
            }
        }
    }
    tile(plan.cursor, None, Some((2.0, hud::palette::EMPHASIS)));
}
