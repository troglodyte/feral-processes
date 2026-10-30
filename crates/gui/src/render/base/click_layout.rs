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

/// The map pane's layout as `draw_surface_map` drew it this frame — pane
/// rect, camera centre, half-extent and sub-tile offset, all of them
/// `tile_origin_px`'s own arguments. Stashed by the draw pass into
/// `Fx::set_map_click` and read back by the pointer system
/// (`lib.rs::handle_map_pointer`) rather than recomputed there: the camera
/// offset eases toward its target over time (`Fx::camera_offset`), so a
/// second call would advance it again and answer a different frame's
/// numbers than the one the player actually saw and clicked on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapClickLayout {
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
pub(crate) fn tile_at_px(px: f32, py: f32, layout: &MapClickLayout) -> (i32, i32) {
    let gx = ((px - layout.pane.x) / layout.tile_px + layout.off.0).floor() as i32;
    let gy = ((py - layout.pane.y) / layout.tile_px + layout.off.1).floor() as i32;
    (
        gx + layout.center.0 - layout.half.0,
        gy + layout.center.1 - layout.half.1,
    )
}
