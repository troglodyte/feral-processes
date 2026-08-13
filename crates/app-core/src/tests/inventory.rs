//! Item actions, erasing, and the equip preview tag.

use feral_processes_engine::affixes::AffixId;
use feral_processes_engine::items::ids;

use super::support::*;
use crate::*;

#[test]
fn erasing_asks_for_a_quantity_and_removes_exactly_that_many() {
    let mut app = test_app(900);
    let before = app
        .game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item == ItemId::from(ids::CORE_FRAGMENT))
        .map(|r| r.qty)
        .unwrap();

    app.pending_inventory_item = Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0));
    app.mode = Mode::InventoryItemAction;
    app.handle_key(GameKey::Char('x'));
    assert_eq!(
        app.mode,
        Mode::EraseQuantity,
        "[X] should ask how many, not dump the whole stack"
    );

    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Enter);

    let after = app
        .game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item == ItemId::from(ids::CORE_FRAGMENT))
        .map(|r| r.qty)
        .unwrap();
    assert_eq!(after, before - 3);
    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn erase_all_dumps_the_whole_stack() {
    let mut app = test_app(901);
    app.pending_inventory_item = Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0));
    app.mode = Mode::InventoryItemAction;
    app.handle_key(GameKey::Char('x'));
    app.handle_key(GameKey::Char('a'));

    let held = app
        .game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item == ItemId::from(ids::CORE_FRAGMENT))
        .map(|r| r.qty);
    assert_eq!(held, None, "[A] should clear the stack entirely");
}

#[test]
fn escaping_the_erase_prompt_erases_nothing() {
    let mut app = test_app(902);
    let before = app.game.as_ref().unwrap().player_status().inventory;
    app.pending_inventory_item = Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0));
    app.mode = Mode::InventoryItemAction;
    app.handle_key(GameKey::Char('x'));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.game.as_ref().unwrap().player_status().inventory, before);
}

#[test]
fn every_equippable_item_offers_equip_fuse_and_erase() {
    let mut app = test_app(904);
    let game = app.game.as_mut().unwrap();
    for item in [
        ItemId::from(ids::OVERCLOCK_CORE),
        ItemId::from(ids::MONOFILAMENT_WHIP),
        ItemId::from(ids::FIREWALL_PLATING),
        ItemId::from(ids::ABLATIVE_PLATING),
        ItemId::from(ids::NEURAL_AMPLIFIER),
        ItemId::from(ids::CORTEX_HACK),
    ] {
        let keys: Vec<char> = inventory_item_actions(game, &item)
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(
            keys,
            vec!['e', 'u', 'd', 'x'],
            "{} should offer fuse regardless of how many copies are held",
            game.item_name(&item)
        );
    }
}

#[test]
fn a_plain_resource_offers_only_describe_and_erase() {
    let mut app = test_app(905);
    let game = app.game.as_mut().unwrap();
    let keys: Vec<char> = inventory_item_actions(game, &ItemId::from(ids::CORE_FRAGMENT))
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        keys,
        vec!['d', 'x'],
        "even a plain resource has an authored description worth reading"
    );
}

#[test]
fn describe_opens_a_page_and_esc_returns_to_the_action_list() {
    let mut app = test_app(906);
    app.pending_inventory_item = Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0));
    app.mode = Mode::InventoryItemAction;

    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inventory_item,
        Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0)),
        "the page needs to still know which item it is describing"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::InventoryItemAction,
        "Esc should step back to the actions, not out to the inventory"
    );
}

/// Descriptions are authored in `assets/items/*.ron` so they can be edited
/// without touching Rust — the page must read that text, not a derived
/// gloss like `Game::item_blurb`.
#[test]
fn the_describe_page_reads_the_authored_ron_description() {
    let app = test_app(907);
    let game = app.game.as_ref().unwrap();
    let text = game
        .item_description(&ItemId::from(ids::CORE_FRAGMENT))
        .expect("core_fragment.ron authors a description");
    assert!(
        !text.is_empty(),
        "an authored description should not come back blank"
    );
}

