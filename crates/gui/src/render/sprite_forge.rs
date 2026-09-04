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
//! **The live preview cell is pixels, not a texture.** `Painter::sprite`
//! only draws a name already registered in the frontend's texture table
//! (`crates/gui/src/sprites.rs`, refreshed by a bevy system this crate's
//! `render/` never touches) — an edited-but-unsaved canvas is not in it. So
//! the preview paints the same `CanvasView::cells` the canvas panel does,
//! scaled to `text::map_cell`'s tile size at the app's own `zoom`, with no
//! grid lines and no cursor: the two things that would make it read as "the
//! editor" instead of "the map."

use super::canvas;
use super::*;
use feral_processes_app_core::{CanvasFocus, SpriteArt, SpriteEditorView, SpriteSubject};
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

/// One picker row: the highlight caret, the subject's glyph in the map's
/// one palette table (`super::glyph_color` — never a literal colour authored
/// here, the HUD seam's rule that a subject cannot read as one colour on
/// this list and another wherever else it is drawn), then its name and art
/// state.
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
                color: glyph_color(GlyphColor::White),
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

pub(super) fn draw_sprite_editor(app: &App, painter: &Painter, m: &Metrics) {
    match app.sprite_editor_view() {
        Some(view) => draw_sprite_editor_session(&view, app.zoom, painter, m),
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

fn draw_sprite_editor_session(view: &SpriteEditorView, zoom: u16, painter: &Painter, m: &Metrics) {
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
    draw_preview_cell(painter, g.preview, view);

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

/// The canvas, painted at `rect`'s own size rather than the editor's own
/// magnified cell — no grid lines, no cursor, so what's left is what a tile
/// actually shows once this is saved and drawn on the map.
fn draw_preview_cell(painter: &Painter, rect: Rect, view: &SpriteEditorView) {
    painter.rect(rect.x, rect.y, rect.w, rect.h, PANEL_BG);
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);

    let edge = view.canvas.edge as usize;
    let cell = rect.w / edge as f32;
    for y in 0..edge {
        for x in 0..edge {
            let idx = view.canvas.cells[y * edge + x];
            let color = match idx {
                0 => SCREEN_BG,
                n => canvas::palette_color(view.palette[n as usize - 1]),
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

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::{GameKey, Mode};
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
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_picker(&mut app, p, &m));
        let want = glyph_color(GlyphColor::White);
        let painted = crate::paint::painted_runs_in(&shapes, want, false);
        // At least the highlighted (first) subject's glyph must appear —
        // every subject's does, but this only needs one to hold the wire-up.
        assert!(
            painted.iter().any(|t| t == &subjects[0].glyph.to_string()),
            "the first subject's glyph {:?} was not painted in {want:?}: {painted:?}",
            subjects[0].glyph
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

    /// A cell painted in the editor shows up in three places at once: the
    /// canvas grid, the swatch it was painted in (always drawn, whether or
    /// not it was used), and the live preview reproducing the same pixel —
    /// the whole point of the preview existing.
    #[test]
    fn a_painted_cell_shows_on_the_canvas_the_palette_and_the_preview() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        // Space paints the cursor's cell (0,0) with the opening swatch,
        // index 1 (`canvas_editor::FIRST_COLOUR`) — `SPRITE_PALETTE[0]`.
        app.handle_key(GameKey::Char(' '));

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
        let want = canvas::palette_color(SPRITE_PALETTE[0]);
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, want),
            3,
            "expected the swatch, the painted grid cell and its preview pixel"
        );
    }

    /// A subject with no session open (unreachable through the real picker,
    /// but reachable if a caller sets the mode directly) draws something
    /// rather than nothing — a blank window would be a soft lock.
    #[test]
    fn no_session_draws_a_fallback_instead_of_a_blank_window() {
        let app = sprite_forge_app();
        assert!(app.sprite_editor_view().is_none());
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
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
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
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
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            drawn
                .iter()
                .any(|t| t.contains(&subject.label) || t.contains(&subject.name)),
            "the header must name the subject: {drawn:?}"
        );
    }
}
