//! The player's 16x16 icon editor screen.
//!
//! Drawn instead of the wizard's own popup while `App::icon_editor_view` is
//! `Some` (see `render::draw`'s dispatch) — the wizard's Icon step opens
//! this, `Enter`/`Esc` inside it close it, and app-core's `IconEditor` is
//! the only thing that changes what it draws (`crates/app-core/src/app/
//! icon_editor.rs`).
//!
//! **The canvas is rectangles, not a texture.** A 16x16 grid needs a
//! per-cell rect anyway for the grid lines and the cursor, and drawing it
//! that way is what keeps a texture from being minted on every keystroke —
//! `paint::Painter::sprite`'s doc comment is the one texture this feature
//! ever uploads, and it is Task 8's, not this screen's. Index 0 (transparent)
//! draws `super::SCREEN_BG` — the same colour `draw` clears the window to —
//! rather than black, so a hole in the drawing reads as a hole and not as a
//! pixel the player painted on purpose.

use super::*;
use feral_processes_app_core::{IconEditorView, IconFocus};
use feral_processes_engine::{ICON_PALETTE, ICON_SIZE};

/// A canvas cell's side, in `Metrics::line_height` units.
const CANVAS_CELL_LINES: f32 = 1.2;
/// A palette swatch's side, in the same units. Smaller than a canvas cell —
/// the strip is fifteen wide and has to sit under the canvas without
/// growing past it.
const SWATCH_LINES: f32 = 1.4;
/// Gap between two palette swatches, in `Metrics::line_height` units.
const SWATCH_GAP_LINES: f32 = 0.3;
/// Vertical gap between the screen's blocks (header, each panel's caption,
/// the panel itself, the footer), in `Metrics::line_height` units.
const SECTION_GAP_LINES: f32 = 1.0;

/// An unfocused panel's border thickness and colour. `super::BORDER` is the
/// focused one, reused rather than a second cyan, so "this panel is
/// listening" reads as the same colour everywhere else it does.
const UNFOCUSED_BORDER_THICKNESS: f32 = 1.0;
const UNFOCUSED_BORDER: Color = TEXT_DIM;
/// The focused panel's border is thicker as well as a different colour, so
/// focus is never carried by hue alone.
const FOCUSED_BORDER_THICKNESS: f32 = 3.0;

/// Grid lines between cells — dim enough that 256 of them read as texture,
/// not as 256 more things to look at.
const GRID_LINE: Color = Color::new(0.18, 0.20, 0.24, 1.0);

/// The canvas cursor's outline.
const CURSOR_COLOR: Color = WHITE;
const CURSOR_THICKNESS: f32 = 2.0;
/// The selected swatch's outline.
const SELECTED_SWATCH_COLOR: Color = WHITE;
const SELECTED_SWATCH_THICKNESS: f32 = 2.0;

const HEADER_TEXT: &str = "Draw Your Icon";

/// Names all eight bound keys — `handle_key`'s whole table but `u`/`x`,
/// which are unconditional there too. Colons make each token an
/// unambiguous substring for a test to look for.
const FOOTER_TEXT: &str = "Tab: switch panel   Arrows: move   Space: paint   \
    Backspace: erase   u: undo   x: clear   Enter: keep   Esc: discard";

/// Everything geometric about the screen, computed once so drawing and the
/// layout census below share one derivation — `frame_map::layout`'s
/// pattern for the same reason: two copies of this arithmetic is how a
/// census could pass against a screen that no longer draws where it says.
struct Geometry {
    header_y: f32,
    canvas_label_y: f32,
    canvas: Rect,
    cell: f32,
    palette_label_y: f32,
    palette: Rect,
    swatch: f32,
    swatch_gap: f32,
    footer_y: f32,
    footer_lines: Vec<String>,
}