#[test]
fn fusing_without_enough_copies_explains_why_instead_of_ignoring_the_key() {
    let mut app = test_app(903);
    app.pending_inventory_item = Some(gear(&ItemId::from(ids::OVERCLOCK_CORE), 0));
    app.mode = Mode::InventoryItemAction;
    app.handle_key(GameKey::Char('u'));

    assert_eq!(
        app.status_line.as_deref(),
        Some("Need 2 Overclock Core to fuse (have 0)."),
        "[U] on a too-small stack must refuse out loud, not silently do nothing"
    );
}

#[test]
fn equip_preview_tag_leads_with_the_slot_the_item_would_take() {
    let app = test_app(900);
    let game = app.game.as_ref().expect("test_app builds a game");

    assert_eq!(
        equip_preview_tag(game, &gear(&ItemId::from(ids::MONOFILAMENT_WHIP), 0), 1),
        " (WEP +4 ATK)"
    );
    assert_eq!(
        equip_preview_tag(game, &gear(&ItemId::from(ids::ABLATIVE_PLATING), 0), 1),
        " (ARM +4 DEF)"
    );
    assert_eq!(
        equip_preview_tag(game, &gear(&ItemId::from(ids::CORTEX_HACK), 0), 1),
        " (MOD +3 DECOMP)"
    );
}

/// The trade screen tags its rows with the same helper the inventory
/// uses, so anything a trading post stocks has to produce a real
/// WEP/ARM/MOD tag — an empty one there is a blank column, not a
/// harmless omission.
#[test]
fn every_equippable_item_a_trading_post_stocks_has_a_slot_tag() {
    let app = test_app(902);
    let game = app.game.as_ref().expect("test_app builds a game");

    let stocked: Vec<ItemId> = game
        .structure_defs()
        .into_iter()
        .filter_map(|d| d.trade)
        .flat_map(|t| t.buy.into_iter().map(|(item, _)| item))
        .collect();
    assert!(
        !stocked.is_empty(),
        "a shipped trading post should stock something to buy"
    );

    for item in stocked {
        if !game.is_equippable(&item) {
            continue;
        }
        let tag = equip_preview_tag(game, &gear(&item, 0), 1);
        assert!(
            tag.contains("WEP") || tag.contains("ARM") || tag.contains("MOD"),
            "{item} is equippable stock but its trade row would show {tag:?}"
        );
    }
}

#[test]
fn equip_preview_tag_stays_empty_for_a_non_equippable_item() {
    let app = test_app(901);
    let game = app.game.as_ref().expect("test_app builds a game");

    assert_eq!(
        equip_preview_tag(game, &gear(&ItemId::from(ids::CORE_FRAGMENT), 0), 1),
        "",
        "a non-equippable item must contribute no tag at all, not a bare slot"
    );
}

#[test]
fn equip_preview_tag_keeps_showing_level_scaling_and_fusion_beside_the_slot() {
    let app = test_app(902);
    let game = app.game.as_ref().expect("test_app builds a game");

    // Zone 2 doubles the base bonus (GEAR_LEVEL_GROWTH), and one fusion
    // tier adds ITEM_FUSION_BONUS_PER_TIER on top: 4 -> 8 -> 10.
    assert_eq!(
        equip_preview_tag(game, &gear(&ItemId::from(ids::MONOFILAMENT_WHIP), 1), 2),
        " (WEP +10 ATK fusion T1/3)"
    );
}

/// Gear shares `MAX_FUSIONS` with programs, so its tag names the ceiling
/// the same way a program's `(fused 1/3)` does — and says "maxed" at it,
/// which is the whole reason this screen is where the cap is discovered.
#[test]
fn equip_preview_tag_names_the_ceiling_and_calls_out_a_maxed_item() {
    let app = test_app(903);
    let game = app.game.as_ref().expect("test_app builds a game");
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);

    assert!(
        !equip_preview_tag(game, &gear(&whip, 0), 1).contains("fusion"),
        "an unfused item mentions no tier at all"
    );
    assert!(
        equip_preview_tag(game, &gear(&whip, MAX_FUSIONS), 1).ends_with("fusion T3/3 - maxed)"),
        "got: {}",
        equip_preview_tag(game, &gear(&whip, MAX_FUSIONS), 1)
    );
}

