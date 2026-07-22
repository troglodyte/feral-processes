//! Fonts, text sizing, and terrain color math for the graphics frontend.
//!
//! Split out of `render.rs` for the same reason `fx.rs` was: this is pure
//! and worth unit-testing, while the drawing that consumes it can't be
//! tested at all without a window. Holds `Fonts` (the three loaded faces),
//! the map-glyph and UI-text sizing rules, and `terrain_color`.
//!
//! Two independent sizing rules live here. Map glyphs are sized by zoom
//! alone, in strict integer multiples of the pixel font's native cell.
//! UI text is sized continuously from the window height. They never mix.

use feral_processes_app_core::{MAX_ZOOM, MIN_ZOOM};
use macroquad::prelude::*;

/// The three faces the frontend draws with: a pixel font for the map grid
/// and a vector monospace, regular and bold, for everything else.
///
/// Embedded with `include_bytes!` rather than loaded from `assets_dir` for
/// the same reason the sound effects are (see `sounds.rs`): fonts aren't
/// moddable game content.
pub struct Fonts {
    map: Font,
    ui: Font,
    ui_bold: Font,
}

impl Fonts {
    /// Not async, unlike `SoundBank::load` — macroquad's font loader is
    /// synchronous — but it still reaches for the graphics context, so it
    /// has to run after the window exists.
    pub fn load() -> Self {
        let mut map =
            load_ttf_font_from_bytes(include_bytes!("../../../assets/fonts/unscii-16.ttf"))
                .expect("embedded unscii-16 is valid ttf");
        // unscii is vectorized outlines of a bitmap, so it only stays
        // crisp under nearest-neighbour sampling. The loader applies the
        // context default, which is linear.
        map.set_filter(FilterMode::Nearest);
        Self {
            map,
            ui: load_ttf_font_from_bytes(include_bytes!(
                "../../../assets/fonts/DejaVuSansMono.ttf"
            ))
            .expect("embedded DejaVu Sans Mono is valid ttf"),
            ui_bold: load_ttf_font_from_bytes(include_bytes!(
                "../../../assets/fonts/DejaVuSansMono-Bold.ttf"
            ))
            .expect("embedded DejaVu Sans Mono Bold is valid ttf"),
        }
    }

