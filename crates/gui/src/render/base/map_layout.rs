use super::*;

/// Where a world tile's top-left corner falls in the map pane.
///
/// The tile loop walks grid indices and the spark pass walks world tiles,
/// and both have to land on the same pixel. `half` is the pane's half-extent
/// *without* the extra rings — the rings are fetched to be read, not drawn,
/// so they cost a leading offset that keeps the pane framing the same view
/// it did before the camera existed.
pub(crate) fn tile_origin_px(
    world: (i32, i32),
    player: (i32, i32),
    half: (i32, i32),
    off: (f32, f32),
    tile_px: f32,
    pane: Rect,
) -> (f32, f32) {
    (
        pane.x + ((world.0 - player.0 + half.0) as f32 - off.0) * tile_px,
        pane.y + ((world.1 - player.1 + half.1) as f32 - off.1) * tile_px,
    )
}

/// The map pane's layout as a map draw laid it out this frame — pane rect,
/// camera centre, half-extent and sub-tile offset, all of them
/// `tile_origin_px`'s own arguments. A draw-time helper: the map draw
/// returns it for what is drawn over the map in the same frame (reach
/// circles, a notice's anchor). Nothing keeps it past the frame — the
/// pointer finds a tile through the `MapTile` targets `register_tiles`
/// records, because the camera offset eases toward its target over time
/// (`Fx::camera_offset`) and a second call would advance it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapLayout {
    pub(crate) pane: Rect,
    pub(crate) center: (i32, i32),
    pub(crate) half: (i32, i32),
    pub(crate) off: (f32, f32),
    pub(crate) tile_px: f32,
}

/// The exact inverse of `tile_origin_px`, kept in the same file so the two
/// cannot drift apart: which world tile a pixel inside the map pane falls
/// in. Floored rather than rounded, `tile_origin_px`'s own corner — so a
/// point anywhere inside the cell, not just the pixel at its top-left
/// corner, answers the same tile that corner does.
pub(crate) fn tile_at_px(px: f32, py: f32, layout: &MapLayout) -> (i32, i32) {
    let gx = ((px - layout.pane.x) / layout.tile_px + layout.off.0).floor() as i32;
    let gy = ((py - layout.pane.y) / layout.tile_px + layout.off.1).floor() as i32;
    (
        gx + layout.center.0 - layout.half.0,
        gy + layout.center.1 - layout.half.1,
    )
}

impl MapLayout {
    /// Records every tile the pane shows as a `PointerHit::MapTile` target,
    /// so the pointer resolves to the tile the player is looking at (ADR
    /// 0046). Called by the map draw before anything is drawn over the pane,
    /// so a HUD target over the map wins the hover.
    pub(crate) fn register_tiles(&self, painter: &Painter) {
        let pane = self.pane;
        let (x0, y0) = tile_at_px(pane.x, pane.y, self);
        let (x1, y1) = tile_at_px(pane.x + pane.w - 0.01, pane.y + pane.h - 0.01, self);
        for wy in y0..=y1 {
            for wx in x0..=x1 {
                let (ox, oy) = tile_origin_px(
                    (wx, wy),
                    self.center,
                    self.half,
                    self.off,
                    self.tile_px,
                    pane,
                );
                let left = ox.max(pane.x);
                let top = oy.max(pane.y);
                let right = (ox + self.tile_px).min(pane.x + pane.w);
                let bottom = (oy + self.tile_px).min(pane.y + pane.h);
                if right > left && bottom > top {
                    painter.target(
                        Rect::new(left, top, right - left, bottom - top),
                        PointerHit::MapTile(wx, wy),
                    );
                }
            }
        }
    }
}
