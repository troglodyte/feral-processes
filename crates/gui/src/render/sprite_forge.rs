//! The two dev-only Sprite Forge screens: `Mode::SpritePicker` (every name
//! the map can draw a sprite for, and its art state) and `Mode::SpriteEditor`
//! (`canvas::draw_canvas_grid`, `canvas::draw_swatch_row` and a live preview
//! cell). Both close `render::tests::every_screen_draws_a_refusal_exactly_once`'s
//! two undrawn entries — see `render/mod.rs::ALL_MODES`.
//!
//! **The picker has no scroll**, unlike every menu `draw_popup` sizes: 49
//! subjects (17 species + 30 structures + `player` + `anchor`, as of this
//! writing) is nearly twice `popup::popup_max_rows`'s ceiling at 1280x720,
//! so this screen is a fixed multi-column grid rather than a `draw_popup`
//! body. `PICKER_COLUMNS` and the row height (`Metrics::line_height`) are
//! the layout constraint the brief calls for — held by
//! `the_picker_shows_every_subject_with_no_scroll_at_1280x720` below rather
//! than left to a comment, the `memory-page-has-no-scroll` precedent.
//!
//! **Neither screen draws its own refusal.** Both are full-pane draws with
//! no popup box to put one in, so both join `render::needs_status_banner`'s
//! list — the same door `Mode::FrameMap` and `Mode::FieldRoutineCell`
//! already use for the same reason.
//!
//! **The live preview cell is pixels, not a texture — but it carries the
//! same multiplying tint a texture would.** `Painter::sprite` only draws a
//! name already registered in the frontend's texture table
//! (`crates/gui/src/sprites.rs`, refreshed by a bevy system this crate's
//! `render/` never touches) — an edited-but-unsaved canvas is not in it. So
//! the preview paints the same `CanvasView::cells` the canvas panel does,
//! scaled to `text::map_cell`'s tile size at the app's own `zoom`, with no
//! grid lines and no cursor (the two things that would make it read as "the
//! editor" instead of "the map"), and **each pixel multiplied by the
//! subject's own hue** (`tint_multiply`) exactly as egui multiplies a real
//! sprite's texel by its tint. This is the load-bearing half of the whole
//! feature: the design doc's argument for putting Sprite Forge in the game
//! rather than a standalone tool is that only this route can show a
//! near-white sprite coming out tinted — a preview that drew raw palette
//! colours would show nothing the canvas panel two inches away doesn't
//! already show.

use super::canvas;
use super::*;
use feral_processes_app_core::{
    CanvasFocus, PointerHit, SpriteArt, SpriteEditorView, SpriteSubject,
};
use feral_processes_engine::components::GlyphColor;

// ---------------------------------------------------------------------
// The picker screen
// ---------------------------------------------------------------------

/// Fixed column count for the picker's subject grid — a layout constraint,
/// not a preference: this number and `Metrics::line_height` are what
/// `the_picker_shows_every_subject_with_no_scroll_at_1280x720` holds the
/// full shipped subject list to fitting against, with no scroll.
const PICKER_COLUMNS: usize = 4;

const PICKER_TITLE: &str = "Sprite Forge";
const PICKER_HELP_TEXT: &str = "Up/Down: move   Enter: edit   t: toggle art   Esc: back to menu";

struct PickerGeometry {
    header_y: f32,
    help_y: f32,
    grid_top: f32,
    column_width: f32,
    rows_per_column: usize,
}

/// Everything geometric about the picker, computed once so drawing and the
/// layout census below share one derivation — `icon_editor.rs::geometry`'s
/// pattern for the same reason.
fn picker_geometry(w: f32, m: &Metrics, subject_count: usize) -> PickerGeometry {
    let header_y = m.pad + m.line_height;
    let help_y = header_y + m.gap + m.line_height;
    let grid_top = help_y + m.gap + m.line_height;
    // At least one row even for an empty list, so a modded install with no
    // species or structures at all still gets a grid with nothing in it
    // rather than a divide-by-zero.
    let rows_per_column = subject_count.div_ceil(PICKER_COLUMNS).max(1);
    let column_width = (w - m.pad * 2.0) / PICKER_COLUMNS as f32;
    PickerGeometry {
        header_y,
        help_y,
        grid_top,
        column_width,
        rows_per_column,
    }
}

/// The picker's total drawn height, footer-less (it has none) — the layout
/// census's one number, `icon_editor.rs::content_bottom`'s counterpart.
#[cfg(test)]
fn picker_content_bottom(g: &PickerGeometry, m: &Metrics) -> f32 {
    g.grid_top + g.rows_per_column as f32 * m.line_height + m.pad
}

/// Which column and row `i` lands in — the one derivation drawing and the
/// width census below share, so a row measured is the row drawn.
fn picker_slot(i: usize, rows_per_column: usize) -> (usize, usize) {
    (i / rows_per_column, i % rows_per_column)
}

fn centered_ui(painter: &Painter, text: &str, y: f32, size: u16, color: Color) {
    let width = painter.measure_ui(text, size).width;
    painter.ui(text, (painter.screen_w() - width) / 2.0, y, size, color);
}

pub(super) fn draw_sprite_picker(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let subjects = app.sprite_subjects();
    let g = picker_geometry(painter.screen_w(), m, subjects.len());

    centered_ui(painter, PICKER_TITLE, g.header_y, m.title(), TEXT);
    centered_ui(painter, PICKER_HELP_TEXT, g.help_y, m.small(), TEXT_DIM);

    for (i, subject) in subjects.iter().enumerate() {
        let (col, row) = picker_slot(i, g.rows_per_column);
        let x = m.pad + col as f32 * g.column_width;
        let y = g.grid_top + row as f32 * m.line_height;
        draw_subject_row(painter, x, y, subject, i == selected, m);
    }
}

