//! The TALK face of an owned program's manifest: what it has said and heard.
//!
//! CONVERSATIONS, newest first, one block per exchange with a rule between
//! blocks. The speaker sits in a fixed column and the words wrap in their own,
//! so a long line hangs under its first word and not under its speaker. The
//! sheet does not scroll, so the page is as many whole exchanges as the body
//! has rows for.

use super::manifest::layout::{ManifestLayout, section_row_h};
use super::manifest::{elided_to_fit, subhead, wrap_to_width};
use super::*;
use feral_processes_engine::ExchangeView;

const NO_CONVERSATIONS: &str = "Nothing has been said yet.";

/// Shown when the newest exchange alone is taller than the body, so the page
/// is not mistaken for an empty ring.
const TOO_LONG_TO_SHOW: &str = "The latest exchange is too long to show here.";

/// The speaker column, in cells. A name is cut to it, which is the one thing
/// on the page that is: a speaker is a label for the words beside it, and
/// the words are what the page is for.
const SPEAKER_CELLS: f32 = 12.0;

/// One spoken line as drawn: who, whether it is the subject, and the words
/// wrapped to the text column.
#[derive(Debug, PartialEq)]
pub(super) struct TalkLine {
    pub(super) speaker: String,
    pub(super) own: bool,
    pub(super) text: Vec<String>,
}

fn exchange_lines(
    exchange: &ExchangeView,
    own: &str,
    text_w: f32,
    painter: &Painter,
    m: &Metrics,
) -> Vec<TalkLine> {
    exchange
        .lines
        .iter()
        .map(|l| TalkLine {
            speaker: l.who.clone(),
            own: l.who == own,
            text: wrap_to_width(painter, &l.text, m.font_size, text_w)
                .into_iter()
                .map(|line| elided_to_fit(painter, &line, m.font_size, text_w))
                .collect(),
        })
        .collect()
}

fn rows_of(exchange: &[TalkLine]) -> usize {
    exchange.iter().map(|l| l.text.len().max(1)).sum()
}

/// The page: whole exchanges, newest first, while `budget` rows hold them
/// and the rule row between each pair.
pub(super) fn page(
    exchanges: &[ExchangeView],
    own: &str,
    text_w: f32,
    budget: usize,
    painter: &Painter,
    m: &Metrics,
) -> Vec<Vec<TalkLine>> {
    let mut blocks: Vec<Vec<TalkLine>> = Vec::new();
    let mut used = 0;
    for exchange in exchanges {
        let lines = exchange_lines(exchange, own, text_w, painter, m);
        let cost = rows_of(&lines) + usize::from(!blocks.is_empty());
        if used + cost > budget {
            break;
        }
        used += cost;
        blocks.push(lines);
    }
    blocks
}