fn geometry(painter: &Painter, m: &Metrics) -> Geometry {
    let w = painter.screen_w();
    let gap = m.line_height * SECTION_GAP_LINES;

    let header_y = m.pad + m.line_height;

    let canvas_label_y = header_y + gap + m.line_height;
    let cell = m.line_height * CANVAS_CELL_LINES;
    let canvas_side = cell * ICON_SIZE as f32 + m.inset * 2.0;
    let canvas = Rect::new(
        (w - canvas_side) / 2.0,
        canvas_label_y + m.gap,
        canvas_side,
        canvas_side,
    );

    let palette_label_y = canvas.y + canvas.h + gap + m.line_height;
    let swatch = m.line_height * SWATCH_LINES;
    let swatch_gap = m.line_height * SWATCH_GAP_LINES;
    let n = ICON_PALETTE.len() as f32;
    let palette_w = swatch * n + swatch_gap * (n - 1.0) + m.inset * 2.0;
    let palette_h = swatch + m.inset * 2.0;
    let palette = Rect::new(
        (w - palette_w) / 2.0,
        palette_label_y + m.gap,
        palette_w,
        palette_h,
    );

    let footer_top = palette.y + palette.h + gap;
    // Measured in UI cells, `notify.rs::draw_notification`'s pattern for a
    // screen with no popup body to wrap against.
    let columns = ((w - m.pad * 2.0) / painter.measure_ui_advance("M", m.small()))
        .floor()
        .max(20.0) as usize;
    let footer_lines = feral_processes_engine::text::wrap(FOOTER_TEXT, columns);

    Geometry {
        header_y,
        canvas_label_y,
        canvas,
        cell,
        palette_label_y,
        palette,
        swatch,
        swatch_gap,
        footer_y: footer_top + m.line_height,
        footer_lines,
    }
}

/// Total height the screen draws, footer included — the layout census's one
/// number. `#[cfg(test)]` because nothing outside the census reads it: the
/// drawing itself walks `Geometry` field by field.
#[cfg(test)]
fn content_bottom(g: &Geometry, m: &Metrics) -> f32 {
    g.footer_y + g.footer_lines.len().saturating_sub(1) as f32 * m.line_height + m.pad
}

pub(super) fn draw_icon_editor(view: &IconEditorView, painter: &Painter, m: &Metrics) {
    let g = geometry(painter, m);
    draw_header(painter, &g, m);
    draw_canvas(view, painter, &g, m);
    draw_palette(view, painter, &g, m);
    draw_footer(painter, &g, m);
}

fn centered_ui(painter: &Painter, text: &str, y: f32, size: u16, color: Color) {
    let width = painter.measure_ui(text, size).width;
    painter.ui(text, (painter.screen_w() - width) / 2.0, y, size, color);
}

fn draw_header(painter: &Painter, g: &Geometry, m: &Metrics) {
    centered_ui(painter, HEADER_TEXT, g.header_y, m.title(), TEXT);
}

/// A canvas pixel's colour: the screen's own background for index 0
/// (transparent), or the palette entry a drawn pixel names. `PlayerIcon::
/// set` never stores an index past `ICON_PALETTE`'s length, so the
/// non-zero arm cannot go out of bounds.
fn cell_color(index: u8) -> Color {
    match index {
        0 => SCREEN_BG,
        n => palette_color(ICON_PALETTE[n as usize - 1]),
    }
}

fn palette_color((r, g, b): (u8, u8, u8)) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

/// A panel's border thickness and colour for whether it currently has
/// focus — the one channel `view.focus` is drawn on.
fn panel_border(focused: bool) -> (f32, Color) {
    if focused {
        (FOCUSED_BORDER_THICKNESS, BORDER)
    } else {
        (UNFOCUSED_BORDER_THICKNESS, UNFOCUSED_BORDER)
    }
}

fn draw_canvas(view: &IconEditorView, painter: &Painter, g: &Geometry, m: &Metrics) {
    centered_ui(painter, "Canvas", g.canvas_label_y, m.small(), TEXT_DIM);

    let c = g.canvas;
    painter.rect(c.x, c.y, c.w, c.h, PANEL_BG);
    let (thickness, color) = panel_border(view.focus == IconFocus::Canvas);
    painter.rect_lines(c.x, c.y, c.w, c.h, thickness, color);

    let ox = c.x + m.inset;
    let oy = c.y + m.inset;
    let side = ICON_SIZE as f32 * g.cell;
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let idx = view.pixels[y * ICON_SIZE + x];
            let (px, py) = (ox + x as f32 * g.cell, oy + y as f32 * g.cell);
            painter.rect(px, py, g.cell, g.cell, cell_color(idx));
        }
    }
    for i in 0..=ICON_SIZE {
        let x = ox + i as f32 * g.cell;
        painter.line(x, oy, x, oy + side, 1.0, GRID_LINE);
        let y = oy + i as f32 * g.cell;
        painter.line(ox, y, ox + side, y, 1.0, GRID_LINE);
    }

    let (cx, cy) = (view.cursor.0 as f32, view.cursor.1 as f32);
    painter.rect_lines(
        ox + cx * g.cell,
        oy + cy * g.cell,
        g.cell,
        g.cell,
        CURSOR_THICKNESS,
        CURSOR_COLOR,
    );
}

