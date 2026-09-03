//! The drawing seam: every primitive the `render` module needs, plus the
//! colour and geometry types it passes around.
//!
//! `render/` is ~3,000 lines of immediate-mode drawing. Rather than have all
//! of it name a graphics library directly — which is what made the frontend
//! macroquad-shaped in the first place — it names `Painter`, and the library
//! lives behind that. Swapping backends is a change to this file rather than
//! to every menu.
//!
//! The surface is deliberately tiny: filled rect, outlined rect, line,
//! filled convex polygon, text in one of three faces, a run of
//! differently-styled text on one baseline, text measurement, and the
//! frame's dimensions and frame delta. That is the whole vocabulary the
//! screens are drawn in.
//!
//! **Text is positioned by baseline**, not by top edge — `y` is where the
//! glyph bottoms sit, ignoring descenders. Every layout in `render/` is
//! written against that convention (rows advance by `Metrics::line_height`
//! from one baseline to the next). egui positions a laid-out galley by its
//! top-left, so `text` converts rather than reinterprets; see
//! `baseline_offset`.

use std::collections::HashMap;
use std::sync::Arc;

use bevy_egui::egui;

/// Straight RGBA, each channel 0.0–1.0, non-premultiplied.
///
/// A local type rather than the backend's own so the palette in
/// `render/mod.rs` and the colour math in `render/base.rs` and `fx.rs`
/// survive a backend swap untouched — those are the parts with real
/// reasoning in them (see `biome_tint` and `structure_condition`).
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

/// One styled piece of a line drawn by `Painter::ui_runs` — a stretch of
/// text with its own weight and colour.
///
/// Weight is a bool rather than a `Face` because the two UI faces are the
/// only pair that can share a baseline: they are the same monospace design at
/// two weights, so runs advance identically whichever one a piece uses.
pub struct TextRun<'a> {
    pub text: &'a str,
    pub bold: bool,
    pub color: Color,
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

/// The sprites the frontend has loaded, by name.
///
/// A `TextureId` is a cheap handle egui resolves at paint time; the pixels
/// belong to Bevy's asset server, which is what lets this be rebuilt — or
/// rather refcounted — every frame alongside the `Painter` that reads it.
/// Empty is a supported state and is what `assets/sprites/` being absent
/// looks like: every lookup misses and every caller draws its glyph.
#[derive(Clone, Default)]
pub struct SpriteTable {
    by_name: HashMap<String, egui::TextureId>,
}

impl SpriteTable {
    pub fn insert(&mut self, name: impl Into<String>, texture: egui::TextureId) {
        self.by_name.insert(name.into(), texture);
    }

    /// Drops the entry `name` holds, if any. The runtime-built player icon
    /// is the one sprite whose texture is replaced while the game is
    /// running; a disk sprite is registered once and never taken back.
    pub(crate) fn remove(&mut self, name: &str) {
        self.by_name.remove(name);
    }

    pub(crate) fn get(&self, name: &str) -> Option<egui::TextureId> {
        self.by_name.get(name).copied()
    }
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
    /// Refcounted rather than borrowed, so `Painter` keeps its freedom from
    /// lifetimes and `render/`'s several hundred `&Painter` signatures stay
    /// as they are. The clone is one atomic bump per frame.
    sprites: Arc<SpriteTable>,
}

