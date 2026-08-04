//! Item actions, erasing, and the equip preview tag.

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
        .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
        .map(|(_, q)| *q)
        .unwrap();

    app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
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
        .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
        .map(|(_, q)| *q)
        .unwrap();
    assert_eq!(after, before - 3);
    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn erase_all_dumps_the_whole_stack() {
    let mut app = test_app(901);
    app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
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
        .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
        .map(|(_, q)| *q);
    assert_eq!(held, None, "[A] should clear the stack entirely");
}

#[test]
fn escaping_the_erase_prompt_erases_nothing() {
    let mut app = test_app(902);
    let before = app.game.as_ref().unwrap().player_status().inventory;
    app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
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
    app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
    app.mode = Mode::InventoryItemAction;

    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inventory_item,
        Some(ItemId::from(ids::CORE_FRAGMENT)),
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
    app.pending_inventory_item = Some(ItemId::from(ids::OVERCLOCK_CORE));
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
        equip_preview_tag(game, &ItemId::from(ids::MONOFILAMENT_WHIP), 1, 0),
        " (WEP +4 ATK)"
    );
    assert_eq!(
        equip_preview_tag(game, &ItemId::from(ids::ABLATIVE_PLATING), 1, 0),
        " (ARM +4 DEF)"
    );
    assert_eq!(
        equip_preview_tag(game, &ItemId::from(ids::CORTEX_HACK), 1, 0),
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
        let tag = equip_preview_tag(game, &item, 1, 0);
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
        equip_preview_tag(game, &ItemId::from(ids::CORE_FRAGMENT), 1, 0),
        "",
        "a non-equippable item must contribute no tag at all, not a bare slot"
    );
}

#[test]
fn equip_preview_tag_keeps_showing_level_scaling_and_fusion_beside_the_slot() {
    let app = test_app(902);
    let game = app.game.as_ref().expect("test_app builds a game");

    // Zone 2 doubles the base bonus (GEAR_LEVEL_GROWTH), and one fusion
    // tier adds ITEM_FUSION_BONUS_PER_TIER on top: 4 -> 8 -> 9.
    assert_eq!(
        equip_preview_tag(game, &ItemId::from(ids::MONOFILAMENT_WHIP), 2, 1),
        " (WEP +9 ATK fusion T1)"
    );
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
            .map(|e| e.item),
        Some(ItemId::from("overclock_core")),
        "opening the picker must not strip the slot on the way in"
    );
}

#[test]
fn picking_a_swap_row_equips_it_and_returns_to_the_inventory() {
    let mut app = app_wearing_weapon(911, Some(("overclock_core", 1)), &[("kinetic_edge", 1)], 1);
    app.mode = Mode::Inventory;
    app.handle_key(GameKey::Char('1'));

    let rows = equip_swap_rows(app.game.as_ref().unwrap(), EquipmentSlot::Weapon);
    let idx = rows
        .iter()
        .position(|r| r.choice == SwapChoice::Equip(ItemId::from("kinetic_edge")))
        .expect("the spare weapon should be offered");
    app.handle_key(GameKey::Char(menu_shortcut(idx)));

    let status = app.game.as_ref().unwrap().player_status();
    assert_eq!(
        status.weapon.map(|e| e.item),
        Some(ItemId::from("kinetic_edge"))
    );
    assert!(
        status
            .inventory
            .iter()
            .any(|(i, q)| *i == ItemId::from("overclock_core") && *q == 1),
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

    let rows = equip_swap_rows(app.game.as_ref().unwrap(), EquipmentSlot::Weapon);
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
            .any(|(i, q)| *i == ItemId::from("overclock_core") && *q == 1)
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

    let offered: Vec<ItemId> = equip_swap_rows(app.game.as_ref().unwrap(), EquipmentSlot::Weapon)
        .into_iter()
        .filter_map(|r| match r.choice {
            SwapChoice::Equip(item) => Some(item),
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

    let choices: Vec<SwapChoice> =
        equip_swap_rows(app.game.as_ref().unwrap(), EquipmentSlot::Weapon)
            .into_iter()
            .map(|r| r.choice)
            .collect();

    // Worn is +3 ATK, so the deltas run whip +1, edge -1, shiv -2.
    assert_eq!(
        choices,
        vec![
            SwapChoice::Equip(ItemId::from("monofilament_whip")),
            SwapChoice::Equip(ItemId::from("kinetic_edge")),
            SwapChoice::Equip(ItemId::from("shiv_routine")),
            SwapChoice::Unequip,
        ],
        "the upgrade should be row 1 and emptying the slot the last resort"
    );
}

/// Gear is stamped with the zone level it was equipped at and doubles per
/// level (`GEAR_LEVEL_GROWTH`), so a spare copy of what you already wear is
/// a real upgrade after a breach. The delta has to compare the worn item at
/// its *recorded* level against the candidate at the *current* zone's.
#[test]
fn a_spare_of_the_worn_item_reports_the_gain_from_re_equipping_it() {
    let app = app_wearing_weapon(
        915,
        Some(("overclock_core", 1)),
        &[("overclock_core", 1)],
        3,
    );

    let rows = equip_swap_rows(app.game.as_ref().unwrap(), EquipmentSlot::Weapon);
    let row = rows
        .iter()
        .find(|r| r.choice == SwapChoice::Equip(ItemId::from("overclock_core")))
        .expect("a spare of the worn item is still a candidate");

    // Base +3 ATK: worn remembers level 1, a fresh equip lands at zone 3
    // (3 * 2 * 2 = 12), so re-equipping is worth +9.
    assert!(
        row.label.contains("+12 ATK"),
        "the candidate should be previewed at the level it would equip at; got {:?}",
        row.label
    );
    assert!(
        row.label.contains("+9 ATK"),
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