/// The compact note the equipped panel and the swap picker's stat column
/// share. No "maxed" wording here: `SWAP_STATS_COLUMN` is 20 cells and
/// `+2 ATK +1 DEF T3/3 maxed` is 24 — the row colour carries it instead.
/// The picker's rows are built here and only drawn by the renderer, so the
/// tier travels on the row rather than being re-derived on the far side —
/// a renderer that recomputed it could colour a row its own label
/// contradicts.
#[test]
fn a_swap_row_carries_its_items_fusion_tier() {
    let mut app = app_wearing_weapon(913, None, &[("kinetic_edge", 3)], 1);
    let spare = ItemId::from("kinetic_edge");
    app.game
        .as_mut()
        .unwrap()
        .fuse_item(&gear(&spare, 0))
        .unwrap();

    let rows = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    );
    let row = rows
        .iter()
        .find(|r| r.choice == SwapChoice::Equip(gear(&spare.clone(), 1)))
        .expect("the fused spare should still be offered");

    assert_eq!(row.fusion_tier, 1);
    assert!(
        row.label.contains("T1/3"),
        "the stat column names the ceiling too, got: {}",
        row.label
    );
}

#[test]
fn item_fusion_note_is_the_bare_fraction() {
    assert_eq!(item_fusion_note(0), "");
    assert_eq!(item_fusion_note(1), "T1/3");
    assert_eq!(item_fusion_note(MAX_FUSIONS), "T3/3");
}

#[test]
fn selecting_an_equipped_slot_opens_the_swap_list_instead_of_unequipping() {
    let mut app = app_wearing_weapon(910, Some(("overclock_core", 1)), &[("kinetic_edge", 1)], 1);
    app.mode = Mode::Inventory;

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::EquipSwap);
    assert_eq!(app.pending_swap_slot, Some(EquipmentSlot::Weapon));
    assert_eq!(
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .weapon
            .map(|e| e.copy.item),
        Some(ItemId::from("overclock_core")),
        "opening the picker must not strip the slot on the way in"
    );
}

#[test]
fn picking_a_swap_row_equips_it_and_returns_to_the_inventory() {
    let mut app = app_wearing_weapon(911, Some(("overclock_core", 1)), &[("kinetic_edge", 1)], 1);
    app.mode = Mode::Inventory;
    app.handle_key(GameKey::Char('1'));

    let rows = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    );
    let idx = rows
        .iter()
        .position(|r| r.choice == SwapChoice::Equip(gear(&ItemId::from("kinetic_edge"), 0)))
        .expect("the spare weapon should be offered");
    app.handle_key(GameKey::Char(menu_shortcut(idx)));

    let status = app.game.as_ref().unwrap().player_status();
    assert_eq!(
        status.weapon.map(|e| e.copy.item),
        Some(ItemId::from("kinetic_edge"))
    );
    assert!(
        status
            .inventory
            .iter()
            .any(|r| r.copy.item == ItemId::from("overclock_core") && r.qty == 1),
        "the weapon that came off must land back in cargo"
    );
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.pending_swap_slot, None);
}

#[test]
fn the_unequip_row_empties_the_slot() {
    let mut app = app_wearing_weapon(912, Some(("overclock_core", 1)), &[("kinetic_edge", 1)], 1);
    app.mode = Mode::Inventory;
    app.handle_key(GameKey::Char('1'));

    let rows = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    );
    let idx = rows
        .iter()
        .position(|r| r.choice == SwapChoice::Unequip)
        .expect("an occupied slot must offer to be emptied");
    app.handle_key(GameKey::Char(menu_shortcut(idx)));

    let status = app.game.as_ref().unwrap().player_status();
    assert!(status.weapon.is_none());
    assert!(
        status
            .inventory
            .iter()
            .any(|r| r.copy.item == ItemId::from("overclock_core") && r.qty == 1)
    );
    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn the_swap_list_offers_only_gear_for_that_slot() {
    let app = app_wearing_weapon(
        913,
        Some(("overclock_core", 1)),
        &[
            ("kinetic_edge", 1),
            ("firewall_plating", 1),
            ("cortex_hack", 1),
            ("core_fragment", 4),
        ],
        1,
    );

    let offered: Vec<ItemId> = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    )
    .into_iter()
    .filter_map(|r| match r.choice {
        SwapChoice::Equip(copy) => Some(copy.item),
        SwapChoice::Unequip => None,
    })
    .collect();

    assert_eq!(
        offered,
        vec![ItemId::from("kinetic_edge")],
        "armor, a module and a raw material all fail to fit a weapon slot"
    );
}

