//! Text sizing for the graphics frontend.
//!
//! Split out of `render.rs` for the same reason `fx.rs` was: this is pure
//! and worth unit-testing, while the drawing that consumes it can't be
//! tested at all without a window. Holds the map-glyph and UI-text sizing
//! rules; the faces themselves belong to `Painter`.
//!
//! Two independent sizing rules live here. Map glyphs are sized by zoom
//! alone, in strict integer multiples of the pixel font's native cell.
//! UI text is sized continuously from the window height. They never mix.

use feral_processes_app_core::{MAX_ZOOM, MIN_ZOOM};

/// unscii-16's native cell height. Map glyphs are only ever drawn at
/// integer multiples of this, so the vectorized bitmap keeps landing on
/// the pixel grid instead of resampling into mush.
const MAP_GLYPH_NATIVE: u16 = 16;
/// Tile edge at zoom 1, leaving a native glyph a margin inside its cell.
const BASE_TILE_PX: f32 = 20.0;

/// Tile edge and glyph size in pixels for a zoom step.
///
/// Map sizing is driven by zoom alone and never by window size: a larger
/// window shows *more tiles at the same size*, which is what reads
/// correctly for a grid.
pub fn map_cell(zoom: u16) -> (f32, u16) {
    let z = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    (BASE_TILE_PX * z as f32, MAP_GLYPH_NATIVE * z)
}

/// The `window_conf()` height every hardcoded size in `render.rs` was
/// originally tuned against.
const REFERENCE_HEIGHT: f32 = 900.0;
/// Body text at `REFERENCE_HEIGHT`. Below the 24.0 the layout was originally
/// tuned at: the window opens fullscreen now, and on a 1440-tall display the
/// linear ramp put body text at 38px, which reads as a magnified UI rather
/// than a roomier one. Lowering the base rather than capping the ramp is a
/// deliberate choice to shrink text at *every* window size, not just tall
/// ones. Pinned by `ui_metrics_anchors_the_ramp_at_the_reference_height`.
///
/// This is low enough that `MIN_UI_FONT` now takes over below a ~825-tall
/// window — the ramp is rounded before it is clamped, so the floor bites
/// where the scaled value rounds to 16, not where it reaches it. A 720p
/// window therefore no longer scales at all; the floor is a legibility
/// limit and is left where it is. Lowering the base further raises the
/// height at which the UI stops shrinking, rather than making small
/// windows denser. Pinned by
/// `the_font_floor_takes_over_below_a_825_tall_window`.
const BASE_UI_FONT: f32 = 18.0;
const MIN_UI_FONT: u16 = 16;
const MAX_UI_FONT: u16 = 40;
/// Preserves the 30.0 / 24.0 relationship the fixed constants had.
const LINE_HEIGHT_RATIO: f32 = 1.25;
/// Ratios chosen to reproduce `render.rs`'s original literals — 16.0, 10.0
/// and 6.0 — at the 24px font those were tuned against, which is no longer
/// `BASE_UI_FONT`. They are what holds the *proportions* now, not those
/// three numbers.
const PAD_RATIO: f32 = 2.0 / 3.0;
const INSET_RATIO: f32 = 5.0 / 12.0;
const GAP_RATIO: f32 = 0.25;

/// Every UI dimension that used to be a literal in `render.rs`, scaled to
/// the current window.
pub struct Metrics {
    pub font_size: u16,
    pub line_height: f32,
    /// Inset from a popup's edge to its content.
    pub pad: f32,
    /// Inset from a panel's edge to its content.
    pub inset: f32,
    /// Vertical breathing space between groups of rows.
    pub gap: f32,
}

impl Metrics {
    /// Popup titles.
    pub fn title(&self) -> u16 {
        self.font_size + 4
    }

    /// Bar labels.
    pub fn label(&self) -> u16 {
        self.font_size - 2
    }

    /// Keybind hints and scroll indicators. These are currently 3px and
    /// 4px below the body font respectively; a one-pixel difference
    /// between two unrelated bits of chrome isn't worth carrying through
    /// a scaling system, so both collapse to one size.
    pub fn small(&self) -> u16 {
        self.font_size - 4
    }
}

