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
