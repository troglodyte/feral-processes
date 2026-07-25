//! The HP/power meters, and the ghost band that trails a bar after it drops.

use super::*;

/// Where a stat bar goes. `draw_bar` and the `draw_ghost_band` trailing it
/// take one of these rather than three loose floats, so the two can't drift
/// apart into a band that misses the bar it belongs to.
#[derive(Clone, Copy)]
pub(super) struct BarGeometry {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
}

impl BarGeometry {
    /// Top of the track, shared so `draw_bar` and `draw_ghost_band` can't
    /// disagree about where it is.
    fn track_y(&self, m: &Metrics) -> f32 {
        self.y + m.gap
    }
}

/// A bar's track is a deliberate visual weight — a rule under the label,
/// not a block — so unlike the text it flanks it stays put as the UI font
/// scales.
const BAR_TRACK_H: f32 = 14.0;

/// How a bar is painted, as opposed to where `BarGeometry` puts it. Bundled
/// rather than passed loose because the two together push `draw_bar` past
/// clippy's argument threshold, same reasoning as `BarGeometry` itself.
#[derive(Clone, Copy)]
pub(super) struct BarStyle {
    pub(super) color: Color,
    /// Draws the label in the bold face — reserved for the party member
    /// currently choosing an action, so the row you're addressing wins
    /// against the colour the rest of the roster is already using.
    pub(super) bold: bool,
}

impl BarStyle {
    pub(super) fn plain(color: Color) -> Self {
        Self { color, bold: false }
    }
}

/// Draws a labeled bar (HP/Power/Fatigue) and returns the y coordinate for
/// whatever's drawn next.
pub(super) fn draw_bar(
    g: BarGeometry,
    label: &str,
    value: f32,
    max: f32,
    style: BarStyle,
    fonts: &Fonts,
    m: &Metrics,
) -> f32 {
    let BarStyle { color, bold } = style;
    let ratio = (value / max).clamp(0.0, 1.0);
    // `label` is drawn exactly as given. This used to append
    // `" {value}/{max}"`, which pinned HP to the end of the text and made
    // the battle rosters' HP column impossible — a drawing primitive is the
    // wrong place to be deciding text layout.
    if bold {
        fonts.ui_bold(label, g.x, g.y, m.label(), TEXT);
    } else {
        fonts.ui(label, g.x, g.y, m.label(), TEXT);
    }
    let bar_y = g.track_y(m);
    draw_rectangle(
        g.x,
        bar_y,
        g.w,
        BAR_TRACK_H,
        Color::new(0.15, 0.15, 0.15, 1.0),
    );
    draw_rectangle(g.x, bar_y, g.w * ratio, BAR_TRACK_H, color);
    draw_rectangle_lines(g.x, bar_y, g.w, BAR_TRACK_H, 1.0, BORDER);
    // Leaves the next row's label room above its own baseline, so stacked
    // bars keep their spacing as the label grows.
    bar_y + BAR_TRACK_H + m.font_size as f32 / 2.0
}

/// How far one stacked bar row advances y, including the trailing `m.inset`
/// every caller adds between rows. Mirrors `draw_bar`'s return value, so the
/// two have to move together — it exists because `draw_battle` bottom-anchors
/// the party block and therefore has to know its height *before* drawing it.
pub(super) fn bar_row_height(m: &Metrics) -> f32 {
    m.gap + BAR_TRACK_H + m.font_size as f32 / 2.0 + m.inset
}

/// A lagging "ghost" band trailing a bar's real value, so a hit in battle
/// reads as a visible drain rather than a jump. Call after `draw_bar` with
/// the same geometry — `draw_bar` lays down an opaque track that would
/// otherwise bury this — and it fills only the gap between the two values.
pub(super) fn draw_ghost_band(
    g: BarGeometry,
    value: f32,
    ghost: f32,
    max: f32,
    color: Color,
    m: &Metrics,
) {
    let ratio = (value / max).clamp(0.0, 1.0);
    let ghost_ratio = (ghost / max).clamp(0.0, 1.0);
    if ghost_ratio <= ratio {
        return;
    }
    draw_rectangle(
        g.x + g.w * ratio,
        g.track_y(m),
        g.w * (ghost_ratio - ratio),
        BAR_TRACK_H,
        Color::new(color.r, color.g, color.b, 0.45),
    );
}
