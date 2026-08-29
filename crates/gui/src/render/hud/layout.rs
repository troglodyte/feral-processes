//! Where the five HUD regions sit. Pure arithmetic — no `Painter`, no
//! drawing, no engine.
//!
//! Two of the rules here are load-bearing rather than decorative, and both
//! are asserted below rather than left to the arithmetic:
//!
//! 1. **The info column ends where the log pane does.** The log does not
//!    pass under it, and the column does not overhang it.
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
/// centres its background quad *on* the line it rides
/// (`strip::border_strip`), reaching `size/2 + pad/2` past that line on
/// *both* sides, where `size = m.small()` and `pad = size *
/// strip::PAD_RATIO`. Anything opaque painted on either side of the line
/// after the strip therefore covers the glyphs riding it.
///
/// Both ends of the map pane are that case, which is why this is one
/// constant and not a top one and a bottom one. Its title and threat
/// readout ride the top border at `pane.y`, and `draw_status_bar` fills the
/// bar *after* `draw_map_frame` has run; its vitals ride the bottom border
/// at `pane.y + pane.h`, and `draw_log_pane` fills the log pane after it
/// too — which is what cut the bottom off MIT/ATK/STR while the separation
/// between the two panes was only `m.gap`.
///
/// Expressed as a ratio of `m.small()`, not a pixel figure, so it travels
/// with the font size the same way every other figure here does. Built from
/// `strip::PAD_RATIO` directly rather than a copied `0.5` so the two cannot
/// drift apart.
const STRIP_CLEARANCE_RATIO: f32 = 0.5 + strip::PAD_RATIO / 2.0;

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
    /// Framed. Map-pane width only — never the window's. Its bottom edge
    /// is fixed; expanding it grows it *upward over* `map_pane`, which is
    /// drawn first.
    pub log_pane: Rect,
    /// Drawn on `log_pane`'s bottom border line.
    pub key_bar: Rect,
    /// Ends on `log_pane`'s bottom edge, `key_h` short of the window's:
    /// the keybar's glyphs straddle that line, and a column running past it
    /// overhangs the pane beside it.
    pub info_column: Rect,
}

