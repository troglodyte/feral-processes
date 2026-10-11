//! The three-column ledger and the bands under it.
//!
//! A row is a label, a value beside it, and (for a ranked stat) a compare
//! cell against the roster. Nothing here clips: every string is measured and
//! cut to the room it has, the way `fitted_stat_row` did for the boxes.

use super::layout::{fit_column, ledger_section_h, section_row_h};
use super::sections::{Band, PartyRow};
use super::*;
use crate::paint::Face as FontFace;
use feral_processes_engine::{RankedStat, SubjectRanks};
use hud::bar::bar;

/// Cells a ranked row reserves at its right end: a space, five pips, a
/// space and an ordinal up to four characters (`100th`).
const COMPARE_CELLS: usize = 11;
const PIPS: usize = 5;
const PIP: char = '▮';

/// `1st`, `2nd`, `3rd`, `4th`, `11th`, `12th`, `13th`, `21st`.
pub(super) fn ordinal(n: u32) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A row as it will be drawn: wrapped to the column.
///
/// The label's lines come first, the continuation ones indented. The value
/// then either rides flush right on the label's last line (`inline`) or
/// continues on indented lines of its own below it.
#[derive(Debug, PartialEq)]
pub(super) struct FittedLedgerRow {
    pub(super) label_lines: Vec<String>,
    pub(super) value_lines: Vec<String>,
    pub(super) inline: bool,
    /// The inline value's measured width.
    pub(super) value_w: f32,
    /// Whether the compare cell survives, which only a one-line row keeps.
    pub(super) compare: bool,
}

impl FittedLedgerRow {
    /// Lines the row takes in the column, which `fit_column` counts.
    pub(super) fn lines(&self) -> usize {
        self.label_lines.len()
            + if self.inline {
                0
            } else {
                self.value_lines.len()
            }
    }

    pub(super) fn value(&self) -> String {
        self.value_lines.join(" ")
    }
}

/// Cells a continuation line is indented by.
const INDENT_CELLS: f32 = 2.0;

/// Lays one row out in `width`, the column's whole width.
///
/// **A row that does not fit continues; it is not cut.** When the label and
/// the value cannot share a line they stack: the label (itself wrapped if it
/// is long), then the value on indented lines below, or flush right on the
/// label's last line when it still fits there. Elision is the last resort,
/// for one unbroken word wider than the column. Before any of that the
/// compare cell goes, then the row wraps. `reserve` is whether the section
/// holds a compare cell, so every value in it lines up whether or not its
/// own row has one.
pub(super) fn fit_ledger_row(
    painter: &Painter,
    label: &str,
    value: &str,
    width: f32,
    reserve: bool,
    m: &Metrics,
) -> FittedLedgerRow {
    let size = m.font_size;
    let char_w = painter.measure_ui_advance("M", size);
    let compare_w = char_w * COMPARE_CELLS as f32;
    let adv = |t: &str| painter.measure_ui_advance(t, size);
    let pair = adv(label) + adv(value) + m.gap;
    if reserve && pair <= width - compare_w {
        return FittedLedgerRow {
            label_lines: vec![label.to_string()],
            value_lines: vec![value.to_string()],
            inline: true,
            value_w: adv(value),
            compare: true,
        };
    }
    if pair <= width {
        return FittedLedgerRow {
            label_lines: vec![label.to_string()],
            value_lines: vec![value.to_string()],
            inline: true,
            value_w: adv(value),
            compare: false,
        };
    }
    let indent = INDENT_CELLS * char_w;
    let cols = (((width - indent) / char_w).floor() as usize).max(1);
    let lines_of = |text: &str| -> Vec<String> {
        let mut lines = crate::render::popup::wrap_text(text, cols);
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
            .into_iter()
            .map(|l| elided_to_fit(painter, &l, size, width - indent))
            .collect()
    };
    let mut label_lines = lines_of(label);
    // The label keeps the full width on its first line.
    if label_lines.len() == 1 {
        label_lines[0] = elided_to_fit(painter, label, size, width);
    }
    let value_lines = if value.is_empty() {
        Vec::new()
    } else {
        lines_of(value)
    };
    let last = label_lines.last().map_or(0.0, |l| adv(l));
    let indented = label_lines.len() > 1;
    let value_w = value_lines.first().map_or(0.0, |v| adv(v));
    let inline = value_lines.len() == 1
        && last + if indented { indent } else { 0.0 } + value_w + m.gap <= width;
    FittedLedgerRow {
        label_lines,
        value_lines,
        inline,
        value_w,
        compare: false,
    }
}

