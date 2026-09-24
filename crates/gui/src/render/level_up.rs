//! The level-up summary (`Mode::LevelUp`), drawn over the map the same way
//! `notify.rs` draws a notice: a scrim, a bordered panel, `draw_playing_base`
//! still visible underneath. Not a `draw_popup` — those scroll and take a
//! refusal, and this page has neither: it is registered in
//! `needs_status_banner` instead, `notify.rs`'s own arrangement.
//!
//! Fixed rows, no scroll, so the panel's size is a layout constraint held by
//! a census test over the widest reachable report (`the_widest_report_fits_
//! its_screen`) rather than fitted to the content at draw time.

use feral_processes_engine::LevelUpReport;
use feral_processes_engine::progression::StatRow;

use super::{BORDER, Metrics, PANEL_BG, TEXT, TEXT_DIM};
use crate::paint::{Color, Painter, Rect};

/// How much of the window the panel takes, each way. Narrower than
/// `notify.rs`'s 0.75: that page centres a paragraph, this one is a fixed
/// table of short rows that does not need the width.
const PANEL_W_FRACTION: f32 = 0.5;
const PANEL_H_FRACTION: f32 = 0.62;

/// Drawn behind the panel, `notify.rs`'s own scrim.
const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);

const ESC_HINT: &str = "[Esc] Close";
const PERKS_HINT: &str = "[P] Perks";

fn panel_rect(w: f32, h: f32) -> Rect {
    let (pw, ph) = (w * PANEL_W_FRACTION, h * PANEL_H_FRACTION);
    Rect::new((w - pw) / 2.0, (h - ph) / 2.0, pw, ph)
}

fn stat_line(row: &StatRow) -> String {
    format!("  {}   {} \u{2192} {}", row.label, row.before, row.after)
}

/// The duel section's four rows, against `report.zone`'s typical foe.
/// Percentages are whole numbers, the per-swing figure one decimal — the
/// rest of the page is integers already, and those are the two figures
/// `Game::take_level_up_report` hands back as `f64`.
fn duel_lines(report: &LevelUpReport) -> [String; 4] {
    let (hit_before, hit_after) = report.hit_chance;
    let (swing_before, swing_after) = report.per_swing;
    let (win_before, win_after) = report.swings_to_win;
    let (down_before, down_after) = report.swings_to_down_you;
    [
        format!(
            "  Hit chance      {:.0}% \u{2192} {:.0}%",
            hit_before * 100.0,
            hit_after * 100.0
        ),
        format!("  Per swing       {swing_before:.1} \u{2192} {swing_after:.1}"),
        format!("  Swings to win    {win_before} \u{2192} {win_after}"),
        format!("  Its swings to down you   {down_before} \u{2192} {down_after}"),
    ]
}

/// The whole block's height, art-free unlike `notify.rs`'s — a title line,
/// the stat rows, a gap, the duel header and its four rows, a gap, the
/// spend header and its two rows, a gap, and the hint line. **The one sum**
/// `draw_level_up`'s own `y` walk matches and the census below measures, so
/// it cannot pass against a layout the screen no longer draws.
///
/// Test-only: `draw_level_up` is top-anchored rather than centred, unlike
/// `notify.rs`'s panel, so nothing in the draw path needs this figure ahead
/// of time — only the census does.
#[cfg(test)]
fn block_height(painter: &Painter, m: &Metrics, title: &str, stat_rows: usize) -> f32 {
    let title_h = painter.measure_ui(title, m.title()).height;
    title_h
        + stat_rows as f32 * m.line_height
        + m.gap
        + m.line_height // duel header
        + 4.0 * m.line_height
        + m.gap
        + m.line_height // spend header
        + 2.0 * m.line_height
        + m.gap
        + m.line_height // hint line
}

/// The widest line the page draws, at `report`'s own figures — used only by
/// the census, since the screen itself never needs to know it (every line is
/// left-aligned from the panel's own inset).
#[cfg(test)]
fn widest_line(painter: &Painter, m: &Metrics, report: &LevelUpReport) -> f32 {
    let title = format!("LEVEL {} \u{2192} {}", report.from_level, report.to_level);
    let duel_header = format!("AGAINST A TYPICAL ZONE {} PROGRAM", report.zone);
    let perk_line = format!(
        "  +{} Perk Points ({} unspent)   {PERKS_HINT}",
        report.perk_points_gained, report.perk_points_unspent
    );
    let decompiler_line = format!("  +{} Decompiler skill", report.decompiler_gained);

    let mut widest = painter.measure_ui_advance(&title, m.title());
    for row in &report.stats {
        widest = widest.max(painter.measure_ui_advance(stat_line(row), m.font_size));
    }
    widest = widest.max(painter.measure_ui_advance(&duel_header, m.font_size));
    for line in duel_lines(report) {
        widest = widest.max(painter.measure_ui_advance(&line, m.font_size));
    }
    widest = widest.max(painter.measure_ui_advance("TO SPEND", m.font_size));
    widest = widest.max(painter.measure_ui_advance(&perk_line, m.font_size));
    widest = widest.max(painter.measure_ui_advance(&decompiler_line, m.font_size));
    widest
}