impl Painter {
    pub fn for_frame(ctx: &egui::Context, delta: f32, sprites: Arc<SpriteTable>) -> Self {
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
            sprites,
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

    /// Fills a convex polygon given in order around its perimeter.
    ///
    /// The one primitive `rect` cannot stand in for: the side walls of the
    /// first-person Stack corridor recede toward a vanishing point, so
    /// they are trapezoids, not axis-aligned boxes. Convex only — every
    /// shape `render/stack.rs` builds is a quad, and the backend's
    /// triangulation assumes it.
    ///
    /// Fewer than three points draws nothing rather than erroring: a
    /// degenerate wall is a wall of zero width, which is exactly what should
    /// appear.
    pub fn poly(&self, points: &[(f32, f32)], color: Color) {
        if points.len() < 3 {
            return;
        }
        self.painter.add(egui::Shape::convex_polygon(
            points.iter().map(|&(x, y)| egui::pos2(x, y)).collect(),
            to_egui(color),
            egui::Stroke::NONE,
        ));
    }

    /// Runs `f` against a painter that throws away anything outside
    /// `(x, y, w, h)`.
    ///
    /// The one operation about *not* drawing, and it exists because the
    /// first-person Stack corridor's lateral columns are wider than their
    /// pane by construction: the cell beside the party is off the edge of
    /// the view and only its far end swings into frame. Trimming the
    /// projection to fit instead would mean clipping trapezoids by hand and
    /// getting a different, wrong perspective for the trouble.
    ///
    /// `screen_w`/`screen_h` are untouched inside — they mean the window,
    /// not the pane, and every caller that reads them is laying out against
    /// the window.
    pub fn clipped(&self, x: f32, y: f32, w: f32, h: f32, f: impl FnOnce(&Painter)) {
        f(&Painter {
            painter: self.painter.with_clip_rect(rect_of(x, y, w, h)),
            width: self.width,
            height: self.height,
            delta: self.delta,
            // Carried through, or a sprite drawn inside the Stack corridor's
            // clip would silently fall back to its glyph.
            sprites: Arc::clone(&self.sprites),
        });
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

    /// `runs` concatenated into one line with its baseline at `y`, each piece
    /// keeping its own weight and colour.
    ///
    /// Laid out as a single galley rather than drawn piece by piece: the
    /// caller would otherwise have to advance `x` itself, and the only width
    /// this module reports is the *ink* extent (see `TextDims`), which is
    /// narrower than the advance and wrong outright for a run ending in a
    /// space. Letting the layout engine place the pieces sidesteps that.
    pub fn ui_runs(&self, runs: &[TextRun], x: f32, y: f32, size: u16) {
        let galley = self.painter.layout_job(runs_job(runs, size));
        let top = y - baseline_offset(&galley);
        // Each run carries its own colour, so the galley never falls back to
        // this one; egui only reaches for it where a run left `PLACEHOLDER`.
        self.painter
            .galley(egui::pos2(x, top), galley, to_egui(WHITE));
    }

    pub fn map(&self, glyph: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        self.text(Face::Map, glyph, x, y, size, color);
    }

    /// Draws a one-cell sprite into the `size`-square at `(x, y)`, reporting
    /// whether it had one to draw.
    ///
    /// The fifteenth operation, and the first that names a texture. It is a
    /// widening of the seam `CLAUDE.md` protects, taken deliberately: the
    /// alternative is `render/` reaching for egui directly, which is what
    /// the seam exists to prevent.
    ///
    /// Unlike `map`, which takes a *baseline* and is centred by the caller
    /// against measured ink extents, this takes a **top-left** corner and
    /// fills the square exactly. A square sprite has neither side bearing
    /// nor descender, so there is nothing to measure; reading the two as one
    /// convention is a half-cell offset that reads as a camera fault.
    ///
    /// `color` is a **tint**, and an egui tint multiplies — so a white
    /// sprite inherits every existing colour rule for free: the species'
    /// own authored hue, `biome_tint`, the damage dimming. The con read and
    /// the boss and nemesis marks are channels of their own now — a bar and
    /// two corners — so they reach a tile without going through here.
    ///
    /// Returns `false` for a name the table has nothing under, which is what
    /// makes `assets/sprites/` optional: the caller draws its glyph instead,
    /// so a species with no art ships visible rather than blank.
    #[must_use]
    pub fn sprite(&self, name: &str, x: f32, y: f32, size: f32, color: Color) -> bool {
        let Some(texture) = self.sprites.get(name) else {
            return false;
        };
        self.painter.image(
            texture,
            rect_of(x, y, size, size),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            to_egui(color),
        );
        true
    }

    pub fn measure_ui(&self, text: impl AsRef<str>, size: u16) -> TextDims {
        self.measure(Face::Ui, text, size)
    }

    pub fn measure_map(&self, glyph: impl AsRef<str>, size: u16) -> TextDims {
        self.measure(Face::Map, glyph, size)
    }

    /// How far the pen advances across `text` in the UI face — the layout
    /// width, *not* `measure_ui`'s ink box.
    ///
    /// The distinction matters whenever something is placed after a run of
    /// text rather than centred on it. The ink box starts at the first
    /// visible glyph, so leading whitespace contributes nothing to it, and
    /// every `Row::Item` label carries a two-space prefix — measuring the
    /// ink there puts the next thing two characters back on top of the row.
    pub fn measure_ui_advance(&self, text: impl AsRef<str>, size: u16) -> f32 {
        self.painter
            .layout_no_wrap(
                text.as_ref().to_owned(),
                Face::Ui.font_id(size),
                egui::Color32::WHITE,
            )
            .rect
            .width()
    }
}

/// The layout job behind `Painter::ui_runs`. Split out so the arithmetic it
/// produces can be measured in a test without a window.
///
/// Wrapping is switched off explicitly: every caller is drawing one line at a
/// fixed baseline, and a wrapped second row would be drawn over whatever the
/// screen put below it rather than pushing it down.
fn runs_job(runs: &[TextRun], size: u16) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for run in runs {
        let face = if run.bold { Face::UiBold } else { Face::Ui };
        job.append(
            run.text,
            0.0,
            egui::TextFormat {
                font_id: face.font_id(size),
                color: to_egui(run.color),
                ..Default::default()
            },
        );
    }
    job
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

/// Runs `f` against a real `Painter` backed by a headless egui context.
///
/// egui lays text out and records shapes on the CPU, so a draw routine can be
/// exercised end to end without a window or a GPU — which is enough to catch
/// a panic in one, and enough to read back what it painted:
/// `Context::end_pass`'s `FullOutput::shapes` carries one `Shape::Text` per
/// galley egui laid out, and `painted_text` below pulls the string back out
/// of it. Nothing here can assert about pixels, but text content is not
/// pixels — an earlier version of this doc comment claimed the shapes
/// "need a display" to be useful, which was never true and cost this repo a
/// popup content test it could have had; don't repeat that claim.
///
/// Lives at module level rather than inside `mod tests` so `render/`'s own
/// tests can drive a screen through it.
#[cfg(test)]
pub(crate) fn with_painter<R>(
    f: impl FnOnce(&Painter) -> R,
) -> (R, Vec<egui::epaint::ClippedShape>) {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1440.0, 900.0),
        )),
        ..Default::default()
    });
    let out = f(&Painter::for_frame(&ctx, 1.0 / 60.0, Arc::default()));
    let shapes = ctx.end_pass().shapes;
    (out, shapes)
}

