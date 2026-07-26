//! The detail page for an inspected tile.

use super::popup::*;
use super::*;

pub(super) fn draw_inspect_detail(
    game: &mut Game,
    entity: Option<Entity>,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(view) = entity.and_then(|e| game.inspect(e)) else {
        draw_popup(
            "Inspect",
            PopupSize::Small,
            &[text_row("That program is gone. Press any key to go back.")],
            fonts,
            m,
        );
        return;
    };
    let status = if view.is_tamed {
        "compiled (yours)".to_string()
    } else if view.is_hostile {
        "rogue".to_string()
    } else {
        "idle".to_string()
    };
    let habitats: Vec<String> = view.habitats.iter().map(|b| format!("{b:?}")).collect();
    let moves: Vec<String> = view
        .moves
        .iter()
        .map(|m| format!("{} (pow {})", m.name, m.power))
        .collect();

    let mut rows = vec![
        Row::TextColored(
            format!(
                "{}{}{}",
                view.name,
                view.level.map(|l| format!(" - Lv{l}")).unwrap_or_default(),
                if view.is_boss { " [BOSS]" } else { "" }
            ),
            if view.is_boss { RED } else { WHITE },
        ),
        text_row(format!("Status: {status}")),
        text_row(format!("Integrity: {}/{}", view.hp, view.max_hp)),
        text_row(format!(
            "Attack {}   Defense {}   Power {}",
            view.atk, view.def, view.power
        )),
        text_row(format!(
            "Decompile difficulty: {:.0}%",
            view.taming_difficulty * 100.0
        )),
    ];
    if let Some(quality) = &view.quality {
        rows.push(text_row(format!("Potential: {quality}")));
    }
    if view.fusions > 0 {
        rows.push(text_row(format!(
            "Fusions: {}/{MAX_FUSIONS}{}",
            view.fusions,
            if view.fusions >= MAX_FUSIONS {
                " (can't be fused again)"
            } else {
                ""
            }
        )));
    }
    if view.is_hostile && !view.is_tamed {
        rows.push(Row::TextColored(
            decompile_chance_line(view.decompile_chance),
            MAGENTA,
        ));
    }
    rows.push(text_row(format!(
        "Habitats: {}",
        if habitats.is_empty() {
            "unknown".to_string()
        } else {
            habitats.join(", ")
        }
    )));
    rows.push(text_row(format!(
        "Moves: {}",
        if moves.is_empty() {
            "none".to_string()
        } else {
            moves.join(", ")
        }
    )));
    if let Some(res) = view.work_resource {
        rows.push(text_row(format!("Work aptitude: {}", game.item_name(&res))));
    }
    rows.push(text_row(""));
    rows.push(text_row("Press any key to go back, Esc to close"));
    draw_popup("Inspect", PopupSize::Large, &rows, fonts, m);
}

/// The inspect panel's decompile-odds readout. A full sentence because the
/// panel has room for one; the battle roster quotes the same number as a
/// `DECOMP` column instead (see `battle::odds_cell`). With no taming catalyst
/// in inventory there are no odds to quote — decompiling isn't available at
/// all — so the line says what's missing instead of a percentage. It stays
/// deliberately generic: which item is a catalyst is item data, not something
/// a renderer gets to name.
fn decompile_chance_line(chance: Option<f32>) -> String {
    match chance {
        Some(c) => format!("Decompile chance right now: {:.0}%", c * 100.0),
        None => "Decompile chance right now: needs a taming catalyst".to_string(),
    }
}