/// Draws `report` in a panel over the map.
pub(super) fn draw_level_up(report: &LevelUpReport, painter: &Painter, m: &Metrics) {
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, SCRIM);
    let panel = panel_rect(w, h);
    painter.rect(panel.x, panel.y, panel.w, panel.h, PANEL_BG);
    painter.rect_lines(panel.x, panel.y, panel.w, panel.h, 2.0, BORDER);

    let left = panel.x + m.pad;
    let right = panel.x + panel.w - m.pad;
    let mut y = panel.y + m.pad;

    let title = format!("LEVEL {} \u{2192} {}", report.from_level, report.to_level);
    let title_size = m.title();
    y += painter.measure_ui(&title, title_size).height;
    painter.ui_bold(&title, left, y, title_size, TEXT);

    for row in &report.stats {
        y += m.line_height;
        painter.ui(stat_line(row), left, y, m.font_size, TEXT);
    }

    y += m.gap;
    y += m.line_height;
    painter.ui_bold(
        format!("AGAINST A TYPICAL ZONE {} PROGRAM", report.zone),
        left,
        y,
        m.font_size,
        TEXT_DIM,
    );
    for line in duel_lines(report) {
        y += m.line_height;
        painter.ui(line, left, y, m.font_size, TEXT);
    }

    y += m.gap;
    y += m.line_height;
    painter.ui_bold("TO SPEND", left, y, m.font_size, TEXT_DIM);

    y += m.line_height;
    let perk_line = format!(
        "  +{} Perk Points ({} unspent)",
        report.perk_points_gained, report.perk_points_unspent
    );
    painter.ui(&perk_line, left, y, m.font_size, TEXT);
    let hint_w = painter.measure_ui_advance(PERKS_HINT, m.small());
    painter.ui(PERKS_HINT, right - hint_w, y, m.small(), TEXT_DIM);

    y += m.line_height;
    painter.ui(
        format!("  +{} Decompiler skill", report.decompiler_gained),
        left,
        y,
        m.font_size,
        TEXT,
    );

    y += m.gap;
    y += m.line_height;
    let esc_w = painter.measure_ui_advance(ESC_HINT, m.small());
    painter.ui(ESC_HINT, right - esc_w, y, m.small(), TEXT_DIM);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widest reachable report by hand: two stat rows at four digits, a
    /// level pair that never needs a third digit (`Game::level_cap`'s own
    /// ceiling sits well under 999), the largest zone this census bothers
    /// naming, and swing counts at `LEVEL_UP_SWINGS_UNREACHABLE` (999) — the
    /// real clamp `swings_to` returns, not a bigger number nothing can ship.
    fn widest_report() -> LevelUpReport {
        LevelUpReport {
            from_level: 99,
            to_level: 99,
            zone: 99,
            stats: vec![
                StatRow::new("Max HP", 1234, 9999),
                StatRow::new("ATK", 1234, 9999),
            ],
            hit_chance: (0.05, 0.99),
            per_swing: (1234.9, 9999.9),
            swings_to_win: (999, 999),
            swings_to_down_you: (999, 999),
            perk_points_gained: 98,
            perk_points_unspent: 998,
            decompiler_gained: 98,
        }
    }

    fn smallest_panel() -> Rect {
        panel_rect(1280.0, 720.0)
    }

    /// **The page has no scroll.** The widest report by hand fits the panel
    /// in both height and width, at the smallest window the game is built
    /// for (`notify.rs`'s own bound).
    #[test]
    fn the_widest_report_fits_its_screen() {
        let m = crate::text::ui_metrics(720.0);
        let report = widest_report();
        crate::paint::with_painter(|p| {
            let panel = smallest_panel();
            let title = format!("LEVEL {} \u{2192} {}", report.from_level, report.to_level);
            let block = block_height(p, &m, &title, report.stats.len());
            assert!(
                block + 2.0 * m.pad < panel.h,
                "the widest level-up report is {block}px tall in a {}px panel — \
                 this screen has no scroll, so give it one or cut a row",
                panel.h
            );
            let width = widest_line(p, &m, &report);
            assert!(
                width + 2.0 * m.pad < panel.w,
                "the widest level-up report's widest line is {width}px in a {}px \
                 panel — this screen has no scroll, so give it one or shorten a line",
                panel.w
            );
        });
    }

    /// Every field the widest report carries is drawn somewhere — a
    /// regression on a row silently dropped from `draw_level_up` rather than
    /// from `widest_line`/`block_height`'s own count.
    #[test]
    fn every_figure_is_drawn() {
        let report = widest_report();
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_level_up(&report, p, &m));
        let text = crate::paint::painted_text(&shapes).join(" | ");
        for want in [
            "LEVEL 99 \u{2192} 99",
            "1234 \u{2192} 9999",
            "AGAINST A TYPICAL ZONE 99 PROGRAM",
            "5% \u{2192} 99%",
            "1234.9 \u{2192} 9999.9",
            "999 \u{2192} 999",
            "+98 Perk Points (998 unspent)",
            "+98 Decompiler skill",
            PERKS_HINT,
            ESC_HINT,
        ] {
            assert!(text.contains(want), "missing {want:?} in {text}");
        }
    }
}