/// `with_painter`, with sprites loaded — for the one operation that has a
/// texture table to consult.
#[cfg(test)]
pub(crate) fn with_sprites<R>(
    sprites: SpriteTable,
    f: impl FnOnce(&Painter) -> R,
) -> (R, Vec<egui::epaint::ClippedShape>) {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.begin_pass(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1440.0, 900.0),
        )),
        ..Default::default()
    });
    let out = f(&Painter::for_frame(&ctx, 1.0 / 60.0, Arc::new(sprites)));
    let shapes = ctx.end_pass().shapes;
    (out, shapes)
}

/// Every textured mesh `with_painter` recorded, as `(texture, bounds, tint)`.
///
/// `egui::Shape::image` is a `Mesh` carrying a texture id and a UV'd quad —
/// there is no `Shape::Image` variant — so this reads the mesh back rather
/// than matching one. The untextured meshes egui builds for its own
/// primitives carry `TextureId::default()` and are skipped.
#[cfg(test)]
pub(crate) fn painted_images(
    shapes: &[egui::epaint::ClippedShape],
) -> Vec<(egui::TextureId, egui::Rect, egui::Color32)> {
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => {
                Some((m.texture_id, m.calc_bounds(), m.vertices[0].color))
            }
            _ => None,
        })
        .collect()
}

/// The text of every `Shape::Text` `with_painter` recorded, in paint order —
/// what a test reaches for to check *what* a draw call painted, since
/// `Painter` itself offers no way to ask.
#[cfg(test)]
pub(crate) fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

