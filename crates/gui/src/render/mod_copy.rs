//! `Mode::ModCopy` and `Mode::ModPickAffix`: the Mod Bench's slot list and
//! affix picker. Rows are built by pure functions over `Game` so the width
//! census can drive them without a frame.

use super::popup::*;
use super::*;
use feral_processes_app_core::mod_slot_rows;
use feral_processes_engine::affixes::{AffixDef, AffixId};

/// An affix as one row reads it: its label and what it grants.
fn affix_text(game: &Game, def: &AffixDef) -> String {
    let stats = game.stat_summary(def.stats);
    if stats.is_empty() {
        def.label()
    } else {
        format!("{}  {stats}", def.label())
    }
}

/// What applying `def` costs, from the affix's own block.
fn cost_text(game: &Game, def: &AffixDef) -> String {
    let Some(research) = &def.research else {
        return String::new();
    };
    let bill: Vec<String> = research
        .apply_cost
        .iter()
        .map(|(item, qty)| format!("{qty} {}", game.item_name(item)))
        .collect();
    format!("Cost: {}", bill.join(", "))
}

/// DECOMP is a companion's stat, so on a weapon it is dead weight the player
/// should be told about before paying for it.
fn decomp_note(def: &AffixDef, slot: Option<EquipmentSlot>) -> Option<&'static str> {
    (def.stats.decompiler != 0 && slot == Some(EquipmentSlot::Weapon))
        .then_some("    DECOMP does nothing on a companion.")
}

fn find_def<'a>(defs: &'a [AffixDef], id: &AffixId) -> Option<&'a AffixDef> {
    defs.iter().find(|d| &d.id == id)
}

pub(super) fn mod_copy_rows(game: &Game, copy: &GearCopy, selected: usize) -> Vec<Row> {
    let defs = game.affix_defs();
    let slot = game.equipment_of(&copy.item).map(|(s, _)| s);
    let mut rows = vec![
        text_row(format!(
            "{}: {} of {} affix slots used.",
            game.copy_name(copy),
            copy.affixes.len(),
            game.affix_slots(copy)
        )),
        text_row(""),
    ];
    for (i, row) in mod_slot_rows(game, copy).iter().enumerate() {
        let key = menu_shortcut(i);
        match row {
            Some(id) => {
                let text = find_def(&defs, id)
                    .map_or_else(|| id.as_str().to_string(), |d| affix_text(game, d));
                rows.push(item_row(format!("[{key}] {text}"), i == selected));
                if let Some(note) = find_def(&defs, id).and_then(|d| decomp_note(d, slot)) {
                    rows.push(colored_item_row(note, false, TEXT_DIM));
                }
            }
            None => rows.push(colored_item_row(
                format!("[{key}] (empty slot)"),
                i == selected,
                TEXT_DIM,
            )),
        }
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Select a slot; R strips an affix, Enter on an empty slot fits one. Esc to leave.",
    ));
    rows
}

pub(super) fn mod_pick_rows(game: &Game, copy: &GearCopy, selected: usize) -> Vec<Row> {
    let defs = game.affix_defs();
    let slot = game.equipment_of(&copy.item).map(|(s, _)| s);
    let mut rows = vec![text_row("Fit which affix?")];
    let offered = game.appliable_affixes(copy);
    if offered.is_empty() {
        rows.push(text_row("No researched affix fits this. Research more."));
    }
    for (i, id) in offered.iter().enumerate() {
        let Some(def) = find_def(&defs, id) else {
            continue;
        };
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), affix_text(game, def)),
            i == selected,
        ));
        rows.push(colored_item_row(
            format!("    {}", cost_text(game, def)),
            false,
            TEXT_DIM,
        ));
        if let Some(note) = decomp_note(def, slot) {
            rows.push(colored_item_row(note, false, TEXT_DIM));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to go back."));
    rows
}

pub(super) fn draw_mod_copy(
    game: &Game,
    copy: Option<GearCopy>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else { return };
    let rows = mod_copy_rows(game, &copy, selected);
    draw_popup("Mod Bench", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_mod_pick_affix(
    game: &Game,
    copy: Option<GearCopy>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else { return };
    let rows = mod_pick_rows(game, &copy, selected);
    draw_popup("Fit an affix", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::super::popup::{popup_body_width, row_label_text};
    use super::*;
    use crate::text::ui_metrics;
    use feral_processes_engine::DifficultyMode;

    fn game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(91, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// Every shipped research affix's picker lines, on each slot it can be
    /// fitted to, fit the popup body at 1280x720 — the cost line is the long
    /// one, and a longer bill would clip without a test saying so.
    #[test]
    fn no_picker_row_overflows_the_popup_body_at_1280x720() {
        let game = game();
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        let mut checked = 0;
        crate::paint::with_painter(|p| {
            for def in game.affix_defs().iter().filter(|d| d.research.is_some()) {
                let mut lines = vec![
                    format!("[9] {}", affix_text(&game, def)),
                    format!("    {}", cost_text(&game, def)),
                ];
                lines.extend(decomp_note(def, Some(EquipmentSlot::Weapon)).map(String::from));
                for line in lines {
                    let width = p.measure_ui_advance(&line, m.font_size);
                    assert!(
                        width <= body,
                        "a picker line draws {width}px into a {body}px body: {line:?}"
                    );
                }
                checked += 1;
            }
        });
        assert!(checked >= 3, "the census walked only {checked} affixes");
    }

    #[test]
    fn a_decompiler_affix_on_a_weapon_carries_the_companion_note() {
        let mut def = game()
            .affix_defs()
            .into_iter()
            .find(|d| d.research.is_some())
            .unwrap();
        def.stats.decompiler = 2;
        assert!(decomp_note(&def, Some(EquipmentSlot::Weapon)).is_some());
        assert!(decomp_note(&def, Some(EquipmentSlot::Armor)).is_none());
        def.stats.decompiler = 0;
        assert!(decomp_note(&def, Some(EquipmentSlot::Weapon)).is_none());
    }

    #[test]
    fn a_plain_copy_lists_one_empty_slot() {
        let game = game();
        let copy = GearCopy::plain(ItemId::from("nullsteel_plate"));
        let rows = mod_copy_rows(&game, &copy, 0);
        let labels: Vec<String> = rows.iter().map(row_label_text).collect();
        assert!(
            labels.iter().any(|l| l.contains("[1] (empty slot)")),
            "{labels:?}"
        );
    }
}
