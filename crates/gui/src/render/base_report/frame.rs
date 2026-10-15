//! The hub's chrome: the page rail, the title line with its screen dots, and
//! the keybar. Returns the rect the screen's rows go in.

use super::manifest::elided_to_fit;
use super::manifest::keybar::{draw_groups, group};
use super::manifest::layout::{RailGeom, frame_rect, rail_window};
use super::*;
use feral_processes_engine::BaseReportFlags;
use hud::log_frame::keycap;

/// The rail's width in cells: a caret, the longest page title and the `!`.
pub(in crate::render) const RAIL_CELLS: usize = 22;

/// Where the parts of the frame go. Pure over the metrics so a test can
/// check it without a window.
pub(super) struct HubLayout {
    pub(super) rail: RailGeom,
    pub(super) title: Rect,
    pub(super) content: Rect,
    pub(super) keybar: Rect,
}

pub(super) fn hub_layout(frame: Rect, m: &Metrics, char_w: f32) -> HubLayout {
    let inner = Rect::new(
        frame.x + m.pad,
        frame.y + m.pad,
        frame.w - m.pad * 2.0,
        frame.h - m.pad * 2.0,
    );
    let keybar = Rect::new(
        inner.x,
        inner.y + inner.h - m.line_height,
        inner.w,
        m.line_height,
    );
    let body_bottom = keybar.y - m.gap;
    let rail_rect = Rect::new(
        inner.x,
        inner.y,
        RAIL_CELLS as f32 * char_w,
        body_bottom - inner.y,
    );
    let title_row = Rect::new(rail_rect.x, rail_rect.y, rail_rect.w, m.line_height);
    let rail = RailGeom {
        full: true,
        rect: rail_rect,
        title: Some(title_row),
        rows: Rect::new(
            rail_rect.x,
            rail_rect.y + m.line_height,
            rail_rect.w,
            body_bottom - rail_rect.y - m.line_height,
        ),
        row_h: m.line_height,
    };
    let main_x = rail_rect.x + rail_rect.w + m.pad;
    let main_w = inner.x + inner.w - main_x;
    let title = Rect::new(main_x, inner.y, main_w, m.line_height);
    let content_y = title.y + title.h + m.gap;
    HubLayout {
        rail,
        title,
        content: Rect::new(main_x, content_y, main_w, body_bottom - content_y),
        keybar,
    }
}

/// Paints the frame (its keybar only when `keybar_mode` is given) and returns the content rect, already shortened by a
/// red refusal line when one is up. `on_rail` is whether the rail holds the
/// hub's one cursor; the rows draw it otherwise. `flags` is the base
/// report's attention marks; a hub without any passes `None`.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_frame(
    title: &str,
    pages: &[HubPage],
    page: usize,
    screen: usize,
    keybar_mode: Option<Mode>,
    on_rail: bool,
    flags: Option<&BaseReportFlags>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) -> Rect {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let frame = frame_rect(painter.screen_w(), painter.screen_h());
    let l = hub_layout(frame, m, char_w);

    painter.rect(frame.x, frame.y, frame.w, frame.h, hud::palette::STATUS_BG);
    painter.rect_lines(
        frame.x,
        frame.y,
        frame.w,
        frame.h,
        2.0,
        hud::palette::PANE_BORDER,
    );
    draw_rail(
        &l.rail, title, pages, page, on_rail, flags, char_w, painter, m,
    );
    draw_title(&l.title, pages, page, screen, painter, m);
    if let Some(mode) = keybar_mode {
        draw_groups(
            l.keybar,
            &keybar_groups(mode, pages, page, on_rail),
            painter,
            m,
        );
    }

    let mut content = l.content;
    if let Some(text) = refusal {
        painter.ui(
            elided_to_fit(painter, text, m.font_size, content.w - m.pad * 2.0),
            content.x + m.pad,
            content.y + m.font_size as f32,
            m.font_size,
            RED,
        );
        content.y += m.line_height;
        content.h = (content.h - m.line_height).max(0.0);
    }
    content
}