/// The text of every styled run `with_painter` recorded in exactly this
/// colour and weight, in paint order.
///
/// `painted_text` flattens a galley to one string, which is the right answer
/// for *what* was drawn and no answer at all for a span drawn with its own
/// emphasis inside a line — a popup row's category column, chiefly (see
/// `Row::Item::tag`). This reads the layout job's sections back out instead.
///
/// The comparison is made here rather than in `render/`, for
/// `painted_rect_fill_count`'s reason: quantisation to egui's 8-bit channels
/// happens on the way in, so a caller comparing colours itself would have to
/// round them the same way.
#[cfg(test)]
pub(crate) fn painted_runs_in(
    shapes: &[egui::epaint::ClippedShape],
    color: Color,
    bold: bool,
) -> Vec<String> {
    let want = to_egui(color);
    let face = if bold { Face::UiBold } else { Face::Ui };
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(&t.galley.job),
            _ => None,
        })
        .flat_map(|job| {
            job.sections
                .iter()
                .filter(move |section| {
                    section.format.color == want && section.format.font_id.family == face.family()
                })
                .map(|section| {
                    job.text[section.byte_range.start.0..section.byte_range.end.0].to_string()
                })
        })
        .collect()
}

/// Every run `with_painter` recorded in the **map** face, with the colour it
/// was painted in.
///
/// `painted_runs_in` filters on an exact colour and would find nothing here:
/// the map dims everything it draws by `vignette` and by a per-tile shade, so
/// a glyph's colour is its role's colour multiplied by something just under
/// one. A caller wants the distance, which means it needs the colour back.
#[cfg(test)]
pub(crate) fn painted_map_glyphs(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, Color)> {
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(&t.galley.job),
            _ => None,
        })
        .flat_map(|job| {
            job.sections
                .iter()
                .filter(|section| section.format.font_id.family == Face::Map.family())
                .map(|section| {
                    let c = section.format.color;
                    (
                        job.text[section.byte_range.start.0..section.byte_range.end.0].to_string(),
                        Color::new(
                            c.r() as f32 / 255.0,
                            c.g() as f32 / 255.0,
                            c.b() as f32 / 255.0,
                            c.a() as f32 / 255.0,
                        ),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The width of every `Shape::Rect` `with_painter` recorded. `render/`
/// deliberately never names the graphics library directly (see this file's
/// module doc) — this stays that boundary's one exception, so a test that
/// wants to know how wide a popup drew (e.g. to tell `PopupSize::Large`
/// from `Small`, which otherwise leaves no trace outside `popup.rs`) does
/// not have to reach for `egui::Shape` itself to get it.
/// How many filled rects `with_painter` recorded in exactly `color`.
///
/// The same deliberate exception `painted_rect_widths` is: a test that wants
/// to know whether a particular cue was painted has no way to ask `Painter`,
/// and quantisation to egui's 8-bit channels happens on the way in — so the
/// comparison is made here, against `to_egui(color)`, rather than handing
/// `render/` a colour it would have to round itself.
#[cfg(test)]
pub(crate) fn painted_rect_fill_count(
    shapes: &[egui::epaint::ClippedShape],
    color: Color,
) -> usize {
    let want = to_egui(color);
    shapes
        .iter()
        .filter(|cs| match &cs.shape {
            egui::Shape::Rect(r) => r.fill == want,
            _ => false,
        })
        .count()
}

/// How many line segments `with_painter` recorded. The spark bursts a
/// `VisualEffect` throws are the map's only `Painter::line` work, so a test
/// can ask whether debris was drawn without naming its colour — which fades
/// with the burst and so is not a fixed value to compare against.
#[cfg(test)]
pub(crate) fn painted_line_count(shapes: &[egui::epaint::ClippedShape]) -> usize {
    shapes
        .iter()
        .filter(|cs| matches!(&cs.shape, egui::Shape::LineSegment { .. }))
        .count()
}

/// How many rect *outlines* `with_painter` recorded in exactly `color`.
///
/// `painted_rect_fill_count`'s companion, and not a widening of it: an
/// outline leaves a stroke rather than a fill, so the fill count sees a
/// `Painter::rect_lines` cue not at all.
#[cfg(test)]
pub(crate) fn painted_rect_stroke_count(
    shapes: &[egui::epaint::ClippedShape],
    color: Color,
) -> usize {
    let want = to_egui(color);
    shapes
        .iter()
        .filter(|cs| match &cs.shape {
            egui::Shape::Rect(r) => r.stroke.color == want,
            _ => false,
        })
        .count()
}

#[cfg(test)]
pub(crate) fn painted_rect_widths(shapes: &[egui::epaint::ClippedShape]) -> Vec<f32> {
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) => Some(r.rect.width()),
            _ => None,
        })
        .collect()
}

/// A shape's kind, coarsely, for a test that cares which of two things was
/// painted first rather than what either of them was.
#[cfg(test)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Painted {
    Rect,
    Text,
    Line,
    Other,
}

/// Paint order as a sequence of kinds.
///
/// Exists so a test outside this module can ask an ordering question without
/// naming a graphics library — `render/` is held to that rule and a test in
/// it is still in it.
#[cfg(test)]
pub(crate) fn paint_order(shapes: &[egui::epaint::ClippedShape]) -> Vec<Painted> {
    shapes
        .iter()
        .map(|cs| match &cs.shape {
            egui::Shape::Rect(_) => Painted::Rect,
            egui::Shape::Text(_) => Painted::Text,
            egui::Shape::LineSegment { .. } => Painted::Line,
            _ => Painted::Other,
        })
        .collect()
}

/// Every filled rect's whole box, for a caller asking *where* something was
/// painted rather than how wide it is.
#[cfg(test)]
pub(crate) fn painted_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Rect> {
    shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) => Some(r.rect),
            _ => None,
        })
        .collect()
}

