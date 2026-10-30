//! A Shield's circle: how it is styled and drawn, shared by the hover, the
//! deploy and upgrade previews, and the flash a softened hit raises.

use super::base::tile_origin_px;
use super::*;
use feral_processes_app_core::{ShieldCircle, ShieldCircleStyle};

const SHIELD_BLUE: (f32, f32, f32) = (0.4, 0.8, 1.0);
const HOVER_FILL_ALPHA: f32 = 0.12;
const HOVER_EDGE_ALPHA: f32 = 0.9;
const HOVER_EDGE_PX: f32 = 2.0;
const PREVIEW_EDGE_ALPHA: f32 = 0.35;
const PREVIEW_EDGE_PX: f32 = 1.0;

/// How one circle is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CircleStyle {
    /// The cursor is on the Shield: a solid edge and a light fill.
    Hover,
    /// A deploy or upgrade prompt: a faint edge and no fill.
    Preview,
    /// A softened hit: fill only, at this alpha.
    Flash(f32),
}

fn blue(a: f32) -> Color {
    Color::new(SHIELD_BLUE.0, SHIELD_BLUE.1, SHIELD_BLUE.2, a)
}

pub(crate) fn draw_shield_circle(
    p: &Painter,
    centre: (f32, f32),
    radius_px: f32,
    style: CircleStyle,
) {
    let (cx, cy) = centre;
    match style {
        CircleStyle::Hover => {
            p.circle(cx, cy, radius_px, blue(HOVER_FILL_ALPHA));
            p.circle_stroke(cx, cy, radius_px, HOVER_EDGE_PX, blue(HOVER_EDGE_ALPHA));
        }
        CircleStyle::Preview => {
            p.circle_stroke(cx, cy, radius_px, PREVIEW_EDGE_PX, blue(PREVIEW_EDGE_ALPHA));
        }
        CircleStyle::Flash(alpha) => p.circle(cx, cy, radius_px, blue(alpha)),
    }
}

/// Draws the pane's Shield circles and the flashes of softened hits, over
/// the map `fx.map_click()` describes (set by whichever map drew this
/// frame). `in_battle` picks the siege board's flashes over the field's.
pub(crate) fn draw_shield_circles(
    fx: &Fx,
    painter: &Painter,
    circles: &[ShieldCircle],
    in_battle: bool,
) {
    let Some(lay) = fx.map_click() else { return };
    let tile = lay.tile_px;
    let centre_of = |anchor: (i32, i32)| {
        let (px, py) = tile_origin_px(anchor, lay.center, lay.half, lay.off, tile, lay.pane);
        (px + tile / 2.0, py + tile / 2.0)
    };
    painter.clipped(lay.pane.x, lay.pane.y, lay.pane.w, lay.pane.h, |p| {
        for (anchor, radius, alpha) in fx.shield_flashes(in_battle) {
            let style = CircleStyle::Flash(alpha);
            draw_shield_circle(p, centre_of(anchor), radius as f32 * tile, style);
        }
        for c in circles {
            let style = match c.style {
                ShieldCircleStyle::Hover => CircleStyle::Hover,
                ShieldCircleStyle::Preview => CircleStyle::Preview,
            };
            draw_shield_circle(p, centre_of(c.anchor), c.radius as f32 * tile, style);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_circles, with_painter};

    #[test]
    fn hover_style_draws_edge_and_fill() {
        let (_, shapes) =
            with_painter(|p| draw_shield_circle(p, (50.0, 50.0), 64.0, CircleStyle::Hover));
        let circles = painted_circles(&shapes);
        assert_eq!(circles.len(), 2);
        let fill = circles.iter().find(|c| c.1 > 0.0).unwrap();
        assert!((fill.1 - 0.12).abs() < 0.01 && fill.0 == 64.0);
        let edge = circles.iter().find(|c| c.2 > 0.0).unwrap();
        assert!(edge.2 == 2.0 && edge.0 == 64.0);
    }

    #[test]
    fn preview_style_draws_only_an_edge() {
        let (_, shapes) =
            with_painter(|p| draw_shield_circle(p, (50.0, 50.0), 64.0, CircleStyle::Preview));
        let circles = painted_circles(&shapes);
        assert_eq!(circles.len(), 1);
        assert!(circles[0].1 == 0.0 && circles[0].2 > 0.0);
    }

    #[test]
    fn a_hovered_shield_draws_a_circle_on_the_surface_map() {
        use crate::paint::painted_circle_centres;
        let mut fx = Fx::new();
        let pane = Rect::new(10.0, 20.0, 400.0, 300.0);
        fx.set_map_click(Some(MapClickLayout {
            pane,
            center: (5, 5),
            half: (6, 4),
            off: (0.0, 0.0),
            tile_px: 32.0,
        }));
        let circle = ShieldCircle {
            anchor: (7, 4),
            radius: 4,
            style: ShieldCircleStyle::Hover,
        };
        let (_, shapes) = with_painter(|p| draw_shield_circles(&fx, p, &[circle], false));
        let circles = painted_circles(&shapes);
        assert_eq!(circles.len(), 2, "one fill and one stroke");
        assert!(circles.iter().all(|c| c.0 == 128.0));
        let (px, py) = tile_origin_px((7, 4), (5, 5), (6, 4), (0.0, 0.0), 32.0, pane);
        for (x, y) in painted_circle_centres(&shapes) {
            assert_eq!((x, y), (px + 16.0, py + 16.0));
        }
    }
}
