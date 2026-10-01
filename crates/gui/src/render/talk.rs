//! The TALK face of an owned program's manifest: what it has said and heard.
//!
//! One full-width CONVERSATIONS box built as `Section`s and laid out by
//! `manifest_layout`, like SOCIAL, so the frame and footer are one
//! implementation. The sheet does not scroll, so the page is as many whole
//! exchanges as the frame has rows for, newest first.

use super::manifest_layout::{Section, SectionRow, manifest_layout};
use super::*;
use feral_processes_engine::ExchangeView;

const NO_CONVERSATIONS: &str = "Nothing has been said yet.";

/// Continuation rows of a wrapped line sit under the words, not the speaker.
const CONTINUATION_INDENT: &str = "  ";

/// Clear pixels kept between the last row and the footer, the figure the
/// SOCIAL page's fit census holds too.
pub(super) const FOOTER_CLEARANCE: f32 = 10.0;

/// `text` broken at spaces into rows that each measure within `max_w`,
/// continuation rows indented. A single word wider than `max_w` is the one
/// thing left running wide, since there is no space to break it at.
fn wrap_row(text: &str, max_w: f32, painter: &Painter, m: &Metrics) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.trim().is_empty() {
            current.push_str(word);
            continue;
        }
        let candidate = format!("{current} {word}");
        if painter.measure_ui_advance(&candidate, m.font_size) <= max_w {
            current = candidate;
        } else {
            rows.push(current);
            current = format!("{CONTINUATION_INDENT}{word}");
        }
    }
    if !current.trim().is_empty() {
        rows.push(current);
    }
    rows
}

fn exchange_rows(
    exchange: &ExchangeView,
    max_w: f32,
    painter: &Painter,
    m: &Metrics,
) -> Vec<String> {
    exchange
        .lines
        .iter()
        .flat_map(|l| wrap_row(&format!("{}: {}", l.who, l.text), max_w, painter, m))
        .collect()
}

fn conversations_section(rows: Vec<SectionRow>) -> Section {
    Section {
        title: "CONVERSATIONS",
        rows,
        full_width: true,
        overflow: 0,
    }
}

/// The page: whole exchanges, newest first, until the next would leave less
/// than `FOOTER_CLEARANCE` above the footer. The fit is asked of
/// `manifest_layout` itself, so the rows counted are the boxes drawn.
pub(super) fn talk_sections(
    exchanges: &[ExchangeView],
    (w, h): (f32, f32),
    painter: &Painter,
    m: &Metrics,
) -> Vec<Section> {
    let inner = manifest_layout(w, h, 0, &[], m).header.w - m.inset * 2.0;
    let mut rows: Vec<SectionRow> = Vec::new();
    for exchange in exchanges {
        let mut candidate = rows.clone();
        if !candidate.is_empty() {
            candidate.push(SectionRow::Note(String::new()));
        }
        candidate.extend(
            exchange_rows(exchange, inner, painter, m)
                .into_iter()
                .map(SectionRow::Note),
        );
        let page = [conversations_section(candidate)];
        let l = manifest_layout(w, h, 0, &page, m);
        let bottom = l.sections[0].y + l.sections[0].h;
        if l.footer.y - bottom < FOOTER_CLEARANCE {
            break;
        }
        let [section] = page;
        rows = section.rows;
    }
    if rows.is_empty() {
        rows.push(SectionRow::Note(NO_CONVERSATIONS.to_string()));
    }
    vec![conversations_section(rows)]
}

#[cfg(test)]
mod tests {
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

    fn assets() -> std::path::PathBuf {
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")).to_path_buf()
    }

