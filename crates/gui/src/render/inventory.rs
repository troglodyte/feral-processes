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
        let mut lines =
            inventory_row_lines(game, menu_shortcut(i + 3), &row.copy, row.qty, status.zone)
                .into_iter();
        let head = lines
            .next()
            .expect("inventory_row_lines always emits the item's own row");
        rows.push(tier_row(
            head,
            selected == i + 3,
            row.copy.tier,
            row.copy.rarity,
        ));
        // A continuation carries this row's own tail rather than a second kind
        // of information, so it keeps the tier colour. Only the head is ever
        // `selected`: the highlight belongs on the line carrying the row key,
        // and `popup_layout`'s scroll anchor is the first selected `Item`.
        for line in lines {
            rows.push(tier_row(line, false, row.copy.tier, row.copy.rarity));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "[S] sell one to a trader in range; [U] fuse all pairs; Esc to close; Up/Down + Enter also work",
    ));
    draw_popup("Inventory", PopupSize::Large, &rows, painter, m);
}

/// The indented lines an item's extra effects contribute under its row,
/// empty for an item that has none.
///
/// **The one place a listing screen turns `Game::item_effects` into rows.**
/// The inventory list, a trader's three shelves and a Stack market all draw
/// it, and each rebuilding the same `continuation_lines` call is how four
/// screens end up indenting an effect three different ways.
///
/// Wrapped rather than packed onto the head: an effect is prose, not one of
/// the parenthesised tag segments `wrapped_row_lines` shuffles, and it
/// answers a different question from the row's own columns. Sitting under
/// the row unconditionally is also what lets a player scan a shelf for the
/// one module that grants something without opening each in turn.
pub(super) fn effect_lines(game: &Game, item: &ItemId) -> Vec<String> {
    game.item_effects(item)
        .iter()
        .flat_map(|effect| continuation_lines(effect))
        .collect()
}

