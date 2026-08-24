//! The inventory list, its per-item action page, and the erase prompt.

use feral_processes_engine::battle::DamageRange;
use feral_processes_engine::views::RoutineDetailView;

use super::popup::*;
use super::*;

pub(super) fn draw_erase_quantity(
    game: &mut Game,
    pending: Option<GearCopy>,
    quantity_input: &str,
    refusal: Option<&str>,
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
    draw_popup("Erase", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_inventory(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(
            format!(
                "Level {}   Attack {}   Defense {}   Power {}   Decompiler {}",
                status.level, status.atk, status.mitigation, status.power, status.decompiler
            ),
            CYAN,
        ),
        text_row(""),
        text_row("Equipped (number to swap or unequip):"),
        // A wielded program and a worn weapon are mutually exclusive, so the
        // weapon line renders one or the other and never both. It takes the
        // column through `tagged_text` rather than `with_tag` because it is
        // not a row you can pick and there is no copy to have a quality: the
        // column is here so the block reads straight down, not to say
        // anything about the program standing in the slot.
        match &status.wielded {
            Some(w) => text_row(tagged_text(
                &row_lead(menu_shortcut(0), None),
                EquipmentSlot::Weapon.short_label(),
                &format!(
                    "{} Lv{} (+{} ATK, +{} DEF)",
                    w.name, w.level, w.bonus.0, w.bonus.1
                ),
            )),
            None => equipped_row(
                0,
                EquipmentSlot::Weapon,
                status.weapon.clone(),
                selected == 0,
                game,
            ),
        },
        equipped_row(
            1,
            EquipmentSlot::Armor,
            status.armor.clone(),
            selected == 1,
            game,
        ),
        equipped_row(
            2,
            EquipmentSlot::Module,
            status.module.clone(),
            selected == 2,
            game,
        ),
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
        let (lead, tag, lines) =
            inventory_row_lines(game, menu_shortcut(i + 3), &row.copy, row.qty, status.zone);
        let mut lines = lines.into_iter();
        let head = lines
            .next()
            .expect("inventory_row_lines always emits the item's own row");
        rows.push(with_tag(
            tier_row(head, selected == i + 3, row.copy.tier, row.copy.rarity),
            lead,
            tag,
            Some(row.copy.quality),
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
    rows.push(text_row(
        "[I] inspect — full stats, and what a granted routine actually does",
    ));
    draw_popup("Inventory", PopupSize::Large, &rows, refusal, painter, m);
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
) -> (String, &'static str, Vec<String>) {
    let lead = row_lead(shortcut, Some(qty));
    let tag = game.item_category(&copy.item).short_label();
    let mut lines = wrapped_tagged_lines(
        &lead,
        tag,
        &game.copy_name(copy),
        &[equip_preview_tag(game, copy, zone)],
    );
    lines.extend(effect_lines(game, &copy.item));
    (lead, tag, lines)
}

/// One slot of an equipment panel: its key, its category column, and what is
/// worn in it.
///
/// **The column is `with_tag`'s, so the worn copy's quality reads here
/// exactly as it does on the cargo rows below.** That emphasis is the only
/// thing on either screen saying how well a copy was compiled, and the three
/// rows naming what the player has actually put on were the ones without it.
///
/// It replaces the spelled-out slot rather than sitting beside it — `WEP` and
/// `Weapon:` say the same thing, and printing both puts one column on a row
/// twice. Which is also why this takes the `EquipmentSlot` rather than a
/// label: the word and the token are two forms of one value, and a caller
/// passing its own string could hand over a pair that disagree.
pub(super) fn equipped_row(
    index: usize,
    slot: EquipmentSlot,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    selected: bool,
    game: &Game,
) -> Row {
    // The tier the worn copy was equipped at, not the ledger's current one —
    // the same number `worn_summary` prints beside it, so the colour and the
    // text on this row cannot disagree.
    let fusions = equipped.as_ref().map(|e| e.copy.tier).unwrap_or(0);
    let rarity = equipped.as_ref().map(|e| e.copy.rarity).unwrap_or_default();
    let quality = equipped.as_ref().map(|e| e.copy.quality);
    with_tag(
        tier_row(worn_summary(equipped, game), selected, fusions, rarity),
        row_lead(menu_shortcut(index), None),
        slot.short_label(),
        quality,
    )
}

/// `Weapon: Arc Lance Lv3 T1 (+16 ATK)`, or `Weapon: (empty)` — the labelled
/// form, for the swap picker's heading, where the label is `Wearing` and so
/// is not a slot's name at all.
///
/// A row in an equipment panel takes `worn_summary` instead and puts the slot
/// in its category column; see `equipped_row`.
fn equipped_summary(
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    game: &Game,
) -> String {
    format!("{label}: {}", worn_summary(equipped, game))
}

/// `Arc Lance Lv3 T1 (+16 ATK)`, or `(empty)`.
///
/// The figure comes from `Game::copy_bonus` and is formatted by
/// `stat_summary`, so the equipped panel, the inventory list's tag and the
/// swap picker's columns cannot disagree about what an item grants. Sharing
/// only the formatter is what let them disagree before — see `copy_bonus`.
fn worn_summary(
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    game: &Game,
) -> String {
    let Some((equipped, mods)) =
        equipped.and_then(|e| game.copy_bonus(&e.copy, e.level).map(|mods| (e, mods)))
    else {
        return "(empty)".to_string();
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
        "{}{note} ({})",
        game.copy_name(&equipped.copy),
        stat_summary(game, mods)
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
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(slot) = slot else {
        draw_popup(
            "Gear",
            PopupSize::Small,
            &[text_row("No slot selected.")],
            refusal,
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
        // Wrapped, not joined: the name column alone runs 57 cells, and six
        // stat axes plus their six deltas do not fit the same line — so both
        // trailing columns are tags and shed onto a continuation when they
        // have to, the treatment `inventory_row_lines` gives an equip tag.
        // Ordinary rows keep all three on one line; the packer only sheds
        // what will not fit. Both lines carry the row's selection and tier
        // colour, so a wrapped entry still highlights as one thing.
        for line in wrapped_row_lines(
            format!("[{}] {}", menu_shortcut(i), row.label),
            &[row.stats.clone(), format!(" {}", row.delta)],
        ) {
            rows.push(tier_row(line, i == selected, row.fusion_tier, row.rarity));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Middle column is what you'd get; right column is the change",
    ));
    rows.push(text_row(
        "[I] inspect — full stats, and what a granted routine actually does",
    ));
    rows.push(text_row("Esc to go back; Up/Down + Enter also work"));
    draw_popup(
        &format!("Replace {}", slot.label()),
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The gear inspect page — `[I]` from every list that names a piece of
/// gear, and `[d]` from the item-action list.
///
/// **Every figure on it comes out of one `Game::gear_detail` call.** A
/// renderer that priced the copy itself is the bug `Game::copy_bonus`'s doc
/// records four times over; a renderer that read `AbilityEffect::Damage`'s
/// authored `power` would be the same bug on a second axis, quoting a
/// routine's level-1 figure to a level-12 player forever.
///
/// The page does not scroll — `draw_popup` pages a `Row::Item` span and
/// there are none here — so its height is held by
/// `the_tallest_gear_page_fits_its_popup` rather than by a scrollbar.
pub(super) fn draw_gear_inspect(
    game: &Game,
    inspect: Option<GearInspect>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(inspect) = inspect else {
        draw_popup(
            "Item",
            PopupSize::Small,
            &[text_row("Nothing selected.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let rows = gear_inspect_rows(game, &inspect);
    draw_popup("Item", PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, split out so a height test can count them without a
/// window — the same split `inventory_row_lines` makes for its width test.
pub(super) fn gear_inspect_rows(game: &Game, inspect: &GearInspect) -> Vec<Row> {
    let copy = &inspect.copy;
    let wearer = inspect.wearer.unwrap_or_else(|| game.player_entity());
    let detail = game.gear_detail(copy, wearer);

    // The title carries the copy's colour for the same reason every list
    // row does — see `render/mod.rs::tier_color`.
    let mut rows = vec![tier_row(detail.name.clone(), false, copy.tier, copy.rarity)];
    rows.push(text_row(""));
    match &detail.description {
        Some(text) => rows.extend(
            wrap_text(text, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        ),
        None => rows.push(text_row("(no description)")),
    }

    if let Some(worn) = &detail.worn {
        rows.push(text_row(""));
        // Through app-core's one stat formatter, so the page and the row
        // that opened it cannot quote different numbers.
        rows.push(text_row(format!(
            "{}: {}",
            worn.slot.label(),
            stat_summary(game, worn.stats)
        )));
        // Under the stats it explains, and unconditional — the name carries
        // the figure only when it is off spec, so this is the only place a
        // copy at spec says what it compiled at.
        rows.push(text_row(format!("Compiled at {}% of spec", worn.quality)));
        // Only for a piece that bears on the swing at all. Armour and most
        // modules carry neither a band nor accuracy, and a hit chance under
        // one reads as a claim about the armour rather than about the body
        // wearing it — the figure would be the player's bare accuracy with
        // an irrelevant item's name above it.
        //
        // A projection either way, and labelled as one: there is no opponent
        // until a fight starts, so the page names the one it measured
        // against. See `views::NominalHostile`.
        if worn.stats.damage != DamageRange::default() || worn.stats.accuracy != 0 {
            rows.push(text_row(format!(
                "Accuracy {:.0} — {:.0}% to land a swing on a typical zone-{} program",
                worn.accuracy,
                worn.hit_chance * 100.0,
                worn.nominal.zone
            )));
        }
    }

    if !detail.effects.is_empty() {
        rows.push(text_row(""));
        rows.extend(detail.effects.iter().map(text_row));
    }

    if let Some(grant) = &detail.grant {
        rows.push(text_row(""));
        rows.push(text_row(format!("Grants: {}", grant.name)));
        rows.extend(
            wrap_text(&grant.description, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        );
        for line in routine_mechanics(grant) {
            rows.push(text_row(format!("  {line}")));
        }
    }

    rows.push(text_row(""));
    rows.push(text_row("Any key to go back"));
    rows
}

/// The four mechanics lines under a granted routine's prose: when it runs,
/// what it lands on, what it does, and what it costs.
///
/// Every one of them is a field of `views::RoutineDetailView` and none is
/// derived here — a renderer deciding what a routine does is a second
/// opinion about the same `.ron` file.
fn routine_mechanics(grant: &RoutineDetailView) -> Vec<String> {
    let mut lines = vec![
        grant.when.clone(),
        format!("Hits {}", grant.target),
        match grant.rolls_to_hit {
            true => format!("{}, and can miss", grant.effect),
            false => grant.effect.clone(),
        },
    ];
    let mut price = vec![match grant.cooldown {
        0 => "No cooldown".to_string(),
        1 => "Cooldown 1 round".to_string(),
        n => format!("Cooldown {n} rounds"),
    }];
    // A free routine says nothing about its price rather than printing a
    // zero: `power_cost` defaults to 0.0 and most of the roster leaves it
    // there, so a "Costs 0 Power" row would be noise on nearly every page.
    if grant.power_cost > 0.0 {
        price.push(format!("Costs {:.0} Power", grant.power_cost));
    }
    lines.push(price.join(" · "));
    lines
}

pub(super) fn draw_inventory_item_action(
    game: &mut Game,
    copy: Option<GearCopy>,
    zone_level: u32,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else {
        draw_popup(
            "Item",
            PopupSize::Small,
            &[text_row("Nothing selected.")],
            refusal,
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
    draw_popup("Item", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::{equipped_row, equipped_summary, gear_inspect_rows, inventory_row_lines};
    use crate::paint::{painted_runs_in, with_painter};
    use crate::render::popup::{
        PopupSize, REFUSAL_MAX_LINES, draw_row, item_text, popup_max_rows, tagged_text,
        wrapped_row_lines,
    };
    use crate::render::{Row, quality_tag_style};
    use crate::text::ui_metrics;
    use feral_processes_app_core::{GearInspect, Mode, menu_shortcut};
    use feral_processes_engine::components::Rarity;
    use feral_processes_engine::items::{EquipmentSlot, GearCopy};
    use feral_processes_engine::tuning::{MAX_FUSIONS, QUALITY_MAX};
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

    /// **The equipped panel carries the same category column the cargo rows
    /// do**, and with it the only thing on either screen saying how well a
    /// copy was compiled.
    ///
    /// The column's emphasis is `quality_tag_style`'s — dim under spec, bold
    /// above it, gold at the top — so the three rows naming what the player
    /// is *actually wearing* were the only gear rows in the game that could
    /// not answer it. It replaces the spelled-out slot rather than sitting
    /// beside it: `WEP` and `Weapon:` say the same thing, and printing both
    /// puts one column on a row twice.
    ///
    /// Drawn through the real painter for
    /// `a_drawn_tag_carries_its_band_and_the_row_keeps_its_own_colour`'s
    /// reason — the three runs are only three styles once egui has laid the
    /// job out, and `painted_text` would flatten them back into one.
    #[test]
    fn an_equipped_row_draws_its_slot_in_the_worn_copys_quality() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let mut game = Game::new(77, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let path = std::env::temp_dir().join("feral_processes_gui_equipped_column.sav");
        game.save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.player.weapon = Some("kinetic_edge".into());
        data.player.weapon_quality = QUALITY_MAX;
        save::save_to_file(&path, &data).unwrap();
        let game = Game::load(&path, assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let worn = game
            .worn(game.player_entity(), EquipmentSlot::Weapon)
            .expect("wearing it");
        let m = ui_metrics(1080.0);
        let row = equipped_row(0, EquipmentSlot::Weapon, Some(worn), false, &game);
        let (_, shapes) = with_painter(|p| {
            draw_row(&row, 0.0, 800.0, 40.0, 400.0, p, &m);
        });
        let (color, bold) = quality_tag_style(Some(QUALITY_MAX));
        assert_eq!(
            painted_runs_in(&shapes, color, bold),
            vec!["WEP"],
            "the worn copy's band has to reach the column, or the panel says \
             nothing about how well the thing was compiled"
        );

        // An empty slot still names the slot it is empty of, and takes its
        // key and column from the same two definitions the worn row does.
        let empty = equipped_row(1, EquipmentSlot::Armor, None, false, &game);
        let Row::Item { text, tag, .. } = &empty else {
            panic!("an equipment slot is a row you can pick")
        };
        assert_eq!(item_text(text, tag.as_ref()), "[2] ARM  (empty)");
    }

    /// **The tallest page any shipped item can produce fits its popup.**
    ///
    /// This page is built entirely out of text rows, and `draw_popup` only
    /// pages a `Row::Item` span — so there is no scroll here and nothing
    /// clamps a row back into the box. A page one row too tall silently
    /// loses its last line, which on this screen is the routine's cooldown
    /// and price, or the "any key to go back" that says how to leave.
    ///
    /// A census over `item_defs` rather than a fixture, for the reason the
    /// width test above gives: the tallest page is a property of the assets
    /// (a long description, a long routine description) *and* of how this
    /// screen stacks them, so a wordier item added later has to fail here.
    /// Plain copies are enough — rarity, fusion and an affix all decorate
    /// the name, and none of them adds a row.
    /// **A hit chance belongs under gear that bears on the swing.**
    ///
    /// The figure is the wearer's, not the item's, so under a piece
    /// carrying neither a damage band nor accuracy it is the player's bare
    /// accuracy with an irrelevant item's name above it — which reads as a
    /// claim about the armour. The Deadman Relay is a module with nothing
    /// but mitigation; the Kinetic Edge is the case the line exists for.
    #[test]
    fn only_gear_that_bears_on_the_swing_quotes_a_hit_chance() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let quotes = |id: &str| {
            let inspect = GearInspect {
                copy: GearCopy::plain(id.into()),
                wearer: None,
                from: Mode::Inventory,
            };
            gear_inspect_rows(&game, &inspect).iter().any(|row| {
                matches!(row, Row::Text(t) | Row::TextColored(t, _) if t.contains("to land a swing"))
            })
        };

        assert!(quotes("kinetic_edge"), "a weapon decides what you hit with");
        assert!(
            !quotes("deadman_relay"),
            "a mitigation-only module says nothing about landing a swing"
        );
    }

    /// The figure reaches the page as a row of its own. `copy_name` puts it
    /// in the title too, but only off spec — so without this row a copy at
    /// spec has no statement of its quality anywhere on the one screen built
    /// for comparing two copies.
    #[test]
    fn the_gear_page_carries_a_quality_row() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(43, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let quality_row = |copy: GearCopy| {
            let inspect = GearInspect {
                copy,
                wearer: None,
                from: Mode::Inventory,
            };
            gear_inspect_rows(&game, &inspect).iter().any(|row| {
                matches!(row, Row::Text(t) | Row::TextColored(t, _) if t.contains("Compiled at 115%"))
            })
        };

        assert!(quality_row(GearCopy {
            quality: 115,
            ..GearCopy::plain("kinetic_edge".into())
        }));
        assert!(
            !quality_row(GearCopy::plain("kinetic_edge".into())),
            "a copy at spec must not claim 115%"
        );
    }

    #[test]
    fn the_tallest_gear_page_fits_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(41, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let mut tallest = (0usize, String::new());
        for def in game.item_defs() {
            let inspect = GearInspect {
                copy: GearCopy::plain(def.id.clone()),
                wearer: None,
                from: Mode::Inventory,
            };
            let rows = gear_inspect_rows(&game, &inspect).len();
            if rows > tallest.0 {
                tallest = (rows, def.name.clone());
            }
        }

        // Swept rather than measured at one window, because `ui_metrics`
        // clamps the font at both ends: below the clamp the box keeps
        // shrinking while the line height stops, so the tightest window is
        // the smallest one and not the one a test happens to run at.
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            // Plus the room a refusal standing over the page needs: this
            // one has no scroll, so a row past the bottom is dropped in
            // silence rather than paged to.
            assert!(
                tallest.0 + REFUSAL_MAX_LINES <= cap,
                "{} builds a {}-row page into a {cap}-row popup at {h}px",
                tallest.1,
                tallest.0
            );
        }
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
                        rarity: Rarity::Gold,
                        tier,
                        affix: affix.clone(),
                        // The widest a copy gets on the quality axis: the
                        // figure is three digits and the stats it prices are
                        // 30% higher, so a figure that gained a digit would
                        // show up here.
                        quality: QUALITY_MAX,
                        ..GearCopy::plain(def.id.clone().into())
                    })
                })
            })
            .map(|copy| {
                (
                    game.copy_name(&copy),
                    // A four-digit count rather than the three the column
                    // reserves: the buffer is unbounded and a long run's
                    // scrap pile reaches it, at which point the row grows.
                    drawn_inventory_lines(&game, menu_shortcut(35), &copy, 1234, zone),
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

    /// The lines `draw_inventory` actually draws for a carried copy: its head
    /// with the tag column joined back on, then its continuations.
    ///
    /// The column is a reserved slot in the drawn label, so a width census
    /// measuring the head as `inventory_row_lines` returns it would be
    /// measuring a row the screen never draws. `item_text` is the one join.
    fn drawn_inventory_lines(
        game: &Game,
        shortcut: char,
        copy: &GearCopy,
        qty: u32,
        zone: u32,
    ) -> Vec<String> {
        let (lead, tag, mut lines) = inventory_row_lines(game, shortcut, copy, qty, zone);
        lines[0] = tagged_text(&lead, tag, &lines[0]);
        lines
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

        let lines = drawn_inventory_lines(&game, menu_shortcut(3), &copy, 1, 1);

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
            drawn_inventory_lines(&game, menu_shortcut(3), &copy, 1, 1).len(),
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
            drawn_inventory_lines(&game, 'a', &plain, 1, 1).len(),
            1,
            "an ordinary copy keeps its tag on the row it belongs to"
        );

        // The measured worst case: Gold, affixed, maxed, at the deepest zone
        // `balance_sim` sweeps to.
        let worst = GearCopy {
            rarity: Rarity::Gold,
            tier: MAX_FUSIONS,
            affix: Some("of_the_ghost_protocol".into()),
            ..GearCopy::plain("singularity_matrix".into())
        };
        let lines = drawn_inventory_lines(&game, 'a', &worst, 1234, 10);
        assert_eq!(lines.len(), 2, "{lines:#?}");
        assert!(
            lines[1].trim_start().starts_with('(') && lines[1].contains("maxed"),
            "the tag sheds whole rather than being broken across the two: {lines:#?}"
        );
    }

    /// A rare tier is drawn as a *word* in front of the item name, and
    /// `swap_name_column` pads with `{:<N}` — which never truncates. So a
    /// name past `SWAP_NAME_COLUMN` does not clip: it pushes the stat and
    /// delta columns right and misaligns every row below it.
    ///
    /// Measured against the real font rather than counted in characters,
    /// because counting is exactly what missed this — the column was 20
    /// cells and "Overclocked Monofilament Whip" is 29.
    ///
    /// **Measured again when quality shipped.** At 900px the popup body is
    /// 1243.2px and a UI cell is 10.8438px — 114.65 cells. The old joined
    /// head (`[a] {name:<50} {stats:<20}`, plus `draw_row`'s two-space
    /// prefix) was 111 cells and fitted with 3.7 to spare; the quality
    /// figure costs seven, which put it 35.6px past the edge. That is why
    /// the stat column is a tag now and not part of the head — and why this
    /// builds its lines the way `draw_equip_swap` does rather than joining
    /// them itself.
    #[test]
    fn the_widest_swap_row_still_fits_its_popup() {
        // The widest row this screen can build out of the shipped assets: the
        // longest tier word, the longest affix, the longest equippable name,
        // a maxed fusion note and a six-axis summary. The figures are the
        // ones `no_shipped_gear_summary_outgrows_the_swap_stats_column`
        // measures off the real items at the top gear level, fusion tier and
        // rare tier — that census is what says this string is still the
        // worst case.
        //
        // Measured **as wrapped**, because six axes printed twice do not fit
        // one line and `draw_equip_swap` no longer asks them to. Every line
        // the row produces has to fit, which is what this iterates.
        let head = format!(
            "[a] {:<57}",
            format!(
                "{} Singularity Matrix of Quiet Handshakes (130%)",
                Rarity::Gold.label().expect("Gold reads as a word")
            ),
        );
        let lines = wrapped_row_lines(
            head,
            &[
                " 206–310 DMG +103 ATK +103 MIT +69 ACC +103 DECOMP T3/3".to_string(),
                " -206–-310 DMG -103 ATK -103 MIT -69 ACC -103 DECOMP".to_string(),
            ],
        );
        assert_eq!(
            lines.len(),
            3,
            "the worst case has to shed both tags, or this measures the easy \
             case: {lines:#?}"
        );

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                assert!(
                    drawn <= room,
                    "a gear-swap row line overflows by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
