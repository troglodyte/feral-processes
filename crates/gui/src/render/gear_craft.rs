//! The Workbench's weapon-crafting pages (ADR 0031): the Forge, New design,
//! Weapon parts and Salvage, plus the two popups the new-design page opens.
//!
//! Every figure is a view off the engine (`Game::design_rows`,
//! `design_slot_rows`, `design_part_options`, `design_preview`,
//! `weapon_part_rows`, `salvage_rows`) and every refusal is worded by
//! app-core, so this file only lays them out. Rows are returned rather than
//! drawn so the structure hub frames them like any other page.

use super::list_filter::{filter_line, no_match_row};
use super::popup::*;
use super::routine_craft::{count_label, dim_lines, text_lines};
use super::*;
use feral_processes_app_core::{
    DESIGN_AFFIX_ROW, DESIGN_NAME_ROW, DESIGN_SAVE_ROW, DesignDraft, DesignPicking, affix_label,
    design_pick_visible, design_row_category, gear_craft_refusal_line,
};
use feral_processes_engine::views::DesignPreview;

/// The Forge: every saved design, Enter builds one and `D` deletes it.
pub(super) fn forge_page_rows(game: &Game, selected: usize) -> Vec<Row> {
    let designs = game.design_rows();
    let mut rows = vec![text_row(
        "Enter builds a weapon from a design. D deletes a design.",
    )];
    if designs.is_empty() {
        rows.push(text_row(
            "(no designs yet - make one on the New design page)",
        ));
    }
    for (i, design) in designs.iter().enumerate() {
        rows.push(item_row(design.name.clone(), i == selected));
        rows.extend(dim_lines(&design.description));
        rows.extend(dim_lines(&format!("Parts: {}", design.parts.join(", "))));
        if let Some(power) = design.power {
            rows.extend(dim_lines(&format!("Rating {power}")));
        }
        if let Some(why) = &design.blocked {
            rows.extend(dim_lines(&gear_craft_refusal_line(game, why)));
        }
    }
    rows
}

fn preview_rows(game: &Game, preview: &DesignPreview) -> Vec<Row> {
    let mut rows = vec![text_row("")];
    if preview.description.is_empty() {
        rows.push(text_row("Pick a Frame to see what the weapon does."));
    } else {
        rows.extend(text_lines(&preview.description));
        let rating = preview
            .power
            .map(|p| format!("Rating {p} · "))
            .unwrap_or_default();
        rows.push(text_row(format!("{rating}range {}", preview.range)));
    }
    rows.push(text_row(format!(
        "Modifiers {} of {}",
        preview.part_count, preview.part_cap
    )));
    if !preview.parts_used.is_empty() {
        rows.extend(text_lines(&format!(
            "Build uses: {}",
            count_label(&preview.parts_used)
        )));
    }
    if !preview.parts_missing.is_empty() {
        rows.extend(text_lines(&format!(
            "Parts short: {}",
            count_label(&preview.parts_missing)
        )));
    }
    if let Some(why) = &preview.blocked {
        rows.extend(text_lines(&gear_craft_refusal_line(game, why)));
    }
    rows
}

/// The new-design page: Frame and modifier rows, affixes, name, Save, then
/// the preview.
pub(super) fn new_design_page_rows(game: &Game, draft: &DesignDraft, selected: usize) -> Vec<Row> {
    let mut rows = vec![
        text_row("Enter on a row to choose a part. Name the design, then Save."),
        text_row(""),
    ];
    for (i, slot) in game.design_slot_rows(&draft.parts).iter().enumerate() {
        let (text, color) = match slot.chosen {
            Some(part) => (format!("{}: {}", slot.category.label(), part.label()), TEXT),
            None => (format!("{}: (none)", slot.category.label()), TEXT_DIM),
        };
        debug_assert_eq!(design_row_category(i), Some(slot.category));
        rows.push(colored_item_row(text, i == selected, color));
    }
    let (affixes, color) = if draft.affixes.is_empty() {
        ("(none)".to_string(), TEXT_DIM)
    } else {
        let labels: Vec<String> = draft.affixes.iter().map(affix_label).collect();
        (labels.join(", "), TEXT)
    };
    rows.push(colored_item_row(
        format!("Affixes: {affixes}"),
        selected == DESIGN_AFFIX_ROW,
        color,
    ));
    let name = if draft.name.is_empty() {
        "(select to type a name, 1-24 characters)".to_string()
    } else {
        draft.name.clone()
    };
    rows.push(item_row(
        format!("Name: {name}"),
        selected == DESIGN_NAME_ROW,
    ));
    let preview = game.design_preview(&draft.name, &draft.gear_parts());
    let color = if preview.blocked.is_some() {
        TEXT_DIM
    } else {
        TEXT
    };
    rows.push(colored_item_row("Save", selected == DESIGN_SAVE_ROW, color));
    rows.extend(preview_rows(game, &preview));
    rows
}