/// UI text scales continuously with window height, unlike map glyphs.
/// `TextParams::font_size` is a `u16`, so sizes are already quantized to
/// whole pixels and the font atlas gains at most a few dozen entries over
/// a resize drag — no separate stepping scheme is needed.
pub fn ui_metrics(window_height: f32) -> Metrics {
    let scaled = (BASE_UI_FONT * window_height / REFERENCE_HEIGHT).round();
    let font_size = (scaled as u16).clamp(MIN_UI_FONT, MAX_UI_FONT);
    let f = font_size as f32;
    Metrics {
        font_size,
        line_height: f * LINE_HEIGHT_RATIO,
        pad: f * PAD_RATIO,
        inset: f * INSET_RATIO,
        gap: f * GAP_RATIO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zoom_step_gets_its_own_tile_size() {
        // Zoom 4 used to render identically to zoom 3, so `+` at zoom 3
        // was a keypress that changed state and nothing else.
        let sizes: Vec<f32> = (MIN_ZOOM..=MAX_ZOOM).map(|z| map_cell(z).0).collect();
        for pair in sizes.windows(2) {
            assert!(
                pair[1] > pair[0],
                "each zoom step must grow the tile: {sizes:?}"
            );
        }
    }

    #[test]
    fn map_glyphs_always_land_on_an_integer_multiple_of_the_native_cell() {
        for z in MIN_ZOOM..=MAX_ZOOM {
            let (_, glyph) = map_cell(z);
            assert_eq!(
                glyph % MAP_GLYPH_NATIVE,
                0,
                "zoom {z} wants a {glyph}px glyph, which is off the pixel grid"
            );
        }
    }

    #[test]
    fn map_cell_clamps_zoom_outside_the_supported_range() {
        assert_eq!(map_cell(0), map_cell(MIN_ZOOM));
        assert_eq!(map_cell(99), map_cell(MAX_ZOOM));
    }

    #[test]
    fn the_zoom_ladder_is_pinned_end_to_end() {
        // Zooms 1-3 keep the tile sizes they had before the pixel font.
        // Zoom 4 had no size of its own — it rendered as zoom 3 — so 80.0
        // is new rather than preserved.
        //
        // The glyph sizes are also spelled out as a literal `LADDER` in
        // tests/font_rasterization.rs, which cannot call `map_cell`
        // because `text` is a private module. Pinning all four here is
        // what keeps that hand-copy honest.
        assert_eq!(map_cell(1), (20.0, 16));
        assert_eq!(map_cell(2), (40.0, 32));
        assert_eq!(map_cell(3), (60.0, 48));
        assert_eq!(map_cell(4), (80.0, 64));
    }

    #[test]
    fn ui_metrics_anchors_the_ramp_at_the_reference_height() {
        // The ramp is linear and unclamped through here, so this one height
        // fixes every other: a change to `BASE_UI_FONT` that wasn't meant to
        // move the whole UI fails here first. `pad` and `inset` come off
        // ratios that aren't exact in binary, so they're compared loosely —
        // the point is the proportion, not the last bit.
        let m = ui_metrics(REFERENCE_HEIGHT);
        assert_eq!(m.font_size, BASE_UI_FONT as u16);
        assert_eq!(m.font_size, 18);
        assert_eq!(m.line_height, 22.5);
        assert!((m.pad - 12.0).abs() < 0.01, "pad was {}", m.pad);
        assert!((m.inset - 7.5).abs() < 0.01, "inset was {}", m.inset);
        assert_eq!(m.gap, 4.5);
        assert_eq!(m.title(), 22);
        assert_eq!(m.label(), 16);
        assert_eq!(m.small(), 14);
    }

    #[test]
    fn the_font_floor_takes_over_below_a_825_tall_window() {
        // Stated in `BASE_UI_FONT`'s doc comment, and the reason a further
        // drop buys nothing on a small window. Straddled rather than
        // pinned at 825 itself: that height puts the ramp exactly on 16.5,
        // where the answer turns on a rounding tie no behaviour depends on.
        assert_eq!(ui_metrics(824.0).font_size, MIN_UI_FONT);
        assert!(ui_metrics(826.0).font_size > MIN_UI_FONT);
    }

    #[test]
    fn ui_metrics_clamps_at_both_extremes() {
        assert_eq!(ui_metrics(1.0).font_size, MIN_UI_FONT);
        assert_eq!(ui_metrics(0.0).font_size, MIN_UI_FONT);
        assert_eq!(ui_metrics(100_000.0).font_size, MAX_UI_FONT);
    }

    #[test]
    fn ui_metrics_keeps_lines_taller_than_their_text_at_every_window_size() {
        for h in (100..4000).step_by(37) {
            let m = ui_metrics(h as f32);
            assert!(
                m.line_height > m.font_size as f32,
                "line height collapsed onto the font at window height {h}"
            );
        }
    }

    #[test]
    fn ui_metrics_keeps_the_size_ramp_ordered_including_at_the_clamps() {
        for h in (100..4000).step_by(37) {
            let m = ui_metrics(h as f32);
            assert!(m.small() < m.font_size, "small() inverted at height {h}");
            assert!(m.label() < m.font_size, "label() inverted at height {h}");
            assert!(m.font_size < m.title(), "title() inverted at height {h}");
        }
    }

    #[test]
    fn ui_metrics_scales_monotonically_between_the_clamps() {
        let mut previous = 0;
        for h in (100..4000).step_by(37) {
            let size = ui_metrics(h as f32).font_size;
            assert!(size >= previous, "font shrank as the window grew at {h}");
            previous = size;
        }
    }
}