fn draw_palette(view: &IconEditorView, painter: &Painter, g: &Geometry, m: &Metrics) {
    centered_ui(painter, "Palette", g.palette_label_y, m.small(), TEXT_DIM);

    let p = g.palette;
    painter.rect(p.x, p.y, p.w, p.h, PANEL_BG);
    let (thickness, color) = panel_border(view.focus == IconFocus::Palette);
    painter.rect_lines(p.x, p.y, p.w, p.h, thickness, color);

    let ox = p.x + m.inset;
    let oy = p.y + m.inset;
    for (i, &rgb) in ICON_PALETTE.iter().enumerate() {
        let x = ox + i as f32 * (g.swatch + g.swatch_gap);
        painter.rect(x, oy, g.swatch, g.swatch, palette_color(rgb));
        // `selected` is 1-based — index 0 means transparent and is not a
        // swatch, see `app::icon_editor::FIRST_COLOUR`.
        if view.selected as usize == i + 1 {
            painter.rect_lines(
                x,
                oy,
                g.swatch,
                g.swatch,
                SELECTED_SWATCH_THICKNESS,
                SELECTED_SWATCH_COLOR,
            );
        }
    }
}

fn draw_footer(painter: &Painter, g: &Geometry, m: &Metrics) {
    let mut y = g.footer_y;
    for line in &g.footer_lines {
        centered_ui(painter, line, y, m.small(), TEXT_DIM);
        y += m.line_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::GameKey;

    fn blank_view() -> IconEditorView {
        IconEditorView {
            pixels: [0; ICON_SIZE * ICON_SIZE],
            cursor: (0, 0),
            selected: 1,
            focus: IconFocus::Canvas,
        }
    }

    /// **The layout census this task exists for.** Every block — header,
    /// both panel captions, the canvas, the palette and the whole
    /// (possibly wrapped) footer — must fit inside 1280x720 with no
    /// scroll, since this screen has none.
    ///
    /// **Verified by mutation**: bumping `CANVAS_CELL_LINES` from 1.2 to
    /// 3.0 turns this red (measured 1252.7px against a 720px window; see
    /// the task's own report for the transcript), then it was reverted.
    #[test]
    fn the_screen_fits_at_1280x720() {
        let m = crate::text::ui_metrics(720.0);
        let ((bottom, lines), _shapes) = crate::paint::with_painter(|p| {
            let g = geometry(p, &m);
            (content_bottom(&g, &m), g.footer_lines.len())
        });
        assert!(
            bottom < 720.0,
            "the icon editor draws {bottom}px of content ({lines} footer lines) \
             in a 720px window — this screen has no scroll"
        );
    }

    /// Every block sits inside the window horizontally too — the axis the
    /// height census above cannot see.
    #[test]
    fn the_screen_fits_1280_wide() {
        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let g = geometry(p, &m);
            for (name, rect) in [("canvas", g.canvas), ("palette", g.palette)] {
                assert!(
                    rect.x >= 0.0 && rect.x + rect.w <= 1280.0,
                    "{name} panel runs off a 1280px window: {rect:?}"
                );
            }
            for line in &g.footer_lines {
                let width = p.measure_ui_advance(line, m.small());
                assert!(
                    width <= 1280.0,
                    "a footer line is {width}px wide in a 1280px window: {line:?}"
                );
            }
        });
    }

    /// Transparent cells (index 0) must draw the screen's own background,
    /// never black — the bug the brief calls the most likely one here. A
    /// blank canvas is 256 cells, all of them transparent.
    #[test]
    fn a_blank_canvas_draws_the_screen_background_not_black() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&blank_view(), p, &m));
        assert!(
            crate::paint::painted_rect_fill_count(&shapes, SCREEN_BG) >= ICON_SIZE * ICON_SIZE,
            "every transparent cell must be filled with the screen background"
        );
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, Color::new(0.0, 0.0, 0.0, 1.0)),
            0,
            "no cell may be drawn in flat black — a hole must read as a hole, \
             not as a painted dark pixel"
        );
    }

    /// A painted cell shows its palette colour — once on the canvas and
    /// once more where that same colour sits in the palette strip.
    #[test]
    fn a_painted_cell_shows_its_palette_colour() {
        let mut view = blank_view();
        // Index 7: away from both ends of the palette and from `selected`'s
        // opening value of 1, so this cannot pass by coincidence.
        view.pixels[0] = 7;
        let want = palette_color(ICON_PALETTE[6]);

        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&view, p, &m));
        assert_eq!(
            crate::paint::painted_rect_fill_count(&shapes, want),
            2,
            "expected exactly the painted cell and its palette swatch in {want:?}"
        );
    }

    /// The cursor is marked on the canvas.
    #[test]
    fn the_cursor_is_drawn() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&blank_view(), p, &m));
        assert!(
            crate::paint::painted_rect_stroke_count(&shapes, CURSOR_COLOR) >= 1,
            "the cursor must be outlined somewhere on the canvas"
        );
    }

    /// The selected swatch is marked on the palette.
    #[test]
    fn the_selected_swatch_is_drawn() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&blank_view(), p, &m));
        assert!(
            crate::paint::painted_rect_stroke_count(&shapes, SELECTED_SWATCH_COLOR) >= 1,
            "the selected swatch must be outlined somewhere on the palette"
        );
    }

    /// **Which panel has focus is drawn, and it is drawn on the border.**
    /// With the canvas focused, the palette's border is the dim,
    /// unfocused one; with the palette focused, the canvas's is. Both
    /// panels always draw a border, so this is the one test that fails if
    /// focus stops being readable off the screen.
    #[test]
    fn focus_is_shown_on_exactly_one_panels_border() {
        let m = crate::text::ui_metrics(900.0);

        let mut on_canvas = blank_view();
        on_canvas.focus = IconFocus::Canvas;
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&on_canvas, p, &m));
        assert_eq!(
            crate::paint::painted_rect_stroke_count(&shapes, UNFOCUSED_BORDER),
            1,
            "with the canvas focused, exactly the palette's border is dim"
        );

        let mut on_palette = blank_view();
        on_palette.focus = IconFocus::Palette;
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&on_palette, p, &m));
        assert_eq!(
            crate::paint::painted_rect_stroke_count(&shapes, UNFOCUSED_BORDER),
            1,
            "with the palette focused, exactly the canvas's border is dim"
        );
    }

    /// The footer names every one of the editor's eight bound keys —
    /// `IconEditor::handle_key`'s whole table.
    #[test]
    fn the_footer_names_every_bound_key() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&blank_view(), p, &m));
        let drawn = crate::paint::painted_text(&shapes).join(" ");
        for key in [
            "Tab",
            "Arrows",
            "Space",
            "Backspace",
            "u",
            "x",
            "Enter",
            "Esc",
        ] {
            assert!(
                drawn.contains(key),
                "the footer must name {key:?}: {drawn:?}"
            );
        }
    }

    /// The header names the screen.
    #[test]
    fn the_header_is_drawn() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_icon_editor(&blank_view(), p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            drawn.iter().any(|t| t.contains("Icon")),
            "the header must say something about the icon: {drawn:?}"
        );
    }

    /// A fresh app on the main menu with a scratch profile —
    /// `creation.rs::wizard_app`'s fixture, duplicated rather than shared
    /// since it is `#[cfg(test)]`-private to that module.
    fn wizard_app() -> feral_processes_app_core::App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_icon_editor_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut app = feral_processes_app_core::App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.handle_key(GameKey::Char('n'));
        app
    }

    /// Walks a fresh wizard to the Icon step and opens the editor from its
    /// sixth row — the same route `creation.rs`'s own tests take.
    ///
    /// `CreationStep::ALL` puts Icon right after Kit (Points comes later,
    /// after Colour) — see `feral_processes_app_core::lib::CreationStep::
    /// ALL`.
    fn open_the_editor(app: &mut feral_processes_app_core::App) {
        use feral_processes_app_core::CreationStep;
        app.handle_key(GameKey::Char('f')); // Difficulty
        app.handle_key(GameKey::Enter); // Profile
        app.handle_key(GameKey::Char('1')); // Class
        for i in 0..app.creation_rows().len() {
            app.menu_selected = i;
            app.handle_key(GameKey::ShiftRight);
        }
        app.menu_selected = 0;
        app.handle_key(GameKey::Enter); // Kit spent
        assert_eq!(app.creation_step(), CreationStep::Icon);
        while app.menu_selected != app.creation_rows().len() - 1 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        assert!(app.icon_editor_view().is_some(), "the editor did not open");
    }

    /// **The dispatch this task adds.** `render::draw` must draw the
    /// editor screen instead of the wizard's own popup while
    /// `App::icon_editor_view` is `Some`, ahead of `Mode::CreateCharacter`'s
    /// ordinary arm.
    #[test]
    fn the_dispatcher_draws_the_editor_over_the_wizard_once_it_is_open() {
        let mut app = wizard_app();
        open_the_editor(&mut app);
        let mut fx = Fx::new();

        let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p));
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            drawn.iter().any(|t| t.contains("Draw Your Icon")),
            "the editor screen must be drawn once it is open: {drawn:?}"
        );
        assert!(
            !drawn.iter().any(|t| t.contains("Draw your own")),
            "the wizard's own Icon step row must not be drawn underneath: {drawn:?}"
        );
    }
}
