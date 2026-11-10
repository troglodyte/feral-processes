//! The two dev-only Sprite Forge screens: `Mode::SpritePicker` (every name
//! the map can draw a sprite for, and its art state) and `Mode::SpriteEditor`
//! (`canvas::draw_canvas_grid`, `canvas::draw_swatch_grid` and a live preview
//! cell). Both close `render::tests::every_screen_draws_a_refusal_exactly_once`'s
//! two undrawn entries — see `render/mod.rs::ALL_MODES`.
//!
//! **The picker has no scroll**, unlike every menu `draw_popup` sizes: 54
//! subjects (every species, structure and floor finish, plus `player` and
//! `anchor`, as of this writing) is nearly twice `popup::popup_max_rows`'s
//! ceiling at 1280x720, so this screen is a fixed multi-column grid rather
//! than a `draw_popup` body. `PICKER_COLUMNS` and the row height
//! (`Metrics::line_height`) are
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
    CanvasFocus, EditorButton, PointerHit, SpriteArt, SpriteEditorView, SpriteSubject, SubjectTint,
    Tool,
};

// ---------------------------------------------------------------------
// The picker screen
// ---------------------------------------------------------------------

/// Fixed column count for the picker's subject grid — a layout constraint,
/// not a preference: this number and `Metrics::line_height` are what
/// `the_picker_shows_every_subject_with_no_scroll_at_1280x720` holds the
/// full shipped subject list to fitting against, with no scroll.
const PICKER_COLUMNS: usize = 4;

const PICKER_TITLE: &str = "Sprite Forge";
const PICKER_HELP: &[&str] = &[
    "Up/Down or click move",
    "Enter or double-click edit",
    "[t] toggle art",
    "[r] revert",
    "Esc back",
];

/// Marks a row the player has redrawn. Short on purpose: the row census
/// holds the longest label plus this against its column.
const EDITED_MARK: &str = " (edited)";

struct PickerGeometry {
    header_y: f32,
    help_y: f32,
    grid_top: f32,
    column_width: f32,
    rows_per_column: usize,
}

/// Everything geometric about the picker, computed once so drawing and the
/// layout census below share one derivation, so a test cannot measure a layout the
/// draw does not make.
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
/// census's one number.
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
    match app.sprite_revert_pending().and_then(|name| {
        subjects
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.label.clone())
    }) {
        Some(label) => {
            let ask = super::popup::key_hints(&[
                &format!("Revert `{label}` to the original art?"),
                "[y] revert",
                "[n] or Esc cancel",
            ]);
            centered_ui(painter, &ask, g.help_y, m.small(), TEXT);
        }
        None => centered_ui(
            painter,
            &super::popup::key_hints(PICKER_HELP),
            g.help_y,
            m.small(),
            TEXT_DIM,
        ),
    }

    for (i, subject) in subjects.iter().enumerate() {
        let (col, row) = picker_slot(i, g.rows_per_column);
        let x = m.pad + col as f32 * g.column_width;
        let y = g.grid_top + row as f32 * m.line_height;
        // Text is placed by baseline, so a row spans the line above its `y`.
        painter.target(
            Rect::new(x, y - m.line_height, g.column_width, m.line_height),
            PointerHit::Subject(i),
        );
        draw_subject_row(painter, x, y, subject, i == selected, m);
    }
}

fn edited_mark(subject: &SpriteSubject) -> &'static str {
    if subject.edited { EDITED_MARK } else { "" }
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
/// one subject that has none. `SpriteSubject::tint`'s own doc comment is
/// why `player` alone is `Glyph(None)`: its `@` wears a role colour, not an
/// authored hue, and `render/base/` never reads it through `glyph_color`
/// either. `super::player_look_color(None)` is the existing door to that
/// role colour's fallback — reused rather than naming `hud::palette::PLAYER`
/// a second time.
///
/// A floor finish's `Shade` arm resolves through `terrain::shade_color`, the
/// same table `draw_finish` paints the map with — a finish previewed in one
/// hue here and drawn in another on the base would be exactly the drift the
/// HUD seam's rule exists to catch.
fn subject_hue(tint: &SubjectTint) -> Color {
    match *tint {
        SubjectTint::Glyph(Some(c)) => glyph_color(c),
        SubjectTint::Glyph(None) => player_look_color(None),
        SubjectTint::Shade(shade) => super::terrain::shade_color(shade),
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
    let tail = format!(
        " {} ({}){}",
        subject.label,
        art_label(subject.art),
        edited_mark(subject)
    );
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
                color: subject_hue(&subject.tint),
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
        "{prefix}{} {} ({}){}",
        subject.glyph,
        subject.label,
        art_label(subject.art),
        edited_mark(subject)
    )
}

// ---------------------------------------------------------------------
// The editor screen
// ---------------------------------------------------------------------

/// The canvas cell's height in UI lines, halved from the 8x8 editor's
/// figure this screen replaced: the sprite canvas is 16x16 against that 8x8,
/// and halving keeps the canvas panel the same physical size on screen — a size known to fit at
/// the smallest supported window.
const SPRITE_CANVAS_CELL_LINES: f32 = 1.2;
const SPRITE_SECTION_GAP_LINES: f32 = 1.0;

struct EditorGeometry {
    header_y: f32,
    canvas_label_y: f32,
    canvas: Rect,
    cell: f32,
    palette_label_y: f32,
    palette: Rect,
    buttons: Vec<(EditorButton, Rect)>,
    preview_label_y: f32,
    preview: Rect,
    footer_y: f32,
    footer_lines: Vec<String>,
}