/// The list one design row opens: each part with how many are held, greyed
/// with the reason when it cannot go in, and a last row that empties the row.
pub(super) fn pick_rows(
    game: &Game,
    draft: &DesignDraft,
    filter: &str,
    selected: usize,
) -> Vec<Row> {
    let Some(picking) = draft.picking else {
        return vec![text_row("Nothing to choose.")];
    };
    let visible = design_pick_visible(game, draft, filter);
    let mut rows = Vec::new();
    match picking {
        DesignPicking::Category(category) => {
            rows.push(text_row(format!(
                "Choose a {}. Type to filter.",
                category.label()
            )));
            rows.extend(filter_line(filter));
            if visible.is_empty() && !filter.is_empty() {
                rows.push(no_match_row(filter));
            }
            let options = game.design_part_options(category, &draft.parts, draft.affixes.len());
            for (i, &src) in visible.iter().enumerate() {
                let option = &options[src];
                let (note, color) = match (&option.unavailable, option.held) {
                    (Some(_), _) => ("greyed".to_string(), TEXT_DIM),
                    (None, 0) => ("none held".to_string(), TEXT_DIM),
                    (None, held) => (format!("{held} held"), TEXT),
                };
                rows.push(annotated_item_row(
                    option.label.clone(),
                    Some(note),
                    i == selected,
                    color,
                ));
                if let Some(why) = &option.unavailable {
                    rows.extend(dim_lines(&gear_craft_refusal_line(game, why)));
                }
            }
            rows.push(colored_item_row(
                format!("Clear {}", category.label()),
                selected == visible.len(),
                TEXT_DIM,
            ));
        }
        DesignPicking::Affixes => {
            rows.push(text_row("Enter adds or removes an affix. Type to filter."));
            rows.extend(filter_line(filter));
            if visible.is_empty() && !filter.is_empty() {
                rows.push(no_match_row(filter));
            }
            let options = game.design_affix_options(&draft.affixes);
            if options.is_empty() {
                rows.push(text_row(
                    "(none known - research affixes or take apart a weapon that carries one)",
                ));
            }
            for (i, &src) in visible.iter().enumerate() {
                let option = &options[src];
                let mark = if option.chosen { "[x]" } else { "[ ]" };
                rows.push(item_row(
                    format!("{mark} {}", affix_label(&option.affix)),
                    i == selected,
                ));
            }
            rows.push(colored_item_row(
                "Clear affixes",
                selected == visible.len(),
                TEXT_DIM,
            ));
        }
    }
    rows
}

pub(super) fn name_rows(draft: &DesignDraft) -> Vec<Row> {
    vec![
        text_row("Name the design (1-24 characters). Enter or Esc to finish."),
        text_row(""),
        text_row(format!("> {}_", draft.name)),
    ]
}

/// The Weapon parts page: every part, locked ones greyed.
pub(super) fn parts_page_rows(game: &Game, selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Enter compiles a part from the material shown. Grey parts are not researched.",
    )];
    for (i, row) in game.weapon_part_rows().iter().enumerate() {
        let (note, color) = if !row.known {
            ("not researched".to_string(), TEXT_DIM)
        } else {
            let color = if row.affordable { TEXT } else { TEXT_DIM };
            (
                format!(
                    "held {} · {} {}",
                    row.held, row.material_count, row.material
                ),
                color,
            )
        };
        rows.push(annotated_item_row(
            format!("{} ({})", row.label, row.category.label()),
            Some(note),
            i == selected,
            color,
        ));
    }
    rows
}

/// The Salvage page: authored weapons in the pack.
pub(super) fn salvage_page_rows(game: &Game, selected: usize) -> Vec<Row> {
    let salvage = game.salvage_rows();
    let mut rows = vec![text_row(
        "Enter takes a weapon apart for parts. The weapon is lost.",
    )];
    if salvage.is_empty() {
        rows.push(text_row("(no weapons in your pack to take apart)"));
    }
    for (i, row) in salvage.iter().enumerate() {
        let note = (row.qty > 1).then(|| format!("x{}", row.qty));
        rows.push(annotated_item_row(
            row.name.clone(),
            note,
            i == selected,
            TEXT,
        ));
        rows.extend(dim_lines(&format!("Gives: {}", row.parts.join(", "))));
    }
    rows
}

pub(super) fn draw_pick(
    game: &Game,
    draft: &DesignDraft,
    filter: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = pick_rows(game, draft, filter, selected);
    draw_popup("Choose part", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_name(
    draft: &DesignDraft,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Name design",
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
    use feral_processes_engine::DifficultyMode;
    use feral_processes_engine::gear_craft::weapon::Category;

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// Every part row, the empty new-design page and every category list
    /// fit the hub page at the smallest supported window.
    #[test]
    fn the_weapon_pages_fit_the_hub_content_rect() {
        let game = shipped_game();
        let parts = parts_page_rows(&game, 0);
        super::super::base_report::assert_rows_fit(&parts, "weapon parts");
        let design = new_design_page_rows(&game, &DesignDraft::default(), 0);
        super::super::base_report::assert_rows_fit(&design, "new design");
        for category in std::iter::once(Category::Frame).chain(Category::MODIFIERS) {
            let draft = DesignDraft {
                picking: Some(DesignPicking::Category(category)),
                ..DesignDraft::default()
            };
            let rows = pick_rows(&game, &draft, "", 0);
            super::super::base_report::assert_rows_fit(&rows, "part list");
        }
        let forge = forge_page_rows(&game, 0);
        super::super::base_report::assert_rows_fit(&forge, "forge");
        let salvage = salvage_page_rows(&game, 0);
        super::super::base_report::assert_rows_fit(&salvage, "salvage");
    }
}
