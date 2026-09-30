//! The page a perk purchase opens (`Mode::PerkBought`), `level_up.rs`'s
//! shape: a scrim, a bordered panel over the map, no `draw_popup` and so no
//! refusal of its own — `needs_status_banner` carries one raised underneath.
//!
//! A perk that moves a fight figure shows the same block the level-up page
//! and the Points screen do, through `level_up::duel_lines`. One that moves
//! none (a scavenging perk, say) shows its description instead, so the page
//! always says what the level bought.
//!
//! Fixed rows, no scroll: the height and width are held by a census over the
//! shipped perks and a hand-built widest report.

use feral_processes_engine::PerkReport;
use feral_processes_engine::text;

use super::level_up::{duel_heading, duel_lines, duel_moved, panel_rect, stat_line};
use super::{BORDER, Metrics, PANEL_BG, TEXT, TEXT_DIM};
use crate::paint::{Color, Painter, Rect};

const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);

const HINT: &str = "[Enter] Back to perks";

fn title(report: &PerkReport) -> String {
    format!(
        "{} \u{2014} LEVEL {}",
        report.name.to_uppercase(),
        report.level
    )
}

/// What sits under the stat rows: the duel block, or the description
/// wrapped to `columns`.
struct Tail {
    heading: Option<String>,
    lines: Vec<String>,
}

fn tail(report: &PerkReport, columns: usize) -> Tail {
    let duel = &report.preview.duel;
    if duel_moved(duel) {
        Tail {
            heading: Some(duel_heading(duel)),
            lines: duel_lines(duel).into(),
        }
    } else {
        Tail {
            heading: None,
            lines: text::wrap(&report.description, columns),
        }
    }
}

/// How many cells of body text fit across `panel`, inside its padding.
fn body_columns(painter: &Painter, panel: Rect, m: &Metrics) -> usize {
    let columns = (panel.w - 2.0 * m.pad) / painter.measure_ui_advance("M", m.font_size);
    (columns.floor() as usize).max(20)
}

/// The block's height. **The one sum** `draw_perk_bought`'s `y` walk matches
/// and the census measures, `level_up::block_height`'s reason. Test-only for
/// the same reason: the draw is top-anchored.
#[cfg(test)]
fn block_height(painter: &Painter, m: &Metrics, report: &PerkReport, columns: usize) -> f32 {
    let tail = tail(report, columns);
    painter.measure_ui(title(report), m.title()).height
        + report.preview.stats.len() as f32 * m.line_height
        + m.gap
        + if tail.heading.is_some() {
            m.line_height
        } else {
            0.0
        }
        + tail.lines.len() as f32 * m.line_height
        + m.gap
        + m.line_height // hint line
}