#[test]
fn swap_rows_are_sorted_best_first_with_unequip_last() {
    let app = app_wearing_weapon(
        914,
        Some(("overclock_core", 1)),
        &[
            ("shiv_routine", 1),
            ("monofilament_whip", 1),
            ("kinetic_edge", 1),
        ],
        1,
    );

    let choices: Vec<SwapChoice> = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    )
    .into_iter()
    .map(|r| r.choice)
    .collect();

    // Worn is +3 ATK, so the deltas run whip +1, edge -1, shiv -2.
    assert_eq!(
        choices,
        vec![
            SwapChoice::Equip(gear(&ItemId::from("monofilament_whip"), 0)),
            SwapChoice::Equip(gear(&ItemId::from("kinetic_edge"), 0)),
            SwapChoice::Equip(gear(&ItemId::from("shiv_routine"), 0)),
            SwapChoice::Unequip,
        ],
        "the upgrade should be row 1 and emptying the slot the last resort"
    );
}

/// Gear is stamped with the zone level it was equipped at and gains a flat
/// step per level (`GEAR_LEVEL_STEP`), so a spare copy of what you already
/// wear is a real upgrade after a breach. The delta has to compare the worn
/// item at its *recorded* level against the candidate at the *current*
/// zone's.
#[test]
fn a_spare_of_the_worn_item_reports_the_gain_from_re_equipping_it() {
    let app = app_wearing_weapon(
        915,
        Some(("overclock_core", 1)),
        &[("overclock_core", 1)],
        3,
    );

    let rows = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    );
    let row = rows
        .iter()
        .find(|r| r.choice == SwapChoice::Equip(gear(&ItemId::from("overclock_core"), 0)))
        .expect("a spare of the worn item is still a candidate");

    // Base +3 ATK: worn remembers level 1, a fresh equip lands at zone 3
    // (3 * 3 = 9), so re-equipping is worth +6.
    assert!(
        row.label.contains("+9 ATK"),
        "the candidate should be previewed at the level it would equip at; got {:?}",
        row.label
    );
    assert!(
        row.label.contains("+6 ATK"),
        "the delta should be the gain over the worn copy; got {:?}",
        row.label
    );
}

#[test]
fn an_empty_slot_with_nothing_to_fill_it_stays_on_the_inventory() {
    let mut app = app_wearing_weapon(916, None, &[("core_fragment", 4)], 1);
    app.mode = Mode::Inventory;

    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.mode,
        Mode::Inventory,
        "an empty picker should not open at all"
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("Nothing in cargo fits your Weapon slot.")
    );
}

