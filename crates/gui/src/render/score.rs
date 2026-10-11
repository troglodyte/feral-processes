//! The run score card as text. One builder turns a `ScoreCard` into display
//! lines; the death/escape popups and the player's manifest Score face are
//! both thin wrappers over it, so neither can word a line the other doesn't.

use feral_processes_engine::score::ScoreCard;

use super::manifest::draw_pairs;
use super::manifest::layout::{SectionRow, ledger_gap};
use super::popup::*;
use super::*;

const MULTIPLIER_LABEL: &str = "Difficulty multiplier";
const TOTAL_LABEL: &str = "Run score";

/// One line of the card: the engine's own rows, then the multiplier and the
/// total. `count` is `None` for the two that are not a count times a rate.
pub(super) struct DisplayLine {
    pub(super) label: &'static str,
    pub(super) count: Option<u64>,
    pub(super) points: String,
}

impl DisplayLine {
    pub(super) fn value(&self) -> String {
        match self.count {
            Some(count) => format!("{count} = {}", self.points),
            None => self.points.clone(),
        }
    }
}

pub(super) fn display_lines(card: &ScoreCard) -> Vec<DisplayLine> {
    let mut lines: Vec<DisplayLine> = card
        .lines
        .iter()
        .map(|l| DisplayLine {
            label: l.label,
            count: Some(l.count),
            points: l.points.to_string(),
        })
        .collect();
    lines.push(DisplayLine {
        label: MULTIPLIER_LABEL,
        count: None,
        points: format!("x{:.2}", card.multiplier),
    });
    lines.push(DisplayLine {
        label: TOTAL_LABEL,
        count: None,
        points: card.total.to_string(),
    });
    lines
}

/// The card as popup text rows. A popup has no right-aligned column, so each
/// line is `label: value`.
pub(super) fn popup_rows(card: &ScoreCard) -> Vec<Row> {
    display_lines(card)
        .iter()
        .map(|l| text_row(format!("{}: {}", l.label, l.value())))
        .collect()
}

/// How many of the card's engine lines sit in the first ledger; the rest
/// fill the second.
const FIRST_BOX_LINES: usize = 4;

/// The SCORE face's body: two ledgers of earned lines and the closing band.
pub(super) struct ScorePage {
    pub(super) fight: Vec<DisplayLine>,
    pub(super) run: Vec<DisplayLine>,
    pub(super) total: Vec<SectionRow>,
}

pub(super) fn page(card: &ScoreCard) -> ScorePage {
    let mut lines = display_lines(card);
    let engine = card.lines.len();
    let closing = lines.split_off(engine);
    let run = lines.split_off(FIRST_BOX_LINES);
    ScorePage {
        fight: lines,
        run,
        total: closing
            .iter()
            .map(|l| SectionRow::Stat(l.label.to_string(), l.value()))
            .collect(),
    }
}

/// THE FIGHT and THE RUN side by side across the body, each row
/// `label  count = points` with the points in `hud-field-label`. The TOTAL
/// band is drawn by the caller, from the layout's bands rectangle.
pub(super) fn paint_score(page: &ScorePage, body: Rect, painter: &Painter, m: &Metrics) {
    let gap = ledger_gap(m);
    let half = (body.w - gap) / 2.0;
    for (i, (title, lines)) in [("THE FIGHT", &page.fight), ("THE RUN", &page.run)]
        .into_iter()
        .enumerate()
    {
        let rect = Rect::new(body.x + i as f32 * (half + gap), body.y, half, body.h);
        let rows: Vec<(&str, String)> = lines.iter().map(|l| (l.label, l.value())).collect();
        draw_pairs(
            title,
            &rows,
            Some(hud::palette::FIELD_LABEL),
            rect,
            painter,
            m,
        );
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::score::{ScoreCard, ScoreLine};

    /// Max-width counts on every line and the longest label.
    pub(in crate::render) fn widest_card() -> ScoreCard {
        let labels = [
            "Programs defeated",
            "Bosses defeated",
            "Deepest Stack depth",
            "Phase keys held",
            "Escaped the Basin",
            "Structures standing",
            "Programs compiled",
            "Achievements earned this run",
        ];
        ScoreCard {
            lines: labels
                .iter()
                .map(|&label| ScoreLine {
                    label,
                    count: 9_999_999,
                    points: 99_999_999_999,
                })
                .collect(),
            multiplier: 9.99,
            total: 999_999_999_999,
        }
    }

    #[test]
    fn the_card_is_its_engine_lines_then_multiplier_then_total() {
        let card = widest_card();
        let lines = display_lines(&card);
        assert_eq!(lines.len(), card.lines.len() + 2);
        assert_eq!(lines[card.lines.len()].points, "x9.99");
        assert_eq!(lines.last().unwrap().points, "999999999999");
    }

    #[test]
    fn popup_and_manifest_word_every_line_the_same_way() {
        let card = widest_card();
        let p = page(&card);
        let from_manifest: Vec<String> = p
            .fight
            .iter()
            .chain(&p.run)
            .map(|l| format!("{}: {}", l.label, l.value()))
            .chain(p.total.iter().map(|r| match r {
                SectionRow::Stat(l, v) => format!("{l}: {v}"),
                other => unreachable!("the score card builds no {other:?}"),
            }))
            .collect();
        let from_popup: Vec<String> = display_lines(&card)
            .iter()
            .map(|l| format!("{}: {}", l.label, l.value()))
            .collect();
        assert_eq!(from_manifest, from_popup);
        assert_eq!(p.fight.len(), FIRST_BOX_LINES);
    }

    #[test]
    fn the_widest_card_fits_both_popup_sizes() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            for size in [PopupSize::Large, PopupSize::Small] {
                let room = popup_body_width(1280.0, size, &m);
                for l in display_lines(&widest_card()) {
                    let line = format!("{}: {}", l.label, l.value());
                    let drawn = p.measure_ui_advance(&line, m.font_size);
                    assert!(
                        drawn <= room,
                        "a score line overflows its popup by {:.0}px:\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }
}