    pub fn ui(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(&self.ui),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn ui_bold(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(&self.ui_bold),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn map(&self, glyph: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            glyph,
            x,
            y,
            TextParams {
                font: Some(&self.map),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn measure_ui(&self, text: impl AsRef<str>, size: u16) -> TextDimensions {
        measure_text(text, Some(&self.ui), size, 1.0)
    }

    pub fn measure_map(&self, glyph: impl AsRef<str>, size: u16) -> TextDimensions {
        measure_text(glyph, Some(&self.map), size, 1.0)
    }
}

/// unscii-16's native cell height. Map glyphs are only ever drawn at
/// integer multiples of this, so the vectorized bitmap keeps landing on
/// the pixel grid instead of resampling into mush.
const MAP_GLYPH_NATIVE: u16 = 16;
/// Tile edge at zoom 1, leaving a native glyph a margin inside its cell.
const BASE_TILE_PX: f32 = 20.0;

/// Tile edge and glyph size in pixels for a zoom step.
///
/// Map sizing is driven by zoom alone and never by window size: a larger
/// window shows *more tiles at the same size*, which is what the TUI
/// already does and what reads correctly for a grid.
pub fn map_cell(zoom: u16) -> (f32, u16) {
    let z = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    (BASE_TILE_PX * z as f32, MAP_GLYPH_NATIVE * z)
}

/// The `window_conf()` height every hardcoded size in `render.rs` was
/// originally tuned against.
const REFERENCE_HEIGHT: f32 = 900.0;
const BASE_UI_FONT: f32 = 24.0;
const MIN_UI_FONT: u16 = 16;
const MAX_UI_FONT: u16 = 40;
/// Preserves the 30.0 / 24.0 relationship the fixed constants had.
const LINE_HEIGHT_RATIO: f32 = 1.25;
/// Ratios chosen to reproduce the existing literals exactly at
/// `BASE_UI_FONT`: 16.0, 10.0 and 6.0 respectively.
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

/// Terrain keeps a quarter of its hue and the rest goes to grey, leaving
/// entity glyphs the only saturated thing on the map.
///
/// Brightness can't be the discriminator: `fx::structure_condition` dims a
/// damaged structure toward a floor of `MIN_TINT`, and `GlyphColor::DarkGreen`
/// is already `(0.0, 0.4, 0.0)`, so a damaged one sits near 0.11 luminance.
/// Terrain would have to be nearly black to stay reliably dimmer than that.
/// Saturation has no equivalent failure case.
const TERRAIN_SATURATION: f32 = 0.25;
const TERRAIN_BRIGHTNESS: f32 = 0.70;

/// Pushes a biome's glyph color back toward grey so entities read out of
/// the terrain without needing a faked bold weight.
pub fn terrain_color(c: Color) -> Color {
    let luminance = 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;
    let toward_grey = |channel: f32| {
        (luminance + (channel - luminance) * TERRAIN_SATURATION) * TERRAIN_BRIGHTNESS
    };
    Color::new(toward_grey(c.r), toward_grey(c.g), toward_grey(c.b), c.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Representative sample colors for testing `terrain_color`: a
    /// saturated primary, a near-black `DarkGreen`, and a brown, so the
    /// assertions cover a spread of hues and luminances rather than one
    /// convenient case. `DarkGreen` in particular is the dark extreme —
    /// see the discussion on `terrain_color` of why brightness alone can't
    /// do this job.
    const SAMPLE_COLORS: [Color; 8] = [
        Color::new(0.3, 0.55, 0.95, 1.0),  // BLUE
        Color::new(0.9, 0.25, 0.25, 1.0),  // RED
        Color::new(0.25, 0.85, 0.85, 1.0), // CYAN
        Color::new(0.35, 0.85, 0.4, 1.0),  // GREEN
        Color::new(0.95, 0.55, 0.15, 1.0), // ORANGE
        Color::new(0.8, 0.35, 0.85, 1.0),  // MAGENTA
        Color::new(0.0, 0.4, 0.0, 1.0),    // DarkGreen — the dark extreme
        Color::new(0.55, 0.27, 0.07, 1.0), // Brown
    ];

    fn luminance(c: Color) -> f32 {
        0.299 * c.r + 0.587 * c.g + 0.114 * c.b
    }

    /// Chroma: how far apart the channels are, standing in for saturation.
    fn spread(c: Color) -> f32 {
        c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
    }

    #[test]
    fn terrain_color_desaturates_every_palette_color() {
        for c in SAMPLE_COLORS {
            assert!(
                spread(terrain_color(c)) < spread(c),
                "{c:?} came back no less saturated than it went in"
            );
        }
    }

    #[test]
    fn terrain_color_shrinks_saturation_by_exactly_the_configured_factor() {
        // Desaturating and dimming are both affine on each channel, so the
        // channel spread scales by exactly the product of the two factors.
        for c in SAMPLE_COLORS {
            let expected = spread(c) * TERRAIN_SATURATION * TERRAIN_BRIGHTNESS;
            assert!(
                (spread(terrain_color(c)) - expected).abs() < 1e-5,
                "{c:?}: expected spread {expected}, got {}",
                spread(terrain_color(c))
            );
        }
    }

    #[test]
    fn terrain_color_dims_luminance_by_exactly_the_brightness_factor() {
        // Luminance-weighted desaturation is luminance-preserving, so the
        // only thing left acting on luminance is the brightness factor.
        for c in SAMPLE_COLORS {
            let expected = luminance(c) * TERRAIN_BRIGHTNESS;
            assert!(
                (luminance(terrain_color(c)) - expected).abs() < 1e-5,
                "{c:?}: expected luminance {expected}, got {}",
                luminance(terrain_color(c))
            );
        }
    }

    #[test]
    fn terrain_color_leaves_a_grey_input_grey() {
        let grey = Color::new(0.5, 0.5, 0.5, 1.0);
        let out = terrain_color(grey);
        assert!(
            (out.r - out.g).abs() < 1e-6 && (out.g - out.b).abs() < 1e-6,
            "grey picked up a color cast: {out:?}"
        );
    }

    #[test]
    fn terrain_color_preserves_alpha() {
        for c in SAMPLE_COLORS {
            assert_eq!(terrain_color(c).a, c.a);
        }
    }

    #[test]
    fn terrain_color_preserves_channel_ordering() {
        // The exact-factor tests (saturation and brightness) are invariant to a
        // sign inversion in the lerp: if the desaturation formula were written
        // as (luminance - channel) instead of (channel - luminance), the function
        // would invert hue rather than desaturate, but both of those tests would
        // still pass because they only look at magnitude. This test closes that
        // hole by checking that which channel is largest (and smallest) stays the same.
        for c in SAMPLE_COLORS {
            if spread(c) < 1e-6 {
                continue; // Grey samples have no meaningful channel order.
            }

            let in_vals = [c.r, c.g, c.b];
            let out = terrain_color(c);
            let out_vals = [out.r, out.g, out.b];

            // Which channel index holds the max value in input?
            let max_idx_in = in_vals
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap()
                .0;
            // Which channel index holds the min value in input?
            let min_idx_in = in_vals
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap()
                .0;

            // Which channel index holds the max value in output?
            let max_idx_out = out_vals
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap()
                .0;
            // Which channel index holds the min value in output?
            let min_idx_out = out_vals
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap()
                .0;

            let channel_names = ["R", "G", "B"];
            assert!(
                max_idx_in == max_idx_out && min_idx_in == min_idx_out,
                "{c:?} ({:.3}, {:.3}, {:.3}) → ({:.3}, {:.3}, {:.3}): {} was max, now {}; {} was min, now {}",
                c.r,
                c.g,
                c.b,
                out.r,
                out.g,
                out.b,
                channel_names[max_idx_in],
                channel_names[max_idx_out],
                channel_names[min_idx_in],
                channel_names[min_idx_out]
            );
        }
    }

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
    fn ui_metrics_reproduces_todays_sizes_at_the_reference_window_height() {
        // The whole refactor rests on this: at the default window, every
        // number must come out exactly where it is hardcoded today, so a
        // layout shift at default size means the refactor is wrong.
        let m = ui_metrics(REFERENCE_HEIGHT);
        assert_eq!(m.font_size, 24);
        assert_eq!(m.line_height, 30.0);
        assert_eq!(m.pad, 16.0);
        assert_eq!(m.inset, 10.0);
        assert_eq!(m.gap, 6.0);
        assert_eq!(m.title(), 28);
        assert_eq!(m.label(), 22);
        assert_eq!(m.small(), 20);
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
