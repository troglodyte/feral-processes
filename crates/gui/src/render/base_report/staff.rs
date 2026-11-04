//! The Staff page's three screens: Wellbeing, Coverage and Discontent. Every
//! figure comes from the engine's report; this only formats.

use super::*;
use feral_processes_engine::{
    CoverageRow, DiscontentRow, StaffActivity, WellbeingNeed, WellbeingRow,
};

/// Cells in a need's bar.
const BAR_CELLS: usize = 5;

/// The widest a program's name column grows.
const NAME_CHARS: usize = 20;

/// The widest a need's name column grows on the Coverage screen, whose rows
/// carry six counts after it.
const NEED_NAME_CHARS: usize = 14;

/// Letters of a need's name kept in front of its bar.
const NEED_TAG_CHARS: usize = 4;

/// A meter of `BAR_CELLS` cells, `fraction` of them filled.
fn bar(fraction: f32) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR_CELLS as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(BAR_CELLS - filled))
}

/// `name` cut to `width` cells with an ellipsis, never clipped silently.
pub(super) fn elide(name: &str, width: usize) -> String {
    if name.chars().count() <= width {
        return name.to_string();
    }
    let mut cut: String = name.chars().take(width.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

fn need_cell(need: &WellbeingNeed) -> String {
    let tag: String = need.name.chars().take(NEED_TAG_CHARS).collect();
    let mark = if need.below_critical { '!' } else { ' ' };
    format!("{tag} {}{mark}", bar(need.fraction))
}

fn activity_text(activity: &StaffActivity) -> String {
    match activity {
        StaffActivity::OnShift => "on shift".to_string(),
        StaffActivity::Servicing(verb) => verb.to_lowercase(),
        StaffActivity::Waiting => "waiting".to_string(),
        StaffActivity::Downed => "downed".to_string(),
        StaffActivity::Break => "on a break".to_string(),
    }
}

pub(super) fn wellbeing_rows(report: &[WellbeingRow], selected: usize) -> Vec<Row> {
    if report.is_empty() {
        return vec![text_row("No program is on the base staff.")];
    }
    let name_w = report
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(NAME_CHARS);
    let mood_w = report
        .iter()
        .map(|r| r.mood.chars().count())
        .max()
        .unwrap_or(0);
    report
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let needs: Vec<String> = r.needs.iter().map(need_cell).collect();
            let name = elide(&r.name, name_w);
            let text = format!(
                "{name:<name_w$}  {:<mood_w$} {:>3}/{:<3}  {}  {}",
                r.mood,
                r.hp,
                r.max_hp,
                needs.join("  "),
                activity_text(&r.activity),
            );
            let color = if r.is_critical() || r.activity == StaffActivity::Downed {
                RED
            } else if r.grievance.is_some() || matches!(r.activity, StaffActivity::Waiting) {
                YELLOW
            } else {
                TEXT
            };
            colored_item_row(text, i == selected, color)
        })
        .collect()
}

pub(super) fn coverage_rows(report: &[CoverageRow], selected: usize) -> Vec<Row> {
    if report.is_empty() {
        return vec![text_row("Nothing here needs servicing.")];
    }
    let name_w = report
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(NEED_NAME_CHARS);
    let mut rows = vec![Row::TextColored(
        "Approximate: walking time is not counted.".to_string(),
        TEXT_DIM,
    )];
    for (i, r) in report.iter().enumerate() {
        let short = if r.is_short() { " !" } else { "" };
        let text = format!(
            "{:<name_w$}  built {} · places {}/{} · waited {}{short}  critical {} · low {}  lost ~{}%",
            elide(&r.name, name_w),
            r.built,
            r.places,
            r.capacity,
            r.waited,
            r.below_critical,
            r.below_content,
            (r.lost_share * 100.0).round() as u32,
        );
        rows.push(colored_item_row(
            text,
            i == selected,
            if r.is_short() { YELLOW } else { TEXT },
        ));
        if r.is_short() && !r.built_by.is_empty() {
            rows.push(Row::TextColored(build_hint(&r.built_by), TEXT_DIM));
        }
        for b in &r.boxed_in {
            rows.push(Row::TextColored(
                format!(
                    "    boxed in: {} at ({}, {}) has {} open tile{}",
                    b.label,
                    b.x,
                    b.y,
                    b.open_tiles,
                    if b.open_tiles == 1 { "" } else { "s" },
                ),
                YELLOW,
            ));
        }
    }
    rows
}

/// "    build a Sandbox or an Idle Loop": the structures that would close a
/// short need's gap, since the need's own name tells the player nothing.
fn build_hint(names: &[String]) -> String {
    let options: Vec<String> = names
        .iter()
        .map(|n| {
            let article = if n.starts_with(['A', 'E', 'I', 'O', 'U']) {
                "an"
            } else {
                "a"
            };
            format!("{article} {n}")
        })
        .collect();
    format!("    build {}", options.join(" or "))
}

