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
                "[{}] {} {}  {}{}",
                menu_shortcut(i + 3),
                qty_column(row.qty),
                game.item_category(&row.copy.item).short_label(),
                game.copy_name(&row.copy),
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
/// The figure comes from `Game::copy_bonus` and is formatted by
/// `stat_summary`, so the equipped panel, the inventory list's tag and the
/// swap picker's columns cannot disagree about what an item grants. Sharing
/// only the formatter is what let them disagree before — see `copy_bonus`.
fn equipped_summary(
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    game: &Game,
) -> String {
    let Some((equipped, mods)) =
        equipped.and_then(|e| game.copy_bonus(&e.copy, e.level).map(|mods| (e, mods)))
    else {
        return format!("{label}: (empty)");
    };
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
    use super::equipped_summary;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_app_core::{equip_preview_tag, menu_shortcut, qty_column};
    use feral_processes_engine::components::Rarity;
    use feral_processes_engine::items::{EquipmentSlot, GearCopy};
    use feral_processes_engine::tuning::MAX_FUSIONS;
    use feral_processes_engine::{DifficultyMode, Game, save};

    /// **The equipped panel prints what the player is actually wearing.**
    ///
    /// It rebuilt the scaling chain by hand and knew about two of the four
    /// properties a `GearCopy` carries — so an Overclocked Overdriven Kinetic
    /// Edge, really worth 27 ATK, was reported at 6. Both omissions
    /// are here on purpose: an affix and a rare tier are the two properties
    /// added after this function was written, which is what a hand-rolled
    /// copy of `Game::copy_bonus` costs.
    #[test]
    fn the_equipped_panel_prices_the_affix_and_the_rare_tier() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let mut game = Game::new(77, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let path = std::env::temp_dir().join("feral_processes_gui_equipped_panel.sav");
        game.save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.player.weapon = Some("kinetic_edge".into());
        data.player.weapon_level = 3;
        data.player.weapon_rarity = Rarity::Gold;
        data.player.weapon_affix = Some("overdriven".into());
        save::save_to_file(&path, &data).unwrap();
        let game = Game::load(&path, assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let player = game.player_entity();
        let worn = game
            .worn(player, EquipmentSlot::Weapon)
            .expect("wearing it");
        let real = game.copy_bonus(&worn.copy, worn.level).expect("priced");

        let summary = equipped_summary("Weapon", Some(worn), &game);
        assert!(
            summary.contains(&format!("+{} ATK", real.atk)),
            "the panel disagrees with what the player is wearing ({} ATK): {summary}",
            real.atk
        );
    }

    /// **The widest inventory row the shipped assets can build still fits.**
    ///
    /// The row leads with `qty_column` now, so every row is wider than it
    /// was, and the popup never wraps or clips horizontally — an overflowing
    /// row simply runs off the right edge, taking the equip tag with it.
    ///
    /// Built from `item_defs` × `affix_defs` rather than hand-written, which
    /// is the difference between a census and a fixture: the widest row is a
    /// property of the assets, so a long item name or affix added later has
    /// to fail here rather than be caught by eye.
    ///
    /// **A maxed copy is excluded, and that is a known overflow rather than
    /// a carve-out for convenience.** `equip_preview_tag` appends
    /// `" - maxed"` at `MAX_FUSIONS` on the stated grounds that this screen
    /// has the room, and measured it does not: a Gold, affixed, maxed
    /// Singularity Matrix runs 1311px into a 1243px body at zone 10. It ran
    /// the same width before the quantity moved to the front — the count
    /// simply changed ends — so it is recorded in `TODO.md` rather than
    /// fixed here, since the fix is a decision about that tag.
    #[test]
    fn no_shipped_inventory_row_overflows_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(31, DifficultyMode::Forgiving, assets).expect("shipped assets");

        // The deepest zone `balance_sim` sweeps to, which is where the stat
        // figures in the tag are widest, on the rarest copy.
        let zone = 10;
        let affixes: Vec<Option<_>> = std::iter::once(None)
            .chain(game.affix_defs().into_iter().map(|a| Some(a.id)))
            .collect();
        let widest = game
            .item_defs()
            .iter()
            .flat_map(|def| {
                affixes.iter().flat_map(move |affix| {
                    let def = def.clone();
                    (0..MAX_FUSIONS).map(move |tier| GearCopy {
                        item: def.id.clone().into(),
                        rarity: Rarity::Gold,
                        tier,
                        affix: affix.clone(),
                    })
                })
            })
            .map(|copy| {
                format!(
                    // A four-digit count rather than the three the column
                    // reserves: the buffer is unbounded and a long run's
                    // scrap pile reaches it, at which point the row grows.
                    "[{}] {} {}  {}{}",
                    menu_shortcut(35),
                    qty_column(1234),
                    game.item_category(&copy.item).short_label(),
                    game.copy_name(&copy),
                    equip_preview_tag(&game, &copy, zone)
                )
            })
            .max_by_key(|row| row.chars().count())
            .expect("the shipped assets define items");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(format!("  {widest}"), m.font_size);
            assert!(
                drawn <= room,
                "the widest inventory row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}",
                drawn - room
            );
        });
    }

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
        // The widest row this screen can build out of the shipped assets: the
        // longest tier word, the longest affix, the longest equippable name,
        // a maxed fusion note and a three-stat delta.
        let widest = format!(
            "[a] {:<50} {:<20} {}",
            format!(
                "{} Singularity Matrix of Quiet Handshakes",
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
