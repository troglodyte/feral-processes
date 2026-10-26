//! The Workbench's routine-crafting pages (ADR 0029): Build routine,
//! Subroutines and Built routines, plus the two popups the build page opens.
//!
//! Every figure is a view off the engine (`Game::build_rows`,
//! `build_part_options`, `build_preview`, `subroutine_rows`,
//! `built_routine_rows`); this file only lays them out. Rows are returned
//! rather than drawn so the structure hub frames them like any other page.

use super::list_filter::{filter_line, no_match_row};
use super::popup::*;
use super::*;
use feral_processes_app_core::{RoutineDraft, subroutines_visible};
use feral_processes_engine::routine_craft::Subroutine;
use feral_processes_engine::views::BuildPreview;

/// A description or note, wrapped dim under the row it belongs to.
fn dim_lines(text: &str) -> Vec<Row> {
    continuation_lines_within(text, HUB_WRAP_COLUMNS)
        .into_iter()
        .map(|line| colored_item_row(line, false, TEXT_DIM))
        .collect()
}

/// A plain wrapped line of the preview. `Row::Text` so the block after the
/// last item is the pinned footer.
fn text_lines(text: &str) -> Vec<Row> {
    wrap_text(text, HUB_WRAP_COLUMNS)
        .into_iter()
        .map(text_row)
        .collect()
}

fn count_label(counted: &[(String, u32)]) -> String {
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

/// The preview block under the build rows.
fn preview_rows(preview: &BuildPreview) -> Vec<Row> {
    let mut rows = vec![text_row("")];
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
    if let Some(why) = &preview.blocked {
        rows.extend(text_lines(why));
    }
    rows
}

/// The build page: six part rows, the name field, Compile, then the preview.
pub(super) fn build_page_rows(game: &Game, draft: &RoutineDraft, selected: usize) -> Vec<Row> {
    let mut rows = vec![
        text_row("Enter on a row to choose a part. Name the routine, then Compile."),
        text_row(""),
    ];
    for (i, row) in game.build_rows(&draft.parts).iter().enumerate() {
        let text = match row.unavailable {
            Some(why) => format!("{}: {why}", row.slot),
            None if row.chosen.is_empty() => format!("{}: (none)", row.slot),
            None => {
                let labels: Vec<(String, u32)> = tally(&row.chosen);
                format!("{}: {}", row.slot, count_label(&labels))
            }
        };
        let color = if row.unavailable.is_some() || row.chosen.is_empty() {
            TEXT_DIM
        } else {
            TEXT
        };
        rows.push(colored_item_row(text, i == selected, color));
    }
    let name = if draft.name.is_empty() {
        "(select to type a name, 1-24 characters)".to_string()
    } else {
        draft.name.clone()
    };
    rows.push(item_row(format!("Name: {name}"), selected == 6));
    let preview = game.build_preview(&draft.name, &draft.parts);
    let color = if preview.blocked.is_some() {
        TEXT_DIM
    } else {
        TEXT
    };
    rows.push(colored_item_row("Compile", selected == 7, color));
    rows.extend(preview_rows(&preview));
    rows
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
        let (note, color) = if option.held > 0 {
            (format!("{} held", option.held), TEXT)
        } else {
            ("none held".to_string(), TEXT_DIM)
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
    vec![
        text_row("Name the routine (1-24 characters). Enter or Esc to finish."),
        text_row(""),
        text_row(format!("> {}_", draft.name)),
    ]
}

/// The Subroutines page: every researched part, crafted with Enter, filtered by typing.
pub(super) fn subroutine_page_rows(game: &Game, filter: &str, selected: usize) -> Vec<Row> {
    let all = game.subroutine_rows();
    let mut rows = vec![text_row(
        "Type to filter. Enter compiles one for a Routine Disk and the material shown.",
    )];
    rows.extend(filter_line(filter));
    let visible = subroutines_visible(game, filter);
    if visible.is_empty() && !filter.is_empty() {
        rows.push(no_match_row(filter));
    }
    for (i, row) in visible.iter().map(|&src| &all[src]).enumerate() {
        let note = format!(
            "held {} · 1 Routine Disk + {} {}",
            row.held, row.material_count, row.material
        );
        rows.push(annotated_item_row(
            format!("{} ({})", row.label, row.slot),
            Some(note),
            i == selected,
            TEXT,
        ));
    }
    rows
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