/// The subject's own lines in `hud-pane-title`, the others' in `hud-body`.
pub(super) fn paint_talk(
    exchanges: &[ExchangeView],
    own: &str,
    l: &ManifestLayout,
    painter: &Painter,
    m: &Metrics,
) {
    let rect = l.body();
    subhead("CONVERSATIONS", "", rect, rect.y, painter, m);
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let row_h = section_row_h(m);
    let text_x = rect.x + (SPEAKER_CELLS + 1.0) * char_w;
    let top = rect.y + m.line_height;
    let budget = ((rect.h - m.line_height) / row_h).floor().max(0.0) as usize;
    let blocks = page(exchanges, own, rect.x + rect.w - text_x, budget, painter, m);
    if blocks.is_empty() {
        let note = if exchanges.is_empty() {
            NO_CONVERSATIONS
        } else {
            TOO_LONG_TO_SHOW
        };
        painter.ui(
            note,
            rect.x,
            top + row_h * 0.85,
            m.font_size,
            hud::palette::FAINT,
        );
        return;
    }
    let mut row = 0;
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            let y = top + row as f32 * row_h + row_h / 2.0;
            painter.rect(rect.x, y, rect.w, 1.0, hud::palette::DIVIDER);
            row += 1;
        }
        for line in block {
            let base = |r: usize| top + (row + r) as f32 * row_h + row_h * 0.85;
            let ink = if line.own {
                hud::palette::PANE_TITLE
            } else {
                hud::palette::BODY
            };
            painter.ui(
                elided_to_fit(painter, &line.speaker, m.font_size, SPEAKER_CELLS * char_w),
                rect.x,
                base(0),
                m.font_size,
                ink,
            );
            for (r, text) in line.text.iter().enumerate() {
                painter.ui(text, text_x, base(r), m.font_size, hud::palette::BODY);
            }
            row += line.text.len().max(1);
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::SpokenLine;
    use feral_processes_engine::interactions::{InteractionDb, fill};
    use feral_processes_engine::memories::MemoryDb;

    const WINDOWS: [(f32, f32); 8] = [
        (1280.0, 720.0),
        (1366.0, 768.0),
        (1440.0, 900.0),
        (1600.0, 900.0),
        (1680.0, 1050.0),
        (1920.0, 1080.0),
        (1920.0, 1200.0),
        (2560.0, 1440.0),
    ];

    use feral_processes_engine::components::TaskKind;

    fn assets() -> std::path::PathBuf {
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")).to_path_buf()
    }

    /// The longest name `Game::conversations` can give a topic: a species or
    /// structure of the shipped catalogue, an activity's phrase, or a base tile
    /// at a wide coordinate pair.
    fn longest_topic() -> String {
        let game = feral_processes_engine::Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            &assets(),
        )
        .expect("shipped assets load");
        game.species_defs()
            .into_iter()
            .map(|d| d.name)
            .chain(game.structure_defs().into_iter().map(|d| d.name))
            .chain(TaskKind::ALL.map(|k| k.phrase().to_string()))
            .chain(["the base at (-1000, -1000)".to_string()])
            .max_by_key(|n| n.chars().count())
            .expect("the catalogue has names")
    }

    /// Every shipped exchange spoken between the widest names a program can
    /// have: a full custom name or a handle, with the zone suffix.
    pub(in crate::render) fn worst_exchanges() -> Vec<ExchangeView> {
        let (memories, _) = MemoryDb::load_dir(&assets().join("memories")).expect("memories");
        let (db, _) =
            InteractionDb::load_dir(&assets().join("interactions"), &memories).expect("defs");
        let wide = format!(
            "{} 10",
            "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        let handle = "0x000000 10".to_string();
        let name = if wide.len() > handle.len() {
            wide
        } else {
            handle
        };
        let topic = longest_topic();
        let out: Vec<ExchangeView> = db
            .iter()
            .flat_map(|d| d.exchanges.iter())
            .map(|lines| ExchangeView {
                lines: lines
                    .iter()
                    .map(|l| SpokenLine {
                        who: name.clone(),
                        text: fill(&l.text, |slot| match slot {
                            "topic" => topic.clone(),
                            _ => name.clone(),
                        }),
                    })
                    .collect(),
            })
            .collect();
        assert!(!out.is_empty());
        out
    }

    pub(in crate::render) fn short(n: usize) -> ExchangeView {
        ExchangeView {
            lines: vec![
                SpokenLine {
                    who: "Kestrel".into(),
                    text: format!("Line {n}."),
                },
                SpokenLine {
                    who: "Wren".into(),
                    text: "Ack.".into(),
                },
            ],
        }
    }

    /// The page's rows for a text column `text_w` wide in a body of `budget`
    /// rows.
    fn paged(exchanges: &[ExchangeView], budget: usize, (w, h): (f32, f32)) -> Vec<Vec<TalkLine>> {
        let m = ui_metrics(h);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let text_w = w * 0.8 - (SPEAKER_CELLS + 1.0) * char_w;
            page(exchanges, "Kestrel", text_w, budget, p, &m)
        })
        .0
    }

    /// A line wider than the column wraps and never runs past it; the
    /// shipped worst case may fit one row, a modded line will not.
    #[test]
    fn a_line_wider_than_the_column_wraps_inside_it() {
        let long = ExchangeView {
            lines: vec![SpokenLine {
                who: "Kestrel".into(),
                text: "word ".repeat(120),
            }],
        };
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let text_w = w * 0.6;
                let lines = exchange_lines(&long, "Kestrel", text_w, p, &m);
                assert!(lines[0].text.len() > 1, "no wrap at {w}x{h}");
                for row in &lines[0].text {
                    assert!(p.measure_ui_advance(row, m.font_size) <= text_w, "{row:?}");
                }
            }
        });
    }

    #[test]
    fn only_the_subjects_own_lines_are_marked() {
        let blocks = paged(&[short(1)], 10, (1440.0, 900.0));
        let own: Vec<bool> = blocks[0].iter().map(|l| l.own).collect();
        assert_eq!(own, [true, false], "Kestrel is the subject");
    }

    #[test]
    fn an_exchange_too_tall_for_the_body_is_not_reported_as_silence() {
        let huge = ExchangeView {
            lines: vec![SpokenLine {
                who: "Kestrel".into(),
                text: "word ".repeat(2000),
            }],
        };
        assert!(paged(&[huge], 20, (1440.0, 900.0)).is_empty());
    }

    /// At every budget the page is exactly the first k exchanges, each
    /// whole, and the rule rows between them are paid for.
    #[test]
    fn the_page_never_shows_part_of_an_exchange() {
        let exchanges: Vec<ExchangeView> = (0..30).map(short).collect();
        for budget in 0..40 {
            let blocks = paged(&exchanges, budget, (1440.0, 900.0));
            let cost =
                blocks.iter().map(|b| rows_of(b)).sum::<usize>() + blocks.len().saturating_sub(1);
            assert!(cost <= budget, "{cost} rows in a budget of {budget}");
            // Each short exchange is two rows.
            assert_eq!(blocks.len(), (budget + 1) / 3, "budget {budget}");
            for (i, block) in blocks.iter().enumerate() {
                assert_eq!(block[0].text, vec![format!("Line {i}.")]);
            }
        }
    }
}