fn art_label(art: SpriteArt) -> &'static str {
    match art {
        SpriteArt::None => "--",
        SpriteArt::On => "On",
        SpriteArt::Off => "Off",
    }
}

/// A subject's own hue — an authored `GlyphColor` through the map's one
/// palette table (`super::glyph_color`), or the `PLAYER` role colour for the
/// one subject that has none. `SpriteSubject::color`'s own doc comment is
/// why `player` alone is `None`: its `@` wears a role colour, not an
/// authored hue, and `render/base.rs` never reads it through `glyph_color`
/// either. `super::player_look_color(None)` is the existing door to that
/// role colour's fallback — reused rather than naming `hud::palette::PLAYER`
/// a second time.
fn subject_hue(color: Option<GlyphColor>) -> Color {
    match color {
        Some(c) => glyph_color(c),
        None => player_look_color(None),
    }
}

/// One picker row: the highlight caret, the subject's glyph in its own hue
/// (`subject_hue` — never a literal colour authored here, the HUD seam's
/// rule that a subject cannot read as one colour on this list and another
/// wherever else it is drawn), then its name and art state.
fn draw_subject_row(
    painter: &Painter,
    x: f32,
    y: f32,
    subject: &SpriteSubject,
    selected: bool,
    m: &Metrics,
) {
    let prefix = if selected { "> " } else { "  " };
    let tail = format!(" {} ({})", subject.label, art_label(subject.art));
    painter.ui_runs(
        &[
            TextRun {
                text: prefix,
                bold: false,
                color: TEXT,
            },
            TextRun {
                text: &subject.glyph.to_string(),
                bold: false,
                color: subject_hue(subject.color),
            },
            TextRun {
                text: &tail,
                bold: false,
                color: if selected { TEXT } else { TEXT_DIM },
            },
        ],
        x,
        y,
        m.font_size,
    );
}

/// The row text a subject draws, exactly as `draw_subject_row` composes it
/// (the UI face is monospace, so one joined string measures the same as the
/// three runs drawn separately) — what the width census below measures, so
/// a row that fits the test is the row that fits the column.
#[cfg(test)]
fn subject_row_text(subject: &SpriteSubject, selected: bool) -> String {
    let prefix = if selected { "> " } else { "  " };
    format!(
        "{prefix}{} {} ({})",
        subject.glyph,
        subject.label,
        art_label(subject.art)
    )
}

// ---------------------------------------------------------------------
// The editor screen
// ---------------------------------------------------------------------

const EDITOR_HEADER: &str = "Sprite Forge";
const EDITOR_FOOTER_TEXT: &str = "Tab: switch panel   Arrows: move   Space: paint   \
    Backspace: erase   u: undo   x: clear   g: brush size   s: save   Esc: back";

/// `icon_editor.rs::CANVAS_CELL_LINES`, halved: the sprite canvas is 16x16
/// against the icon's 8x8, and this is what keeps the two screens' canvas
/// panel the same physical size on screen — a size already known to fit at
/// the smallest supported window, since it's the icon editor's own.
const SPRITE_CANVAS_CELL_LINES: f32 = 1.2;
const SPRITE_SECTION_GAP_LINES: f32 = 1.0;

struct EditorGeometry {
    header_y: f32,
    canvas_label_y: f32,
    canvas: Rect,
    cell: f32,
    palette_label_y: f32,
    palette: Rect,
    swatch: f32,
    preview_label_y: f32,
    preview: Rect,
    footer_y: f32,
    footer_lines: Vec<String>,
}

/// `edge` and `palette_len` come from the view being drawn rather than a
/// hardcoded 16 and `SPRITE_PALETTE.len()`, so this stays correct if either
/// ever changes without a second place to update.
fn editor_geometry(
    painter: &Painter,
    w: f32,
    m: &Metrics,
    edge: usize,
    palette_len: usize,
    zoom: u16,
) -> EditorGeometry {
    let gap = m.line_height * SPRITE_SECTION_GAP_LINES;

    let header_y = m.pad + m.line_height;

    let canvas_label_y = header_y + gap + m.line_height;
    let cell = m.line_height * SPRITE_CANVAS_CELL_LINES;
    let canvas_side = cell * edge as f32 + m.inset * 2.0;
    let canvas = Rect::new(
        (w - canvas_side) / 2.0,
        canvas_label_y + m.gap,
        canvas_side,
        canvas_side,
    );

    let palette_label_y = canvas.y + canvas.h + gap + m.line_height;
    // Solved for rather than authored as a tuned constant — unlike
    // `icon_editor.rs::SWATCH_LINES`, which shipped 120px past `canvas.w`
    // for a whole task before a test caught it (Task 6's report). Deriving
    // the swatch size from the width it must fit closes that class of bug
    // by construction: whatever `palette_len` is, the strip cannot disagree
    // with `canvas.w` about how wide a swatch is allowed to be.
    // `draw_swatch_row`'s own gap is a third of the swatch
    // (`canvas::SWATCH_GAP_RATIO`), so the strip's total width is
    // `swatch * (n + (n - 1) / 3)`.
    let target_w = canvas.w - m.inset * 2.0;
    let n = palette_len.max(1) as f32;
    let swatch = target_w / (n + (n - 1.0) / 3.0);
    let swatch_gap = swatch / 3.0;
    let palette_w = swatch * n + swatch_gap * (n - 1.0) + m.inset * 2.0;
    let palette_h = swatch + m.inset * 2.0;
    let palette = Rect::new(
        (w - palette_w) / 2.0,
        palette_label_y + m.gap,
        palette_w,
        palette_h,
    );

    let preview_label_y = palette.y + palette.h + gap + m.line_height;
    let (tile_px, _) = map_cell(zoom);
    let preview = Rect::new(
        (w - tile_px) / 2.0,
        preview_label_y + m.gap,
        tile_px,
        tile_px,
    );

    let footer_top = preview.y + preview.h + gap;
    // Measured in UI cells, `icon_editor.rs::geometry`'s pattern for a
    // screen with no popup body to wrap against.
    let columns = ((w - m.pad * 2.0) / painter.measure_ui_advance("M", m.small()))
        .floor()
        .max(20.0) as usize;
    let footer_lines = feral_processes_engine::text::wrap(EDITOR_FOOTER_TEXT, columns);

    EditorGeometry {
        header_y,
        canvas_label_y,
        canvas,
        cell,
        palette_label_y,
        palette,
        swatch,
        preview_label_y,
        preview,
        footer_y: footer_top + m.line_height,
        footer_lines,
    }
}