#[test]
fn esc_returns_from_the_swap_list_to_the_inventory() {
    let mut app = app_wearing_weapon(917, Some(("overclock_core", 1)), &[("kinetic_edge", 1)], 1);
    app.mode = Mode::Inventory;
    app.handle_key(GameKey::Char('1'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.pending_swap_slot, None);
    assert_eq!(
        app.menu_selected, 0,
        "Esc should leave the highlight back on the slot it came from"
    );
}

/// Same rule on the screen that lists what the player is carrying. Three
/// equipment slot rows come first, so row 3 is the first item — and it must
/// be the salvage, not the bank.
#[test]
fn the_inventory_screen_lists_no_row_for_a_banked_item() {
    let app = app_at_a_trading_post(925, &[(ids::RESEARCH_DATA, 40), (ids::CORE_FRAGMENT, 5)]);
    let listed = app.game.as_ref().unwrap().player_status().inventory;

    assert!(
        !listed
            .iter()
            .any(|r| r.copy.item.as_str() == ids::RESEARCH_DATA),
        "a bank must not be an inventory row: {listed:?}"
    );
    assert!(
        listed
            .iter()
            .any(|r| r.copy.item.as_str() == ids::CORE_FRAGMENT),
        "ordinary cargo is untouched: {listed:?}"
    );
}

/// The reported bug, at the screen it was reported on. Fusing used to
/// upgrade the item *type*, so the whole stack redrew as fused; the fix is
/// one row per `(item, tier)`, which is exactly what the player asked to
/// see. This walks the fusion through `Mode::InventoryItemAction` rather
/// than calling the engine, because the row the handler acts on is picked
/// out of the same list the renderer draws.
#[test]
fn fusing_from_the_inventory_screen_splits_the_stack_into_two_rows() {
    let spare = ItemId::from("kinetic_edge");
    let mut app = app_wearing_weapon(9130, None, &[("kinetic_edge", 6)], 1);
    app.mode = Mode::Inventory;
    // Three equipment slot rows come before the pack, and the pack holds
    // only the one item.
    app.menu_selected = 3;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::InventoryItemAction);
    app.handle_key(GameKey::Char('u'));

    let rows: Vec<(u32, u32)> = app
        .game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .filter(|r| r.copy.item == spare)
        .map(|r| (r.copy.tier, r.qty))
        .collect();

    assert_eq!(
        rows,
        vec![(0, 4), (1, 1)],
        "one fused copy and four ordinary spares, listed apart"
    );
}

/// Two copies of one item at different tiers are two different pieces of
/// gear, so the picker offers each on its own row at its own bonus.
#[test]
fn the_swap_picker_lists_a_fused_copy_beside_its_spares() {
    let spare = ItemId::from("kinetic_edge");
    let mut app = app_wearing_weapon(9131, None, &[("kinetic_edge", 4)], 1);
    app.game
        .as_mut()
        .unwrap()
        .fuse_item(&gear(&spare, 0))
        .unwrap();

    let tiers: Vec<u32> = equip_swap_rows(
        app.game.as_ref().unwrap(),
        app.game.as_ref().unwrap().player_entity(),
        EquipmentSlot::Weapon,
    )
    .into_iter()
    .filter_map(|r| match r.choice {
        SwapChoice::Equip(copy) if copy.item == spare => Some(copy.tier),
        _ => None,
    })
    .collect();

    assert_eq!(
        tiers,
        vec![1, 0],
        "the fused copy is the better row and sorts first"
    );
}

/// **The drift guard for `SWAP_NAME_COLUMN`.** The gui's
/// `the_widest_swap_row_still_fits_its_popup` measures one hand-written
/// worst-case string against the real font; this asks the shipped assets
/// whether that string is still the worst case.
///
/// Without it the pair goes stale exactly the way the palette test did: a
/// long affix or a long item name lands, the hand-written string still fits,
/// and the column silently starts shunting every row below a long name.
/// Names are built through `Game::copy_name`, so this covers both tier words
/// and affixes at once.
#[test]
fn no_shipped_copy_name_outgrows_the_swap_name_column() {
    let mut app = test_app(931);
    let game = app.game.as_mut().expect("test_app builds a game");

    let equippables: Vec<ItemId> = game
        .item_defs()
        .into_iter()
        .filter(|d| d.equipment.is_some())
        .map(|d| d.id)
        .collect();
    assert!(
        !equippables.is_empty(),
        "the shipped set has equippable gear"
    );

    let affixes: Vec<Option<AffixId>> = std::iter::once(None)
        .chain(game.affix_defs().into_iter().map(|a| Some(a.id)))
        .collect();

    let mut worst = (String::new(), 0usize);
    for item in &equippables {
        for rarity in Rarity::ALL {
            for affix in &affixes {
                let name = game.copy_name(&GearCopy {
                    item: item.clone(),
                    rarity,
                    tier: 0,
                    affix: affix.clone(),
                });
                if name.chars().count() > worst.1 {
                    worst = (name.clone(), name.chars().count());
                }
            }
        }
    }

    assert!(
        worst.1 <= SWAP_NAME_COLUMN_FOR_TESTS,
        "{:?} is {} cells and the column is {} — widen SWAP_NAME_COLUMN and \
         update the gui's worst-case string, or shorten the asset",
        worst.0,
        worst.1,
        SWAP_NAME_COLUMN_FOR_TESTS
    );
}
