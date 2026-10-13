//! The Build orders page's three screens: Builds, Digging and Floors. Every
//! figure comes from the engine's report; this only formats.

use super::staff::elide;
use super::*;
use feral_processes_engine::{BuildOrderRow, DigMark, FloorMark};

/// The widest a structure's name column grows.
const NAME_CHARS: usize = 22;

/// The widest the list of parts still to fetch is drawn.
const PARTS_CHARS: usize = 60;

/// The screen's name and count, pinned above its rows: the page's dots say
/// there are three screens and not which this is.
fn heading(name: &str, count: usize, noun: &str) -> Row {
    Row::TextColored(format!("{name} - {count} {noun}"), TEXT_DIM)
}

pub(super) fn build_rows(report: &[BuildOrderRow], selected: usize) -> Vec<Row> {
    let head = heading("Builds", report.len(), "on order");
    if report.is_empty() {
        return vec![head, text_row("No structure is on order.")];
    }
    let name_w = report
        .iter()
        .map(|r| r.label().chars().count())
        .max()
        .unwrap_or(0)
        .min(NAME_CHARS);
    let mut rows = vec![head];
    for (i, r) in report.iter().enumerate() {
        let who = if r.awaiting_program {
            "awaiting a program".to_string()
        } else {
            match &r.builder {
                Some(who) => who.clone(),
                None => "pending".to_string(),
            }
        };
        let color = if r.awaiting_program { RED } else { TEXT };
        rows.push(colored_item_row(
            format!(
                "{:<name_w$}  ({}, {})  {:>3}%  {who}",
                elide(&r.label(), name_w),
                r.pos.0,
                r.pos.1,
                r.percent(),
            ),
            i == selected,
            color,
        ));
        if !r.awaiting_program && !r.outstanding.is_empty() {
            let parts: Vec<String> = r
                .outstanding
                .iter()
                .map(|(name, qty)| format!("{qty} {name}"))
                .collect();
            rows.push(Row::TextColored(
                format!(
                    "    still to fetch: {}",
                    elide(&parts.join(", "), PARTS_CHARS)
                ),
                TEXT_DIM,
            ));
        }
    }
    rows
}

pub(super) fn dig_rows(report: &[DigMark], selected: usize) -> Vec<Row> {
    let head = heading("Digging", report.len(), "marked");
    if report.is_empty() {
        return vec![head, text_row("No cell is marked for digging.")];
    }
    std::iter::once(head)
        .chain(report.iter().enumerate().map(|(i, m)| {
            let what = match m.cut {
                Some(cut) => format!("{}% cut", (cut * 100.0).round() as u32),
                None => "open, to be tiled".to_string(),
            };
            item_row(format!("({}, {})  {what}", m.pos.0, m.pos.1), i == selected)
        }))
        .collect()
}

pub(super) fn floor_rows(report: &[FloorMark], selected: usize) -> Vec<Row> {
    let head = heading("Floors", report.len(), "marked");
    if report.is_empty() {
        return vec![head, text_row("No cell is marked for a floor finish.")];
    }
    std::iter::once(head)
        .chain(report.iter().enumerate().map(|(i, m)| {
            let what = match &m.finish {
                Some(name) => format!("lay {name}"),
                None => "strip its finish".to_string(),
            };
            item_row(format!("({}, {})  {what}", m.pos.0, m.pos.1), i == selected)
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;
    use feral_processes_engine::Entity;
    use feral_processes_engine::components::BuildGoal;

    fn build(awaiting: bool) -> BuildOrderRow {
        BuildOrderRow {
            entity: Entity::from_raw_u32(1).unwrap(),
            pos: (2, 3),
            goal: BuildGoal::New,
            structure: "Lathe".into(),
            delivered: 1,
            materials: 4,
            outstanding: vec![("Core Fragment".into(), 3)],
            ticks: 0,
            required_ticks: 10,
            builder: None,
            awaiting_program: awaiting,
        }
    }

    #[test]
    fn a_build_row_says_who_is_on_it_and_what_is_still_to_fetch() {
        let rows = build_rows(&[build(false)], 0);
        assert!(
            text_of(&rows[1]).contains("pending"),
            "{}",
            text_of(&rows[1])
        );
        assert!(text_of(&rows[2]).contains("3 Core Fragment"));
        let rows = build_rows(&[build(true)], 0);
        assert!(text_of(&rows[1]).contains("awaiting a program"));
        assert_eq!(rows.len(), 2, "a waiting wreck is not fetching anything");
    }

    #[test]
    fn dig_and_floor_rows_word_their_marks() {
        let dig = dig_rows(
            &[
                DigMark {
                    pos: (1, 2),
                    cut: Some(0.5),
                },
                DigMark {
                    pos: (3, 4),
                    cut: None,
                },
            ],
            0,
        );
        assert!(text_of(&dig[1]).contains("50% cut"));
        assert!(text_of(&dig[2]).contains("tiled"));
        let floor = floor_rows(
            &[
                FloorMark {
                    pos: (1, 2),
                    finish: Some("Moss Weave".into()),
                },
                FloorMark {
                    pos: (3, 4),
                    finish: None,
                },
            ],
            0,
        );
        assert!(text_of(&floor[1]).contains("lay Moss Weave"));
        assert!(text_of(&floor[2]).contains("strip"));
    }

    #[test]
    fn the_widest_order_rows_fit_the_content_rect() {
        let mut worst = build(false);
        worst.structure = "N".repeat(40);
        worst.pos = (-9999, -9999);
        worst.builder = Some("M".repeat(20));
        worst.outstanding = (0..8).map(|i| (format!("Material {i}"), 9999)).collect();
        assert_rows_fit(&build_rows(&[worst], 0), "Builds");
        assert_rows_fit(
            &dig_rows(
                &[DigMark {
                    pos: (-9999, -9999),
                    cut: None,
                }],
                0,
            ),
            "Digging",
        );
        assert_rows_fit(
            &floor_rows(
                &[FloorMark {
                    pos: (-9999, -9999),
                    finish: Some("N".repeat(60)),
                }],
                0,
            ),
            "Floors",
        );
    }
}