/// Total height the screen draws, footer included — `icon_editor.rs::
/// content_bottom`'s counterpart.
#[cfg(test)]
fn editor_content_bottom(g: &EditorGeometry, m: &Metrics) -> f32 {
    g.footer_y + g.footer_lines.len().saturating_sub(1) as f32 * m.line_height + m.pad
}

/// `&mut App`, not `&App`: resolving the session's own hue means a second
/// call into `App::sprite_subjects` (see below), which is `&mut self` —
/// Task 4's caching fix-round-1. `SpriteEditor` itself stores only the
/// subject's *name* (`crates/app-core/src/app/sprite_forge.rs`'s own
/// comment on why: that list re-sorts on every read, so an index would go
/// stale), so the name is all this has to look the hue back up with.
pub(super) fn draw_sprite_editor(app: &mut App, painter: &Painter, m: &Metrics) {
    let zoom = app.zoom;
    match app.sprite_editor_view() {
        Some(view) => {
            // A subject not found in its own picker list is unreachable —
            // `Enter` only ever opens a name `sprite_subjects()` just
            // listed — but falling back to a neutral hue rather than
            // panicking keeps a stray future caller from turning a lookup
            // gap into a crash.
            let hue = app
                .sprite_subjects()
                .into_iter()
                .find(|s| s.name == view.subject)
                .map(|s| subject_hue(s.color))
                .unwrap_or(TEXT);
            draw_sprite_editor_session(&view, zoom, hue, painter, m);
        }
        // Unreachable through the picker's own `Enter` (it always opens a
        // session before switching mode) but not through a test, or a
        // future caller that sets `Mode::SpriteEditor` directly — a blank
        // window would be a soft lock the player cannot read their way out
        // of, so this still names the way back rather than drawing nothing.
        None => centered_ui(
            painter,
            "No sprite selected. Esc to go back.",
            m.pad + m.line_height,
            m.title(),
            TEXT_DIM,
        ),
    }
}

fn draw_sprite_editor_session(
    view: &SpriteEditorView,
    zoom: u16,
    hue: Color,
    painter: &Painter,
    m: &Metrics,
) {
    let edge = view.canvas.edge as usize;
    let g = editor_geometry(
        painter,
        painter.screen_w(),
        m,
        edge,
        view.palette.len(),
        zoom,
    );

    centered_ui(
        painter,
        &format!("{EDITOR_HEADER} — {}", view.subject),
        g.header_y,
        m.title(),
        TEXT,
    );

    centered_ui(painter, "Canvas", g.canvas_label_y, m.small(), TEXT_DIM);
    let c = g.canvas;
    painter.rect(c.x, c.y, c.w, c.h, PANEL_BG);
    let (thickness, color) = panel_border(view.canvas.focus == CanvasFocus::Canvas);
    painter.rect_lines(c.x, c.y, c.w, c.h, thickness, color);
    let side = edge as f32 * g.cell;
    let inner = Rect::new(c.x + m.inset, c.y + m.inset, side, side);
    canvas::draw_canvas_grid(painter, inner, &view.canvas, view.palette);

    centered_ui(painter, "Palette", g.palette_label_y, m.small(), TEXT_DIM);
    let p = g.palette;
    painter.rect(p.x, p.y, p.w, p.h, PANEL_BG);
    let (thickness, color) = panel_border(view.canvas.focus == CanvasFocus::Palette);
    painter.rect_lines(p.x, p.y, p.w, p.h, thickness, color);
    let palette_inner = Rect::new(p.x + m.inset, p.y + m.inset, p.w - m.inset * 2.0, g.swatch);
    canvas::draw_swatch_row(painter, palette_inner, view.canvas.selected, view.palette);

    centered_ui(painter, "Preview", g.preview_label_y, m.small(), TEXT_DIM);
    draw_preview_cell(painter, g.preview, view, hue);

    let mut y = g.footer_y;
    for line in &g.footer_lines {
        centered_ui(painter, line, y, m.small(), TEXT_DIM);
        y += m.line_height;
    }
}

/// A panel's border thickness and colour for whether it currently has
/// focus — `icon_editor.rs::panel_border`'s own rule, not shared as a
/// function across the two files since each owns its screen's chrome.
fn panel_border(focused: bool) -> (f32, Color) {
    if focused {
        (3.0, BORDER)
    } else {
        (1.0, TEXT_DIM)
    }
}