/// One carried item's lines: the count, category and name every row carries,
/// then `equip_preview_tag` if it still fits — shed onto an indented
/// continuation by `wrapped_row_lines` when it doesn't.
///
/// The engine hands the list back grouped, so the category column reads as a
/// heading for the run of rows beneath it rather than as noise repeated at
/// random. A fused copy is its own row beside its ordinary spares, which is
/// the whole point of the screen.
///
/// The head ends at the name for `companion_row_lines`' reason: a shed has to
/// fall on a boundary between segments, and the head is the part every row
/// carries. The tag is handed over as *one* segment rather than several
/// because it is parenthesised — breaking inside it would leave a line ending
/// on an unclosed bracket and a continuation opening on a stat with nothing
/// to say which item it belongs to.
///
/// Measured, a Gold, affixed, maxed Singularity Matrix ran 1311px into a
/// 1243px body at zone 10, and nothing clamps a popup row horizontally: the
/// tag ran off the right edge, taking the stat figures the screen is read for
/// with it. That is the whole tag, so a chop here deletes the row's only
/// answer to "what would this do if I put it on".
fn inventory_row_lines(
    game: &Game,
    shortcut: char,
    copy: &GearCopy,
    qty: u32,
    zone: u32,
) -> Vec<String> {
    let head = format!(
        "[{shortcut}] {} {}  {}",
        qty_column(qty),
        game.item_category(&copy.item).short_label(),
        game.copy_name(copy),
    );
    let mut lines = wrapped_row_lines(head, &[equip_preview_tag(game, copy, zone)]);
    lines.extend(effect_lines(game, &copy.item));
    lines
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
    // The routine's own name and prose, off `Game::item_grant` — an item's
    // authored description is free text and may say nothing about what it
    // grants, or say it about a routine it no longer carries.
    if let Some((name, effect)) = game.item_grant(&copy.item) {
        rows.push(text_row(""));
        rows.push(text_row(format!("Grants: {name}")));
        rows.extend(
            wrap_text(effect, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        );
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
    let mut rows = vec![Row::TextColored(title, TEXT)];
    // The same lines the list this screen was opened from carries, so the
    // effect does not vanish the moment you select the item to act on it.
    rows.extend(effect_lines(game, &copy.item).into_iter().map(text_row));
    rows.push(text_row(""));
    for (i, (_, label)) in inventory_item_actions(game, &copy.item).iter().enumerate() {
        rows.push(item_row(label.clone(), i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Item", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::{equipped_summary, inventory_row_lines};
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_app_core::menu_shortcut;
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

    /// **Every line every shipped item can put on this screen fits inside
    /// it.** The popup never wraps or clips horizontally, so a line past the
    /// right edge is simply lost — which for an inventory row means the equip
    /// tag, the only thing on it answering what the item would do if worn.
    ///
    /// Built from `item_defs` × `affix_defs` through `inventory_row_lines`
    /// rather than hand-written, which is the difference between a census and
    /// a fixture: the widest row is a property of the assets *and* of how the
    /// screen packs them, so a long item name added later, or a fourth tag
    /// appended to `equip_preview_tag`, has to fail here rather than be
    /// caught by eye.
    ///
    /// **The maxed tier is the case this is most for.** `equip_preview_tag`
    /// appends `" - maxed"` at `MAX_FUSIONS` on the stated grounds that this
    /// screen has the room, and measured it does not: a Gold, affixed, maxed
    /// Singularity Matrix ran 1311px into a 1243px body at zone 10, 68px
    /// over. It fits now because the tag sheds onto its own line rather than
    /// because it got shorter.
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
        let rows: Vec<(String, Vec<String>)> = game
            .item_defs()
            .iter()
            .flat_map(|def| {
                affixes.iter().flat_map(move |affix| {
                    let def = def.clone();
                    (0..=MAX_FUSIONS).map(move |tier| GearCopy {
                        item: def.id.clone().into(),
                        rarity: Rarity::Gold,
                        tier,
                        affix: affix.clone(),
                    })
                })
            })
            .map(|copy| {
                (
                    game.copy_name(&copy),
                    // A four-digit count rather than the three the column
                    // reserves: the buffer is unbounded and a long run's
                    // scrap pile reaches it, at which point the row grows.
                    inventory_row_lines(&game, menu_shortcut(35), &copy, 1234, zone),
                )
            })
            .collect();
        assert!(!rows.is_empty(), "the shipped assets define items");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for (name, lines) in &rows {
                for line in lines {
                    // The two-space prefix `draw_row` puts in front of every
                    // `Row::Item` label, which a continuation carries too.
                    let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                    assert!(
                        drawn <= room,
                        "a {name} row overflows by {:.0}px ({drawn:.0} into {room:.0}):\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }

    /// **An item's extra effects get their own line under it.** The equip
    /// tag answers "what would this do if I put it on"; nothing on a listing
    /// row answered "and what else does it do", so seven shipped modules
    /// granted a passive routine that was visible only on the describe page
    /// two keypresses away.
    ///
    /// Asserted through `Game::item_effects` rather than against a literal,
    /// so the renderer cannot be the place a wording drifts.
    #[test]
    fn a_granting_module_carries_its_passive_on_a_line_of_its_own() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(32, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let copy = GearCopy::plain("watchdog_tap".into());
        let effects = game.item_effects(&copy.item);
        assert_eq!(effects.len(), 1, "precondition: {effects:?}");

        let lines = inventory_row_lines(&game, menu_shortcut(3), &copy, 1, 1);

        assert!(
            lines.len() > 1,
            "the effect must land on a line of its own: {lines:?}"
        );
        assert!(
            lines[1..].iter().any(|l| l.contains(&effects[0])),
            "and must be the sentence the engine wrote: {lines:?}"
        );
        assert!(
            !lines[0].contains(&effects[0]),
            "never packed onto the head, which carries the row key: {lines:?}"
        );
    }

    /// And an item with nothing extra to say is the single line it always
    /// was — the list a player scrolls must not double in length for free.
    #[test]
    fn an_item_with_no_effects_is_still_one_line() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(33, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let copy = GearCopy::plain("core_fragment".into());
        assert!(game.item_effects(&copy.item).is_empty(), "precondition");

        assert_eq!(
            inventory_row_lines(&game, menu_shortcut(3), &copy, 1, 1).len(),
            1
        );
    }

    /// The wrap is paid by the row that needs it and by no other. The census
    /// above says every line fits, which a builder that shed every tag onto a
    /// continuation would also satisfy — at the cost of doubling the length of
    /// a list the player scrolls, and of splitting the name from its stats on
    /// rows that had room for both.
    #[test]
    fn only_the_overflowing_inventory_row_spends_a_second_line() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(31, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let plain = GearCopy::plain("kinetic_edge".into());
        assert_eq!(
            inventory_row_lines(&game, 'a', &plain, 1, 1).len(),
            1,
            "an ordinary copy keeps its tag on the row it belongs to"
        );

        // The measured worst case: Gold, affixed, maxed, at the deepest zone
        // `balance_sim` sweeps to.
        let worst = GearCopy {
            item: "singularity_matrix".into(),
            rarity: Rarity::Gold,
            tier: MAX_FUSIONS,
            affix: Some("of_the_ghost_protocol".into()),
        };
        let lines = inventory_row_lines(&game, 'a', &worst, 1234, 10);
        assert_eq!(lines.len(), 2, "{lines:#?}");
        assert!(
            lines[1].trim_start().starts_with('(') && lines[1].contains("maxed"),
            "the tag sheds whole rather than being broken across the two: {lines:#?}"
        );
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