    /// The longest name `Game::conversations` can give a topic: a species or
    /// structure of the shipped catalogue, or the longest of the fixed
    /// activity phrases and a base tile at a wide coordinate pair.
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
            .chain(["raising a structure", "the base at (-1000, -1000)"].map(String::from))
            .max_by_key(|n| n.chars().count())
            .expect("the catalogue has names")
    }

    /// Every shipped exchange spoken between the widest names a program can
    /// have: a full custom name or a handle, with the zone suffix.
    fn worst_exchanges() -> Vec<ExchangeView> {
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

    fn short(n: usize) -> ExchangeView {
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

    fn texts(section: &Section) -> Vec<String> {
        section
            .rows
            .iter()
            .map(|r| match r {
                SectionRow::Note(t) => t.clone(),
                SectionRow::Stat(a, b) => format!("{a} {b}"),
            })
            .collect()
    }

    /// The shipped worst case may fit one row at every window; a modded line
    /// will not, and that is the path that must wrap rather than truncate.
    #[test]
    fn a_line_wider_than_the_box_wraps_and_indents() {
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let max_w = manifest_layout(w, h, 0, &[], &m).header.w - m.inset * 2.0;
                let rows = wrap_row(&"word ".repeat(120), max_w, p, &m);
                assert!(rows.len() > 1, "no wrap at {w}x{h}");
                assert!(rows[1].starts_with(CONTINUATION_INDENT));
                for r in &rows {
                    assert!(p.measure_ui_advance(r, m.font_size) <= max_w, "{r:?}");
                }
            }
        });
    }

    #[test]
    fn an_empty_ring_says_so_instead_of_drawing_a_hollow_box() {
        with_painter(|p| {
            let s = talk_sections(&[], (1440.0, 900.0), p, &ui_metrics(900.0));
            assert_eq!(s.len(), 1);
            assert_eq!(texts(&s[0]), vec![NO_CONVERSATIONS.to_string()]);
        });
    }

    /// The longest shipped line between the widest names wraps inside the
    /// frame at every window, and the worst exchange alone still fits.
    #[test]
    fn the_worst_exchange_wraps_inside_the_tightest_window() {
        let exchanges = worst_exchanges();
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let l = manifest_layout(w, h, 0, &[], &m);
                let max_w = l.header.w - m.inset * 2.0;
                for ex in &exchanges {
                    for row in exchange_rows(ex, max_w, p, &m) {
                        assert!(
                            p.measure_ui_advance(&row, m.font_size) <= max_w,
                            "{row:?} runs past the box at {w}x{h}"
                        );
                    }
                    let page = [conversations_section(
                        exchange_rows(ex, max_w, p, &m)
                            .into_iter()
                            .map(SectionRow::Note)
                            .collect(),
                    )];
                    let l = manifest_layout(w, h, 0, &page, &m);
                    let bottom = l.sections[0].y + l.sections[0].h;
                    assert!(
                        l.footer.y - bottom >= FOOTER_CLEARANCE,
                        "{ex:?} does not fit alone at {w}x{h}"
                    );
                }
            }
        });
    }

    /// At every window the rows drawn are whole exchanges, newest first: the
    /// page is exactly the first k exchanges' own rows.
    #[test]
    fn the_page_never_shows_part_of_an_exchange() {
        let mut exchanges = worst_exchanges();
        exchanges.extend((0..30).map(short));
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let page = talk_sections(&exchanges, (w, h), p, &m);
                let rows = texts(&page[0]);
                assert!(rows.len() < exchanges.len() * 2, "the page should be cut");
                let mut expected: Vec<String> = Vec::new();
                let max_w = manifest_layout(w, h, 0, &[], &m).header.w - m.inset * 2.0;
                for ex in &exchanges {
                    if rows.len() == expected.len() {
                        break;
                    }
                    if !expected.is_empty() {
                        expected.push(String::new());
                    }
                    expected.extend(exchange_rows(ex, max_w, p, &m));
                    assert!(
                        rows.len() >= expected.len(),
                        "the page ends inside an exchange"
                    );
                }
                assert_eq!(rows, expected, "at {w}x{h}");
            }
        });
    }
}