fn find_rank(
    ranks: Option<&SubjectRanks>,
    stat: RankedStat,
) -> Option<feral_processes_engine::RankRow> {
    ranks?.rows.iter().copied().find(|r| r.stat == stat)
}

/// A section holds a compare cell when the roster ranked any of its rows.
fn reserves_compare(section: &Section, ranks: Option<&SubjectRanks>) -> bool {
    section
        .rows
        .iter()
        .any(|r| matches!(r, SectionRow::Ranked(_, _, stat) if find_rank(ranks, *stat).is_some()))
}

/// The `rank of N` the FIGHT subhead carries: `N` is how many subjects the
/// roster ranked, read off its first ranked row.
fn rank_hint(section: &Section, ranks: Option<&SubjectRanks>) -> Option<String> {
    section.rows.iter().find_map(|r| match r {
        SectionRow::Ranked(_, _, stat) => {
            find_rank(ranks, *stat).map(|r| format!("rank of {}", r.of))
        }
        _ => None,
    })
}

/// Draws every column. Each stacks its sections in builder order; whole
/// trailing sections that do not fit are counted in one `+N more` row
/// (`fit_column`), and the bands below are never this function's concern.
pub(super) fn draw_ledger(
    columns: &[Rect; 3],
    sections: &[Section],
    ranks: Option<&SubjectRanks>,
    painter: &Painter,
    m: &Metrics,
) {
    for (col, rect) in columns.iter().enumerate() {
        let stacked: Vec<&Section> = sections
            .iter()
            .filter(|s| s.column.is_some_and(|c| c.index() == col))
            .collect();
        let heights: Vec<usize> = stacked
            .iter()
            .map(|s| section_lines(s, rect.w, ranks, painter, m))
            .collect();
        let fit = fit_column(&heights, rect.h, m);
        let mut y = rect.y;
        for (section, lines) in stacked[..fit.kept].iter().zip(&heights) {
            draw_section(section, *rect, y, ranks, painter, m);
            y += ledger_section_h(*lines, m);
        }
        if fit.dropped > 0 {
            painter.ui(
                format!("+{} more", fit.dropped),
                rect.x,
                y + section_row_h(m) * 0.85,
                m.font_size,
                hud::palette::FAINT,
            );
        }
    }
}

/// Subhead and its rule, then the rows.
fn subhead(title: &str, hint: &str, rect: Rect, y: f32, painter: &Painter, m: &Metrics) {
    let base = y + m.line_height * 0.7;
    painter.ui_bold(title, rect.x, base, m.small(), hud::palette::PANE_TITLE);
    if !hint.is_empty() {
        let title_w = painter.measure(FontFace::UiBold, title, m.small()).width;
        let hint_w = painter.measure_ui_advance(hint, m.small());
        // The title keeps its place; a hint that would touch it is dropped.
        if title_w + hint_w + m.pad <= rect.w {
            painter.ui(
                hint,
                rect.x + rect.w - hint_w,
                base,
                m.small(),
                hud::palette::FAINT,
            );
        }
    }
    painter.rect(
        rect.x,
        y + m.line_height - 1.0,
        rect.w,
        1.0,
        hud::palette::DIVIDER,
    );
}

/// How many lines a section's rows take in a column `width` wide: one each
/// unless a row wraps. The column's height budget is counted in these.
fn section_lines(
    section: &Section,
    width: f32,
    ranks: Option<&SubjectRanks>,
    painter: &Painter,
    m: &Metrics,
) -> usize {
    let reserve = reserves_compare(section, ranks);
    section
        .rows
        .iter()
        .map(|row| match row {
            SectionRow::Stat(l, v) | SectionRow::Alert(l, v) | SectionRow::Ranked(l, v, _) => {
                fit_ledger_row(painter, l, v, width, reserve, m).lines()
            }
            SectionRow::Note(_) | SectionRow::Callout(_) => 1,
        })
        .sum()
}

