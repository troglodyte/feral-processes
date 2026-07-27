//! The drawing seam: every primitive the `render` module needs, plus the
//! colour and geometry types it passes around.
//!
//! `render/` is ~3,000 lines of immediate-mode drawing. Rather than have all
//! of it name a graphics library directly — which is what made the frontend
//! macroquad-shaped in the first place — it names `Painter`, and the library
//! lives behind that. Swapping backends is then a change to this file rather
//! than to every menu.
//!
//! The surface is deliberately tiny: filled rect, outlined rect, line, text
//! in one of three faces, text measurement, and the frame's dimensions and
//! clock. That is the whole vocabulary the screens are drawn in.
//!
//! **Text is positioned by baseline**, not by top edge — `y` is where the
//! glyph bottoms sit, ignoring descenders. Every layout in `render/` is
//! written against that convention (rows advance by `Metrics::line_height`
//! from one baseline to the next), so a backend that positions text by its
//! top-left has to convert rather than reinterpret.

use macroquad::prelude::{
    FilterMode, Font, TextParams, draw_line, draw_rectangle, draw_rectangle_lines, draw_text_ex,
    get_time, load_ttf_font_from_bytes, measure_text, screen_height, screen_width,
};

/// Straight RGBA, each channel 0.0–1.0, non-premultiplied.
///
/// A local type rather than the backend's own so the palette in
/// `render/mod.rs` and the colour math in `text.rs` and `fx.rs` survive a
/// backend swap untouched — those are the parts with real reasoning in them
/// (see `terrain_color` and `structure_condition`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// The handful of named colours that used to come in from the backend's
/// prelude. The palette proper lives in `render/mod.rs`; these are only the
/// greys and white the map's biome and glyph tables reach for.
pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
pub const GRAY: Color = Color::new(0.51, 0.51, 0.51, 1.0);
pub const DARKGRAY: Color = Color::new(0.31, 0.31, 0.31, 1.0);

/// An axis-aligned box in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
}

/// Measured extent of a run of text. `height` is the cap height the callers
/// centre glyphs by, not the font's full line box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextDims {
    pub width: f32,
    pub height: f32,
}

/// Which of the three loaded faces to draw a string in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// Vector monospace, for every menu, label and message.
    Ui,
    /// The same at bold weight, for emphasis.
    UiBold,
    /// The pixel font the map grid is drawn in. Only ever used at integer
    /// multiples of its native cell — see `text::map_cell`.
    Map,
}

/// Everything a screen needs in order to draw itself.
///
/// Threaded through `render/` in the parameter slot that used to carry
/// `&Fonts`; it additionally carries the frame's dimensions and clock, which
/// the drawing code previously read from backend globals. Created once (font
/// loading is not cheap) and refreshed each frame by `begin_frame`.
pub struct Painter {
    fonts: Fonts,
    width: f32,
    height: f32,
    time: f64,
    delta: f32,
}

impl Painter {
    /// Loads the embedded faces. Must run after the window exists — the font
    /// loader reaches for the graphics context.
    pub fn new() -> Self {
        Self {
            fonts: Fonts::load(),
            width: 0.0,
            height: 0.0,
            time: 0.0,
            delta: 0.0,
        }
    }

    /// Latches the frame's dimensions and clock, so that everything drawn
    /// inside one frame agrees about both. Reading them per-call instead
    /// would let a resize land midway through a screen and tear its layout.
    pub fn begin_frame(&mut self) {
        self.width = screen_width();
        self.height = screen_height();
        self.time = get_time();
        self.delta = macroquad::prelude::get_frame_time();
    }

    pub fn screen_w(&self) -> f32 {
        self.width
    }

    pub fn screen_h(&self) -> f32 {
        self.height
    }

    /// Seconds since startup. The one clock the frontend's animations run
    /// on — `Fx` timing and the toast expiry both read it from here.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Seconds the previous frame took. Drives the rate-based animations —
    /// the ghost bars drain per second, not per frame, so they look the
    /// same regardless of framerate.
    pub fn delta(&self) -> f32 {
        self.delta
    }

    pub fn clear(&self, color: Color) {
        macroquad::prelude::clear_background(to_backend(color));
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        draw_rectangle(x, y, w, h, to_backend(color));
    }

    pub fn rect_lines(&self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
        draw_rectangle_lines(x, y, w, h, thickness, to_backend(color));
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        draw_line(x1, y1, x2, y2, thickness, to_backend(color));
    }

    /// Draws `text` with its baseline at `y`. See the module docs on why
    /// that, and not the top edge, is the anchor.
    pub fn text(&self, face: Face, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            text.as_ref(),
            x,
            y,
            TextParams {
                font: Some(self.fonts.face(face)),
                font_size: size,
                color: to_backend(color),
                ..Default::default()
            },
        );
    }

    pub fn measure(&self, face: Face, text: impl AsRef<str>, size: u16) -> TextDims {
        let d = measure_text(text.as_ref(), Some(self.fonts.face(face)), size, 1.0);
        TextDims {
            width: d.width,
            height: d.height,
        }
    }

    /// `text` in the regular UI face — the overwhelmingly common case.
    pub fn ui(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        self.text(Face::Ui, text, x, y, size, color);
    }

    pub fn ui_bold(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        self.text(Face::UiBold, text, x, y, size, color);
    }

    pub fn map(&self, glyph: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        self.text(Face::Map, glyph, x, y, size, color);
    }

    pub fn measure_ui(&self, text: impl AsRef<str>, size: u16) -> TextDims {
        self.measure(Face::Ui, text, size)
    }

    pub fn measure_map(&self, glyph: impl AsRef<str>, size: u16) -> TextDims {
        self.measure(Face::Map, glyph, size)
    }
}

/// The three faces the frontend draws with: a pixel font for the map grid
/// and a vector monospace, regular and bold, for everything else.
///
/// Embedded with `include_bytes!` rather than loaded from `assets_dir` for
/// the same reason the sound effects are (see `sounds.rs`): fonts aren't
/// moddable game content.
struct Fonts {
    map: Font,
    ui: Font,
    ui_bold: Font,
}

impl Fonts {
    fn load() -> Self {
        let mut map =
            load_ttf_font_from_bytes(include_bytes!("../../../assets/fonts/unscii-16.ttf"))
                .expect("embedded unscii-16 is valid ttf");
        // unscii is vectorized outlines of a bitmap, so it only stays crisp
        // under nearest-neighbour sampling. The loader applies the context
        // default, which is linear.
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

    fn face(&self, face: Face) -> &Font {
        match face {
            Face::Ui => &self.ui,
            Face::UiBold => &self.ui_bold,
            Face::Map => &self.map,
        }
    }
}

fn to_backend(c: Color) -> macroquad::prelude::Color {
    macroquad::prelude::Color::new(c.r, c.g, c.b, c.a)
}