/// Every run of text `with_painter` recorded, as `(paint index, text, inked
/// box in window coordinates)`.
///
/// The box is the **ink** — `TextDims`' rule — and not the galley's layout
/// box, because the question this exists to answer is whether something was
/// painted over glyphs the player can see, and a galley carries leading
/// above and below them that nothing is ever drawn into. Text that
/// rasterizes to nothing (an empty run, a run of spaces) has no box and is
/// skipped rather than reported as an infinite one.
///
/// The index is the position in paint order, so a caller can ask what came
/// *after* a given piece of text — which is the only order in which one
/// shape can cover another.
#[cfg(test)]
pub(crate) fn painted_text_boxes(
    shapes: &[egui::epaint::ClippedShape],
) -> Vec<(usize, String, Rect)> {
    shapes
        .iter()
        .enumerate()
        .filter_map(|(i, cs)| match &cs.shape {
            egui::Shape::Text(t) => {
                let ink = t.galley.mesh_bounds;
                ink.is_finite().then(|| {
                    (
                        i,
                        t.galley.text().to_string(),
                        Rect::new(
                            t.pos.x + ink.min.x,
                            t.pos.y + ink.min.y,
                            ink.width(),
                            ink.height(),
                        ),
                    )
                })
            }
            _ => None,
        })
        .collect()
}