fn draw_section(
    section: &Section,
    rect: Rect,
    y: f32,
    ranks: Option<&SubjectRanks>,
    painter: &Painter,
    m: &Metrics,
) {
    let hint = if !section.hint.is_empty() {
        section.hint.clone()
    } else if section.column == Some(LedgerColumn::Fight) {
        rank_hint(section, ranks).unwrap_or_default()
    } else {
        String::new()
    };
    subhead(&section.title, &hint, rect, y, painter, m);
    let reserve = reserves_compare(section, ranks);
    let mut top = y + m.line_height;
    for row in &section.rows {
        let base = top + section_row_h(m) * 0.85;
        let mut lines = 1;
        match row {
            SectionRow::Stat(label, value) => {
                let pair = Pair {
                    label,
                    value,
                    value_color: hud::palette::BODY,
                    rank: None,
                    reserve,
                };
                lines = draw_pair(&pair, rect, base, painter, m);
            }
            SectionRow::Alert(label, value) => {
                let pair = Pair {
                    label,
                    value,
                    value_color: hud::palette::ATTENTION,
                    rank: None,
                    reserve,
                };
                lines = draw_pair(&pair, rect, base, painter, m);
            }
            SectionRow::Ranked(label, value, stat) => {
                let rank = find_rank(ranks, *stat);
                let pair = Pair {
                    label,
                    value,
                    value_color: hud::palette::BODY,
                    rank,
                    reserve,
                };
                lines = draw_pair(&pair, rect, base, painter, m);
            }
            SectionRow::Note(text) => painter.ui(
                elided_to_fit(painter, text, m.font_size, rect.w),
                rect.x,
                base,
                m.font_size,
                hud::palette::FAINT,
            ),
            SectionRow::Callout(text) => painter.ui(
                elided_to_fit(painter, text, m.font_size, rect.w),
                rect.x,
                base,
                m.font_size,
                hud::palette::ATTENTION,
            ),
        }
        top += section_row_h(m) * lines as f32;
    }
}

/// One label/value row and what decorates it.
struct Pair<'a> {
    label: &'a str,
    value: &'a str,
    value_color: Color,
    rank: Option<feral_processes_engine::RankRow>,
    /// The section holds a compare cell, so values line up whether or not
    /// this row has one.
    reserve: bool,
}

/// Draws the row and returns the lines it took.
fn draw_pair(pair: &Pair, rect: Rect, base: f32, painter: &Painter, m: &Metrics) -> usize {
    let f = fit_ledger_row(painter, pair.label, pair.value, rect.w, pair.reserve, m);
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let indent = INDENT_CELLS * char_w;
    let line_h = section_row_h(m);
    let at = |i: usize| base + line_h * i as f32;
    for (i, line) in f.label_lines.iter().enumerate() {
        let x = rect.x + if i > 0 { indent } else { 0.0 };
        painter.ui(line, x, at(i), m.font_size, hud::palette::FIELD_LABEL);
    }
    let compare_w = char_w * COMPARE_CELLS as f32;
    if f.inline {
        // A reserving section keeps its values aligned; a row that gave the
        // cell up takes the whole width back.
        let right = if pair.reserve && f.compare {
            rect.x + rect.w - compare_w
        } else {
            rect.x + rect.w
        };
        let value = f.value();
        painter.ui(
            &value,
            right - f.value_w,
            at(f.label_lines.len() - 1),
            m.font_size,
            pair.value_color,
        );
    } else {
        for (i, line) in f.value_lines.iter().enumerate() {
            painter.ui(
                line,
                rect.x + indent,
                at(f.label_lines.len() + i),
                m.font_size,
                pair.value_color,
            );
        }
    }
    if let (true, Some(rank)) = (f.compare, pair.rank) {
        draw_compare(rank, rect.x + rect.w - compare_w, base, painter, m);
    }
    f.lines()
}

/// Five pips (`▮` in `hud-body` over `hud-bar-trough`) and the ordinal.
fn draw_compare(
    rank: feral_processes_engine::RankRow,
    x: f32,
    base: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let lit = usize::from(rank.pips).min(PIPS);
    let filled = PIP.to_string().repeat(lit);
    let empty = PIP.to_string().repeat(PIPS - lit);
    let ordinal = format!(" {}", ordinal(rank.place));
    let char_w = painter.measure_ui_advance("M", m.font_size);
    painter.ui_runs(
        &[
            TextRun {
                text: &filled,
                bold: false,
                color: hud::palette::BODY,
            },
            TextRun {
                text: &empty,
                bold: false,
                color: hud::palette::BAR_TROUGH,
            },
            TextRun {
                text: &ordinal,
                bold: false,
                color: hud::palette::FIELD_LABEL,
            },
        ],
        x + char_w,
        base,
        m.font_size,
    );
}

