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
use crate::paint::{Color, Painter, Rect};

const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);

/// How wide the prose runs inside the panel, `notify`'s measure.
const BODY_WIDTH_FRACTION: f32 = 0.84;

/// The last line of a page cut to fit.
const CUT_MARK: &str = "...";

fn hint(page: usize, pages: usize) -> String {
    let next = match page + 1 < pages {
        true => "Enter/Right: next",
        false => "Enter/Right: continue",
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

/// A page ready to draw: the title wrapped to the panel and the body cut to
/// what the panel holds. A cut body ends in the line `...`.
struct Page {
    title: Vec<String>,
    body: Vec<String>,
}

/// The block's height, title to hint. **The one sum** the draw centres on
/// and the census measures.
fn block_height(painter: &Painter, m: &Metrics, title: &[String], lines: usize) -> f32 {
    let title_size = m.title() + 6;
    title
        .iter()
        .map(|line| painter.measure_ui(line, title_size).height)
        .sum::<f32>()
        + m.gap
        + lines as f32 * m.line_height
        + m.gap * 2.0
        + painter.measure_ui("M", m.small()).height
}

/// Wraps the title and cuts the body with an ellipsis rather than let a long
/// modded page run off the panel - the screen has no scroll. Shipped pages
/// are held un-cut by `every_shipped_ending_screen_fits`.
fn lay_out(painter: &Painter, m: &Metrics, panel: Rect, screen: &EndingScreen) -> Page {
    let columns = body_columns(painter, panel, m.font_size);
    let title_columns = (columns as f32 * m.font_size as f32 / (m.title() + 6) as f32) as usize;
    let title = text::wrap(&screen.title, title_columns.max(1));
    let mut body = wrapped(screen, columns);
    let room = panel.h - 2.0 * m.pad;
    let mut cut = false;
    while block_height(painter, m, &title, body.len()) > room && !body.is_empty() {
        body.pop();
        cut = true;
    }
    if cut {
        body.pop();
        body.push(CUT_MARK.to_string());
    }
    Page { title, body }
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
    let laid = lay_out(painter, m, panel, screen);
    let block = block_height(painter, m, &laid.title, laid.body.len());
    let mut y = panel.y + ((panel.h - block) / 2.0).max(m.pad);

    for line in &laid.title {
        let dims = painter.measure_ui(line, title_size);
        painter.ui(
            line,
            centre_x(dims.width),
            y + dims.height,
            title_size,
            CYAN,
        );
        y += dims.height;
    }
    y += m.gap;

    let left = centre_x(panel.w * BODY_WIDTH_FRACTION);
    for line in &laid.body {
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
            for screen in ending.screens() {
                let page = lay_out(p, &m, panel, screen);
                assert!(
                    page.body.last().map(String::as_str) != Some(CUT_MARK),
                    "{:?} does not fit the {}px panel ({} lines) — no scroll, so cut it",
                    screen.title,
                    panel.h,
                    page.body.len()
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

    /// A modded page far taller and wider than the panel is wrapped and cut
    /// to fit instead of running off it.
    #[test]
    fn an_oversized_page_is_wrapped_and_cut_to_the_panel() {
        let screen = EndingScreen {
            title: "A very long modded ending title ".repeat(12),
            body: (0..60)
                .map(|i| format!("Paragraph {i} of a body that goes on and on. ").repeat(6))
                .collect(),
        };
        let m = crate::text::ui_metrics(720.0);
        let panel = panel_rect(1280.0, 720.0);
        crate::paint::with_painter(|p| {
            let page = lay_out(p, &m, panel, &screen);
            assert!(page.title.len() > 1, "the title wraps: {:?}", page.title);
            assert_eq!(page.body.last().map(String::as_str), Some(CUT_MARK));
            let block = block_height(p, &m, &page.title, page.body.len());
            assert!(block + 2.0 * m.pad <= panel.h, "{block}px in {}px", panel.h);
            draw_ending(std::slice::from_ref(&screen), 0, p, &m);
        });
    }

    #[test]
    fn the_hint_names_the_last_page() {
        assert!(hint(0, 3).contains("next"));
        assert!(hint(2, 3).contains("continue"));
    }
}
