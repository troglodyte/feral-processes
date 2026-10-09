//! The run score card as text. One builder turns a `ScoreCard` into display
//! lines; the death/escape popups and the player's manifest Score face are
//! both thin wrappers over it, so neither can word a line the other doesn't.

use feral_processes_engine::score::ScoreCard;

use super::manifest_layout::{Section, SectionRow};
use super::popup::*;

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

/// How many of the card's engine lines sit in the manifest's first box; the
/// rest fill the second, so neither passes `MAX_SECTION_ROWS`.
const FIRST_BOX_LINES: usize = 4;

/// The card as manifest boxes: two columned boxes of earned lines and a
/// full-width band for the multiplier and the total.
pub(super) fn manifest_sections(card: &ScoreCard) -> Vec<Section> {
    let lines = display_lines(card);
    let engine = card.lines.len();
    let rows = |range: std::ops::Range<usize>| -> Vec<SectionRow> {
        lines[range]
            .iter()
            .map(|l| SectionRow::Stat(l.label.to_string(), l.value()))
            .collect()
    };
    let boxed = |title, rows| Section {
        title,
        rows,
        full_width: false,
        overflow: 0,
        hint: "",
    };
    vec![
        boxed("THE FIGHT", rows(0..FIRST_BOX_LINES)),
        boxed("THE RUN", rows(FIRST_BOX_LINES..engine)),
        Section {
            full_width: true,
            ..boxed("TOTAL", rows(engine..lines.len()))
        },
    ]
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
        let from_manifest: Vec<String> = manifest_sections(&card)
            .iter()
            .flat_map(|s| s.rows.iter())
            .map(|r| match r {
                SectionRow::Stat(l, v) => format!("{l}: {v}"),
                SectionRow::Note(n) => n.clone(),
            })
            .collect();
        let from_popup: Vec<String> = display_lines(&card)
            .iter()
            .map(|l| format!("{}: {}", l.label, l.value()))
            .collect();
        assert_eq!(from_manifest, from_popup);
        assert!(
            manifest_sections(&card)
                .iter()
                .all(|s| s.rows.len() <= super::super::manifest_layout::MAX_SECTION_ROWS)
        );
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

    /// The Score face is a page of its own: it must sit inside the frame with
    /// room above the footer at every window, and no row may be elided or
    /// shrunk, since a cut score is a wrong score.
    #[test]
    fn the_widest_card_fits_the_manifest_at_every_window() {
        use super::super::manifest::fitted_stat_row;
        use super::super::manifest_layout::manifest_layout;
        const MIN_CLEARANCE: f32 = 4.0;
        let sections = manifest_sections(&widest_card());
        with_painter(|p| {
            for (w, h) in [
                (1280.0, 720.0),
                (1366.0, 768.0),
                (1600.0, 900.0),
                (1920.0, 1080.0),
                (2560.0, 1440.0),
            ] {
                let m = ui_metrics(h);
                let l = manifest_layout(w, h, 0, &sections, &m);
                let bottom = l.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
                assert!(
                    l.footer.y - bottom >= MIN_CLEARANCE,
                    "the Score face runs into the footer at {w}x{h}"
                );
                for (section, rect) in sections.iter().zip(&l.sections) {
                    for row in &section.rows {
                        let SectionRow::Stat(label, value) = row else {
                            continue;
                        };
                        let fitted = fitted_stat_row(p, label, value, *rect, &m);
                        assert!(
                            fitted.label == *label
                                && fitted.value == *value
                                && fitted.size == m.font_size,
                            "{label} was cut or shrunk at {w}x{h}"
                        );
                    }
                }
            }
        });
    }
}