/// Draws `report` in a panel over the map.
pub(super) fn draw_perk_bought(report: &PerkReport, painter: &Painter, m: &Metrics) {
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, SCRIM);
    let panel = panel_rect(w, h);
    painter.rect(panel.x, panel.y, panel.w, panel.h, PANEL_BG);
    painter.rect_lines(panel.x, panel.y, panel.w, panel.h, 2.0, BORDER);

    let left = panel.x + m.pad;
    let right = panel.x + panel.w - m.pad;
    let mut y = panel.y + m.pad;

    let heading = title(report);
    let title_size = m.title();
    y += painter.measure_ui(&heading, title_size).height;
    painter.ui_bold(&heading, left, y, title_size, TEXT);

    for row in &report.preview.stats {
        y += m.line_height;
        painter.ui(stat_line(row), left, y, m.font_size, TEXT);
    }

    y += m.gap;
    let tail = tail(report, body_columns(painter, panel, m));
    if let Some(heading) = &tail.heading {
        y += m.line_height;
        painter.ui_bold(heading, left, y, m.font_size, TEXT_DIM);
    }
    for line in &tail.lines {
        y += m.line_height;
        painter.ui(line, left, y, m.font_size, TEXT);
    }

    y += m.gap;
    y += m.line_height;
    let hint_w = painter.measure_ui_advance(HINT, m.small());
    painter.ui(HINT, right - hint_w, y, m.small(), TEXT_DIM);
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::progression::StatRow;
    use feral_processes_engine::{DifficultyMode, DuelComparison, Game, PerkPreview};

    fn moved_duel() -> DuelComparison {
        DuelComparison {
            zone: 99,
            hit_chance: (0.05, 0.99),
            per_swing: (1234.9, 9999.9),
            swings_to_win: (999, 998),
            swings_to_down_you: (999, 998),
        }
    }

    fn still_duel() -> DuelComparison {
        DuelComparison {
            hit_chance: (0.5, 0.5),
            per_swing: (5.0, 5.0),
            swings_to_win: (4, 4),
            swings_to_down_you: (7, 7),
            ..moved_duel()
        }
    }

    /// Every stat row moving at four digits and the longest description the
    /// assets could plausibly carry, on the tail that draws it.
    fn widest_report(duel: DuelComparison) -> PerkReport {
        PerkReport {
            name: "Tighten Tolerances Of Every Kind".into(),
            level: 99,
            description: "What this perk does, at well over the length the shipped perks run \
                          to: a sentence naming the fantasy, then another naming the number \
                          it moves and roughly how far each level moves it, then a third for \
                          good measure so the wrap has something to do."
                .into(),
            preview: PerkPreview {
                stats: vec![
                    StatRow::new("Max HP", 1234, 9999),
                    StatRow::new("ATK", 1234, 9999),
                    StatRow::new("Mitigation", 1234, 9999),
                ],
                duel,
            },
        }
    }

    fn assert_fits(report: &PerkReport, what: &str) {
        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let panel = panel_rect(1280.0, 720.0);
            let columns = body_columns(p, panel, &m);
            let block = block_height(p, &m, report, columns);
            assert!(
                block + 2.0 * m.pad < panel.h,
                "{what}: the purchase page is {block}px tall in a {}px panel — \
                 this screen has no scroll, so give it one or cut a row",
                panel.h
            );
            let t = tail(report, columns);
            let mut lines: Vec<String> = report.preview.stats.iter().map(stat_line).collect();
            lines.extend(t.heading);
            lines.extend(t.lines);
            let mut widths = vec![(
                title(report),
                p.measure_ui_advance(title(report), m.title()),
            )];
            widths.extend(lines.into_iter().map(|l| {
                let w = p.measure_ui_advance(&l, m.font_size);
                (l, w)
            }));
            for (line, width) in widths {
                assert!(
                    width + 2.0 * m.pad < panel.w,
                    "{what}: {line:?} is {width}px in a {}px panel",
                    panel.w
                );
            }
        });
    }

    /// **The page has no scroll.** The widest report on either tail fits at
    /// the smallest window the game is built for, and so does every shipped
    /// perk's own report.
    #[test]
    fn the_widest_purchase_page_fits_its_screen() {
        assert_fits(&widest_report(moved_duel()), "duel tail");
        assert_fits(&widest_report(still_duel()), "description tail");

        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let mut game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets");
        let defs = game.perk_defs();
        assert!(!defs.is_empty(), "the shipped assets declare perks");
        for def in defs {
            let preview = game.preview_perk(def.id).expect("a preview");
            assert_fits(
                &PerkReport {
                    name: def.name.clone(),
                    level: 99,
                    description: def.description,
                    preview,
                },
                &def.name,
            );
        }
    }

    /// A perk that moves no fight figure still says what it did, and one
    /// that does shows the block instead of the description.
    #[test]
    fn the_page_shows_the_description_when_no_fight_figure_moved() {
        let m = crate::text::ui_metrics(900.0);
        let text = |report: &PerkReport| {
            let (_, shapes) = crate::paint::with_painter(|p| draw_perk_bought(report, p, &m));
            crate::paint::painted_text(&shapes).join(" | ")
        };

        let still = widest_report(still_duel());
        let drawn = text(&still);
        assert!(drawn.contains("What this perk does"), "{drawn}");
        assert!(!drawn.contains("AGAINST A TYPICAL"), "{drawn}");
        assert!(drawn.contains("TIGHTEN TOLERANCES OF EVERY KIND \u{2014} LEVEL 99"));
        assert!(drawn.contains("Max HP   1234 \u{2192} 9999"), "{drawn}");
        assert!(drawn.contains(HINT), "{drawn}");

        let moved = widest_report(moved_duel());
        let drawn = text(&moved);
        assert!(
            drawn.contains("AGAINST A TYPICAL ZONE 99 PROGRAM"),
            "{drawn}"
        );
        assert!(drawn.contains("5% \u{2192} 99%"), "{drawn}");
        assert!(!drawn.contains("What this perk does"), "{drawn}");
    }
}