/// Multiplies `t` into `c`, channel-wise — egui's own sprite tint, done by
/// hand: the preview paints raw pixels rather than a texture (see the
/// module doc comment for why `Painter::sprite` isn't reachable here), so
/// nothing else applies it. Both `Color`s are already normalised to
/// `0.0..=1.0`, so this is `Painter::sprite`'s doc comment's "an egui tint
/// multiplies" at the same magnitude egui's own byte-domain `texel * tint /
/// 255` reaches — the division is already folded into both operands having
/// been divided by 255 once, at load.
fn tint_multiply(c: Color, t: Color) -> Color {
    Color::new(c.r * t.r, c.g * t.g, c.b * t.b, c.a)
}

/// The canvas, painted at `rect`'s own size rather than the editor's own
/// magnified cell — no grid lines, no cursor, so what's left is what a tile
/// actually shows once this is saved and drawn on the map. `hue` is the
/// same multiplying tint `Painter::sprite` would apply to a texture — this
/// is the one place the argument for putting Sprite Forge in the game
/// rather than a standalone tool actually shows up on screen: a near-white
/// canvas comes out hued, and a saturated one goes muddy, right here.
fn draw_preview_cell(painter: &Painter, rect: Rect, view: &SpriteEditorView, hue: Color) {
    painter.rect(rect.x, rect.y, rect.w, rect.h, PANEL_BG);
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);

    let edge = view.canvas.edge as usize;
    let cell = rect.w / edge as f32;
    for y in 0..edge {
        for x in 0..edge {
            let idx = view.canvas.cells[y * edge + x];
            let color = match idx {
                // A transparent cell stays transparent — there is no pixel
                // for the tint to multiply into.
                0 => SCREEN_BG,
                n => tint_multiply(canvas::palette_color(view.palette[n as usize - 1]), hue),
            };
            painter.rect(
                rect.x + x as f32 * cell,
                rect.y + y as f32 * cell,
                cell,
                cell,
                color,
            );
        }
    }
}

// ---------------------------------------------------------------------
// The mouse
// ---------------------------------------------------------------------
//
// `crates/gui` has no mouse handling anywhere else — this is the first, and
// it stays confined to this screen. Everything below is pure geometry: no
// egui, no bevy, nothing that needs a window. `lib.rs::handle_sprite_pointer`
// is the one caller, and it owns the only bevy/egui-dependent half (reading
// the pointer, and the frame-to-frame Down/Drag/Up bookkeeping) — kept out
// of this file so `cell_at`/`swatch_at`/`HitRects::resolve` stay testable
// with nothing but a `Rect`.

/// Resolves a pointer position to a canvas cell — pure in `(pos, rect,
/// edge)`, so this needs no window to test. `rect` is the exact grid rect
/// `draw_canvas_grid` fills (the canvas panel inset by `m.inset`, not the
/// panel itself). Both bounds are inclusive, so the rect's own four corners
/// resolve to their own four corner cells — `min(edge - 1)` is what makes
/// the far corner (exactly `rect.x + rect.w`) land on the last column
/// instead of reading one cell short of "inside."
pub(crate) fn cell_at(pos: (f32, f32), rect: Rect, edge: u8) -> Option<(u8, u8)> {
    if pos.0 < rect.x || pos.0 > rect.x + rect.w || pos.1 < rect.y || pos.1 > rect.y + rect.h {
        return None;
    }
    let cell_w = rect.w / edge as f32;
    let cell_h = rect.h / edge as f32;
    let x = (((pos.0 - rect.x) / cell_w) as u8).min(edge - 1);
    let y = (((pos.1 - rect.y) / cell_h) as u8).min(edge - 1);
    Some((x, y))
}

/// Resolves a pointer position to a swatch index — `draw_swatch_row`'s own
/// 0-based loop index (`PointerHit::Swatch`'s convention, not
/// `CanvasView::selected`'s 1-based one). `rect` is the exact strip
/// `draw_swatch_row` fills, and `canvas::SWATCH_GAP_RATIO` is the same gap
/// it draws with — a pointer landing in that gap between two swatches
/// resolves to no hit rather than snapping to whichever is nearer, so an
/// accidental miss between two swatches stays a miss.
pub(crate) fn swatch_at(pos: (f32, f32), rect: Rect, count: u8) -> Option<u8> {
    if count == 0 || pos.0 < rect.x || pos.1 < rect.y || pos.1 > rect.y + rect.h {
        return None;
    }
    let swatch = rect.h;
    let stride = swatch * (1.0 + canvas::SWATCH_GAP_RATIO);
    let offset = pos.0 - rect.x;
    let i = (offset / stride) as u8;
    if i >= count {
        return None;
    }
    if offset - i as f32 * stride > swatch {
        return None;
    }
    Some(i)
}

/// The sprite editor's own two hit-test rects, recomputed from the exact
/// `editor_geometry` `draw_sprite_editor_session` draws from — a pointer
/// resolved through `resolve` can never disagree with what's on screen.
/// Fields stay private: `lib.rs` never reads one directly, only calls
/// `resolve`.
pub(crate) struct HitRects {
    canvas: Rect,
    edge: u8,
    palette: Rect,
    palette_len: u8,
}

impl HitRects {
    /// The canvas rect first, the palette rect second — the two panels
    /// never overlap on screen, so trying both in this order and returning
    /// the first hit is exactly "which panel was the pointer over."
    pub(crate) fn resolve(&self, pos: (f32, f32)) -> Option<PointerHit> {
        if let Some((x, y)) = cell_at(pos, self.canvas, self.edge) {
            return Some(PointerHit::Cell(x, y));
        }
        swatch_at(pos, self.palette, self.palette_len).map(PointerHit::Swatch)
    }
}