pub(super) fn discontent_rows(report: &[DiscontentRow], selected: usize) -> Vec<Row> {
    if report.is_empty() {
        return vec![text_row("Nobody on the staff has a grievance.")];
    }
    let mut rows = Vec::new();
    for (i, r) in report.iter().enumerate() {
        rows.push(colored_item_row(
            format!("{}  {} · mood {}", r.name, r.rung, r.mood),
            i == selected,
            if r.stopped_work { RED } else { YELLOW },
        ));
        for g in &r.grudges {
            let text = match &g.subject {
                Some(who) => format!("    {} ({who})", g.name),
                None => format!("    {}", g.name),
            };
            rows.push(Row::TextColored(text, TEXT_DIM));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;
    use feral_processes_engine::{BoxedInAmenity, Entity, GrudgeRow};

    fn wellbeing(critical: bool) -> WellbeingRow {
        WellbeingRow {
            entity: Entity::from_raw_u32(1).unwrap(),
            name: "Ada".into(),
            morale: -3.0,
            mood: "uneasy",
            hp: 12,
            max_hp: 30,
            needs: vec![WellbeingNeed {
                name: "Coherence".into(),
                fraction: if critical { 0.1 } else { 0.8 },
                below_critical: critical,
            }],
            activity: StaffActivity::Waiting,
            grievance: None,
        }
    }

    #[test]
    fn a_wellbeing_row_names_the_program_its_bar_and_what_it_waits_for() {
        let rows = wellbeing_rows(&[wellbeing(true)], 0);
        let t = text_of(&rows[0]);
        assert!(t.starts_with("Ada"), "{t}");
        assert!(t.contains("12/30") && t.contains("Cohe █"), "{t}");
        assert!(t.contains("!"), "a need under critical is marked: {t}");
        assert!(t.contains("waiting"), "{t}");
    }

    #[test]
    fn coverage_marks_a_short_need_and_lists_the_boxed_in_amenity() {
        let row = CoverageRow {
            name: "Coherence".into(),
            built: 1,
            places: 1,
            capacity: 2,
            visits: 4,
            waited: 2,
            stranded: 0,
            below_critical: 1,
            below_content: 3,
            lost_share: 0.07,
            built_by: vec!["Defrag Bay".into()],
            boxed_in: vec![BoxedInAmenity {
                label: "Defrag Bay".into(),
                x: 3,
                y: 2,
                open_tiles: 1,
            }],
        };
        let rows = coverage_rows(&[row], 0);
        assert!(text_of(&rows[0]).contains("Approximate"));
        let t = text_of(&rows[1]);
        assert!(t.contains("places 1/2") && t.contains("waited 2 !") && t.contains("lost ~7%"));
        assert_eq!(text_of(&rows[2]), "    build a Defrag Bay");
        assert!(text_of(&rows[3]).contains("1 open tile"));
    }

    fn covered(built: usize, needed: usize, built_by: &[&str]) -> CoverageRow {
        CoverageRow {
            name: "Slack".into(),
            built,
            places: built * 2,
            capacity: built * 2,
            visits: needed,
            waited: usize::from(built < needed),
            stranded: 0,
            below_critical: 0,
            below_content: 0,
            lost_share: 0.1,
            built_by: built_by.iter().map(|s| (*s).into()).collect(),
            boxed_in: Vec::new(),
        }
    }

    #[test]
    fn a_short_need_names_the_building_that_serves_it_indented_and_dim() {
        let rows = coverage_rows(&[covered(0, 1, &["Sandbox"])], 0);
        let Row::TextColored(t, color) = &rows[2] else {
            panic!("the hint is a plain dim row");
        };
        assert_eq!(t, "    build a Sandbox");
        assert_eq!(*color, TEXT_DIM);
    }

    #[test]
    fn the_build_hint_joins_alternatives_and_uses_an_before_a_vowel() {
        let rows = coverage_rows(&[covered(1, 2, &["Sandbox", "Idle Loop"])], 0);
        assert_eq!(text_of(&rows[2]), "    build a Sandbox or an Idle Loop");
    }

    #[test]
    fn a_covered_need_has_no_build_hint() {
        let rows = coverage_rows(&[covered(1, 1, &["Sandbox"])], 0);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn discontent_lists_each_grudge_under_its_program() {
        let row = DiscontentRow {
            entity: Entity::from_raw_u32(1).unwrap(),
            name: "Ada".into(),
            rung: "refusing to work",
            stopped_work: true,
            mood: "low",
            grudges: vec![GrudgeRow {
                name: "Mauled by".into(),
                subject: Some("a Wolf".into()),
                intensity: -5.0,
            }],
        };
        let rows = discontent_rows(&[row], 0);
        assert!(text_of(&rows[0]).contains("refusing to work"));
        assert!(text_of(&rows[1]).contains("Mauled by (a Wolf)"));
    }

    #[test]
    fn the_widest_wellbeing_row_fits_the_content_rect() {
        let mut worst = wellbeing(true);
        worst.name = "N".repeat(40);
        worst.mood = "devoted";
        worst.hp = 9999;
        worst.max_hp = 9999;
        worst.needs = ["Coherence", "Slack", "Warmth"]
            .iter()
            .map(|n| WellbeingNeed {
                name: (*n).into(),
                fraction: 0.5,
                below_critical: true,
            })
            .collect();
        assert_rows_fit(&wellbeing_rows(&[worst], 0), "Wellbeing");
    }

    #[test]
    fn the_widest_coverage_row_fits_the_content_rect() {
        let worst = CoverageRow {
            name: "N".repeat(40),
            built: 100,
            places: 200,
            capacity: 200,
            visits: 100,
            waited: 10,
            stranded: 100,
            below_critical: 100,
            below_content: 100,
            lost_share: 1.0,
            built_by: vec!["N".repeat(40)],
            boxed_in: vec![BoxedInAmenity {
                label: "Defrag Bay".into(),
                x: -100,
                y: -100,
                open_tiles: 1,
            }],
        };
        let short = CoverageRow {
            built: 0,
            ..worst.clone()
        };
        assert_rows_fit(&coverage_rows(&[worst, short], 0), "Coverage");
    }

    #[test]
    fn empty_reports_say_so() {
        assert!(text_of(&wellbeing_rows(&[], 0)[0]).contains("No program"));
        assert!(text_of(&discontent_rows(&[], 0)[0]).contains("Nobody"));
    }
}
