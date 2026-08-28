//! Where the five HUD regions sit. Pure arithmetic — no `Painter`, no
//! drawing, no engine.
//!
//! Two of the rules here are load-bearing rather than decorative, and both
//! are asserted below rather than left to the arithmetic:
//!
//! 1. **The info column reaches the bottom edge.** The log does not pass
//!    under it.
//! 2. **The log pane is only as wide as the map pane.** The screen's
//!    bottom-right corner belongs to the column.
//!
//! `char_w` is a parameter and not a term on `Metrics`, for two reasons:
//! `Metrics` carries no character-width figure, and the UI face is DejaVu
//! Sans Mono rather than the handoff's assumed 0.6-advance font, so a
//! character count is not a width. The caller measures once and passes it
//! in; the tests pass a literal, which is the whole point of the parameter.

use crate::paint::Rect;
use crate::text::Metrics;

/// The info column takes this fraction of the window before clamping.
const INFO_W_FRAC: f32 = 0.30;
/// Clamped in *characters*, so the column's tables neither crush nor
/// sprawl. Applied to `frac * screen_w` — applying it to the fraction
/// instead is the silent way to get a column that ignores one end.
const INFO_W_MIN_CH: f32 = 44.0;
const INFO_W_MAX_CH: f32 = 56.0;
/// The log pane's four text rows.
const LOG_TEXT_ROWS: f32 = 4.0;

/// The five regions, in window pixels.
///
/// `key_bar` overlaps the bottom edge of `log_pane` deliberately: the
/// keybar is drawn *on* that border run, not below it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(in crate::render) struct HudRegions {
    /// Full width, row 0. Spans over the info column.
    pub status_bar: Rect,
    /// Framed. Its borders carry the title, the threat readout and the
    /// vitals strip.
    pub map_pane: Rect,
    /// Framed. Map-pane width only — never the window's.
    pub log_pane: Rect,
    /// Drawn on `log_pane`'s bottom border line.
    pub key_bar: Rect,
    /// Runs to the window's bottom edge.
    pub info_column: Rect,
}

/// Derives the five regions for a window.
///
/// `char_w` is the UI face's advance for one character, measured by the
/// caller — see the module comment for why it is not read off `m`.
pub(in crate::render) fn regions(
    screen_w: f32,
    screen_h: f32,
    char_w: f32,
    m: &Metrics,
) -> HudRegions {
    let info_w = (screen_w * INFO_W_FRAC).clamp(INFO_W_MIN_CH * char_w, INFO_W_MAX_CH * char_w);
    // One cell between the column and everything to its left.
    let gutter = char_w;
    let left_w = screen_w - info_w - gutter;

    // One row plus the inset above and below it — the height the stock
    // strip claimed before the status bar absorbed it, kept so the panes
    // below start clear of it exactly as they did.
    let head_h = m.line_height + m.inset;
    let log_h = m.line_height * LOG_TEXT_ROWS + m.inset * 2.0;
    let key_h = m.line_height;
    let map_h = screen_h - head_h - log_h - key_h - m.gap;

    let log_y = head_h + map_h + m.gap;
    HudRegions {
        status_bar: Rect::new(0.0, 0.0, screen_w, head_h),
        map_pane: Rect::new(0.0, head_h, left_w, map_h),
        log_pane: Rect::new(0.0, log_y, left_w, log_h),
        key_bar: Rect::new(0.0, log_y + log_h - key_h, left_w, key_h),
        info_column: Rect::new(screen_w - info_w, head_h, info_w, screen_h - head_h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::ui_metrics;

    /// The three window sizes the design is stated against, plus the
    /// smallest the UI font floor supports.
    const SIZES: [(f32, f32); 3] = [(1280.0, 720.0), (1440.0, 810.0), (1920.0, 1080.0)];

    /// A plausible measured advance for the UI face. Tests pass a literal
    /// rather than measuring, which is why `regions` takes it as a
    /// parameter at all.
    const CHAR_W: f32 = 9.0;

    fn at(w: f32, h: f32) -> HudRegions {
        regions(w, h, CHAR_W, &ui_metrics(h))
    }

    /// Rule 1. The column owns the screen's bottom-right corner, so it has
    /// to actually reach it — a column stopping short of the bottom leaves
    /// a strip of canvas that reads as a rendering fault.
    #[test]
    fn the_info_column_reaches_the_bottom_edge() {
        for (w, h) in SIZES {
            let r = at(w, h);
            assert!(
                (r.info_column.y + r.info_column.h - h).abs() < 0.001,
                "column stops at {} of {h} at {w}x{h}",
                r.info_column.y + r.info_column.h
            );
        }
    }

    /// Rule 2. The log is the map's width, not the window's.
    #[test]
    fn the_log_never_passes_under_the_info_column() {
        for (w, h) in SIZES {
            let r = at(w, h);
            assert!(
                r.log_pane.x + r.log_pane.w <= r.info_column.x,
                "log ends at {} but the column starts at {} at {w}x{h}",
                r.log_pane.x + r.log_pane.w,
                r.info_column.x
            );
            assert!(
                r.key_bar.x + r.key_bar.w <= r.info_column.x,
                "keybar runs under the column at {w}x{h}"
            );
        }
    }

    /// The clamp is in characters and applies to the *product*. Applied to
    /// the fraction instead it silently ignores one end of the range, which
    /// is invisible at the reference size and wrong everywhere else.
    #[test]
    fn the_info_column_stays_within_its_character_clamp() {
        let mut w = 800.0f32;
        while w <= 3840.0 {
            let r = at(w, 900.0);
            assert!(
                r.info_column.w >= INFO_W_MIN_CH * CHAR_W - 0.001
                    && r.info_column.w <= INFO_W_MAX_CH * CHAR_W + 0.001,
                "column is {} chars wide at {w} px",
                r.info_column.w / CHAR_W
            );
            w += 40.0;
        }
    }

    /// A negative extent is a pane drawn inside-out, and `Painter` will
    /// happily do it. The subtractions are tightest at the small end.
    #[test]
    fn no_region_has_negative_extent() {
        let mut w = 800.0f32;
        while w <= 3840.0 {
            let mut h = 600.0f32;
            while h <= 2160.0 {
                let r = at(w, h);
                for (name, rect) in [
                    ("status_bar", r.status_bar),
                    ("map_pane", r.map_pane),
                    ("log_pane", r.log_pane),
                    ("key_bar", r.key_bar),
                    ("info_column", r.info_column),
                ] {
                    assert!(rect.w > 0.0, "{name} has width {} at {w}x{h}", rect.w);
                    assert!(rect.h > 0.0, "{name} has height {} at {w}x{h}", rect.h);
                }
                h += 120.0;
            }
            w += 240.0;
        }
    }
}