/// The current page always carries the caret; the highlight bar is the
/// cursor, drawn only while the rail has it and in the rows' own
/// `SELECT_BG`, so it reads as the one cursor moving rather than a second.
#[allow(clippy::too_many_arguments)]
fn draw_rail(
    rail: &RailGeom,
    hub_title: &str,
    pages: &[HubPage],
    selected: usize,
    focused: bool,
    flags: Option<&BaseReportFlags>,
    char_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    painter.rect(
        rail.rect.x + rail.rect.w + m.pad / 2.0,
        rail.rect.y,
        1.0,
        rail.rect.h,
        hud::palette::DIVIDER,
    );
    if let Some(title) = rail.title {
        painter.ui_bold(
            hub_title,
            title.x,
            title.y + title.h * 0.75,
            m.small(),
            hud::palette::PANE_TITLE,
        );
    }
    let w = rail_window(pages.len(), selected, rail.fits());
    let mut slot = 0;
    let more = |slot: usize, arrow: char, n: usize| {
        painter.ui(
            format!("{arrow} {n} more"),
            rail.rows.x + char_w,
            rail.rows.y + slot as f32 * rail.row_h + rail.row_h * 0.85,
            m.small(),
            hud::palette::FAINT,
        );
    };
    if w.above > 0 {
        more(slot, '↑', w.above);
        slot += 1;
    }
    for (page, entry) in pages.iter().enumerate().take(w.end).skip(w.start) {
        let top = rail.rows.y + slot as f32 * rail.row_h;
        let base = top + rail.row_h * 0.85;
        let current = page == selected;
        if current && focused {
            painter.rect(rail.rows.x, top, rail.rows.w, rail.row_h, SELECT_BG);
        }
        if current {
            painter.ui(
                "▸",
                rail.rows.x,
                base,
                m.font_size,
                hud::palette::PANE_TITLE,
            );
        }
        painter.ui(
            entry.title,
            rail.rows.x + char_w * 2.0,
            base,
            m.font_size,
            if current {
                hud::palette::PANE_TITLE
            } else {
                hud::palette::EMPHASIS
            },
        );
        if flags.is_some_and(|f| page_flag(f, page).is_some()) {
            painter.ui_bold(
                "!",
                rail.rows.x + rail.rows.w - char_w * 2.0,
                base,
                m.font_size,
                hud::palette::ATTENTION,
            );
        }
        slot += 1;
    }
    if w.below > 0 {
        more(slot, '↓', w.below);
    }
}

/// The page's title, then one dot per screen with the current one filled.
/// A one-screen page draws no dots.
fn draw_title(
    title: &Rect,
    pages: &[HubPage],
    page: usize,
    screen: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let base = title.y + title.h * 0.75;
    let name = pages[page].title;
    painter.ui_bold(name, title.x, base, m.title(), hud::palette::PANE_TITLE);
    let screens = pages[page].screens.len();
    if screens > 1 {
        let dots: Vec<&str> = (0..screens)
            .map(|i| if i == screen { "●" } else { "○" })
            .collect();
        let x = title.x + painter.measure_ui_advance(name, m.title()) + m.pad;
        painter.ui(dots.join(" "), x, base, m.font_size, hud::palette::FAINT);
    }
}

/// The keys that work here, most important first (the bar drops groups from
/// the end when it is too narrow). The rail and the rows each list only
/// what the cursor does where it is.
pub(super) fn keybar_groups(
    mode: Mode,
    pages: &[HubPage],
    page: usize,
    on_rail: bool,
) -> Vec<Vec<hud::strip::Piece>> {
    if on_rail {
        let nav = vec![keycap("↑/↓", "page"), keycap("→", "open")];
        return [nav, vec![keycap("Esc", "close")]]
            .into_iter()
            .map(group)
            .collect();
    }
    let step = if pages[page].screens.len() > 1 {
        keycap("←/→", "screen")
    } else {
        keycap("←", "menu")
    };
    let nav = vec![step, keycap("[ ]", "page")];
    let acts = match mode {
        Mode::BaseOverview => vec![keycap("↑/↓", "select"), keycap("Enter", "open")],
        Mode::Alerts => vec![keycap("↑/↓", "select"), keycap("x/d", "dismiss")],
        Mode::StaffWellbeing
        | Mode::StaffCoverage
        | Mode::StaffDiscontent
        | Mode::BaseMachines
        | Mode::BasePower
        | Mode::BaseStock
        | Mode::BuildOrders
        | Mode::DigOrders
        | Mode::FloorOrders => vec![keycap("↑/↓", "scroll")],
        Mode::WorkOrders => vec![
            keycap("↑/↓", "select"),
            keycap("Enter", "queue"),
            keycap("Bksp", "drop"),
            keycap("< >", "move"),
        ],
        Mode::BaseStaff => vec![
            keycap("↑/↓ ←/→", "cell"),
            keycap("Space", "toggle"),
            keycap("A", "column"),
            keycap("< >", "move"),
        ],
        Mode::Structures => vec![keycap("↑/↓", "select"), keycap("Enter", "staff")],
        Mode::Inventory => vec![
            keycap("↑/↓", "select"),
            keycap("S", "sell"),
            keycap("U", "fuse all"),
            keycap("I", "inspect"),
            keycap("D", "downed"),
        ],
        Mode::PhaseKeys => vec![keycap("↑/↓", "read")],
        Mode::DownedPrograms => vec![keycap("↑/↓", "select"), keycap("Enter", "extract")],
        Mode::Tools => vec![
            keycap("F", "forge"),
            keycap("I", "install"),
            keycap("X", "pull"),
        ],
        _ => Vec::new(),
    };
    [acts, nav, vec![keycap("Esc", "menu")]]
        .into_iter()
        .filter(|g| !g.is_empty())
        .map(group)
        .collect()
}

