//! The Splice Rig's screen: Load against the cap, what is built in, and the
//! implants in the pack.
//!
//! One list, installed rows first and then the pack's, matching
//! `SpliceRigScreen::rows` index for index — the cursor is `menu_selected`
//! over the whole of it. Upkeep is shown as the extra share of Power an
//! implant drinks, because the engine's figure is per tick and the player
//! never reads in ticks.

use super::popup::*;
use super::*;
use feral_processes_app_core::SpliceRigScreen;
use feral_processes_engine::tuning::HUNGER_DECAY_PER_TICK;
use feral_processes_engine::{InstallableImplantRow, InstalledImplantRow};

const COLUMN_GAP: &str = "  ";

pub(super) fn draw_splice_rig(
    screen: Option<&SpliceRigScreen>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = match screen {
        Some(screen) => body_rows(screen, selected),
        // Unreachable through `App`, which closes the screen when the rig
        // stops being beside the party — drawn rather than asserted, because
        // a renderer that panics takes the window with it.
        None => vec![text_row("There is no Splice Rig here.")],
    };
    draw_popup("Splice Rig", PopupSize::Large, &body, refusal, painter, m);
}

/// The extra share of Power drain an upkeep figure stands for, as a whole
/// percent.
fn drain_pct(upkeep: f32) -> u32 {
    (upkeep / HUNGER_DECAY_PER_TICK * 100.0).round() as u32
}

fn installed_label(row: &InstalledImplantRow) -> String {
    if !row.known {
        return format!("{}{COLUMN_GAP}unknown implant", row.name);
    }
    format!(
        "{}{COLUMN_GAP}load {}{COLUMN_GAP}+{}% power drain{COLUMN_GAP}removal {} core fragments",
        row.name,
        row.load,
        drain_pct(row.upkeep),
        row.removal_fragments
    )
}

fn installable_label(row: &InstallableImplantRow) -> String {
    format!(
        "{} x{}{COLUMN_GAP}load {}{COLUMN_GAP}+{}% power drain",
        row.name,
        row.count,
        row.load,
        drain_pct(row.upkeep)
    )
}

