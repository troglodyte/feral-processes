//! The Workbench's weapon-crafting pages (ADR 0031): Build weapon and
//! Salvage, plus the two popups the Build weapon page opens.
//!
//! Every figure is a view off the engine (`Game::design_slot_rows`,
//! `design_part_options`, `design_preview`, `salvage_rows`) and every refusal is worded by
//! app-core, so this file only lays them out. Rows are returned rather than
//! drawn so the structure hub frames them like any other page.

use super::list_filter::{filter_line, no_match_row};
use super::popup::*;
use super::routine_craft::{count_label, dim_lines, text_lines};
use super::*;
use feral_processes_app_core::{
    DESIGN_AFFIX_ROW, DESIGN_COMPILE_ROW, DESIGN_NAME_ROW, DesignDraft, DesignPicking, affix_label,
    design_pick_visible, design_row_category, gear_craft_refusal_line,
};
use feral_processes_engine::gear_craft::GearCraftRefusal;
use feral_processes_engine::views::{DesignPreview, MaterialLine};

/// What a build would spend, green where the stock in reach covers it, and
/// the missing parts no craft can make.
fn materials_rows(materials: &[MaterialLine], uncraftable: &[String]) -> Vec<Row> {
    if materials.is_empty() && uncraftable.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![text_row("Materials")];
    for line in materials {
        let color = if line.have >= line.need { GREEN } else { RED };
        rows.push(Row::TextColored(
            format!("{} {}/{}", line.label, line.have, line.need),
            color,
        ));
    }
    for line in uncraftable {
        rows.push(Row::TextColored(line.clone(), RED));
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
    rows.extend(materials_rows(&preview.materials, &preview.uncraftable));
    rows
}

/// The Build weapon page: Frame and modifier rows, affixes, name, Compile, then
/// the preview.
pub(super) fn weapon_build_page_rows(
    game: &Game,
    draft: &DesignDraft,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        text_row("Enter on a row to choose a part. Name the weapon, then Compile."),
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
    rows.push(colored_item_row(
        "Compile",
        selected == DESIGN_COMPILE_ROW,
        color,
    ));
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
            rows.push(text_row(format!("Choose a {}.", category.label())));
            rows.extend(filter_line(filter));
            if visible.is_empty() && !filter.is_empty() {
                rows.push(no_match_row(filter));
            }
            let options = game.design_part_options(category, &draft.parts, draft.affixes.len());
            for (i, &src) in visible.iter().enumerate() {
                let option = &options[src];
                let craft = match &option.craft_cost {
                    Some((material, n)) => format!("   {n} {material}"),
                    None => String::new(),
                };
                let (note, color) = match (&option.unavailable, option.held) {
                    (Some(GearCraftRefusal::PartNotResearched(_)), _) => {
                        ("not researched".to_string(), TEXT_DIM)
                    }
                    (Some(_), _) => ("does not fit".to_string(), TEXT_DIM),
                    (None, 0) => (format!("none held{craft}"), TEXT_DIM),
                    (None, held) => (format!("{held} held{craft}"), TEXT),
                };
                rows.push(annotated_item_row(
                    option.label.clone(),
                    Some(note),
                    i == selected,
                    color,
                ));
                if let Some(why) = option
                    .unavailable
                    .as_ref()
                    .filter(|w| !matches!(w, GearCraftRefusal::PartNotResearched(_)))
                {
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
            rows.push(text_row("Choose affixes."));
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
    let enter = match picking {
        DesignPicking::Category(_) => "Enter choose",
        DesignPicking::Affixes => "Enter add or remove",
    };
    push_key_footer(
        &mut rows,
        &[
            "Type to filter",
            enter,
            "Esc back (clears the filter first)",
        ],
    );
    rows
}

pub(super) fn name_rows(draft: &DesignDraft) -> Vec<Row> {
    vec![
        text_row("Name the design (1-24 characters)"),
        text_row(""),
        text_row(format!("> {}_", draft.name)),
        text_row(""),
        text_row(key_hints(&["Type a name", "Enter or Esc finish"])),
    ]
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
    use feral_processes_engine::gear_craft::weapon::{Category, Frame, Proc, Tilt, WeaponPart};
    use feral_processes_engine::gear_craft::{GearDesign, GearPart};
    use feral_processes_engine::items::{EquipmentSlot, ItemId};
    use feral_processes_engine::routine_craft::{StatusKind, Version};

    /// The longest Proc label the shipped catalogue carries, so the census
    /// measures the catalogue rather than a literal that drifts from it.
    fn longest_proc() -> Proc {
        *Proc::ALL
            .iter()
            .max_by_key(|p| WeaponPart::Proc(**p).label().len())
            .expect("a Proc exists")
    }

    /// A bench holding a design that needs a Proc (salvage only), an
    /// unresearched Tilt and two known parts, with a double-digit stock of
    /// the known ones, so the Materials block, the uncraftable lines and the
    /// picker notes ("12 held   N <material>") are all measured.
    fn bench_with_every_kind_of_line() -> (Game, Vec<WeaponPart>) {
        let frame = WeaponPart::Frame(Frame::Edge, Version::V1);
        let rider = WeaponPart::Rider(StatusKind::Bleed);
        let parts = vec![
            frame,
            rider,
            WeaponPart::Tilt(Tilt::Heavy),
            WeaponPart::Proc(longest_proc()),
        ];
        let design = GearDesign {
            id: 1,
            name: "W".repeat(24),
            slot: EquipmentSlot::Weapon,
            parts: parts.iter().copied().map(GearPart::Weapon).collect(),
        };
        let mut game =
            super::super::test_support::game_with_a_workbench_and_designs(42, vec![design]);
        for id in ["automation".to_string(), "weapon_bench".to_string()] {
            let _ = game.dev_unlock_research(&id);
        }
        for part in [frame, rider] {
            let _ = game.dev_unlock_research(&part.node_id());
            game.dev_give_item(&ItemId::weapon_part(part), 12)
                .expect("a part item exists");
        }
        (game, parts)
    }

    /// Each row's suffix appended to its label, which is the width the row
    /// draws: `row_label_text` leaves the suffix column out.
    fn with_suffixes_inline(rows: Vec<Row>) -> Vec<Row> {
        rows.into_iter()
            .map(|row| match &row {
                Row::Item {
                    suffix: Some(note), ..
                } => text_row(format!("{}   {note}", row_label_text(&row))),
                _ => row,
            })
            .collect()
    }

    /// Every part row, the Build weapon page, every category list and its
    /// Materials and uncraftable lines fit the hub page at the
    /// smallest supported window.
    #[test]
    fn the_weapon_pages_fit_the_hub_content_rect() {
        let (game, parts) = bench_with_every_kind_of_line();
        let draft = DesignDraft {
            name: "W".repeat(24),
            parts: parts.clone(),
            ..DesignDraft::default()
        };
        let design = weapon_build_page_rows(&game, &draft, 0);
        assert!(
            design
                .iter()
                .any(|r| row_label_text(r).contains("Materials")),
            "premise: the preview lists a bill"
        );
        super::super::base_report::assert_rows_fit(&design, "build weapon");
        for category in std::iter::once(Category::Frame).chain(Category::MODIFIERS) {
            let draft = DesignDraft {
                picking: Some(DesignPicking::Category(category)),
                parts: parts[..2].to_vec(),
                ..DesignDraft::default()
            };
            let rows = with_suffixes_inline(pick_rows(&game, &draft, "", 0));
            if category == Category::Frame {
                assert!(
                    rows.iter().any(|r| row_label_text(r).contains("12 held")),
                    "premise: the Frame list notes a double-digit held count"
                );
            }
            super::super::base_report::assert_rows_fit(&rows, "part list");
            super::super::popup::assert_rows_fit(&rows, PopupSize::Large);
        }
        assert!(
            design.iter().any(|r| matches!(r, Row::TextColored(_, RED))),
            "premise: an uncraftable line is drawn"
        );
        let salvage = salvage_page_rows(&game, 0);
        super::super::base_report::assert_rows_fit(&salvage, "salvage");
    }

    #[test]
    fn materials_rows_colour_covered_and_short_lines_and_list_uncraftable() {
        let line = |label: &str, have, need| MaterialLine {
            label: label.to_string(),
            have,
            need,
        };
        let rows = materials_rows(
            &[line("Scrap", 6, 6), line("Core fragment", 1, 3)],
            &["Venom proc \u{2014} salvage a weapon first".to_string()],
        );
        let colored: Vec<(&str, _)> = rows
            .iter()
            .filter_map(|r| match r {
                Row::TextColored(t, c) => Some((t.as_str(), *c)),
                _ => None,
            })
            .collect();
        assert_eq!(colored[0], ("Scrap 6/6", GREEN));
        assert_eq!(colored[1], ("Core fragment 1/3", RED));
        assert_eq!(colored[2].1, RED);
        assert!(colored[2].0.contains("salvage"));
        assert!(materials_rows(&[], &[]).is_empty());
    }
}
