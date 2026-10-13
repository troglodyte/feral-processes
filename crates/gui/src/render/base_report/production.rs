//! The Production page's Machines and Power screens. Every figure comes from
//! the engine's report; this only formats.

use super::staff::elide;
use super::*;
use feral_processes_engine::components::MachineStatus;
use feral_processes_engine::{MachineRow, PowerReport};

/// The widest a structure's name column grows.
const LABEL_CHARS: usize = 22;

fn status_color(status: MachineStatus) -> Color {
    match status {
        MachineStatus::Unpowered | MachineStatus::Dry | MachineStatus::Stranded => RED,
        MachineStatus::Starved | MachineStatus::Unstaffed | MachineStatus::Clogged => YELLOW,
        MachineStatus::Idle | MachineStatus::Running => TEXT,
    }
}

fn label_width(labels: impl Iterator<Item = usize>) -> usize {
    labels.max().unwrap_or(0).min(LABEL_CHARS)
}

pub(super) fn machine_rows(report: &[MachineRow], selected: usize) -> Vec<Row> {
    if report.is_empty() {
        return vec![
            Row::TextColored("Machines".to_string(), TEXT_DIM),
            text_row("Every machine is running and undamaged."),
        ];
    }
    let label_w = label_width(report.iter().map(|r| r.label.chars().count()));
    let mut rows = vec![Row::TextColored(
        format!("Machines - {} need a look", report.len()),
        TEXT_DIM,
    )];
    rows.extend(report.iter().enumerate().map(|(i, r)| {
        let what = match r.status {
            MachineStatus::Running => "damaged",
            other => other.as_str(),
        };
        let reason = if r.status == MachineStatus::Running {
            String::new()
        } else {
            format!(" - {}", r.status.reason())
        };
        let damage = r
            .damage
            .map(|(hp, max)| format!("  hp {hp}/{max}"))
            .unwrap_or_default();
        colored_item_row(
            format!(
                "{:<label_w$}  ({}, {})  {what}{reason}{damage}",
                elide(&r.label, label_w),
                r.pos.0,
                r.pos.1
            ),
            i == selected,
            status_color(r.status),
        )
    }));
    rows
}

pub(super) fn power_rows(report: &PowerReport, selected: usize) -> Vec<Row> {
    let short = report.draw > report.supply;
    let mut rows = vec![Row::TextColored(
        format!(
            "Power - drawing {} of {} supplied{}",
            report.draw,
            report.supply,
            if short { " - short" } else { "" }
        ),
        if short { RED } else { TEXT },
    )];
    if report.lines.is_empty() {
        rows.push(text_row("No structure supplies or draws power."));
        return rows;
    }
    let label_w = label_width(report.lines.iter().map(|l| l.label.chars().count()));
    rows.extend(report.lines.iter().enumerate().map(|(i, l)| {
        let flow = match (l.supply, l.draw) {
            (s, 0) => format!("supplies {s}"),
            (0, d) => format!("draws {d}"),
            (s, d) => format!("supplies {s}, draws {d}"),
        };
        let dark = if l.dark { "  dark" } else { "" };
        colored_item_row(
            format!(
                "{:<label_w$}  ({}, {})  {flow}{dark}",
                elide(&l.label, label_w),
                l.pos.0,
                l.pos.1
            ),
            i == selected,
            if l.dark { RED } else { TEXT },
        )
    }));
    rows
}

#[cfg(test)]
mod tests {
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;
    use feral_processes_engine::{Entity, PowerLine};

    fn machine(status: MachineStatus, damage: Option<(u32, u32)>) -> MachineRow {
        MachineRow {
            entity: Entity::from_raw_u32(1).unwrap(),
            label: "Mining Node".into(),
            pos: (3, -4),
            status,
            damage,
        }
    }

    #[test]
    fn a_machine_row_names_its_status_reason_and_damage() {
        let rows = machine_rows(&[machine(MachineStatus::Starved, Some((2, 5)))], 0);
        let t = text_of(&rows[1]);
        assert!(t.contains("starved - nothing is feeding it"), "{t}");
        assert!(t.contains("hp 2/5") && t.contains("(3, -4)"), "{t}");
        let rows = machine_rows(&[machine(MachineStatus::Running, Some((1, 5)))], 0);
        assert!(text_of(&rows[1]).contains("damaged"));
    }

    #[test]
    fn the_power_header_says_short_and_a_dark_line_says_dark() {
        let report = PowerReport {
            draw: 5,
            supply: 4,
            lines: vec![PowerLine {
                label: "Lathe".into(),
                pos: (1, 1),
                supply: 0,
                draw: 3,
                dark: true,
            }],
        };
        let rows = power_rows(&report, 0);
        assert!(
            text_of(&rows[0]).contains("Power - drawing 5 of 4")
                && text_of(&rows[0]).contains("short")
        );
        assert!(text_of(&rows[1]).contains("draws 3") && text_of(&rows[1]).ends_with("dark"));
    }

    #[test]
    fn the_widest_machine_and_power_rows_fit_the_content_rect() {
        let mut worst = machine(MachineStatus::Unstaffed, Some((9999, 9999)));
        worst.label = "N".repeat(40);
        worst.pos = (-9999, -9999);
        assert_rows_fit(&machine_rows(&[worst], 0), "Machines");
        let report = PowerReport {
            draw: u32::MAX,
            supply: u32::MAX,
            lines: vec![PowerLine {
                label: "N".repeat(40),
                pos: (-9999, -9999),
                supply: u32::MAX,
                draw: u32::MAX,
                dark: true,
            }],
        };
        assert_rows_fit(&power_rows(&report, 0), "Power");
    }

    #[test]
    fn empty_reports_say_so() {
        assert!(text_of(&machine_rows(&[], 0)[1]).contains("running"));
    }
}