/// The edge, palette and key line come from the view being drawn rather than
/// a hardcoded 16 and `SPRITE_PALETTE.len()`, so this stays correct if any
/// of them ever changes without a second place to update.
fn editor_geometry(
    painter: &Painter,
    w: f32,
    m: &Metrics,
    view: &SpriteEditorView,
    zoom: u16,
) -> EditorGeometry {
    let edge = view.canvas.edge as usize;
    let palette_len = view.palette.len();
    let palette_cols = view.palette_cols as usize;
    let buttons = &view.buttons;
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
    // a tuned line count, which shipped 120px past `canvas.w`
    // for a whole task in the 8x8 editor before a test caught it. Deriving
    // the swatch size from the width it must fit closes that class of bug
    // by construction: whatever `palette_len` is, the grid cannot disagree
    // with `canvas.w` about how wide a swatch is allowed to be.
    // `draw_swatch_grid`'s own gap is a third of the swatch
    // (`canvas::SWATCH_GAP_RATIO`), so a row's total width is
    // `swatch * (cols + (cols - 1) / 3)`.
    let target_w = canvas.w - m.inset * 2.0;
    let cols = palette_cols as f32;
    let swatch = target_w / (cols + (cols - 1.0) * canvas::SWATCH_GAP_RATIO);
    let swatch_gap = swatch * canvas::SWATCH_GAP_RATIO;
    let rows = canvas::swatch_rows(canvas::swatch_count(palette_len), palette_cols);
    let grid_h = swatch * rows as f32 + swatch_gap * (rows - 1) as f32;
    let palette_w = swatch * cols + swatch_gap * (cols - 1.0) + m.inset * 2.0;
    let palette_h = grid_h + m.inset * 2.0;
    let palette = Rect::new(
        (w - palette_w) / 2.0,
        palette_label_y + m.gap,
        palette_w,
        palette_h,
    );

    // Beside the canvas rather than under the palette: the palette grid is
    // four rows tall, and a column of three panels no longer fits 720px.
    let preview_label_y = canvas_label_y;
    let (tile_px, _) = map_cell(zoom);
    let preview = Rect::new(canvas.x + canvas.w + gap, canvas.y, tile_px, tile_px);

    let mut button_rects = button_rects(painter, w, palette.y + palette.h + m.gap, m, buttons);
    let bar_bottom = button_rects.last().map_or(0.0, |(_, r)| r.y + r.h);
    // Under the preview's speed readout, where the screen has room.
    button_rects.extend(frame_button_rects(
        painter,
        preview.x,
        preview.y + preview.h + m.line_height * FRAME_COLUMN_LINES,
        m,
        buttons,
    ));
    let footer_top = bar_bottom + gap;
    // Measured in UI cells, since this screen has no popup body to wrap
    // against.
    let columns = ((w - m.pad * 2.0) / painter.measure_ui_advance("M", m.small()))
        .floor()
        .max(20.0) as usize;
    let footer_lines = feral_processes_engine::text::wrap(&view.keys, columns);

    EditorGeometry {
        header_y,
        canvas_label_y,
        canvas,
        cell,
        palette_label_y,
        palette,
        buttons: button_rects,
        preview_label_y,
        preview,
        footer_y: footer_top + m.line_height,
        footer_lines,
    }
}

/// How many monospace cells a button's label is given. The longest label,
/// `[Esc] Discard`, is thirteen; the rest is padding on both sides.
const BUTTON_CHARS: f32 = 15.0;

/// Buttons on the bottom bar; the frame and speed buttons stack in the
/// column under the preview instead, since eleven in a row would run off a
/// 1280px window and a second row costs the height this screen does not have.
const BAR_BUTTONS: usize = 6;

/// Lines between the preview's bottom and the frame column: one for the
/// speed readout, one of air.
const FRAME_COLUMN_LINES: f32 = 2.0;

/// The bottom button bar's rects, centred on the window at `y`. Every button
/// is the same width, so a label that changes with state (`Brush 1`/`Brush
/// 2`, `Paint`/`Fill`) cannot shift its neighbours or the hit rects.
fn button_rects(
    painter: &Painter,
    w: f32,
    y: f32,
    m: &Metrics,
    buttons: &[EditorButton],
) -> Vec<(EditorButton, Rect)> {
    let (button_w, button_h, gap) = button_size(painter, m);
    let bar = &buttons[..BAR_BUTTONS];
    let n = bar.len() as f32;
    let left = (w - (button_w * n + gap * (n - 1.0))) / 2.0;
    bar.iter()
        .enumerate()
        .map(|(i, &b)| {
            let x = left + i as f32 * (button_w + gap);
            (b, Rect::new(x, y, button_w, button_h))
        })
        .collect()
}

/// The frame and speed buttons, stacked down from `(x, y)`.
fn frame_button_rects(
    painter: &Painter,
    x: f32,
    y: f32,
    m: &Metrics,
    buttons: &[EditorButton],
) -> Vec<(EditorButton, Rect)> {
    let (button_w, button_h, gap) = button_size(painter, m);
    buttons[BAR_BUTTONS..]
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            (
                b,
                Rect::new(x, y + i as f32 * (button_h + gap), button_w, button_h),
            )
        })
        .collect()
}

fn button_size(painter: &Painter, m: &Metrics) -> (f32, f32, f32) {
    (
        painter.measure_ui_advance("M", m.small()) * BUTTON_CHARS,
        m.line_height * 1.3,
        m.gap.max(4.0),
    )
}

/// A button's label: the key it presses, then what it does now. The tool and
/// brush buttons name the state a press would leave, so the bar reads as the
/// current mode.
fn button_label(button: EditorButton, view: &SpriteEditorView) -> String {
    match button {
        EditorButton::Save => "[s] save".to_string(),
        EditorButton::Undo => "[u] undo".to_string(),
        EditorButton::Clear => "[x] clear".to_string(),
        EditorButton::Brush => format!("[g] brush {}", view.canvas.brush),
        EditorButton::Tool => match view.tool {
            Tool::Paint => "[f] paint".to_string(),
            Tool::Fill => "[f] fill".to_string(),
        },
        EditorButton::Back => "Esc back".to_string(),
        EditorButton::Keep => "Enter keep".to_string(),
        EditorButton::Discard => "Esc discard".to_string(),
        EditorButton::Frame1 => "[1] frame 1".to_string(),
        EditorButton::Frame2 => "[2] frame 2".to_string(),
        EditorButton::DeleteFrame => "[D] delete 2".to_string(),
        EditorButton::Slower => "[-] slower".to_string(),
        EditorButton::Faster => "[=] faster".to_string(),
    }
}

/// Text for a control that does nothing on this sprite: dimmer than
/// `TEXT_DIM`, which is the ordinary secondary colour.
const TEXT_OFF: Color = Color::new(0.35, 0.35, 0.4, 1.0);

/// The speed controls only mean something on a sheet: a one-frame sprite
/// saves no `.anim.ron`, so the pace is discarded.
fn speed_is_live(view: &SpriteEditorView) -> bool {
    view.frames > 1
}

fn draw_button_bar(painter: &Painter, g: &EditorGeometry, view: &SpriteEditorView, m: &Metrics) {
    for (button, r) in &g.buttons {
        painter.target(*r, PointerHit::Button(*button));
        painter.rect(r.x, r.y, r.w, r.h, PANEL_BG);
        let lit = match button {
            EditorButton::Tool => view.tool == Tool::Fill,
            EditorButton::Frame1 => view.active_frame == 0,
            EditorButton::Frame2 => view.active_frame == 1,
            _ => false,
        };
        let (thickness, color) = if lit { (3.0, BORDER) } else { (1.0, TEXT_DIM) };
        painter.rect_lines(r.x, r.y, r.w, r.h, thickness, color);
        let label = button_label(*button, view);
        let text_w = painter.measure_ui(&label, m.small()).width;
        let off =
            matches!(button, EditorButton::Slower | EditorButton::Faster) && !speed_is_live(view);
        painter.ui(
            &label,
            r.x + (r.w - text_w) / 2.0,
            r.y + (r.h + m.line_height) / 2.0 - m.gap,
            m.small(),
            if off { TEXT_OFF } else { TEXT },
        );
    }
}

