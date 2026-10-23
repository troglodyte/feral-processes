//! The surface map's ambient specks — `crate::wildlife` decides where they
//! are; this only draws them over ground that can take them.

use super::*;
use crate::wildlife::{Speck, SpeckShape};

/// The most a speck covers the ground under it. Kept well under opaque so
/// a swarm reads as background and never as something to walk into.
const SPECK_ALPHA: f32 = 0.6;

/// How far a speck's colour is pulled from its ground's toward white. The
/// ground's own hue stays in it, so a swarm over different terrain is a
/// different swarm.
const SPECK_PALE: f32 = 0.55;

/// The smallest a speck is drawn, in pixels, so the smallest zoom still
/// shows a point rather than a sub-pixel smear.
const SPECK_MIN_PX: f32 = 2.0;

/// Draws each speck whose tile `ground` answers with a colour for — `None`
/// is a tile the speck may not be over (unseen, impassable or standing
/// room for something real) — at the pixel `to_px` puts that tile, dimmed
/// by `dim` at the speck's own pixel.
pub(super) fn draw_wildlife(
    painter: &Painter,
    specks: &[Speck],
    ground: impl Fn((i32, i32)) -> Option<Color>,
    to_px: impl Fn((i32, i32)) -> (f32, f32),
    dim: impl Fn(f32, f32) -> f32,
    tile_px: f32,
    pane: Rect,
) {
    for speck in specks {
        let tile = speck.tile();
        let Some(base) = ground(tile) else { continue };
        let (tx, ty) = to_px(tile);
        let cx = tx + (speck.pos.0 - tile.0 as f32) * tile_px;
        let cy = ty + (speck.pos.1 - tile.1 as f32) * tile_px;
        if cx < pane.x || cy < pane.y || cx >= pane.x + pane.w || cy >= pane.y + pane.h {
            continue;
        }
        let side = (speck.size * tile_px).max(SPECK_MIN_PX);
        let color = Color::new(
            base.r + (1.0 - base.r) * SPECK_PALE,
            base.g + (1.0 - base.g) * SPECK_PALE,
            base.b + (1.0 - base.b) * SPECK_PALE,
            SPECK_ALPHA * speck.alpha * dim(cx, cy),
        );
        let h = side / 2.0;
        match speck.shape {
            SpeckShape::Square => painter.rect(cx - h, cy - h, side, side, color),
            SpeckShape::Diamond => {
                // A diamond's diagonal is its side, so it is drawn a touch
                // larger to carry the same weight as a square beside it.
                let r = h * 1.3;
                painter.poly(
                    &[(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)],
                    color,
                );
            }
        }
    }
}
