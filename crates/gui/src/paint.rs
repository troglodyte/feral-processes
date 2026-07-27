//! The drawing seam: every primitive the `render` module needs, plus the
//! colour and geometry types it passes around.
//!
//! `render/` is ~3,000 lines of immediate-mode drawing. Rather than have all
//! of it name a graphics library directly — which is what made the frontend
//! macroquad-shaped in the first place — it names `Painter`, and the library
//! lives behind that. Swapping backends is a change to this file rather than
//! to every menu.
//!
//! The surface is deliberately tiny: filled rect, outlined rect, line, text
//! in one of three faces, text measurement, and the frame's dimensions and
//! frame delta. That is the whole vocabulary the screens are drawn in.
//!
//! **Text is positioned by baseline**, not by top edge — `y` is where the
//! glyph bottoms sit, ignoring descenders. Every layout in `render/` is
//! written against that convention (rows advance by `Metrics::line_height`
//! from one baseline to the next). egui positions a laid-out galley by its
//! top-left, so `text` converts rather than reinterprets; see
//! `baseline_offset`.

use std::sync::Arc;

use bevy_egui::egui;

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

/// Measured extent of a run of text — the *inked* extent, the box the
/// visible pixels actually occupy, not the layout advance.
///
/// That is what the previous backend's `measure_text` reported and what the
/// callers assume: `render/base.rs` centres a glyph in its tile by it, so
/// using the advance width instead would push every map glyph off-centre by
/// its side bearing.
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

/// Font family keys registered with egui by `install_fonts`. egui addresses
/// faces by family name, so these strings are the link between `Face` and
/// the loaded bytes, and must match on both sides.
const FAMILY_UI: &str = "fp-ui";
const FAMILY_UI_BOLD: &str = "fp-ui-bold";
const FAMILY_MAP: &str = "fp-map";

impl Face {
    fn family(self) -> egui::FontFamily {
        let name = match self {
            Self::Ui => FAMILY_UI,
            Self::UiBold => FAMILY_UI_BOLD,
            Self::Map => FAMILY_MAP,
        };
        egui::FontFamily::Name(name.into())
    }

    fn font_id(self, size: u16) -> egui::FontId {
        egui::FontId::new(size as f32, self.family())
    }
}

/// Registers the three embedded faces with the egui context. Runs once at
/// startup; egui owns the rasterized atlas from then on.
///
/// Embedded with `include_bytes!` rather than loaded from `assets_dir` for
/// the same reason the sound effects are (see `sounds.rs`): fonts aren't
/// moddable game content.
///
/// Starts from `FontDefinitions::empty()` rather than `default()` so egui's
/// own bundled faces aren't rasterized into the atlas for nothing — every
/// string this frontend draws names one of the three families below.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    let mut add = |key: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(egui::FontData::from_static(bytes)));
        fonts
            .families
            .insert(egui::FontFamily::Name(key.into()), vec![key.to_owned()]);
    };
    add(
        FAMILY_UI,
        include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf"),
    );
    add(
        FAMILY_UI_BOLD,
        include_bytes!("../../../assets/fonts/DejaVuSansMono-Bold.ttf"),
    );
    add(
        FAMILY_MAP,
        include_bytes!("../../../assets/fonts/unscii-16.ttf"),
    );
    ctx.set_fonts(fonts);
}

/// Everything a screen needs in order to draw itself.
///
/// Threaded through `render/` in the parameter slot that used to carry
/// `&Fonts`; it additionally carries the frame's dimensions and delta, which
/// the drawing code previously read from backend globals.
///
/// Built fresh each frame — an `egui::Painter` is a cheap handle onto the
/// context's shape list, not a resource worth keeping — which is also what
/// makes the dimensions consistent for everything drawn inside one
/// frame. Reading them per-call instead would let a resize land midway
/// through a screen and tear its layout.
pub struct Painter {
    painter: egui::Painter,
    width: f32,
    height: f32,
    delta: f32,
}

