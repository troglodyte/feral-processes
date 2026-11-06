//! The Workbench's routine-crafting pages (ADR 0029): Build routine and
//! Built routines, plus the two popups the build page opens.
//!
//! Every figure is a view off the engine (`Game::build_rows`,
//! `build_part_options`, `build_preview`, `built_routine_rows`); this file
//! only lays them out. Rows are returned rather than drawn so the structure
//! hub frames them like any other page.

use super::popup::*;
use super::*;
use feral_processes_app_core::RoutineDraft;
use feral_processes_engine::routine_craft::Subroutine;
use feral_processes_engine::views::BuildPreview;

/// A description or note, wrapped dim under the row it belongs to.
pub(super) fn dim_lines(text: &str) -> Vec<Row> {
    continuation_lines_within(text, HUB_WRAP_COLUMNS)
        .into_iter()
        .map(|line| colored_item_row(line, false, TEXT_DIM))
        .collect()
}

/// A plain line of a build page, wrapped to one of its two columns.
pub(super) fn text_lines(text: &str) -> Vec<Row> {
    wrap_text(text, HUB_COLUMN_WRAP_COLUMNS)
        .into_iter()
        .map(text_row)
        .collect()
}

/// `text_lines` in a colour, for the preview's materials.
pub(super) fn colored_lines(text: &str, color: Color) -> Vec<Row> {
    wrap_text(text, HUB_COLUMN_WRAP_COLUMNS)
        .into_iter()
        .map(|line| Row::TextColored(line, color))
        .collect()
}

/// How many choices a build row's list holds, beside the row's name:
/// `(3)`, or why it holds none, so an empty row reads as one to skip.
pub(super) fn options_note(count: usize, none: &str) -> String {
    if count == 0 {
        format!("({none})")
    } else {
        format!("({count})")
    }
}

/// The page's heading over the preview column.
pub(super) const OUTPUT_HEADING: &str = "Output";

