//! A Shield's circle: how it is styled and drawn, shared by the hover, the
//! deploy and upgrade previews, and the flash a softened hit raises.

use super::*;

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
}
