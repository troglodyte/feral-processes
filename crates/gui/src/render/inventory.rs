//! The inventory list, its per-item action page, and the erase prompt.

use super::popup::*;
use super::*;

pub(super) fn draw_erase_quantity(
    game: &mut Game,
    item: Option<ItemId>,
    quantity_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(item) = item else { return };
    let status = game.player_status();
    let held = status
        .inventory
        .iter()
        .find(|(i, _)| *i == item)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    let shown = if quantity_input.is_empty() {
        "1".to_string()
    } else {
        quantity_input.to_string()
    };
    let rows = vec![
        text_row(format!("Erase how many {}?", game.item_name(&item))),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!(
            "You have: {held}        Buffer: {}",
            status.inventory_used
        )),
        text_row(""),
        text_row("Type digits, Enter to erase"),
        text_row("[A] Erase all   Esc to go back"),
    ];
    draw_popup("Erase", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_inventory(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(
            format!(
                "Level {}   Attack {}   Defense {}   Power {}   Decompiler {}",
                status.level, status.atk, status.def, status.power, status.decompiler
            ),
            CYAN,
        ),
        text_row(""),
        text_row("Equipped (number to unequip):"),
        equipped_row(1, "Weapon", status.weapon.clone(), selected == 0, game),
        equipped_row(2, "Armor", status.armor.clone(), selected == 1, game),
        equipped_row(3, "Module", status.module.clone(), selected == 2, game),
        text_row(""),
        text_row(format!(
            "Inventory - Buffer {} (row key to equip/fuse/erase):",
            status.inventory_used
        )),
    ];
    if status.inventory.is_empty() {
        rows.push(text_row("(empty)"));
    }
    for (i, (item, qty)) in status.inventory.iter().enumerate() {
        let fusion_tier = game.item_fusion_tier(item);
        let tag = equip_preview_tag(game, item, status.zone, fusion_tier);
        rows.push(item_row(
            format!(
                "[{}] {} x{}{}",
                menu_shortcut(i + 3),
                game.item_name(item),
                qty,
                tag
            ),
            selected == i + 3,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to close; Up/Down + Enter also work"));
    draw_popup("Inventory", PopupSize::Large, &rows, painter, m);
}

fn equipped_row(
    num: usize,
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    selected: bool,
    game: &Game,
) -> Row {
    match equipped.and_then(|e| game.equipment_of(&e.item).map(|(_, mods)| (e, mods))) {
        Some((equipped, mods)) => {
            let mods = mods
                .scaled_for_level(equipped.level)
                .fused_for_tier(equipped.fusion_tier);
            let mut parts = Vec::new();
            if mods.atk != 0 {
                parts.push(format!("+{} ATK", mods.atk));
            }
            if mods.def != 0 {
                parts.push(format!("+{} DEF", mods.def));
            }
            if mods.decompiler != 0 {
                parts.push(format!("+{} DECOMP", mods.decompiler));
            }
            let mut notes = Vec::new();
            if equipped.level > 1 {
                notes.push(format!("Lv{}", equipped.level));
            }
            if equipped.fusion_tier > 0 {
                notes.push(format!("T{}", equipped.fusion_tier));
            }
            let note = if notes.is_empty() {
                String::new()
            } else {
                format!(" {}", notes.join(" "))
            };
            item_row(
                format!(
                    "[{num}] {label}: {}{note} ({})",
                    game.item_name(&equipped.item),
                    parts.join(" ")
                ),
                selected,
            )
        }
        None => item_row(format!("[{num}] {label}: (empty)"), selected),
    }
}

pub(super) fn draw_inventory_item_action(
    game: &Game,
    item: Option<ItemId>,
    zone_level: u32,
    fusion_tier: u32,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(item) = item else {
        draw_popup(
            "Item",
            PopupSize::Small,
            &[text_row("Nothing selected.")],
            painter,
            m,
        );
        return;
    };
    let title = format!(
        "{}{}",
        game.item_name(&item),
        equip_preview_tag(game, &item, zone_level, fusion_tier)
    );
    let mut rows = vec![Row::TextColored(title, TEXT), text_row("")];
    for (i, (_, label)) in inventory_item_actions(game, &item).iter().enumerate() {
        rows.push(item_row(label.clone(), i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Item", PopupSize::Large, &rows, painter, m);
}