/// Total height the screen draws, footer included.
#[cfg(test)]
fn editor_content_bottom(g: &EditorGeometry, m: &Metrics) -> f32 {
    g.footer_y + g.footer_lines.len().saturating_sub(1) as f32 * m.line_height + m.pad
}

pub(super) fn draw_sprite_editor(app: &App, painter: &Painter, m: &Metrics) {
    let zoom = app.zoom;
    match app.sprite_editor_view() {
        Some(view) => {
            draw_sprite_editor_session(&view, zoom, painter, m);
        }
        // Unreachable through the picker's own `Enter` (it always opens a
        // session before switching mode) but not through a test, or a
        // future caller that sets `Mode::SpriteEditor` directly — a blank
        // window would be a soft lock the player cannot read their way out
        // of, so this still names the way back rather than drawing nothing.
        None => centered_ui(
            painter,
            &super::popup::key_hints(&["No sprite selected", "Esc back"]),
            m.pad + m.line_height,
            m.title(),
            TEXT_DIM,
        ),
    }
}

pub(super) fn draw_sprite_editor_session(
    view: &SpriteEditorView,
    zoom: u16,
    painter: &Painter,
    m: &Metrics,
) {
    let edge = view.canvas.edge as usize;
    let g = editor_geometry(painter, painter.screen_w(), m, view, zoom);
    // White is the identity tint: a host with no subject hue draws the
    // preview as authored.
    let hue = view
        .tint
        .as_ref()
        .map(subject_hue)
        .unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0));

    centered_ui(painter, &view.title, g.header_y, m.title(), TEXT);

    centered_ui(painter, "Canvas", g.canvas_label_y, m.small(), TEXT_DIM);
    let c = g.canvas;
    painter.rect(c.x, c.y, c.w, c.h, PANEL_BG);
    let (thickness, color) = panel_border(view.canvas.focus == CanvasFocus::Canvas);
    painter.rect_lines(c.x, c.y, c.w, c.h, thickness, color);
    let side = edge as f32 * g.cell;
    let inner = Rect::new(c.x + m.inset, c.y + m.inset, side, side);
    canvas::draw_canvas_grid(painter, inner, &view.canvas, view.palette);
    for y in 0..edge {
        for x in 0..edge {
            painter.target(
                Rect::new(
                    inner.x + x as f32 * g.cell,
                    inner.y + y as f32 * g.cell,
                    g.cell,
                    g.cell,
                ),
                PointerHit::Cell(x as u8, y as u8),
            );
        }
    }

    centered_ui(painter, "Palette", g.palette_label_y, m.small(), TEXT_DIM);
    let p = g.palette;
    painter.rect(p.x, p.y, p.w, p.h, PANEL_BG);
    let (thickness, color) = panel_border(view.canvas.focus == CanvasFocus::Palette);
    painter.rect_lines(p.x, p.y, p.w, p.h, thickness, color);
    let palette_inner = Rect::new(
        p.x + m.inset,
        p.y + m.inset,
        p.w - m.inset * 2.0,
        p.h - m.inset * 2.0,
    );
    canvas::draw_swatch_grid(
        painter,
        palette_inner,
        view.palette_cols as usize,
        view.canvas.selected,
        view.palette,
    );
    // `PointerHit::Swatch` carries the drawn position, which is the palette
    // index itself (`canvas::swatch_count`).
    for (i, r) in canvas::swatch_rects(
        palette_inner,
        view.palette_cols as usize,
        view.palette.len(),
    )
    .into_iter()
    .enumerate()
    {
        painter.target(r, PointerHit::Swatch(i as u8));
    }

    draw_button_bar(painter, &g, view, m);

    let preview_label = "Preview";
    let label_w = painter.measure_ui(preview_label, m.small()).width;
    painter.ui(
        preview_label,
        g.preview.x + (g.preview.w - label_w) / 2.0,
        g.preview_label_y,
        m.small(),
        TEXT_DIM,
    );
    draw_preview_cell(painter, g.preview, view, hue);
    let speed = format!("{} ms", view.frame_ms);
    let speed_w = painter.measure_ui(&speed, m.small()).width;
    painter.ui(
        &speed,
        g.preview.x + (g.preview.w - speed_w) / 2.0,
        g.preview.y + g.preview.h + m.line_height,
        m.small(),
        if speed_is_live(view) {
            TEXT_DIM
        } else {
            TEXT_OFF
        },
    );

    let mut y = g.footer_y;
    for line in &g.footer_lines {
        centered_ui(painter, line, y, m.small(), TEXT_DIM);
        y += m.line_height;
    }
}

