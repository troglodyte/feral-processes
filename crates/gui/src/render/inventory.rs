//! The inventory list, its per-item action page, and the erase prompt.

use super::popup::*;
use super::*;

pub(super) fn draw_erase_quantity(
    game: &mut Game,
    pending: Option<GearCopy>,
    quantity_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = pending else { return };
    let status = game.player_status();
    let held = status
        .inventory
        .iter()
        .find(|r| r.copy == copy)
        .map(|r| r.qty)
        .unwrap_or(0);
    let shown = if quantity_input.is_empty() {
        "1".to_string()
    } else {
        quantity_input.to_string()
    };
    let rows = vec![
        // Both tiers, because erasing is irreversible and a rare or fused
        // copy is worth many plain ones — the prompt is the last chance to
        // notice which copy is highlighted.
        text_row(format!(
            "Erase how many {}{}?",
            game.copy_name(&copy),
            match copy.tier {
                0 => String::new(),
                tier => format!(" {}", item_fusion_note(tier)),
            }
        )),
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
        text_row("Equipped (number to swap or unequip):"),
        // A wielded program and a worn weapon are mutually exclusive, so the
        // weapon line renders one or the other and never both.
        match &status.wielded {
            Some(w) => text_row(format!(
                "[1] Weapon: {} Lv{} (+{} ATK, +{} DEF)",
                w.name, w.level, w.bonus.0, w.bonus.1
            )),
            None => equipped_row(1, "Weapon", status.weapon.clone(), selected == 0, game),
        },
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
    for (i, row) in status.inventory.iter().enumerate() {
        let tag = equip_preview_tag(game, &row.copy, status.zone);
        // The engine hands this list back grouped, so the category column
        // reads as a heading for the run of rows beneath it rather than as
        // noise repeated at random. A fused copy is its own row beside its
        // ordinary spares, which is the whole point of the screen.
        rows.push(tier_row(
            format!(
                "[{}] {}  {} x{}{}",
                menu_shortcut(i + 3),
                game.item_category(&row.copy.item).short_label(),
                game.copy_name(&row.copy),
                row.qty,
                tag
            ),
            selected == i + 3,
            row.copy.tier,
            row.copy.rarity,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "[S] sell one to a trader in range; Esc to close; Up/Down + Enter also work",
    ));
    draw_popup("Inventory", PopupSize::Large, &rows, painter, m);
}

pub(super) fn equipped_row(
    num: usize,
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    selected: bool,
    game: &Game,
) -> Row {
    // The tier the worn copy was equipped at, not the ledger's current one —
    // the same number `equipped_summary` prints beside it, so the colour and
    // the text on this row cannot disagree.
    let fusions = equipped.as_ref().map(|e| e.copy.tier).unwrap_or(0);
    let rarity = equipped.as_ref().map(|e| e.copy.rarity).unwrap_or_default();
    tier_row(
        format!("[{num}] {}", equipped_summary(label, equipped, game)),
        selected,
        fusions,
        rarity,
    )
}

/// `Weapon: Arc Lance Lv3 T1 (+16 ATK)`, or `Weapon: (empty)`.
///
/// The stat bonus goes through `stat_summary` rather than being formatted
/// here, so the equipped panel, the inventory list's tag and the swap
/// picker's columns cannot disagree about what an item grants.
fn equipped_summary(
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    game: &Game,
) -> String {
    let Some((equipped, base)) =
        equipped.and_then(|e| game.equipment_of(&e.copy.item).map(|(_, mods)| (e, mods)))
    else {
        return format!("{label}: (empty)");
    };
    let mods = base
        .scaled_for_level(equipped.level)
        .fused_for_tier(equipped.copy.tier);
    let mut notes = Vec::new();
    if equipped.level > 1 {
        notes.push(format!("Lv{}", equipped.level));
    }
    if equipped.copy.tier > 0 {
        notes.push(item_fusion_note(equipped.copy.tier));
    }
    let note = if notes.is_empty() {
        String::new()
    } else {
        format!(" {}", notes.join(" "))
    };
    format!(
        "{label}: {}{note} ({})",
        game.copy_name(&equipped.copy),
        stat_summary(mods)
    )
}

/// The replacement picker for one equipment slot.
///
/// Rows come from `equip_swap_rows` — the same call the key handler
/// dispatches — so the highlight and the action can't come apart. What is
/// drawn here beyond them is only the heading and the legend the two stat
/// columns need to be readable.
pub(super) fn draw_equip_swap(
    game: &mut Game,
    slot: Option<EquipmentSlot>,
    target: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(slot) = slot else {
        draw_popup(
            "Gear",
            PopupSize::Small,
            &[text_row("No slot selected.")],
            painter,
            m,
        );
        return;
    };
    let wearer = target.unwrap_or_else(|| game.player_entity());
    let worn = game.worn(wearer, slot);
    let mut rows = vec![
        Row::TextColored(equipped_summary("Wearing", worn, game), CYAN),
        text_row(""),
    ];
    for (i, row) in equip_swap_rows(game, wearer, slot).iter().enumerate() {
        rows.push(tier_row(
            format!("[{}] {}", menu_shortcut(i), row.label),
            i == selected,
            row.fusion_tier,
            row.rarity,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Middle column is what you'd get; right column is the change",
    ));
    rows.push(text_row("Esc to go back; Up/Down + Enter also work"));
    draw_popup(
        &format!("Replace {}", slot.label()),
        PopupSize::Large,
        &rows,
        painter,
        m,
    );
}

/// The read-only description page reached with `d` from the action list.
///
/// The prose is the item's own authored `.ron` text — see
/// `Game::item_description` — not `item_blurb`'s derived gloss, so editing
/// an item's flavour never means touching Rust. The stat tag is still shown
/// above it, since the two answer different questions.
pub(super) fn draw_item_describe(
    game: &Game,
    copy: Option<GearCopy>,
    zone_level: u32,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else {
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
        game.copy_name(&copy),
        equip_preview_tag(game, &copy, zone_level)
    );
    let mut rows = vec![Row::TextColored(title, TEXT), text_row("")];
    match game.item_description(&copy.item) {
        Some(text) => rows.extend(
            wrap_text(text, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        ),
        None => rows.push(text_row("(no description)")),
    }
    rows.push(text_row(""));
    rows.push(text_row("Any key to go back"));
    draw_popup("Item", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_inventory_item_action(
    game: &mut Game,
    copy: Option<GearCopy>,
    zone_level: u32,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else {
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
        game.copy_name(&copy),
        equip_preview_tag(game, &copy, zone_level)
    );
    let mut rows = vec![Row::TextColored(title, TEXT), text_row("")];
    for (i, (_, label)) in inventory_item_actions(game, &copy.item).iter().enumerate() {
        rows.push(item_row(label.clone(), i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Item", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::components::Rarity;

    /// A rare tier is drawn as a *word* in front of the item name, and
    /// `swap_label` pads its columns with `{:<N}` — which never truncates.
    /// So a name past `SWAP_NAME_COLUMN` does not clip: it pushes the stat
    /// and delta columns right and misaligns every row below it.
    ///
    /// Measured against the real font rather than counted in characters,
    /// because counting is exactly what missed this — the column was 20
    /// cells and "Overclocked Monofilament Whip" is 29.
    #[test]
    fn the_widest_swap_row_still_fits_its_popup() {
        // The widest row this screen can build: the longest tier word, the
        // longest equippable name, a fusion note, and a three-stat delta.
        let widest = format!(
            "[a] {:<30} {:<20} {}",
            format!(
                "{} Monofilament Whip",
                Rarity::Gold.label().expect("Gold reads as a word")
            ),
            "+12 ATK +9 DEF +9 DECOMP T3/3",
            "-12 ATK -9 DEF -9 DECOMP"
        );

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(format!("  {widest}"), m.font_size);
            assert!(
                drawn <= room,
                "the widest gear-swap row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}",
                drawn - room
            );
        });
    }
}