/// Draws the band under the ledger into `rect`: its subhead, then rows.
pub(super) fn draw_bands(rect: Rect, band: &Band, char_w: f32, painter: &Painter, m: &Metrics) {
    subhead(band.title(), band.hint(), rect, rect.y, painter, m);
    let top = rect.y + m.line_height;
    match band {
        Band::Party(rows) => draw_party(rows, rect, top, char_w, painter, m),
        Band::Lately(rows) => {
            let split = lately_split(painter, rows, rect.w, char_w, m);
            for (i, row) in rows.iter().enumerate() {
                let base = top + section_row_h(m) * (i as f32 + 0.85);
                if let SectionRow::Stat(label, value) = row {
                    painter.ui(
                        elided_to_fit(painter, label, m.font_size, split - char_w),
                        rect.x,
                        base,
                        m.font_size,
                        hud::palette::FIELD_LABEL,
                    );
                    painter.ui(
                        elided_to_fit(painter, value, m.font_size, rect.w - split),
                        rect.x + split,
                        base,
                        m.font_size,
                        hud::palette::BODY,
                    );
                }
            }
        }
    }
}

/// Where the LATELY values start: just past the widest label, so each value
/// sits beside what it is the value of rather than at the band's far edge,
/// which is the very thing this redesign is for. Never so far right that the
/// widest value no longer fits.
pub(super) fn lately_split(
    painter: &Painter,
    rows: &[SectionRow],
    width: f32,
    char_w: f32,
    m: &Metrics,
) -> f32 {
    let widest = |pick: fn(&SectionRow) -> Option<&str>| {
        rows.iter()
            .filter_map(pick)
            .map(|t| painter.measure_ui_advance(t, m.font_size))
            .fold(0.0_f32, f32::max)
    };
    let label_w = widest(|r| match r {
        SectionRow::Stat(l, _) => Some(l.as_str()),
        _ => None,
    });
    let value_w = widest(|r| match r {
        SectionRow::Stat(_, v) => Some(v.as_str()),
        _ => None,
    });
    (label_w + 2.0 * char_w).min(width - value_w)
}

/// Cell offsets of the PARTY table's columns, from the band's left edge.
const PARTY_NAME: usize = 2;
const PARTY_INTEG: usize = 19;
const PARTY_ATK: usize = 37;
const PARTY_MIT: usize = 43;
const PARTY_LV: usize = 49;
const PARTY_DOING: usize = 54;
/// The INTEG column's meter, at `small()`.
const PARTY_METER_CELLS: usize = 6;