impl Painter {
    pub fn for_frame(ctx: &egui::Context, delta: f32) -> Self {
        // The background layer, so the game draws beneath any egui window —
        // there are none today, but a debug overlay shouldn't have to fight
        // the map for z-order. `layer_painter` hands back a full-screen
        // painter, so its clip rect is also the window size; taking both
        // from one place keeps them from disagreeing.
        let painter = ctx.layer_painter(egui::LayerId::background());
        let screen = painter.clip_rect();
        Self {
            painter,
            width: screen.width(),
            height: screen.height(),
            delta,
        }
    }

    pub fn screen_w(&self) -> f32 {
        self.width
    }

    pub fn screen_h(&self) -> f32 {
        self.height
    }

    /// Seconds the previous frame took. Drives the rate-based animations —
    /// the ghost bars drain per second, not per frame, so they look the
    /// same regardless of framerate.
    pub fn delta(&self) -> f32 {
        self.delta
    }

    /// Fills the whole screen. Not a render-pass clear: egui has no such
    /// concept, and this is simply the first shape in the frame's list, so
    /// everything drawn afterwards lands on top of it.
    pub fn clear(&self, color: Color) {
        self.painter
            .rect_filled(self.painter.clip_rect(), 0.0, to_egui(color));
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.painter
            .rect_filled(rect_of(x, y, w, h), 0.0, to_egui(color));
    }

    /// Outline centred on the rect's edge (`StrokeKind::Middle`), which is
    /// where the previous backend put it — an inside or outside stroke would
    /// shift every panel border by half its thickness.
    pub fn rect_lines(&self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: Color) {
        self.painter.rect_stroke(
            rect_of(x, y, w, h),
            0.0,
            egui::Stroke::new(thickness, to_egui(color)),
            egui::StrokeKind::Middle,
        );
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color) {
        self.painter.line_segment(
            [egui::pos2(x1, y1), egui::pos2(x2, y2)],
            egui::Stroke::new(thickness, to_egui(color)),
        );
    }

    /// Draws `text` with its baseline at `y`. See the module docs on why
    /// that, and not the top edge, is the anchor.
    pub fn text(&self, face: Face, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        let color = to_egui(color);
        let galley =
            self.painter
                .layout_no_wrap(text.as_ref().to_owned(), face.font_id(size), color);
        let top = y - baseline_offset(&galley);
        self.painter.galley(egui::pos2(x, top), galley, color);
    }

    pub fn measure(&self, face: Face, text: impl AsRef<str>, size: u16) -> TextDims {
        let galley = self.painter.layout_no_wrap(
            text.as_ref().to_owned(),
            face.font_id(size),
            egui::Color32::WHITE,
        );
        ink_extents(&galley)
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

/// How far below a galley's top edge its first baseline sits.
///
/// `Glyph::pos.y` is the baseline relative to its row, and `PlacedRow::pos.y`
/// is the row relative to the galley, so the two together convert a
/// baseline-anchored `y` into the top-left egui wants. Empty text has no
/// glyph to ask and also nothing to draw, so zero is harmless there.
fn baseline_offset(galley: &egui::Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|placed| placed.row.glyphs.first().map(|g| placed.pos.y + g.pos.y))
        .unwrap_or(0.0)
}

/// The inked extent of a laid-out galley — see `TextDims` on why this is the
/// ink box and not the layout box.
///
/// `mesh_bounds` is `Rect::NOTHING` for text that rasterizes to nothing (an
/// empty string, or a run of spaces), and that sentinel is built from
/// infinities, so it is checked rather than trusted: an infinite width would
/// propagate silently into a panel size.
fn ink_extents(galley: &egui::Galley) -> TextDims {
    let ink = galley.mesh_bounds;
    if ink.is_finite() {
        TextDims {
            width: ink.width(),
            height: ink.height(),
        }
    } else {
        TextDims {
            width: galley.rect.width(),
            height: 0.0,
        }
    }
}

fn rect_of(x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

fn to_egui(c: Color) -> egui::Color32 {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgba_unmultiplied(ch(c.r), ch(c.g), ch(c.b), ch(c.a))
}