/// Derives the five regions for a window.
///
/// `char_w` is the UI face's advance for one character, measured by the
/// caller — see the module comment for why it is not read off `m`.
/// `log_expanded` is `App::log_expanded` — SPACE on the map screen doubles
/// `log_pane`'s row count and back. The extra rows are taken upward over
/// `map_pane`, whose geometry does not depend on the flag at all.
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
    // own rect only; see `STRIP_CLEARANCE_RATIO` for why the panes below it
    // start further down than this.
    let head_h = m.line_height + m.inset;
    let strip_clearance = m.small() as f32 * STRIP_CLEARANCE_RATIO;
    let content_top = head_h + strip_clearance;

    // The map is laid out against the *collapsed* log at every window size:
    // SPACE changes what the log shows, not what the screen is, so the pane
    // it grows over keeps its geometry and the grid does not re-lay-out
    // under the player.
    let log_h = |rows: f32| m.line_height * rows + m.inset * 2.0;
    let collapsed_log_h = log_h(LOG_TEXT_ROWS);
    let key_h = m.line_height;
    // The map pane's vitals ride its bottom border and paint below it, and
    // the log pane's opaque fill lands after them. `m.gap` is narrower than
    // that reach at every window size, so the breathing space between the
    // two panes is whichever is larger.
    let pane_gap = m.gap.max(strip_clearance);
    let map_h = screen_h - content_top - collapsed_log_h - key_h - pane_gap;

    // Pinned at the bottom, so the expanded pane grows upward *over* the
    // map. `draw_playing_base` draws the log last and it fills opaquely, so
    // the overlay costs nothing but this subtraction.
    let log_bottom = content_top + map_h + pane_gap + collapsed_log_h;
    let log_h = if log_expanded {
        log_h(LOG_TEXT_ROWS_EXPANDED)
    } else {
        collapsed_log_h
    };
    let log_y = log_bottom - log_h;
    HudRegions {
        status_bar: Rect::new(0.0, 0.0, screen_w, head_h),
        map_pane: Rect::new(0.0, content_top, left_w, map_h),
        log_pane: Rect::new(0.0, log_y, left_w, log_h),
        key_bar: Rect::new(0.0, log_y + log_h - key_h, left_w, key_h),
        info_column: Rect::new(
            screen_w - info_w,
            content_top,
            info_w,
            log_bottom - content_top,
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

    /// Rule 1. The column and the log pane end on one line. The column used
    /// to run to `screen_h` while the log stopped `key_h` short of it — the
    /// reserve the keybar's glyphs straddle — so the column overhung the
    /// pane beside it by a row, which reads as the column having been drawn
    /// too long rather than as a margin.
    ///
    /// Asserted against the *collapsed* pane as well as the expanded one:
    /// the log's bottom edge is pinned, so one line answers for both.
    #[test]
    fn the_info_column_ends_where_the_log_pane_does() {
        for (w, h) in SIZES {
            for r in [at(w, h), at_expanded(w, h)] {
                let column_bottom = r.info_column.y + r.info_column.h;
                let log_bottom = r.log_pane.y + r.log_pane.h;
                assert!(
                    (column_bottom - log_bottom).abs() < 0.001,
                    "column ends at {column_bottom}, log pane at {log_bottom} \
                     at {w}x{h}"
                );
            }
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
            let clearance = m.small() as f32 * STRIP_CLEARANCE_RATIO;
            let quad_top = r.map_pane.y - clearance;
            assert!(
                quad_top >= r.status_bar.y + r.status_bar.h - 0.001,
                "a top strip on the map pane would paint into the status bar \
                 at {w}x{h}: quad top {quad_top}, bar bottom {}",
                r.status_bar.y + r.status_bar.h
            );
        }
    }

    /// **Bug A's arithmetic, at the other border.** The vitals strip rides
    /// `map_pane`'s bottom line and its quad reaches the same distance
    /// *below* it, where the log pane's opaque fill lands afterwards.
    /// `base.rs::the_map_frames_vitals_strip_clears_the_log_pane` is the
    /// same assertion against the real painted quad; this one pins the
    /// arithmetic, so a separation narrowed back to `m.gap` fails here
    /// without a `Painter`.
    ///
    /// The collapsed state only, and deliberately: an *expanded* log is an
    /// overlay over the bottom of the map, so it covers the vitals for as
    /// long as it is open. That is the state SPACE asks for, not a clipped
    /// strip in the state the player spends the game in.
    #[test]
    fn the_map_panes_bottom_clears_a_border_strips_quad() {
        for (w, h) in SIZES {
            let m = ui_metrics(h);
            let r = at(w, h);
            let clearance = m.small() as f32 * STRIP_CLEARANCE_RATIO;
            let quad_bottom = r.map_pane.y + r.map_pane.h + clearance;
            assert!(
                quad_bottom <= r.log_pane.y + 0.001,
                "the vitals strip would paint into the log pane at {w}x{h}: \
                 quad bottom {quad_bottom}, log top {}",
                r.log_pane.y
            );
        }
    }

    /// **Bug C.** SPACE is a change of what the log shows, not of what the
    /// screen is. The expanded pane grows *upward over* the map — bottom
    /// edge pinned, `map_pane` untouched — because paying for the extra
    /// rows out of the map's height re-lays the whole grid out and the map
    /// jumps under the player's eyes. The draw order already supports it:
    /// `draw_playing_base` paints the map and its frame first and
    /// `draw_log_pane` last, over an opaque fill.
    #[test]
    fn expanding_the_log_overlays_the_map_instead_of_shrinking_it() {
        for (w, h) in SIZES {
            let collapsed = at(w, h);
            let expanded = at_expanded(w, h);
            assert_eq!(
                collapsed.map_pane, expanded.map_pane,
                "the map pane moved when the log expanded at {w}x{h}"
            );
            let collapsed_bottom = collapsed.log_pane.y + collapsed.log_pane.h;
            let expanded_bottom = expanded.log_pane.y + expanded.log_pane.h;
            assert!(
                (collapsed_bottom - expanded_bottom).abs() < 0.001,
                "the log's bottom edge moved from {collapsed_bottom} to \
                 {expanded_bottom} at {w}x{h} — it grew downward, not up"
            );
            assert!(
                expanded.log_pane.y < collapsed.map_pane.y + collapsed.map_pane.h,
                "the expanded log at {w}x{h} starts at {} and never reaches \
                 the map's {} bottom edge, so it overlays nothing",
                expanded.log_pane.y,
                collapsed.map_pane.y + collapsed.map_pane.h
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

    /// The module's second rule and the extents, re-run with the log
    /// expanded — the taller pane grows over `map_pane` and must still stop
    /// at the column's edge. Rule 1 is asserted for both states by
    /// `the_info_column_ends_where_the_log_pane_does`.
    #[test]
    fn expanding_the_log_still_satisfies_the_module_rules() {
        for (w, h) in SIZES {
            let r = at_expanded(w, h);
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