pub(super) fn count_label(counted: &[(String, u32)]) -> String {
    counted
        .iter()
        .map(|(label, n)| {
            if *n > 1 {
                format!("{label} x{n}")
            } else {
                label.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn power_label(power: f32) -> String {
    if power.fract() == 0.0 {
        format!("{power:.0}")
    } else {
        format!("{power:.1}")
    }
}

/// The preview column beside the build rows.
fn preview_rows(preview: &BuildPreview) -> Vec<Row> {
    let mut rows = vec![text_row(OUTPUT_HEADING), text_row("")];
    if preview.description.is_empty() {
        rows.push(text_row("Pick a Core to see what the routine does."));
    } else {
        rows.extend(text_lines(&preview.description));
        let cooldown = match preview.cooldown {
            0 => "no cooldown".to_string(),
            1 => "cooldown 1 round".to_string(),
            n => format!("cooldown {n} rounds"),
        };
        rows.push(text_row(format!(
            "Power cost {} · {cooldown}",
            power_label(preview.power_cost)
        )));
    }
    rows.push(text_row(format!(
        "Parts {} of {}",
        preview.part_count, preview.part_cap
    )));
    if !preview.parts_used.is_empty() {
        rows.extend(text_lines(&format!(
            "Uses: {}",
            count_label(&preview.parts_used)
        )));
    }
    if !preview.parts_missing.is_empty() {
        rows.extend(text_lines(&format!(
            "Missing: {}",
            count_label(&preview.parts_missing)
        )));
    }
    rows.extend(material_rows(preview));
    if let Some(why) = &preview.blocked {
        rows.extend(text_lines(why));
    }
    rows
}

/// What Compile would craft the missing parts from, green where the stock in
/// reach covers it, and the missing parts no craft can make.
fn material_rows(preview: &BuildPreview) -> Vec<Row> {
    if preview.materials.is_empty() && preview.unresearched.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![text_row("Materials")];
    for line in &preview.materials {
        let color = if line.have >= line.need { GREEN } else { RED };
        rows.extend(colored_lines(
            &format!("{} {}/{}", line.label, line.have, line.need),
            color,
        ));
    }
    for label in &preview.unresearched {
        rows.extend(colored_lines(&format!("{label} — research first"), RED));
    }
    rows
}

/// The build page as its two columns: six part rows, the name field and
/// Compile on the left, the preview on the right.
pub(super) fn build_page_rows(
    game: &Game,
    draft: &RoutineDraft,
    selected: usize,
) -> (Vec<Row>, Vec<Row>) {
    let mut rows = text_lines("Enter on a row to choose a part. Name the routine, then Compile.");
    rows.push(text_row(""));
    for (i, row) in game.build_rows(&draft.parts).iter().enumerate() {
        let color = if row.unavailable.is_some() || row.chosen.is_empty() {
            TEXT_DIM
        } else {
            TEXT
        };
        if let Some(why) = row.unavailable {
            rows.extend(slot_with_reason(row.slot, why, i == selected));
            continue;
        }
        let count = game.build_part_options(row.slot, &draft.parts).len();
        let text = match (count, row.chosen.is_empty()) {
            (0, true) => format!("{} {}", row.slot, options_note(0, "nothing researched")),
            (_, true) => format!("{} {}: none", row.slot, options_note(count, "")),
            _ => format!(
                "{} {}: {}",
                row.slot,
                options_note(count, ""),
                count_label(&tally(&row.chosen))
            ),
        };
        rows.push(colored_item_row(text, i == selected, color));
    }
    rows.push(item_row(name_row_text(draft.name.as_str()), selected == 6));
    let preview = game.build_preview(draft.name.as_str(), &draft.parts);
    let color = if preview.blocked.is_some() {
        TEXT_DIM
    } else {
        TEXT
    };
    rows.push(colored_item_row("Compile", selected == 7, color));
    (rows, preview_rows(&preview))
}

/// A slot the Core takes nothing in: the reason inline when the column has
/// room for it, else on a dim line under the row.
fn slot_with_reason(slot: &str, why: &str, selected: bool) -> Vec<Row> {
    let inline = format!("{slot}: {why}");
    if inline.chars().count() <= HUB_COLUMN_WRAP_COLUMNS {
        return vec![colored_item_row(inline, selected, TEXT_DIM)];
    }
    let mut rows = vec![colored_item_row(
        format!("{slot}: not this Core"),
        selected,
        TEXT_DIM,
    )];
    rows.extend(
        continuation_lines_within(why, HUB_COLUMN_WRAP_COLUMNS)
            .into_iter()
            .map(|line| colored_item_row(line, false, TEXT_DIM)),
    );
    rows
}

/// The name row of either build page.
pub(super) fn name_row_text(name: &str) -> String {
    if name.is_empty() {
        "Name: (select to type one)".to_string()
    } else {
        format!("Name: {name}")
    }
}

/// Identical parts folded into `(label, count)`, in first-seen order.
fn tally(parts: &[Subroutine]) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for part in parts {
        let label = part.label();
        match out.iter_mut().find(|(l, _)| *l == label) {
            Some((_, n)) => *n += 1,
            None => out.push((label, 1)),
        }
    }
    out
}

/// The list one build row opens: the parts that fit, each with how many are
/// held, and a last row that empties the slot.
pub(super) fn pick_rows(game: &Game, draft: &RoutineDraft, selected: usize) -> Vec<Row> {
    let Some(slot) = draft.picking else {
        return vec![text_row("Nothing to choose.")];
    };
    let options = game.build_part_options(slot, &draft.parts);
    let mut rows = vec![text_row(format!("Choose a part for {slot}."))];
    for (i, option) in options.iter().enumerate() {
        let craft = match &option.craft_cost {
            Some((material, n)) => format!("   1 disk + {n} {}", game.item_name(material)),
            None => String::new(),
        };
        let (note, color) = if option.held > 0 {
            (format!("{} held{craft}", option.held), TEXT)
        } else {
            (format!("none held{craft}"), TEXT_DIM)
        };
        rows.push(annotated_item_row(
            option.label.clone(),
            Some(note),
            i == selected,
            color,
        ));
    }
    rows.push(colored_item_row(
        format!("Clear {slot}"),
        selected == options.len(),
        TEXT_DIM,
    ));
    rows
}

/// The name field's popup.
pub(super) fn name_rows(draft: &RoutineDraft) -> Vec<Row> {
    name_field_rows(
        "Name the routine (1-24 characters)".to_string(),
        &draft.name,
        &["Type a name", "Left/Right move", "Enter or Esc finish"],
    )
}

/// The Built routines page: routines waiting for a slot.
pub(super) fn built_page_rows(game: &Game, selected: usize) -> Vec<Row> {
    let built = game.built_routine_rows();
    let mut rows = vec![text_row(
        "Built routines not in a slot. Install one from the routines screen (m).",
    )];
    if built.is_empty() {
        rows.push(text_row(
            "(nothing waiting - compile one on the Build routine page)",
        ));
    }
    for (i, row) in built.iter().enumerate() {
        rows.push(item_row(row.name.clone(), i == selected));
        rows.extend(dim_lines(&row.description));
        rows.extend(dim_lines(&format!("Made from {}", row.parts.join(", "))));
    }
    rows
}

pub(super) fn draw_pick(
    game: &Game,
    draft: &RoutineDraft,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = pick_rows(game, draft, selected);
    draw_popup("Choose part", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_name(
    draft: &RoutineDraft,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Name routine",
        PopupSize::Large,
        &name_rows(draft),
        refusal,
        painter,
        m,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::routine_craft::{Core, Version};
    use feral_processes_engine::routine_tree::part_node_id;

    fn item_texts(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .filter_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// Both columns fit at the smallest window under every Core, which is
    /// what puts each slot's longest refusal on the page.
    #[test]
    fn the_build_page_columns_fit_under_every_core() {
        let mut game = super::super::test_support::game_with_a_workbench_and_designs(43, vec![]);
        let cores: Vec<Subroutine> = Subroutine::all()
            .into_iter()
            .filter(|p| p.slot() == "Core")
            .collect();
        for core in &cores {
            let _ = game.dev_unlock_research(&part_node_id(core));
        }
        for core in cores {
            let draft = RoutineDraft {
                parts: vec![core],
                name: "W".repeat(24).as_str().into(),
                ..RoutineDraft::default()
            };
            let (left, output) = build_page_rows(&game, &draft, 0);
            super::super::base_report::assert_column_rows_fit(&left, "build routine rows");
            super::super::base_report::assert_column_rows_fit(&output, "build routine output");
        }
    }

    /// Each slot row says how many choices its list holds, or why it holds
    /// none, so the player knows to skip it.
    #[test]
    fn every_build_row_counts_its_choices() {
        let mut game = super::super::test_support::game_with_a_workbench_and_designs(44, vec![]);
        let heal = Subroutine::Core(Core::Heal, Version::V1);
        let _ = game.dev_unlock_research(&part_node_id(&heal));
        let cores = game.build_part_options("Core", &[]).len();
        assert!(cores >= 1, "premise: Heal is researched");
        let empty = RoutineDraft::default();
        let (left, _) = build_page_rows(&game, &empty, 0);
        let texts = item_texts(&left);
        assert_eq!(texts[0], format!("Core ({cores}): none"));
        assert!(texts[1].starts_with("Range: "), "{texts:?}");

        let draft = RoutineDraft {
            parts: vec![heal],
            ..RoutineDraft::default()
        };
        let (left, output) = build_page_rows(&game, &draft, 0);
        let texts = item_texts(&left);
        assert_eq!(texts[0], format!("Core ({cores}): {}", heal.label()));
        assert!(
            texts.iter().any(|t| t.ends_with("(nothing researched)")),
            "{texts:?}"
        );
        assert_eq!(row_label_text(&output[0]), OUTPUT_HEADING);
    }
}