/// A panel's border thickness and colour for whether it currently has
/// focus.
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
/// canvas comes out hued, and a saturated one goes muddy, right here. A
/// colour sprite (`view.full_colour`) is tinted the way the map tints it,
/// through `sprite_tint`.
fn draw_preview_cell(painter: &Painter, rect: Rect, view: &SpriteEditorView, hue: Color) {
    let hue = crate::paint::sprite_tint(hue, view.full_colour);
    painter.rect(rect.x, rect.y, rect.w, rect.h, PANEL_BG);

    let edge = view.canvas.edge as usize;
    let cell = rect.w / edge as f32;
    // Plays the sheet at the pace the save would write, so the player sees
    // the animation before committing it. Effects off shows frame one, as
    // the map does.
    let shown = painter.anim_now().map_or(0, |now| {
        crate::paint::sprite_frame(now, 0, view.frames, view.frame_ms)
    });
    let cells = &view.frame_cells[shown];
    for y in 0..edge {
        for x in 0..edge {
            let idx = cells[y * edge + x];
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
    // After the cells: a stroke is centred on the edge, so drawn first its
    // inner half is painted over and a sparse sprite shows a stray line.
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pointer::{Reader, tests::Frame};
    use feral_processes_app_core::{FORGE_BUTTONS, GameKey, Mode, PointerHit};
    use feral_processes_engine::components::GlyphColor;
    use feral_processes_engine::icon::SPRITE_PALETTE;

    const CENSUS_W: f32 = 1280.0;
    const CENSUS_H: f32 = 720.0;

    /// A fresh app with real assets loaded — Sprite Forge reads
    /// `assets/species`/`assets/structures` straight off disk
    /// (`App::sprite_subjects`), so its tests need the real tree rather
    /// than a hand-built fixture, `creation.rs::wizard_app`'s reason.
    fn sprite_forge_app() -> feral_processes_app_core::App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        // A per-call counter as well as the pid, `app-core`'s `scratch_path`
        // convention: libtest runs these on a thread pool, so a path keyed on
        // the process alone is shared by every sprite-forge test at once and
        // the `remove_dir_all`/`create_dir_all` pair races against itself —
        // `create_dir_all` answers `AlreadyExists` when another thread wins
        // the mkdir. Seen once in a full-workspace run and never when the
        // file is run alone, which is the shape that reads as unrelated.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!(
            "fp_gui_sprite_forge_{}_{unique}",
            std::process::id()
        ));
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

    /// The Forge's editor view on the first subject.
    fn forge_view() -> SpriteEditorView {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        app.sprite_editor_view().expect("just opened")
    }

    /// A wizard walked to the Icon step with its editor open.
    fn wizard_editor_app() -> feral_processes_app_core::App {
        use super::super::creation::tests::{walk_past, wizard_app};
        use feral_processes_app_core::CreationStep;
        let mut app = wizard_app();
        for step in CreationStep::ALL
            .iter()
            .take_while(|s| **s != CreationStep::Icon)
        {
            walk_past(&mut app, *step);
        }
        while app.menu_selected != app.creation_rows().len() - 1 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        app
    }

    // -------------------------------------------------------------
    // The picker
    // -------------------------------------------------------------

    /// **The layout census this task exists for.** Every one of the real,
    /// shipped subjects must fit inside 1280x720 with no scroll, since this
    /// screen has none — the `memory-page-has-no-scroll` precedent.
    ///
    /// 104 is `sprite_subjects_is_every_species_and_structure_plus_player_and_anchor`'s
    /// own pinned count (the Home's `sprite:` field naming "anchor" and
    /// de-duplicating away, plus the fifteen shipped floor finishes and the
    /// twenty battle-prop sprites), asserted
    /// again here so a shrinking asset tree can't silently make this census
    /// easier than the one it is meant to hold.
    #[test]
    fn the_picker_shows_every_subject_with_no_scroll_at_1280x720() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        assert_eq!(
            subjects.len(),
            90,
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
                // Worst case: every row carries the edited mark.
                let marked = SpriteSubject {
                    edited: true,
                    ..subject.clone()
                };
                let text = subject_row_text(&marked, false);
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
            cipher.tint,
            SubjectTint::Glyph(Some(GlyphColor::Cyan)),
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
    /// `GlyphColor` — `SpriteSubject::tint`'s own doc comment, and the same
    /// rule `render/base/` follows for the real map tile.
    #[test]
    fn the_player_row_wears_the_player_role_colour_not_an_authored_hue() {
        let mut app = sprite_forge_app();
        let subjects = app.sprite_subjects();
        let player = subjects
            .iter()
            .find(|s| s.name == "player")
            .expect("player is one of the two hardcoded subjects");
        assert_eq!(player.tint, SubjectTint::Glyph(None));

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
            let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), 2);
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
            let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), 2);
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
            let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), 2);
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
                let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), zoom);
                let (tile_px, _) = crate::text::map_cell(zoom);
                assert_eq!(
                    g.preview.w, tile_px,
                    "zoom {zoom}'s preview cell is the wrong size"
                );
                assert_eq!(g.preview.h, tile_px);
                assert!(
                    g.preview.x + g.preview.w <= CENSUS_W,
                    "zoom {zoom}'s preview runs off the window: {:?}",
                    g.preview
                );
                assert!(
                    g.preview.x >= g.canvas.x + g.canvas.w,
                    "the preview must sit clear of the canvas panel"
                );
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
            subjects[index].tint,
            SubjectTint::Glyph(Some(GlyphColor::Cyan)),
            "the fixture this test's premise rests on"
        );
        open_editor(&mut app, index);
        // Space paints the cursor's cell (0,0) with the opening swatch,
        // index 1 (`canvas_editor::FIRST_COLOUR`) — `SPRITE_PALETTE[0]`.
        app.handle_key(GameKey::Char(' '));

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
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
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
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

    /// The wizard's editor is the same screen with the Forge's subject hue
    /// taken away: the avatar is drawn untinted everywhere, so the preview
    /// pixel is the palette entry's own colour. Drawn through `render::draw`
    /// so the `Mode::CreateCharacter` arm is the thing under test.
    #[test]
    fn the_wizard_preview_is_drawn_untinted() {
        let mut app = wizard_editor_app();
        app.handle_key(GameKey::Char(' '));
        let mut fx = crate::fx::Fx::new();
        let (_, shapes) =
            crate::paint::with_painter(|p| crate::render::draw(&mut app, &mut fx, p, false, false));
        let want = canvas::palette_color(SPRITE_PALETTE[0]);
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, want),
            3,
            "the swatch, the grid cell and the preview pixel, all in the entry's own colour"
        );
    }

    /// The dispatcher draws the editor, not the wizard's popup, once the
    /// Icon step's editor is open — and in the wizard's own words, not the
    /// dev tool's.
    #[test]
    fn the_dispatcher_draws_the_editor_over_the_wizard_once_it_is_open() {
        let mut app = wizard_editor_app();
        assert_eq!(app.mode, Mode::CreateCharacter);
        let mut fx = crate::fx::Fx::new();
        let (_, shapes) =
            crate::paint::with_painter(|p| crate::render::draw(&mut app, &mut fx, p, false, false));
        let drawn = crate::paint::painted_text(&shapes).join("\n");
        assert!(drawn.contains("Draw your avatar"), "{drawn}");
        assert!(drawn.contains("Enter keep"), "{drawn}");
        assert!(
            drawn.contains("Enter keep · Esc discard"),
            "the key line names Enter: {drawn}"
        );
        assert!(!drawn.contains("Sprite Forge"), "{drawn}");
        assert!(!drawn.contains("[s] save"), "{drawn}");
    }

    /// The preview must show what the map will draw: a colour sprite takes a
    /// grey tint at the subject hue's brightest channel, not the hue.
    #[test]
    fn a_colour_sprite_previews_with_the_maps_grey_tint_not_the_hue() {
        use feral_processes_app_core::InstalledSprite;
        use feral_processes_engine::icon::Canvas;
        let mut app = sprite_forge_app();
        let mut art = Canvas::new(16);
        art.set(0, 0, 9); // pure white
        let mut enabled = std::collections::HashMap::new();
        enabled.insert(
            "cipher".to_string(),
            InstalledSprite {
                frames: vec![art],
                frame_ms: feral_processes_app_core::DEFAULT_SPRITE_FRAME_MS,
                full_colour: true,
            },
        );
        app.install_sprite_library(enabled, Default::default(), Default::default());
        let index = app
            .sprite_subjects()
            .iter()
            .position(|s| s.name == "cipher")
            .unwrap();
        open_editor(&mut app, index);

        let hue = glyph_color(GlyphColor::Cyan);
        let grey = crate::paint::colour_sprite_tint(hue);
        assert_ne!(grey, hue, "the fixture must have a hue to drop");

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
        assert_eq!(crate::paint::painted_rect_fill_count(&shapes, grey), 1);
        assert_eq!(crate::paint::painted_rect_fill_count(&shapes, hue), 0);
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
            "[u]",
            "[x]",
            "[g]",
            "[f]",
            "[i]",
            "[s]",
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

    // -------------------------------------------------------------
    // The mouse, through the pointer layer: a draw registers the targets,
    // the reader resolves a frame's pointer against them.
    // -------------------------------------------------------------

    /// A screen drawn and read frame by frame, as the frontend does: the
    /// reader first, then the draw that registers the next frame's targets.
    struct Drive {
        reader: Reader,
        ctx: bevy_egui::egui::Context,
        fx: crate::fx::Fx,
        time: f64,
    }

    impl Drive {
        fn new() -> Self {
            let ctx = bevy_egui::egui::Context::default();
            crate::paint::install_fonts(&ctx);
            Drive {
                reader: Reader::new(),
                ctx,
                fx: crate::fx::Fx::new(),
                time: 0.0,
            }
        }

        fn frame(
            &mut self,
            app: &mut feral_processes_app_core::App,
            at: (f32, f32),
            f: impl FnOnce(Frame) -> Frame,
        ) {
            self.time += 0.05;
            let input = f(Frame::at(at.0, at.1, self.time)).input();
            let ctx = self.ctx.clone();
            let _ = ctx.run_ui(input, |_| {
                self.reader.frame(&ctx, app);
                let painter = crate::paint::Painter::for_frame(&ctx, 0.0, Default::default())
                    .recording_into(self.reader.sink());
                crate::render::draw(app, &mut self.fx, &painter, false, false);
            });
            self.reader.drawn(app);
        }

        /// Two frames with the pointer off every target, so the reader is
        /// armed and holds the targets the screen registers.
        fn settle(&mut self, app: &mut feral_processes_app_core::App) {
            self.frame(app, (-50.0, -50.0), |f| f);
            self.frame(app, (-50.0, -50.0), |f| f);
        }

        fn targets(&self) -> Vec<(Rect, PointerHit)> {
            self.reader.sink().lock().expect("no panic").clone()
        }

        fn rect_of(&self, hit: PointerHit) -> Rect {
            self.targets()
                .into_iter()
                .find(|(_, h)| *h == hit)
                .unwrap_or_else(|| panic!("{hit:?} was not drawn"))
                .0
        }

        fn centre(&self, hit: PointerHit) -> (f32, f32) {
            let r = self.rect_of(hit);
            (r.x + r.w / 2.0, r.y + r.h / 2.0)
        }

        /// What the pointer is over at `at`, as the app records it.
        fn hover(
            &mut self,
            app: &mut feral_processes_app_core::App,
            at: (f32, f32),
        ) -> Option<PointerHit> {
            self.frame(app, at, |f| f);
            app.pointer_hover
        }

        fn click(&mut self, app: &mut feral_processes_app_core::App, at: (f32, f32)) {
            self.frame(app, at, Frame::press);
            self.frame(app, at, Frame::release);
        }
    }

    fn editor_drive() -> (feral_processes_app_core::App, Drive) {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let mut drive = Drive::new();
        drive.settle(&mut app);
        (app, drive)
    }

    /// The canvas's own four corners resolve to their own corner cells and
    /// a point in the middle of an interior cell to that cell. The far
    /// corners are taken a hair inside: cells are half-open rects now, so
    /// the one pixel exactly on the far edge belongs to no cell.
    #[test]
    fn the_canvas_corners_and_an_interior_point_resolve_to_their_cells() {
        let (mut app, mut drive) = editor_drive();
        let first = drive.rect_of(PointerHit::Cell(0, 0));
        let last = drive.rect_of(PointerHit::Cell(15, 15));
        let (x0, y0) = (first.x, first.y);
        let (x1, y1) = (last.x + last.w - 0.01, last.y + last.h - 0.01);
        for (pos, cell, what) in [
            ((x0, y0), (0, 0), "top-left corner"),
            ((x1, y0), (15, 0), "top-right corner"),
            ((x0, y1), (0, 15), "bottom-left corner"),
            ((x1, y1), (15, 15), "bottom-right corner"),
        ] {
            assert_eq!(
                drive.hover(&mut app, pos),
                Some(PointerHit::Cell(cell.0, cell.1)),
                "{what}"
            );
        }
        let middle = drive.centre(PointerHit::Cell(8, 8));
        assert_eq!(
            drive.hover(&mut app, middle),
            Some(PointerHit::Cell(8, 8)),
            "an interior point"
        );
    }

    /// A position outside the canvas on any of the four sides resolves to no
    /// hit at all.
    #[test]
    fn a_position_outside_the_canvas_resolves_to_nothing() {
        let (mut app, mut drive) = editor_drive();
        let first = drive.rect_of(PointerHit::Cell(0, 0));
        let last = drive.rect_of(PointerHit::Cell(15, 15));
        let (right, bottom) = (last.x + last.w, last.y + last.h);
        for (pos, what) in [
            ((first.x - 0.1, first.y), "left of the canvas"),
            ((right + 0.1, first.y), "right of the canvas"),
            ((first.x, first.y - 0.1), "above the canvas"),
            ((first.x, bottom + 0.1), "below the canvas"),
        ] {
            assert_eq!(drive.hover(&mut app, pos), None, "{what}");
        }
    }

    /// A swatch is hit where it is, and its gap to the next is a miss rather
    /// than a snap to the nearer one.
    #[test]
    fn a_swatch_is_hit_and_the_gap_beside_it_and_the_strip_outside_are_not() {
        let (mut app, mut drive) = editor_drive();
        let (a, b) = (
            drive.rect_of(PointerHit::Swatch(0)),
            drive.rect_of(PointerHit::Swatch(1)),
        );
        let centre = drive.centre(PointerHit::Swatch(0));
        assert_eq!(
            drive.hover(&mut app, centre),
            Some(PointerHit::Swatch(0)),
            "inside the first swatch"
        );
        let gap = ((a.x + a.w + b.x) / 2.0, centre.1);
        assert_eq!(
            drive.hover(&mut app, gap),
            None,
            "in the gap between the two swatches"
        );
        let centre = drive.centre(PointerHit::Swatch(1));
        assert_eq!(
            drive.hover(&mut app, centre),
            Some(PointerHit::Swatch(1)),
            "inside the second swatch"
        );
        assert_eq!(
            drive.hover(&mut app, (a.x - 1.0, a.y + a.h / 2.0)),
            None,
            "left of the strip"
        );
    }

    /// The canvas, the palette and neither: the origin of each resolves to
    /// its first cell or swatch, and the window's corner to nothing.
    #[test]
    fn the_canvas_and_the_palette_each_resolve_and_the_rest_does_not() {
        let (mut app, mut drive) = editor_drive();
        let cell = drive.rect_of(PointerHit::Cell(0, 0));
        assert_eq!(
            drive.hover(&mut app, (cell.x, cell.y)),
            Some(PointerHit::Cell(0, 0))
        );
        let swatch = drive.rect_of(PointerHit::Swatch(0));
        assert_eq!(
            drive.hover(&mut app, (swatch.x, swatch.y)),
            Some(PointerHit::Swatch(0))
        );
        assert_eq!(drive.hover(&mut app, (-1.0, -1.0)), None, "outside both");
    }

    /// **The seam neither side's own test spanned.** A click answers in drawn
    /// positions, `pick_swatch` writes `CanvasView::selected`, and
    /// `draw_swatch_grid` reads that back to outline exactly one swatch — so
    /// the only honest question is whether the outline lands on the swatch
    /// the pointer was actually over. Asked of the first drawn position (the
    /// transparent swatch), a middle one and the **last**, `SPRITE_PALETTE`'s
    /// final entry — the one an off-by-one in either direction cannot reach
    /// at all.
    #[test]
    fn a_click_outlines_the_swatch_under_the_pointer() {
        let (mut app, mut drive) = editor_drive();
        let positions = canvas::swatch_count(SPRITE_PALETTE.len());
        let origin = drive.rect_of(PointerHit::Swatch(0));
        let cols = app.sprite_editor_view().expect("open").palette_cols as usize;
        let rows = canvas::swatch_rows(positions, cols);
        let side = origin.w;
        let grid = Rect::new(
            origin.x,
            origin.y,
            0.0,
            side * (rows as f32 + (rows - 1) as f32 * canvas::SWATCH_GAP_RATIO),
        );

        for i in [0usize, positions / 2, positions - 1] {
            let r = drive.rect_of(PointerHit::Swatch(i as u8));
            let (left, top) = (r.x, r.y);
            drive.click(&mut app, drive.centre(PointerHit::Swatch(i as u8)));

            let selected = app
                .sprite_editor_view()
                .expect("still open")
                .canvas
                .selected;
            let (_, shapes) = crate::paint::with_painter(|p| {
                canvas::draw_swatch_grid(p, grid, cols, selected, &SPRITE_PALETTE)
            });
            let outlined =
                crate::paint::painted_rect_stroke_boxes(&shapes, canvas::SELECTED_SWATCH_COLOR);
            assert_eq!(outlined.len(), 1, "exactly one swatch is outlined");
            assert!(
                (outlined[0].min.x - left).abs() < 0.5 && (outlined[0].min.y - top).abs() < 0.5,
                "clicking swatch {i} (drawn at {left},{top}) must outline that swatch, \
                 not the one at {:?}",
                outlined[0].min
            );
        }
    }

    /// A 16-column grid: every swatch centre hits its own index and every
    /// gap between swatches, in both directions, misses.
    #[test]
    fn every_swatch_centre_hits_and_every_gap_misses_in_a_16_column_grid() {
        let (mut app, mut drive) = editor_drive();
        let count = canvas::swatch_count(SPRITE_PALETTE.len());
        assert_eq!(app.sprite_editor_view().expect("open").palette_cols, 16);
        let stride = {
            let (a, b) = (
                drive.rect_of(PointerHit::Swatch(0)),
                drive.rect_of(PointerHit::Swatch(1)),
            );
            b.x - a.x
        };
        for i in 0..count {
            let (cx, cy) = drive.centre(PointerHit::Swatch(i as u8));
            assert_eq!(
                drive.hover(&mut app, (cx, cy)),
                Some(PointerHit::Swatch(i as u8)),
                "centre {i}"
            );
            assert_eq!(
                drive.hover(&mut app, (cx + stride / 2.0, cy)),
                None,
                "gap right of {i}"
            );
            assert_eq!(
                drive.hover(&mut app, (cx, cy + stride / 2.0)),
                None,
                "gap below {i}"
            );
        }
    }

    /// A short last row has no swatch in its empty tail.
    #[test]
    fn the_empty_tail_of_the_last_swatch_row_has_no_swatch() {
        let rect = Rect::new(
            0.0,
            0.0,
            3.0 * 10.0 + 2.0 * (10.0 / 3.0),
            2.0 * 10.0 + 10.0 / 3.0,
        );
        let swatches = canvas::swatch_rects(rect, 3, 4);
        assert_eq!(swatches.len(), 5);
        let inside = |r: &Rect, pos: (f32, f32)| {
            pos.0 >= r.x && pos.0 < r.x + r.w && pos.1 >= r.y && pos.1 < r.y + r.h
        };
        assert!(inside(&swatches[4], (18.0, 15.0)));
        assert!(!swatches.iter().any(|r| inside(r, (32.0, 15.0))));
    }

    /// A press and a drag across the canvas paint every cell the pointer
    /// reaches, as one stroke: one undo takes it all back. Leaving the canvas
    /// while held ends the stroke, so coming back onto it paints nothing.
    #[test]
    fn a_drag_across_the_canvas_paints_one_stroke_and_leaving_ends_it() {
        let (mut app, mut drive) = editor_drive();
        let cells = |app: &mut feral_processes_app_core::App| {
            app.sprite_editor_view().expect("open").canvas.cells
        };
        let before = cells(&mut app);
        let at = |drive: &Drive, x, y| drive.centre(PointerHit::Cell(x, y));
        let (a, b, c) = (at(&drive, 0, 0), at(&drive, 1, 0), at(&drive, 2, 0));
        drive.frame(&mut app, a, Frame::press);
        drive.frame(&mut app, b, |f| f);
        drive.frame(&mut app, c, Frame::release);
        let painted = cells(&mut app);
        assert!(
            (0..3).all(|x| painted[x] != before[x]),
            "every cell the stroke crossed is painted: {:?}",
            &painted[..3]
        );
        app.handle_key(GameKey::Char('u'));
        assert_eq!(cells(&mut app), before, "one undo takes the whole stroke");

        let (far, away, back) = (at(&drive, 5, 5), (-50.0, -50.0), at(&drive, 6, 5));
        drive.frame(&mut app, far, Frame::press);
        drive.frame(&mut app, away, |f| f);
        drive.frame(&mut app, back, |f| f);
        drive.frame(&mut app, back, Frame::release);
        let after = cells(&mut app);
        assert_ne!(after[5 * 16 + 5], before[5 * 16 + 5], "the press painted");
        assert_eq!(
            after[5 * 16 + 6],
            before[5 * 16 + 6],
            "the stroke ended when the pointer left the canvas"
        );
    }

    /// A click on the middle of each drawn button lands on that button, and
    /// every button the editor has is drawn.
    #[test]
    fn a_click_on_each_button_rect_resolves_to_that_button() {
        let (mut app, mut drive) = editor_drive();
        let buttons: Vec<_> = drive
            .targets()
            .into_iter()
            .filter_map(|(r, h)| match h {
                PointerHit::Button(b) => Some((b, r)),
                _ => None,
            })
            .collect();
        assert_eq!(buttons.len(), FORGE_BUTTONS.len());
        for (button, r) in &buttons {
            let centre = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            assert_eq!(
                drive.hover(&mut app, centre),
                Some(PointerHit::Button(*button))
            );
        }
    }

    /// A button is the key it names: Back leaves the editor as Esc does.
    #[test]
    fn a_click_on_a_button_presses_its_key() {
        let (mut app, mut drive) = editor_drive();
        let back = drive.centre(PointerHit::Button(
            feral_processes_app_core::EditorButton::Back,
        ));
        drive.click(&mut app, back);
        assert_eq!(app.mode, Mode::SpritePicker);
    }

    /// A click on the middle of each picker row resolves to that row's index,
    /// through the geometry the rows are drawn from, and the margin around
    /// the grid is no row.
    #[test]
    fn a_click_on_each_picker_row_resolves_to_its_subject() {
        let mut app = sprite_forge_app();
        app.mode = Mode::SpritePicker;
        let count = app.sprite_subjects().len();
        let mut drive = Drive::new();
        drive.settle(&mut app);
        let rows = (0..count)
            .map(|i| drive.rect_of(PointerHit::Subject(i)))
            .collect::<Vec<_>>();
        for (i, r) in rows.iter().enumerate() {
            let centre = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            assert_eq!(
                drive.hover(&mut app, centre),
                Some(PointerHit::Subject(i)),
                "row {i}"
            );
        }
        assert_eq!(
            drive.hover(&mut app, (1.0, 1.0)),
            None,
            "the title area is no row"
        );
    }

    /// A row's rect holds the baseline its text is drawn on, and nothing of
    /// the row above.
    #[test]
    fn a_picker_row_rect_spans_the_line_its_text_sits_on() {
        let mut app = sprite_forge_app();
        app.mode = Mode::SpritePicker;
        let count = app.sprite_subjects().len();
        let mut drive = Drive::new();
        drive.settle(&mut app);
        let m = crate::text::ui_metrics(600.0);
        let g = picker_geometry(800.0, &m, count);
        let (above, row) = (
            drive.rect_of(PointerHit::Subject(2)),
            drive.rect_of(PointerHit::Subject(3)),
        );
        let baseline = g.grid_top + 3.0 * m.line_height;
        assert!(row.y < baseline && baseline <= row.y + row.h);
        assert!(above.y + above.h <= row.y + 0.01, "rows do not overlap");
    }

    /// Two clicks on a picker row select it, then open it — whatever the
    /// pace, there is no double-click to time.
    #[test]
    fn a_click_selects_a_picker_row_and_a_second_opens_it() {
        let mut app = sprite_forge_app();
        app.mode = Mode::SpritePicker;
        let mut drive = Drive::new();
        drive.settle(&mut app);
        let row = drive.centre(PointerHit::Subject(4));
        drive.click(&mut app, row);
        assert_eq!((app.mode, app.menu_selected), (Mode::SpritePicker, 4));
        drive.click(&mut app, row);
        assert_eq!(app.mode, Mode::SpriteEditor);
    }

    /// The wizard registers editor targets only while its editor is open.
    #[test]
    fn the_wizard_registers_editor_targets_only_while_its_editor_is_open() {
        use super::super::creation::tests::{walk_past, wizard_app};
        use feral_processes_app_core::CreationStep;
        let mut app = wizard_app();
        for step in CreationStep::ALL
            .iter()
            .take_while(|s| **s != CreationStep::Icon)
        {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.mode, Mode::CreateCharacter);
        let mut drive = Drive::new();
        drive.settle(&mut app);
        let editor_targets = |drive: &Drive| {
            drive
                .targets()
                .iter()
                .any(|(_, h)| matches!(h, PointerHit::Cell(..) | PointerHit::Swatch(_)))
        };
        assert!(!editor_targets(&drive));
        while app.menu_selected != app.creation_rows().len() - 1 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        drive.settle(&mut app);
        assert!(editor_targets(&drive));
    }

    /// The sprite editor's whole screen, grid included, still fits inside
    /// the window and the palette is as wide as its canvas allows.
    #[test]
    fn the_palette_grid_is_four_rows_of_sixteen() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let view = app.sprite_editor_view().expect("just opened");
        assert_eq!(view.palette_cols, 16);
        assert_eq!(
            canvas::swatch_rows(canvas::swatch_count(view.palette.len()), 16),
            4
        );
    }

    /// The bar sits below the palette, inside the window, without overlap.
    #[test]
    fn the_button_bar_fits_at_1280x720_below_the_palette() {
        let m = crate::text::ui_metrics(CENSUS_H);
        crate::paint::with_painter(|p| {
            let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), 2);
            let mut right = 0.0_f32;
            for (b, r) in &g.buttons[..BAR_BUTTONS] {
                assert!(r.x >= 0.0 && r.x + r.w <= CENSUS_W, "{b:?} off the window");
                assert!(
                    r.y >= g.palette.y + g.palette.h,
                    "{b:?} overlaps the palette"
                );
                assert!(r.x >= right, "{b:?} overlaps its neighbour");
                assert!(r.y + r.h <= g.footer_y, "{b:?} overlaps the footer");
                right = r.x + r.w;
            }
        });
    }

    /// Every label fits its button, in every state the label can take, for
    /// each host's button list.
    #[test]
    fn every_button_label_fits_its_button() {
        let m = crate::text::ui_metrics(CENSUS_H);
        let mut forge = sprite_forge_app();
        open_editor(&mut forge, 0);
        let mut wizard = wizard_editor_app();
        for app in [&mut forge, &mut wizard] {
            for keys in [vec![], vec!['f'], vec!['g'], vec!['f', 'g']] {
                for k in keys {
                    app.handle_key(GameKey::Char(k));
                }
                let view = app
                    .sprite_editor_view()
                    .or_else(|| app.creation_sprite_view())
                    .expect("open");
                crate::paint::with_painter(|p| {
                    let g = editor_geometry(p, CENSUS_W, &m, &view, 2);
                    assert_eq!(g.buttons.len(), view.buttons.len());
                    for (b, r) in &g.buttons {
                        let label = button_label(*b, &view);
                        assert!(p.measure_ui(&label, m.small()).width < r.w, "{label:?}");
                    }
                });
            }
        }
    }

    /// The bar is drawn: every label shows up on screen.
    #[test]
    fn the_editor_draws_every_button_label() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        for label in [
            "[s] save",
            "[u] undo",
            "[x] clear",
            "[g] brush 1",
            "[f] paint",
            "Esc back",
            "[1] frame 1",
            "[2] frame 2",
            "[D] delete 2",
            "[-] slower",
            "[=] faster",
        ] {
            assert!(
                drawn.iter().any(|t| t == label),
                "{label:?} missing: {drawn:?}"
            );
        }
    }

    /// The preview's frame is drawn over its pixels, not under them: drawn
    /// first, a sparse sprite left only the uncovered outer half of the
    /// stroke showing, which read as a stray vertical line.
    #[test]
    fn the_preview_frame_is_drawn_after_its_pixels() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        let view = app.sprite_editor_view().expect("open");
        let rect = Rect::new(500.0, 40.0, 32.0, 32.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_preview_cell(p, rect, &view, WHITE));
        let mut frame = None;
        let mut last_fill = None;
        for (i, cs) in shapes.iter().enumerate() {
            if let bevy_egui::egui::Shape::Rect(r) = &cs.shape {
                if r.stroke.width > 0.0 {
                    frame = Some(i);
                } else {
                    last_fill = Some(i);
                }
            }
        }
        let (frame, last_fill) = (frame.expect("a frame"), last_fill.expect("pixels"));
        assert!(
            frame > last_fill,
            "frame at {frame}, last pixel at {last_fill}"
        );
    }

    /// A two-frame view whose frames differ in cell (0,0), so which one the
    /// preview drew is readable off the fills.
    fn two_frame_view() -> SpriteEditorView {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        app.handle_key(GameKey::Char('2'));
        app.handle_key(GameKey::Char(' '));
        app.sprite_editor_view().expect("open")
    }

    #[test]
    fn a_two_frame_sprite_draws_both_tab_labels_and_its_speed() {
        let mut app = sprite_forge_app();
        open_editor(&mut app, 0);
        app.handle_key(GameKey::Char('2'));
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        for label in ["[1] frame 1", "[2] frame 2", "600 ms"] {
            assert!(drawn.iter().any(|t| t.contains(label)), "{label:?}");
        }
    }

    #[test]
    fn the_speed_controls_draw_dim_on_a_one_frame_sprite_only() {
        let m = crate::text::ui_metrics(900.0);
        let dim_texts = |frames2: bool| {
            let mut app = sprite_forge_app();
            open_editor(&mut app, 0);
            if frames2 {
                app.handle_key(GameKey::Char('2'));
            }
            let (_, shapes) = crate::paint::with_painter(|p| draw_sprite_editor(&app, p, &m));
            crate::paint::painted_runs_in(&shapes, TEXT_OFF, false)
        };
        let one = dim_texts(false);
        for want in ["[-] slower", "[=] faster", "600 ms"] {
            assert!(
                one.iter().any(|t| t.contains(want)),
                "{want:?} not dim: {one:?}"
            );
        }
        assert!(
            dim_texts(true).is_empty(),
            "a sheet's speed controls are live"
        );
    }

    #[test]
    fn the_preview_draws_the_frame_sprite_frame_picks() {
        let view = two_frame_view();
        assert_ne!(view.frame_cells[0][0], view.frame_cells[1][0]);
        let rect = Rect::new(500.0, 40.0, 32.0, 32.0);
        let first_cell_fill = |now: Option<f64>| {
            let (_, shapes) = crate::paint::with_sprites_at(Default::default(), now, |p| {
                draw_preview_cell(p, rect, &view, WHITE)
            });
            // The first pixel drawn after the background panel is cell (0,0).
            shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    bevy_egui::egui::Shape::Rect(r) if r.stroke.width == 0.0 => Some(r.fill),
                    _ => None,
                })
                .nth(1)
        };
        let frame_ms = f64::from(view.frame_ms) / 1000.0;
        let on_first = first_cell_fill(Some(0.0));
        let on_second = first_cell_fill(Some(frame_ms));
        assert_eq!(crate::paint::sprite_frame(0.0, 0, 2, view.frame_ms), 0);
        assert_eq!(crate::paint::sprite_frame(frame_ms, 0, 2, view.frame_ms), 1);
        assert_ne!(on_first, on_second, "the preview must follow the clock");
        assert_eq!(on_first, first_cell_fill(None), "no clock shows frame one");
    }

    /// The frame column sits inside the window, clear of the canvas, the
    /// palette, the preview and each other, at every zoom the map offers.
    #[test]
    fn the_frame_buttons_fit_beside_the_canvas_without_overlap() {
        let m = crate::text::ui_metrics(CENSUS_H);
        let apart = |a: &Rect, b: &Rect| {
            a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y
        };
        for zoom in 0..=4 {
            crate::paint::with_painter(|p| {
                let g = editor_geometry(p, CENSUS_W, &m, &forge_view(), zoom);
                for (i, (b, r)) in g.buttons.iter().enumerate() {
                    assert!(
                        r.x >= 0.0 && r.x + r.w <= CENSUS_W,
                        "zoom {zoom}: {b:?} off the window"
                    );
                    assert!(
                        r.y + r.h <= g.footer_y,
                        "zoom {zoom}: {b:?} past the footer"
                    );
                    for (c, q) in g.buttons.iter().skip(i + 1) {
                        assert!(apart(r, q), "zoom {zoom}: {b:?} overlaps {c:?}");
                    }
                    if i >= BAR_BUTTONS {
                        for (name, other) in [
                            ("canvas", &g.canvas),
                            ("palette", &g.palette),
                            ("preview", &g.preview),
                        ] {
                            assert!(apart(r, other), "zoom {zoom}: {b:?} overlaps the {name}");
                        }
                    }
                }
            });
        }
    }
}