fn draw_party(
    rows: &[PartyRow],
    rect: Rect,
    top: f32,
    char_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let x = |col: usize| rect.x + col as f32 * char_w;
    let head_base = top + section_row_h(m) * 0.85;
    for (col, name) in [
        (PARTY_NAME, "NAME"),
        (PARTY_INTEG, "INTEG"),
        (PARTY_ATK, "ATK"),
        (PARTY_MIT, "MIT"),
        (PARTY_LV, "LV"),
        (PARTY_DOING, "DOING"),
    ] {
        painter.ui(
            name,
            x(col),
            head_base,
            m.small(),
            hud::palette::FIELD_LABEL,
        );
    }
    for (i, row) in rows.iter().enumerate() {
        let base = top + section_row_h(m) * (i as f32 + 1.85);
        let ink = if row.own {
            hud::palette::EMPHASIS
        } else {
            hud::palette::BODY
        };
        painter.map(
            row.glyph.to_string(),
            x(0),
            base,
            m.font_size,
            glyph_color(row.color),
        );
        painter.ui(
            elided_to_fit(
                painter,
                &row.name,
                m.font_size,
                (PARTY_INTEG - PARTY_NAME - 1) as f32 * char_w,
            ),
            x(PARTY_NAME),
            base,
            m.font_size,
            ink,
        );
        let b = bar(row.hp as f32, row.max_hp.max(1) as f32, PARTY_METER_CELLS);
        let meter_w = painter.measure_ui_advance("M", m.small()) * PARTY_METER_CELLS as f32;
        painter.ui_runs(
            &[
                TextRun {
                    text: &b.filled,
                    bold: false,
                    color: hud::palette::HEALTHY,
                },
                TextRun {
                    text: &b.empty,
                    bold: false,
                    color: hud::palette::BAR_TROUGH,
                },
            ],
            x(PARTY_INTEG),
            base,
            m.small(),
        );
        painter.ui(
            format!("{}/{}", row.hp, row.max_hp),
            x(PARTY_INTEG) + meter_w + char_w / 2.0,
            base,
            m.font_size,
            ink,
        );
        painter.ui(row.atk.to_string(), x(PARTY_ATK), base, m.font_size, ink);
        painter.ui(
            format!("{}%", row.mitigation),
            x(PARTY_MIT),
            base,
            m.font_size,
            ink,
        );
        painter.ui(row.level.to_string(), x(PARTY_LV), base, m.font_size, ink);
        painter.ui(
            elided_to_fit(
                painter,
                &row.doing,
                m.font_size,
                (rect.w - PARTY_DOING as f32 * char_w).max(0.0),
            ),
            x(PARTY_DOING),
            base,
            m.font_size,
            hud::palette::FIELD_LABEL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    #[test]
    fn ordinals_get_their_suffixes_and_the_teens_stay_th() {
        let read: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            read,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd",
                "101st", "111th"
            ]
        );
    }

    /// The spec's order: the compare cell goes first, and only then the
    /// label. The value is never the half that is cut.
    #[test]
    fn a_tight_ranked_row_drops_its_compare_cell_before_it_cuts_its_label() {
        let m = ui_metrics(900.0);
        let (rows, _) = with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pair = |label: &str, value: &str| {
                p.measure_ui_advance(label, m.font_size)
                    + p.measure_ui_advance(value, m.font_size)
                    + m.gap
            };
            let (label, value) = ("Mitigation", "12%");
            // Room for the pair and its compare cell, then for the pair alone,
            // then for less than the pair.
            let roomy = pair(label, value) + COMPARE_CELLS as f32 * char_w;
            let snug = pair(label, value);
            let tight = pair(label, value) - 3.0 * char_w;
            [roomy, snug, tight].map(|width| fit_ledger_row(p, label, value, width, true, &m))
        });
        let [roomy, snug, tight] = rows;
        assert!(roomy.compare && roomy.lines() == 1);
        assert!(!snug.compare && snug.lines() == 1, "the cell goes first");
        assert_eq!(snug.value(), "12%");
        assert!(
            !tight.compare && !tight.inline && tight.lines() == 2,
            "then the row wraps"
        );
        assert_eq!(tight.label_lines, ["Mitigation"], "the label is not cut");
        assert_eq!(tight.value(), "12%", "and never the value");
    }

    /// A long value continues on indented lines, whole; only a single word
    /// wider than the column is elided.
    #[test]
    fn a_long_row_wraps_and_only_an_unbreakable_word_is_elided() {
        let m = ui_metrics(720.0);
        let (rows, _) = with_painter(|p| {
            let cw = p.measure_ui_advance("M", m.font_size);
            let width = 30.0 * cw;
            (
                fit_ledger_row(
                    p,
                    "Purge",
                    "pow 24, ranged, bleed 50% for 3 rounds",
                    width,
                    false,
                    &m,
                ),
                fit_ledger_row(p, "Habitats", &"x".repeat(40), width, false, &m),
            )
        });
        let (wrapped, word) = rows;
        assert_eq!(wrapped.label_lines, ["Purge"]);
        assert!(wrapped.value_lines.len() >= 2 && !wrapped.inline);
        assert_eq!(wrapped.value(), "pow 24, ranged, bleed 50% for 3 rounds");
        assert!(
            word.value().contains('…'),
            "one word past the column is the last resort"
        );
    }

    #[test]
    fn only_a_section_the_roster_ranked_reserves_room() {
        let ranked = Section {
            title: "FIGHT".into(),
            column: Some(LedgerColumn::Fight),
            rows: vec![SectionRow::Ranked(
                "Power".into(),
                "9".into(),
                RankedStat::Power,
            )],
            full_width: false,
            overflow: 0,
            hint: String::new(),
        };
        let some = SubjectRanks {
            rows: vec![feral_processes_engine::RankRow {
                stat: RankedStat::Power,
                place: 2,
                of: 5,
                pips: 4,
            }],
        };
        assert!(reserves_compare(&ranked, Some(&some)));
        assert!(
            !reserves_compare(&ranked, None),
            "a wild subject ranks nothing"
        );
        assert_eq!(
            rank_hint(&ranked, Some(&some)).as_deref(),
            Some("rank of 5")
        );
    }
}