/// Every rect that paints something, as `(paint index, box)`.
///
/// `painted_rects` above reports every `Shape::Rect` and is the right answer
/// for "where is this panel"; this one drops the transparent ones, because
/// `rect_lines` records a rect of the panel's exact geometry with no fill
/// and a caller asking *what covers this* would otherwise be answered by
/// every border on the screen.
#[cfg(test)]
pub(crate) fn painted_fills(shapes: &[egui::epaint::ClippedShape]) -> Vec<(usize, Rect)> {
    shapes
        .iter()
        .enumerate()
        .filter_map(|(i, cs)| match &cs.shape {
            egui::Shape::Rect(r) if r.fill.a() > 0 => Some((
                i,
                Rect::new(r.rect.min.x, r.rect.min.y, r.rect.width(), r.rect.height()),
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lay_out(p: &Painter, face: Face, text: &str, size: u16) -> std::sync::Arc<egui::Galley> {
        p.painter
            .layout_no_wrap(text.to_owned(), face.font_id(size), egui::Color32::WHITE)
    }

    const FACES: [Face; 3] = [Face::Ui, Face::UiBold, Face::Map];

    /// `measure_ui` reports the *ink* box, which starts at the first visible
    /// glyph — so a leading space contributes nothing to it. That is right
    /// for centring a glyph in a map tile, and wrong for placing something
    /// after a run of text: `draw_row` puts a row's suffix at the measured
    /// width past the row's start, and every `Row::Item` label begins with a
    /// two-space prefix. `measure_ui_advance` is the layout width, which
    /// counts that prefix.
    #[test]
    fn advance_width_counts_a_leading_space_where_the_ink_box_cannot() {
        with_painter(|p| {
            let inked = p.measure_ui("  Coolant Flush", 16).width;
            let bare = p.measure_ui("Coolant Flush", 16).width;
            assert!(
                (inked - bare).abs() < 0.5,
                "ink measurement ignores a leading prefix entirely: {inked} vs {bare}"
            );

            let advance = p.measure_ui_advance("  Coolant Flush", 16);
            assert!(
                advance > inked + 1.0,
                "advance width must include the prefix the ink box skips: \
                 advance {advance}, ink {inked}"
            );
        });
    }

    /// A family name that doesn't match what `install_fonts` registered would
    /// leave egui with nothing to draw the string in. Since
    /// `FontDefinitions::empty()` removes the bundled fallbacks too, that
    /// failure is silent — blank text rather than a panic — so it is worth
    /// pinning that every face actually produces glyphs.
    #[test]
    fn every_face_resolves_to_a_registered_font() {
        with_painter(|p| {
            for face in FACES {
                let galley = lay_out(p, face, "Integrity", 24);
                let glyphs: usize = galley.rows.iter().map(|r| r.row.glyphs.len()).sum();
                assert_eq!(glyphs, "Integrity".len(), "{face:?} laid out no glyphs");
            }
        });
    }

    /// The whole reason `text` converts rather than passing `y` through: egui
    /// anchors a galley by its top-left, and every layout in `render/` means
    /// the baseline. A zero offset would mean text drawn a full ascent too
    /// low, which is exactly the regression this catches.
    #[test]
    fn the_baseline_sits_below_the_top_of_the_galley() {
        with_painter(|p| {
            for face in FACES {
                let galley = lay_out(p, face, "Integrity", 24);
                let offset = baseline_offset(&galley);
                assert!(offset > 0.0, "{face:?} put the baseline at the top edge");
                assert!(
                    offset < galley.rect.height(),
                    "{face:?} put the baseline below the whole galley"
                );
            }
        });
    }

    #[test]
    fn the_baseline_offset_grows_with_the_font() {
        with_painter(|p| {
            let small = baseline_offset(&lay_out(p, Face::Ui, "Integrity", 16));
            let large = baseline_offset(&lay_out(p, Face::Ui, "Integrity", 40));
            assert!(
                large > small,
                "a 40px face should sit further below the top than a 16px one ({small} -> {large})"
            );
        });
    }

    /// `mesh_bounds` is `Rect::NOTHING` — built from infinities — for text
    /// that rasterizes to nothing. An infinity here would propagate into a
    /// popup's width and take the layout with it, so the guard in
    /// `ink_extents` matters more than its size suggests.
    #[test]
    fn blank_text_measures_finite() {
        with_painter(|p| {
            for blank in ["", " ", "   "] {
                let dims = p.measure_ui(blank, 24);
                assert!(
                    dims.width.is_finite() && dims.height.is_finite(),
                    "{blank:?} measured {dims:?}, which would poison any layout using it"
                );
                assert!(dims.width >= 0.0 && dims.height >= 0.0);
            }
        });
    }

    #[test]
    fn measurement_grows_with_both_the_text_and_the_font() {
        with_painter(|p| {
            let one = p.measure_ui("M", 24);
            let many = p.measure_ui("MMMM", 24);
            assert!(many.width > one.width, "wider text measured no wider");

            let large = p.measure_ui("M", 40);
            assert!(large.width > one.width, "a bigger font measured no wider");
            assert!(
                large.height > one.height,
                "a bigger font measured no taller"
            );
        });
    }

    /// `render/base.rs` centres a map glyph in its tile by the measured width,
    /// so a measurement that reported the layout advance instead of the ink
    /// would push every glyph off-centre by its side bearing. Ink is narrower
    /// than advance for a monospace face at any normal glyph.
    #[test]
    fn measurement_reports_ink_and_not_the_layout_advance() {
        with_painter(|p| {
            let galley = lay_out(p, Face::Map, "M", 32);
            let ink = ink_extents(&galley);
            assert!(
                ink.width <= galley.rect.width(),
                "ink {} exceeded the advance {}",
                ink.width,
                galley.rect.width()
            );
        });
    }

    /// `ui_runs` places nothing itself — it hands the whole line to the layout
    /// engine — so emphasising a word mid-sentence must not shift the rest of
    /// it sideways. That holds only because the two UI faces are one monospace
    /// design at two weights, which `TextRun` asserts in prose and this pins
    /// in fact.
    #[test]
    fn emphasising_part_of_a_line_does_not_shift_the_rest_of_it() {
        const LINE: &str = "You unleash a data strike for 7 damage.";
        with_painter(|p| {
            let plain = lay_out(p, Face::Ui, LINE, 24);
            let mixed = p.painter.layout_job(runs_job(
                &[
                    TextRun {
                        text: "You unleash a data strike for ",
                        bold: false,
                        color: WHITE,
                    },
                    TextRun {
                        text: "7",
                        bold: true,
                        color: WHITE,
                    },
                    TextRun {
                        text: " damage.",
                        bold: false,
                        color: WHITE,
                    },
                ],
                24,
            ));
            assert_eq!(mixed.rows.len(), 1, "a one-line run job must not wrap");
            assert!(
                (mixed.rect.width() - plain.rect.width()).abs() < 0.5,
                "the mixed-weight line came out {}px against {}px all-regular",
                mixed.rect.width(),
                plain.rect.width()
            );
            let glyphs: usize = mixed.rows.iter().map(|r| r.row.glyphs.len()).sum();
            assert_eq!(glyphs, LINE.chars().count(), "a run laid out no glyphs");
        });
    }

    #[test]
    fn colour_conversion_keeps_the_channels_and_alpha() {
        let c = to_egui(Color::new(1.0, 0.0, 0.5, 0.25));
        assert_eq!(c.to_srgba_unmultiplied(), [255, 0, 128, 64]);
    }

    /// Out-of-range channels would wrap rather than saturate under a bare
    /// `as u8`, turning an over-bright colour black.
    #[test]
    fn colour_conversion_clamps_instead_of_wrapping() {
        assert_eq!(
            to_egui(Color::new(2.0, -1.0, 0.0, 1.0)).to_srgba_unmultiplied(),
            [255, 0, 0, 255]
        );
    }

    /// A name the table has nothing under must leave the caller free to draw
    /// the glyph instead — the whole of how `assets/sprites/` stays optional
    /// and a modded species without art ships visible rather than blank.
    #[test]
    fn an_unknown_sprite_paints_nothing_and_says_so() {
        let (drew, shapes) = with_sprites(SpriteTable::default(), |p| {
            p.sprite("nobody", 10.0, 20.0, 16.0, WHITE)
        });
        assert!(!drew, "an unknown name must report that it drew nothing");
        assert!(
            painted_images(&shapes).is_empty(),
            "an unknown name must paint no image at all"
        );
    }

    /// The sprite fills the square it is given, top-left anchored — unlike
    /// `map`, which takes a baseline and centres by measured ink extents. A
    /// square sprite has neither bearing nor descender, and reading the two
    /// as the same convention is a half-cell offset that looks like a camera
    /// bug rather than a drawing one.
    #[test]
    fn a_sprite_fills_the_square_it_is_given() {
        let id = egui::TextureId::User(7);
        let mut table = SpriteTable::default();
        table.insert("player", id);

        let (drew, shapes) = with_sprites(table, |p| p.sprite("player", 10.0, 20.0, 48.0, WHITE));

        assert!(drew, "a known name must report that it drew");
        let images = painted_images(&shapes);
        assert_eq!(images.len(), 1, "exactly one image per sprite call");
        assert_eq!(images[0].0, id, "the table's texture, not another");
        assert_eq!(
            images[0].1,
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(48.0, 48.0)),
            "a sprite is top-left anchored and square"
        );
    }

    /// The colour is handed over as a tint, so everything that already
    /// colours a glyph — the con read, the boss and nemesis overrides,
    /// `biome_tint`, the damage dimming — keeps working with no second
    /// mechanism. A sprite op that dropped the colour would look correct
    /// against white placeholder art and wrong against every real sprite.
    #[test]
    fn a_sprite_carries_the_colour_it_was_given() {
        let mut table = SpriteTable::default();
        table.insert("player", egui::TextureId::User(1));
        let red = Color::new(1.0, 0.0, 0.0, 1.0);

        let (_, shapes) = with_sprites(table, |p| p.sprite("player", 0.0, 0.0, 16.0, red));

        assert_eq!(
            painted_images(&shapes)[0].2,
            to_egui(red),
            "the sprite must be tinted with the colour the caller passed"
        );
    }
}
