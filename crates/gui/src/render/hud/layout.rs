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

use super::strip;
use crate::paint::Rect;
use crate::text::Metrics;

/// The info column takes this fraction of the window before clamping.
const INFO_W_FRAC: f32 = 0.30;
/// Clamped in *characters*, so the column's tables neither crush nor
/// sprawl. Applied to `frac * screen_w` — applying it to the fraction
/// instead is the silent way to get a column that ignores one end.
const INFO_W_MIN_CH: f32 = 44.0;
const INFO_W_MAX_CH: f32 = 56.0;
/// The log pane's four text rows, collapsed — the pane's normal state.
const LOG_TEXT_ROWS: f32 = 4.0;
/// The log pane's rows expanded — see `App::log_expanded`, toggled by SPACE
/// on the map screen. Twice the collapsed count rather than an
/// independently-tuned figure, so "expanded" always means the same thing.
const LOG_TEXT_ROWS_EXPANDED: f32 = LOG_TEXT_ROWS * 2.0;

/// **Bug A's fix, and the number the bug was missing.** A border strip
/// mounted `Mount::TopLeft`/`TopRight` centres its background quad *on* the
/// line it rides (`strip::border_strip`), reaching `size/2 + pad/2` above
/// it, where `size = m.small()` and `pad = size * strip::PAD_RATIO`. The map
/// pane's title and threat readout ride its top border at `pane.y`, so a
/// pane whose top edge sits exactly on the status bar's bottom edge lets
/// that quad — and the top of the strip's own glyph caps — paint into the
/// bar's opaque fill, which `draw_status_bar` draws *after* `draw_map_frame`
/// has already run.
///
/// So the panes below the bar start `m.small() * TOP_STRIP_CLEARANCE_RATIO`
/// further down than the bar itself is tall — the bar's own rect
/// (`status_bar`) is untouched, only where the *next* thing may start.
/// Expressed as a ratio of `m.small()`, not a pixel figure, so it travels
/// with the font size the same way every other figure here does. Built from
/// `strip::PAD_RATIO` directly rather than a copied `0.5` so the two cannot
/// drift apart.
const TOP_STRIP_CLEARANCE_RATIO: f32 = 0.5 + strip::PAD_RATIO / 2.0;

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
/// `log_expanded` is `App::log_expanded` — SPACE on the map screen doubles
/// `log_pane`'s row count and back.
pub(in crate::render) fn regions(
    screen_w: f32,
    screen_h: f32,
    char_w: f32,
    m: &Metrics,
    log_expanded: bool,
) -> HudRegions {
    let info_w = (screen_w * INFO_W_FRAC).clamp(INFO_W_MIN_CH * char_w, INFO_W_MAX_CH * char_w);
    // One cell between the column and everything to its left.
    let gutter = char_w;
    let left_w = screen_w - info_w - gutter;

    // One row plus the inset above and below it — the height the stock
    // strip claimed before the status bar absorbed it. This sizes the bar's
    // own rect only; see `TOP_STRIP_CLEARANCE_RATIO` for why the panes below
    // it start further down than this.
    let head_h = m.line_height + m.inset;
    let content_top = head_h + m.small() as f32 * TOP_STRIP_CLEARANCE_RATIO;

    let log_rows = if log_expanded {
        LOG_TEXT_ROWS_EXPANDED
    } else {
        LOG_TEXT_ROWS
    };
    let log_h = m.line_height * log_rows + m.inset * 2.0;
    let key_h = m.line_height;
    let map_h = screen_h - content_top - log_h - key_h - m.gap;

    let log_y = content_top + map_h + m.gap;
    HudRegions {
        status_bar: Rect::new(0.0, 0.0, screen_w, head_h),
        map_pane: Rect::new(0.0, content_top, left_w, map_h),
        log_pane: Rect::new(0.0, log_y, left_w, log_h),
        key_bar: Rect::new(0.0, log_y + log_h - key_h, left_w, key_h),
        info_column: Rect::new(
            screen_w - info_w,
            content_top,
            info_w,
            screen_h - content_top,
        ),
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
        regions(w, h, CHAR_W, &ui_metrics(h), false)
    }

    fn at_expanded(w: f32, h: f32) -> HudRegions {
        regions(w, h, CHAR_W, &ui_metrics(h), true)
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

    /// **Bug A's arithmetic half.** `base.rs::the_map_frames_top_strips_
    /// clear_the_status_bar` is the test that catches a regression here
    /// against the *real* painted strip; this pins the ratio itself, so an
    /// edit to `strip::PAD_RATIO` or `strip::BASELINE_RATIO` that reopens
    /// the gap fails here first, without a `Painter`.
    #[test]
    fn the_map_panes_top_clears_a_border_strips_quad() {
        for (w, h) in SIZES {
            let m = ui_metrics(h);
            let r = at(w, h);
            let clearance = m.small() as f32 * TOP_STRIP_CLEARANCE_RATIO;
            let quad_top = r.map_pane.y - clearance;
            assert!(
                quad_top >= r.status_bar.y + r.status_bar.h - 0.001,
                "a top strip on the map pane would paint into the status bar \
                 at {w}x{h}: quad top {quad_top}, bar bottom {}",
                r.status_bar.y + r.status_bar.h
            );
        }
    }

    /// **Bug B.** SPACE doubles the log pane's row count, so its height
    /// grows by exactly the four extra rows — nothing else on the screen is
    /// supposed to move because of it.
    #[test]
    fn expanding_the_log_doubles_its_extra_row_height() {
        for (w, h) in SIZES {
            let m = ui_metrics(h);
            let collapsed = at(w, h);
            let expanded = at_expanded(w, h);
            let want_taller_by = m.line_height * LOG_TEXT_ROWS;
            assert!(
                (expanded.log_pane.h - collapsed.log_pane.h - want_taller_by).abs() < 0.001,
                "log pane grew by {} at {w}x{h}, wanted {want_taller_by}",
                expanded.log_pane.h - collapsed.log_pane.h
            );
            assert!(
                expanded.log_pane.h > collapsed.log_pane.h,
                "SPACE did not grow the log pane at {w}x{h}"
            );
        }
    }

    /// The module's two load-bearing rules, re-run with the log expanded —
    /// a taller log pane eats into `map_pane`, not into the column, so
    /// neither rule may lapse just because SPACE was pressed.
    #[test]
    fn expanding_the_log_still_satisfies_the_module_rules() {
        for (w, h) in SIZES {
            let r = at_expanded(w, h);
            assert!(
                (r.info_column.y + r.info_column.h - h).abs() < 0.001,
                "column stops short of the bottom edge at {w}x{h} expanded"
            );
            assert!(
                r.log_pane.x + r.log_pane.w <= r.info_column.x,
                "the expanded log passes under the info column at {w}x{h}"
            );
            assert!(
                r.key_bar.x + r.key_bar.w <= r.info_column.x,
                "the keybar runs under the column at {w}x{h} expanded"
            );
            for (name, rect) in [
                ("status_bar", r.status_bar),
                ("map_pane", r.map_pane),
                ("log_pane", r.log_pane),
                ("key_bar", r.key_bar),
                ("info_column", r.info_column),
            ] {
                assert!(rect.h > 0.0, "{name} has height {} at {w}x{h}", rect.h);
            }
        }
    }
}
