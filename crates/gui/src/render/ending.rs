//! The ending (`Mode::Ending`) and the question that precedes it
//! (`Mode::BasinExitConfirm`).
//!
//! The ending is `notify.rs`'s arrangement: a scrim, a bordered panel over
//! the map, prose wrapped at the panel's measure, no `draw_popup` and so no
//! refusal of its own (`needs_status_banner` carries one raised underneath).
//! It shares that file's panel and column helpers rather than keeping its
//! own, so the two cannot disagree about how wide a paragraph may run. No
//! scroll: `every_shipped_ending_screen_fits` holds the height.

use feral_processes_engine::story::EndingScreen;
use feral_processes_engine::text;

use super::notify::{body_columns, panel_rect};
use super::popup::*;
use super::{BORDER, CYAN, Metrics, PANEL_BG, TEXT, TEXT_DIM};
use crate::paint::{Color, Painter};

const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);

/// How wide the prose runs inside the panel, `notify`'s measure.
const BODY_WIDTH_FRACTION: f32 = 0.84;

fn hint(page: usize, pages: usize) -> String {
    let next = match page + 1 < pages {
        true => "Enter: next",
        false => "Enter: continue",
    };
    format!("{}/{}   {next}   Left: back   Esc: skip", page + 1, pages)
}

/// Paragraphs wrapped to `columns`, a blank line between them. `text::wrap`
/// has no notion of a paragraph, so each body string wraps on its own.
fn wrapped(screen: &EndingScreen, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, para) in screen.body.iter().enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.extend(text::wrap(para, columns));
    }
    lines
}

/// The block's height, title to hint. **The one sum** the draw centres on
/// and the census measures.
fn block_height(painter: &Painter, m: &Metrics, screen: &EndingScreen, lines: usize) -> f32 {
    painter.measure_ui(&screen.title, m.title() + 6).height
        + m.gap
        + lines as f32 * m.line_height
        + m.gap * 2.0
        + painter.measure_ui("M", m.small()).height
}

pub(super) fn draw_ending(screens: &[EndingScreen], page: usize, painter: &Painter, m: &Metrics) {
    let Some(screen) = screens.get(page) else {
        return;
    };
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, SCRIM);
    let panel = panel_rect(w, h);
    painter.rect(panel.x, panel.y, panel.w, panel.h, PANEL_BG);
    painter.rect_lines(panel.x, panel.y, panel.w, panel.h, 2.0, BORDER);

    let title_size = m.title() + 6;
    let centre_x = |width: f32| panel.x + (panel.w - width) / 2.0;
    let columns = body_columns(painter, panel, m.font_size);
    let lines = wrapped(screen, columns);
    let block = block_height(painter, m, screen, lines.len());
    let mut y = panel.y + ((panel.h - block) / 2.0).max(m.pad);

    let title = painter.measure_ui(&screen.title, title_size);
    painter.ui(
        &screen.title,
        centre_x(title.width),
        y + title.height,
        title_size,
        CYAN,
    );
    y += title.height + m.gap;

    let left = centre_x(panel.w * BODY_WIDTH_FRACTION);
    for line in &lines {
        y += m.line_height;
        painter.ui(line, left, y, m.font_size, TEXT);
    }

    let hint = hint(page, screens.len());
    let dims = painter.measure_ui(&hint, m.small());
    y += m.gap * 2.0 + dims.height;
    painter.ui(&hint, centre_x(dims.width), y, m.small(), TEXT_DIM);
}

pub(super) fn draw_basin_exit_confirm(
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = vec![
        text_row("Leave the Phase-Manifold Basin?"),
        text_row("(The game continues afterward.)"),
        text_row(""),
        item_row("[y] Yes, step through".to_string(), selected == 0),
        item_row("[n] No, stay".to_string(), selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    draw_popup("Basin Exit", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::story::EndingText;

    /// **The screen has no scroll**, so the height of every shipped page is
    /// held at the smallest window the game is built for.
    #[test]
    fn every_shipped_ending_screen_fits() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/story");
        let (ending, warnings) = EndingText::load_dir(&assets).expect("shipped story");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_ne!(ending, EndingText::default(), "the shipped file was read");
        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let panel = panel_rect(1280.0, 720.0);
            let columns = body_columns(p, panel, m.font_size);
            for screen in ending.screens() {
                let lines = wrapped(screen, columns);
                let block = block_height(p, &m, screen, lines.len());
                assert!(
                    block + 2.0 * m.pad < panel.h,
                    "{:?} is {block}px in a {}px panel ({} lines) — no scroll, so cut it",
                    screen.title,
                    panel.h,
                    lines.len()
                );
            }
        });
    }

    /// The built-in ending is the one a modder's mistake lands on.
    #[test]
    fn the_fallback_ending_fits_and_paints() {
        let fallback = EndingText::default();
        let m = crate::text::ui_metrics(720.0);
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_ending(fallback.screens(), 0, p, &m);
        });
        let drawn = crate::paint::painted_text(&shapes);
        assert!(drawn.iter().any(|t| t.contains("Escaped")), "{drawn:?}");
    }

    #[test]
    fn the_hint_names_the_last_page() {
        assert!(hint(0, 3).contains("next"));
        assert!(hint(2, 3).contains("continue"));
    }
}