fn body_rows(screen: &SpliceRigScreen, selected: usize) -> Vec<Row> {
    let view = &screen.view;
    let load = format!("Neural Load {}/{}", view.load, view.cap);
    let mut body = vec![if view.overload > 0 {
        Row::TextColored(
            format!("{load} - over the cap by {}", view.overload),
            ORANGE,
        )
    } else {
        text_row(load)
    }];
    if view.overload > 0 {
        body.push(Row::TextColored(
            "Each fight you start over the cap risks a rejection that hobbles you.".to_string(),
            ORANGE,
        ));
    }
    body.push(text_row(format!(
        "{} core fragments in your pack. Taking an implant out costs them.",
        view.fragments
    )));
    body.push(text_row(""));

    if let Some(question) = &screen.confirm {
        let name = view
            .installable
            .iter()
            .find(|row| &row.item == question)
            .map_or_else(|| question.as_str().to_string(), |row| row.name.clone());
        body.extend([
            Row::TextColored(
                format!(
                    "Splice {name} past the Load cap? Every fight you start over it can open with a rejection."
                ),
                ORANGE,
            ),
            text_row("[Y] splice it anyway   [N] or Esc to think again"),
        ]);
        return body;
    }

    body.push(text_row(
        "[I] install the highlighted pack implant  [R] remove an installed one",
    ));
    body.push(text_row("Esc to go back"));
    body.push(text_row(""));
    body.push(text_row("Installed"));
    if view.installed.is_empty() {
        body.push(text_row("  nothing built in yet"));
    }
    let n = view.installed.len();
    for (i, row) in view.installed.iter().enumerate() {
        body.push(item_row(installed_label(row), i == selected));
        if let Some(downside) = &row.downside {
            body.push(text_row(format!("      {downside}")));
        }
    }
    body.push(text_row(""));
    body.push(text_row("In your pack"));
    if view.installable.is_empty() {
        body.push(text_row("  no implants to splice"));
    }
    for (i, row) in view.installable.iter().enumerate() {
        body.push(item_row(installable_label(row), n + i == selected));
        if let Some(downside) = &row.downside {
            body.push(text_row(format!("      {downside}")));
        }
    }
    let description = view
        .installed
        .get(selected)
        .map(|r| r.description.as_str())
        .or_else(|| {
            view.installable
                .get(selected.wrapping_sub(n))
                .map(|r| r.description.as_str())
        })
        .filter(|d| !d.is_empty());
    if let Some(description) = description {
        body.push(text_row(""));
        body.push(text_row(description));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::ImplantView;
    use feral_processes_engine::implants::ImplantId;
    use feral_processes_engine::items::ItemId;

    fn screen(overload: u32) -> SpliceRigScreen {
        SpliceRigScreen {
            view: ImplantView {
                load: 4 + overload,
                cap: 4,
                overload,
                fragments: u32::MAX,
                installed: vec![
                    InstalledImplantRow {
                        id: ImplantId::from("dermal_lattice"),
                        name: "Dermal Lattice".to_string(),
                        known: true,
                        description: "A rigid mesh.".to_string(),
                        load: u32::MAX,
                        upkeep: 0.15,
                        downside: Some("Trace rises 20% faster.".to_string()),
                        removal_fragments: u32::MAX,
                    },
                    InstalledImplantRow {
                        id: ImplantId::from("gone"),
                        name: "gone".to_string(),
                        known: false,
                        description: String::new(),
                        load: 0,
                        upkeep: 0.0,
                        downside: None,
                        removal_fragments: 0,
                    },
                ],
                installable: vec![InstallableImplantRow {
                    item: ItemId::from("black_ledger"),
                    name: "Black Ledger".to_string(),
                    description: "Skims drops.".to_string(),
                    load: u32::MAX,
                    upkeep: 0.15,
                    downside: None,
                    count: u32::MAX,
                }],
            },
            confirm: None,
        }
    }

    fn texts(rows: &[Row]) -> Vec<String> {
        rows.iter().map(row_label_text).collect()
    }

    #[test]
    fn load_is_used_over_cap_and_overload_is_in_the_warning_colour() {
        let calm = body_rows(&screen(0), 0);
        assert!(texts(&calm)[0].contains("Neural Load 4/4"));
        assert!(!matches!(calm[0], Row::TextColored(..)));

        let over = body_rows(&screen(2), 0);
        assert!(texts(&over)[0].contains("Neural Load 6/4"));
        assert!(matches!(over[0], Row::TextColored(_, c) if c == ORANGE));
    }

    #[test]
    fn an_unknown_implant_reads_as_unknown_and_the_cursor_spans_both_lists() {
        let rows = body_rows(&screen(0), 1);
        let all = texts(&rows);
        assert!(
            all.iter()
                .any(|t| t.contains("gone") && t.contains("unknown"))
        );
        let marked: Vec<_> = rows
            .iter()
            .filter(|r| matches!(r, Row::Item { selected: true, .. }))
            .collect();
        assert_eq!(marked.len(), 1);
        let pack = body_rows(&screen(0), 2);
        assert!(pack.iter().any(
            |r| matches!(r, Row::Item { selected: true, text, .. } if text.contains("Black Ledger"))
        ));
    }

    #[test]
    fn the_confirm_names_the_rejection_risk() {
        let mut s = screen(0);
        s.confirm = Some(ItemId::from("black_ledger"));
        let all = texts(&body_rows(&s, 0)).join("\n");
        assert!(all.contains("rejection"));
        assert!(all.contains("Black Ledger"));
    }

    #[test]
    fn no_splice_row_overflows_its_popup() {
        let s = screen(0);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in texts(&body_rows(&s, 0)) {
                let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                assert!(drawn > 0.0);
                // Prose lines are wrapped by the popup; only the table rows
                // carry unbounded figures, and those must fit unwrapped.
                if line.contains("load ") {
                    assert!(
                        drawn <= room,
                        "a splice row overflows by {:.0}px:\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }
}