/// The content rect at a window size, for the censuses that hold a screen's
/// rows against the room the hub gives them.
#[cfg(test)]
pub(in crate::render) fn content_rect_at(
    screen_w: f32,
    screen_h: f32,
    char_w: f32,
    m: &Metrics,
) -> Rect {
    hub_layout(frame_rect(screen_w, screen_h), m, char_w).content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    /// Rail, title, content and keybar tile the frame without overlapping or
    /// escaping it, at every window height the game supports.
    #[test]
    fn the_hub_layout_stays_inside_its_frame_and_overlaps_nothing() {
        with_painter(|p| {
            for h in (600..=2160).step_by(60) {
                let h = h as f32;
                let w = h * 16.0 / 9.0;
                let m = ui_metrics(h);
                let char_w = p.measure_ui_advance("M", m.font_size);
                let frame = frame_rect(w, h);
                let l = hub_layout(frame, &m, char_w);
                let inside = |r: &Rect| {
                    r.x >= frame.x
                        && r.y >= frame.y
                        && r.x + r.w <= frame.x + frame.w + 0.5
                        && r.y + r.h <= frame.y + frame.h + 0.5
                };
                for r in [&l.rail.rect, &l.title, &l.content, &l.keybar] {
                    assert!(inside(r), "a hub box escapes its frame at {h}px");
                }
                assert!(
                    l.content.w > 0.0 && l.content.h > 0.0,
                    "no content at {h}px"
                );
                assert!(
                    l.rail.rect.x + l.rail.rect.w < l.content.x,
                    "rail overlaps content at {h}px"
                );
                assert!(
                    l.content.y + l.content.h <= l.keybar.y,
                    "content overlaps keybar at {h}px"
                );
                assert!(
                    l.title.y + l.title.h <= l.content.y,
                    "title overlaps content at {h}px"
                );
            }
        });
    }

    #[test]
    fn the_keybar_names_the_way_out_of_each_half_on_every_screen() {
        for mode in [
            Mode::BaseOverview,
            Mode::StaffWellbeing,
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
            Mode::BaseOutput,
            Mode::BaseMachines,
            Mode::BasePower,
            Mode::BaseStock,
            Mode::BuildOrders,
            Mode::DigOrders,
            Mode::FloorOrders,
            Mode::WorkOrders,
            Mode::BaseStaff,
            Mode::Structures,
            Mode::Alerts,
        ] {
            let page = hub_of(mode).unwrap().1;
            let text = |on_rail| -> String {
                keybar_groups(mode, BASE_REPORT_PAGES, page, on_rail)
                    .iter()
                    .flatten()
                    .map(|(t, _, _)| t.as_str())
                    .collect()
            };
            let rail = text(true);
            assert!(rail.contains("page"), "{mode:?}: {rail}");
            assert!(rail.contains("open"), "{mode:?}: {rail}");
            assert!(rail.contains("close"), "{mode:?}: {rail}");
            let rows = text(false);
            assert!(rows.contains("menu"), "{mode:?}: {rows}");
        }
    }
}