/// `render::sprite_editor_hit_rects` is the one caller — `app`'s zoom and
/// `view`'s own edge/palette length feed the same `editor_geometry` the draw
/// call built its rects from, then re-derives `inner`/`palette_inner`
/// exactly as `draw_sprite_editor_session` does.
pub(crate) fn hit_rects(
    painter: &Painter,
    w: f32,
    m: &Metrics,
    view: &SpriteEditorView,
    zoom: u16,
) -> HitRects {
    let edge = view.canvas.edge as usize;
    let g = editor_geometry(painter, w, m, edge, view.palette.len(), zoom);
    let side = edge as f32 * g.cell;
    let canvas = Rect::new(g.canvas.x + m.inset, g.canvas.y + m.inset, side, side);
    let palette = Rect::new(
        g.palette.x + m.inset,
        g.palette.y + m.inset,
        g.palette.w - m.inset * 2.0,
        g.swatch,
    );
    HitRects {
        canvas,
        edge: view.canvas.edge,
        palette,
        palette_len: view.palette.len() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::{GameKey, Mode, PointerButton, PointerPhase};
    use feral_processes_engine::icon::SPRITE_PALETTE;

    const CENSUS_W: f32 = 1280.0;
    const CENSUS_H: f32 = 720.0;

    /// A fresh app with real assets loaded — Sprite Forge reads
    /// `assets/species`/`assets/structures` straight off disk
    /// (`App::sprite_subjects`), so its tests need the real tree rather
    /// than a hand-built fixture, `creation.rs::wizard_app`'s reason.
    fn sprite_forge_app() -> feral_processes_app_core::App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_sprite_forge_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        feral_processes_app_core::App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        )
    }

    /// Opens `Mode::SpriteEditor` on the picker's `index`'th subject through
    /// the real `Enter` key, mirroring `sprite_forge.rs`'s own
    /// `open_editor` test helper in app-core (`crates/app-core/src/tests/
    /// sprite_forge.rs`) rather than reaching into private `App` state this
    /// crate cannot see.
    fn open_editor(app: &mut feral_processes_app_core::App, index: usize) {
        app.mode = Mode::SpritePicker;
        app.menu_selected = index;
        app.handle_key(GameKey::Enter);
        assert_eq!(app.mode, Mode::SpriteEditor, "Enter must open the editor");
    }

    // -------------------------------------------------------------
    // The picker
    // -------------------------------------------------------------

    /// **The layout census this task exists for.** Every one of the real,
    /// shipped subjects must fit inside 1280x720 with no scroll, since this
    /// screen has none — the `memory-page-has-no-scroll` precedent.
    ///
    /// 49 is Task 4's own pinned count
    /// (`sprite_subjects_is_every_species_and_structure_plus_player_and_anchor`);
    /// asserted again here so a shrinking asset tree can't silently make
    /// this census easier than the one it is meant to hold.
    #[test]
    fn the_picker_shows_every_subject_with_no_scroll_at_1280x720() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        assert_eq!(
            subjects.len(),
            49,
            "the shipped subject count moved — re-check this census's premise"
        );

        let m = crate::text::ui_metrics(CENSUS_H);
        let g = picker_geometry(CENSUS_W, &m, subjects.len());
        let bottom = picker_content_bottom(&g, &m);
        let rows = g.rows_per_column;
        assert!(
            bottom < CENSUS_H,
            "the picker draws {bottom}px of content ({rows} rows per column, \
             {PICKER_COLUMNS} columns) in a {CENSUS_H}px window — this screen has no scroll"
        );
    }

    /// Every subject's row fits inside its own column — the axis the height
    /// census above cannot see. Measured against the real, shipped subject
    /// list rather than a synthetic longest-name fixture, so a modded
    /// species with an unusually long name is exactly what this would catch.
    #[test]
    fn every_subject_row_fits_its_column_at_1280_wide() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let m = crate::text::ui_metrics(CENSUS_H);
        crate::paint::with_painter(|p| {
            let g = picker_geometry(CENSUS_W, &m, subjects.len());
            for (i, subject) in subjects.iter().enumerate() {
                let (col, _) = picker_slot(i, g.rows_per_column);
                let x = m.pad + col as f32 * g.column_width;
                let text = subject_row_text(subject, false);
                let width = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    x + width <= CENSUS_W,
                    "{:?}'s row is {width}px wide starting at {x}px — it runs \
                     off a {CENSUS_W}px window: {text:?}",
                    subject.name
                );
            }
        });
    }

    /// Verified by mutation: widening `PICKER_COLUMNS` from 4 to 1 turns the
    /// height census red (49 rows in one column at 20px each is 980px,
    /// against a 720px window) — confirming the test actually exercises the
    /// column count rather than passing vacuously. Reverted before this file
    /// was committed; see the task's own report for the transcript.
    #[test]
    fn picker_slot_packs_columns_before_wrapping_rows() {
        // `i / rows_per_column` is the column, `i % rows_per_column` the
        // row — so index `rows_per_column` starts a fresh column at row 0,
        // not a second row in the first column.
        assert_eq!(picker_slot(0, 13), (0, 0));
        assert_eq!(picker_slot(12, 13), (0, 12));
        assert_eq!(picker_slot(13, 13), (1, 0));
    }

    #[test]
    fn the_picker_names_the_hardcoded_subjects() {
        let mut app = sprite_forge_app();
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_picker(&mut app, p, &m));
        let drawn = crate::paint::painted_text(&shapes).join(" ");
        for want in ["Player", "Anchor", PICKER_TITLE] {
            assert!(
                drawn.contains(want),
                "the picker must draw {want:?}: {drawn:?}"
            );
        }
    }

    /// The glyph is read from `super::glyph_color` (`hud::palette::glyph`),
    /// never a second colour table authored in this file — the HUD seam's
    /// rule.
    #[test]
    fn the_glyph_is_drawn_through_the_maps_own_palette_table() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let cipher = subjects
            .iter()
            .find(|s| s.name == "cipher")
            .expect("cipher ships in assets/species");
        assert_eq!(
            cipher.color,
            Some(GlyphColor::Cyan),
            "the fixture this test reads its expected colour off"
        );

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_picker(&mut app, p, &m));
        let want = glyph_color(GlyphColor::Cyan);
        let painted = crate::paint::painted_runs_in(&shapes, want, false);
        assert!(
            painted.iter().any(|t| t == &cipher.glyph.to_string()),
            "cipher's glyph {:?} was not painted in its own hue {want:?}: {painted:?}",
            cipher.glyph
        );
    }

    /// The player subject wears the `PLAYER` role colour, not an authored
    /// `GlyphColor` — `SpriteSubject::color`'s own doc comment, and the same
    /// rule `render/base.rs` follows for the real map tile.
    #[test]
    fn the_player_row_wears_the_player_role_colour_not_an_authored_hue() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let player = subjects
            .iter()
            .find(|s| s.name == "player")
            .expect("player is one of the two hardcoded subjects");
        assert_eq!(player.color, None);

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_picker(&mut app, p, &m));
        let want = player_look_color(None);
        let painted = crate::paint::painted_runs_in(&shapes, want, false);
        assert!(
            painted.iter().any(|t| t == &player.glyph.to_string()),
            "the player's glyph {:?} was not painted in the PLAYER role colour {want:?}: {painted:?}",
            player.glyph
        );
    }

    // -------------------------------------------------------------
    // The editor
    // -------------------------------------------------------------

    /// **The other layout census this task adds.** The canvas, palette and
    /// preview cell together must fit inside 1280x720 with no scroll.
    #[test]
    fn the_editor_screen_fits_at_1280x720() {
        let m = crate::text::ui_metrics(CENSUS_H);
        let ((bottom, lines), _shapes) = crate::paint::with_painter(|p| {
            let g = editor_geometry(p, CENSUS_W, &m, 16, SPRITE_PALETTE.len(), 2);
            (editor_content_bottom(&g, &m), g.footer_lines.len())
        });
        assert!(
            bottom < CENSUS_H,
            "the sprite editor draws {bottom}px of content ({lines} footer lines) \
             in a {CENSUS_H}px window — this screen has no scroll"
        );
    }

    /// Every block sits inside the window horizontally too.
    #[test]
    fn the_editor_screen_fits_1280_wide() {
        let m = crate::text::ui_metrics(CENSUS_H);
        crate::paint::with_painter(|p| {
            let g = editor_geometry(p, CENSUS_W, &m, 16, SPRITE_PALETTE.len(), 2);
            for (name, rect) in [
                ("canvas", g.canvas),
                ("palette", g.palette),
                ("preview", g.preview),
            ] {
                assert!(
                    rect.x >= 0.0 && rect.x + rect.w <= CENSUS_W,
                    "{name} panel runs off a {CENSUS_W}px window: {rect:?}"
                );
            }
            for line in &g.footer_lines {
                let width = p.measure_ui_advance(line, m.small());
                assert!(
                    width <= CENSUS_W,
                    "a footer line is {width}px wide in a {CENSUS_W}px window: {line:?}"
                );
            }
        });
    }

    /// **The palette sits under the canvas without growing past it.**
    /// Derived rather than tuned (see `editor_geometry`'s own comment), but
    /// still worth pinning: a future edit to the derivation that broke this
    /// would otherwise only show up as an overhanging strip on screen.
    #[test]
    fn the_palette_strip_fits_under_the_canvas() {
        let m = crate::text::ui_metrics(CENSUS_H);
        crate::paint::with_painter(|p| {
            let g = editor_geometry(p, CENSUS_W, &m, 16, SPRITE_PALETTE.len(), 2);
            assert!(
                g.palette.w <= g.canvas.w + 0.01,
                "the palette is {}px wide under a {}px canvas",
                g.palette.w,
                g.canvas.w
            );
        });
    }

    /// The preview cell is sized by `text::map_cell`, at the app's own
    /// zoom — not a fixed literal this file could disagree with the map
    /// about.
    #[test]
    fn the_preview_cell_is_sized_by_map_zoom() {
        let m = crate::text::ui_metrics(CENSUS_H);
        crate::paint::with_painter(|p| {
            for zoom in feral_processes_app_core::MIN_ZOOM..=feral_processes_app_core::MAX_ZOOM {
                let g = editor_geometry(p, CENSUS_W, &m, 16, SPRITE_PALETTE.len(), zoom);
                let (tile_px, _) = crate::text::map_cell(zoom);
                assert_eq!(
                    g.preview.w, tile_px,
                    "zoom {zoom}'s preview cell is the wrong size"
                );
                assert_eq!(g.preview.h, tile_px);
            }
        });
    }

    /// A cell painted in the editor shows up on the canvas grid and its
    /// swatch (always drawn, whether or not it was used) in the swatch's own
    /// raw colour — the preview alone tints, and that is covered on its own
    /// below (`a_near_white_pixel_previews_tinted_by_the_subjects_hue`),
    /// since folding both claims into one assertion would hide a tint bug
    /// behind whichever count happened to still be right.
    ///
    /// **Pinned to `cipher` (M4, final review)**, not "whichever subject
    /// sorts first": the preview cell is always drawn too, tinted by the
    /// subject's own hue (`tint_multiply`), and a hue of identity-white
    /// would make the tinted preview pixel land on the same raw colour as
    /// the swatch and the grid cell, silently inflating the count to 3 — the
    /// earlier fixture (index 0, the alphabetically-first subject) only
    /// passed because that subject's colour happened not to be `White`. A
    /// species named ahead of it with `color: White` would have made this
    /// pass or fail depending on asset content nobody here was testing.
    /// `cipher` is `GlyphColor::Cyan`, asserted below — nowhere near the
    /// identity-tint hazard `SPRITE_PALETTE[8]`'s own near-white test exists
    /// to cover.
    #[test]
    fn a_painted_cell_shows_on_the_canvas_and_the_palette_in_its_raw_colour() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let index = subjects
            .iter()
            .position(|s| s.name == "cipher")
            .expect("cipher ships in assets/species");
        assert_eq!(
            subjects[index].color,
            Some(GlyphColor::Cyan),
            "the fixture this test's premise rests on"
        );
        open_editor(&mut app, index);
        // Space paints the cursor's cell (0,0) with the opening swatch,
        // index 1 (`canvas_editor::FIRST_COLOUR`) — `SPRITE_PALETTE[0]`.
        app.handle_key(GameKey::Char(' '));

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&mut app, p, &m));
        let want = canvas::palette_color(SPRITE_PALETTE[0]);
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, want),
            2,
            "expected the swatch and the painted grid cell, both untinted"
        );
    }

    /// **The tint is the whole point of the preview existing.** A near-white
    /// pixel (`SPRITE_PALETTE[8]`, pure white — `SPRITE_PALETTE`'s own doc
    /// comment: art is authored near white *so that* it inherits the tint)
    /// painted while editing a strongly-hued subject (`cipher`, `GlyphColor::
    /// Cyan`) must come out in that hue on the preview and nowhere else —
    /// this is the assertion that fails if someone later "simplifies" the
    /// tint away, since the untinted alternative (the pre-fix code) would
    /// have drawn white here instead.
    #[test]
    fn a_near_white_pixel_previews_tinted_by_the_subjects_hue() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let index = subjects
            .iter()
            .position(|s| s.name == "cipher")
            .expect("cipher ships in assets/species");
        open_editor(&mut app, index);

        // Focus the palette and step to swatch 9 (1-based) — `SPRITE_
        // PALETTE[8]`, pure white — then paint the cursor's cell (0,0) with
        // it. Space paints with `selected` regardless of which panel has
        // focus, so no `Tab` back is needed.
        app.handle_key(GameKey::Tab);
        for _ in 0..8 {
            app.handle_key(GameKey::Right);
        }
        app.handle_key(GameKey::Char(' '));

        let raw_white = canvas::palette_color(SPRITE_PALETTE[8]);
        let want_hue = glyph_color(GlyphColor::Cyan);
        assert_ne!(
            want_hue, raw_white,
            "the fixture must be near-white tinted by a real hue, or this test proves nothing"
        );

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&mut app, p, &m));
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, raw_white),
            2,
            "the swatch and the grid cell stay raw white — only the preview tints"
        );
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, want_hue),
            1,
            "the near-white preview pixel must come out in the subject's own hue"
        );
    }

    /// A subject with no session open (unreachable through the real picker,
    /// but reachable if a caller sets the mode directly) draws something
    /// rather than nothing — a blank window would be a soft lock.
    #[test]
    fn no_session_draws_a_fallback_instead_of_a_blank_window() {
        let mut app = sprite_forge_app();
        assert!(app.sprite_editor_view().is_none());
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&mut app, p, &m));
        assert!(
            !crate::paint::painted_text(&shapes).is_empty(),
            "an unopened editor must still draw something, not a blank window"
        );
    }

    /// The footer names every one of `CanvasEditor::handle_key`'s shared
    /// keys plus this screen's own two (`g`, `s`).
    #[test]
    fn the_footer_names_every_bound_key() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&mut app, p, &m));
        let drawn = crate::paint::painted_text(&shapes).join(" ");
        for key in [
            "Tab",
            "Arrows",
            "Space",
            "Backspace",
            "u",
            "x",
            "g",
            "s",
            "Esc",
        ] {
            assert!(
                drawn.contains(key),
                "the footer must name {key:?}: {drawn:?}"
            );
        }
    }

    /// The header names the subject being edited.
    #[test]
    fn the_header_names_the_subject() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let subject = app.sprite_subjects()[0].clone();
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&mut app, p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            drawn
                .iter()
                .any(|t| t.contains(&subject.label) || t.contains(&subject.name)),
            "the header must name the subject: {drawn:?}"
        );
    }

    // -------------------------------------------------------------
    // The mouse: `cell_at` and `swatch_at`, pure functions of a position, a
    // rect and an edge/count — no window, no egui, no `App`.
    // -------------------------------------------------------------

    /// **The failing test this task starts from.** A 16x16 grid in a
    /// 160x160 rect (10px cells): each of the rect's own four corners must
    /// resolve to its own corner cell, and a point in the middle of an
    /// interior cell must resolve to that cell.
    #[test]
    fn cell_at_resolves_the_four_corners_and_one_interior_point() {
        let rect = Rect::new(100.0, 50.0, 160.0, 160.0);
        let edge = 16;
        assert_eq!(
            cell_at((100.0, 50.0), rect, edge),
            Some((0, 0)),
            "top-left corner"
        );
        assert_eq!(
            cell_at((260.0, 50.0), rect, edge),
            Some((15, 0)),
            "top-right corner"
        );
        assert_eq!(
            cell_at((100.0, 210.0), rect, edge),
            Some((0, 15)),
            "bottom-left corner"
        );
        assert_eq!(
            cell_at((260.0, 210.0), rect, edge),
            Some((15, 15)),
            "bottom-right corner"
        );
        // The middle of cell (8, 8): rect.x + 8.5 cells, rect.y + 8.5 cells.
        assert_eq!(
            cell_at((185.0, 135.0), rect, edge),
            Some((8, 8)),
            "an interior point"
        );
    }

    /// A position outside the rect on any of the four sides resolves to no
    /// hit at all.
    #[test]
    fn cell_at_returns_none_outside_the_rect() {
        let rect = Rect::new(100.0, 50.0, 160.0, 160.0);
        let edge = 16;
        assert_eq!(cell_at((99.9, 50.0), rect, edge), None, "left of the rect");
        assert_eq!(
            cell_at((260.1, 50.0), rect, edge),
            None,
            "right of the rect"
        );
        assert_eq!(cell_at((100.0, 49.9), rect, edge), None, "above the rect");
        assert_eq!(cell_at((100.0, 210.1), rect, edge), None, "below the rect");
    }

    /// A swatch hit lands on the swatch it is over, and misses the gap
    /// between two swatches rather than snapping to the nearer one.
    #[test]
    fn swatch_at_resolves_a_hit_and_a_gap_and_outside() {
        // Two swatches, 30px side, 10px gap (`SWATCH_GAP_RATIO` = 1/3).
        let rect = Rect::new(0.0, 0.0, 70.0, 30.0);
        assert_eq!(
            swatch_at((15.0, 15.0), rect, 2),
            Some(0),
            "inside the first swatch"
        );
        assert_eq!(
            swatch_at((35.0, 15.0), rect, 2),
            None,
            "in the gap between the two swatches"
        );
        assert_eq!(
            swatch_at((55.0, 15.0), rect, 2),
            Some(1),
            "inside the second swatch"
        );
        assert_eq!(swatch_at((-1.0, 15.0), rect, 2), None, "left of the strip");
        assert_eq!(swatch_at((15.0, 31.0), rect, 2), None, "below the strip");
    }

    /// `HitRects::resolve` tries the canvas rect first and the palette rect
    /// second, and misses entirely outside both — the composition
    /// `render::sprite_editor_hit_rects` hands `lib.rs` each frame.
    #[test]
    fn hit_rects_resolve_tries_canvas_then_palette_then_neither() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let view = app.sprite_editor_view().expect("just opened");
        let m = crate::text::ui_metrics(900.0);
        let rects = crate::paint::with_painter(|p| hit_rects(p, 1280.0, &m, &view, app.zoom)).0;

        let canvas_hit = rects.resolve((rects.canvas.x, rects.canvas.y));
        assert_eq!(canvas_hit, Some(PointerHit::Cell(0, 0)));

        let swatch_hit = rects.resolve((rects.palette.x, rects.palette.y));
        assert_eq!(swatch_hit, Some(PointerHit::Swatch(0)));

        assert_eq!(rects.resolve((-1.0, -1.0)), None, "outside both panels");
    }

    /// **The seam neither side's own test spanned.** `swatch_at` answers in
    /// drawn positions, `pick_swatch` writes `CanvasView::selected`, and
    /// `draw_swatch_row` reads that back to outline exactly one swatch — so
    /// the only honest question is whether the outline lands on the swatch
    /// the pointer was actually over. Asked of the first, a middle and the
    /// **last** entry of `SPRITE_PALETTE`; the last is the one an off-by-one
    /// in either direction cannot reach at all, and the first is the one it
    /// gets right by accident (`pick_swatch`'s clamp floors it).
    #[test]
    fn a_click_outlines_the_swatch_under_the_pointer() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let view = app.sprite_editor_view().expect("just opened");
        let m = crate::text::ui_metrics(900.0);
        let rects = crate::paint::with_painter(|p| hit_rects(p, CENSUS_W, &m, &view, app.zoom)).0;
        let palette = rects.palette;
        let swatch = palette.h;
        let stride = swatch * (1.0 + canvas::SWATCH_GAP_RATIO);

        for i in [0usize, SPRITE_PALETTE.len() / 2, SPRITE_PALETTE.len() - 1] {
            let left = palette.x + i as f32 * stride;
            let pos = (left + swatch * 0.5, palette.y + swatch * 0.5);
            let hit = rects.resolve(pos).expect("inside a swatch");
            app.handle_pointer(hit, PointerButton::Primary, PointerPhase::Down);
            app.handle_pointer(hit, PointerButton::Primary, PointerPhase::Up);

            let selected = app
                .sprite_editor_view()
                .expect("still open")
                .canvas
                .selected;
            let (_, shapes) = crate::paint::with_painter(|p| {
                canvas::draw_swatch_row(p, palette, selected, &SPRITE_PALETTE)
            });
            let outlined =
                crate::paint::painted_rect_stroke_boxes(&shapes, canvas::SELECTED_SWATCH_COLOR);
            assert_eq!(outlined.len(), 1, "exactly one swatch is outlined");
            assert!(
                (outlined[0].min.x - left).abs() < 0.5,
                "clicking swatch {i} (drawn at x={left}) must outline that swatch, \
                 not the one at x={}",
                outlined[0].min.x
            );
        }
    }
}
