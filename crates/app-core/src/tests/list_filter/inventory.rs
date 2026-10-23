//! Type-to-filter on the Items page and the equip swap picker. Each test
//! moves the highlight off the first match and asserts the source row the
//! action used, since a handler indexing the unfiltered pack would still
//! pass on the first match.

use super::super::support::*;
use crate::*;
use feral_processes_engine::items::ids;

const PACK: &[(&str, u32)] = &[
    (ids::CORE_FRAGMENT, 5),
    (ids::POWER_CELL, 2),
    (ids::OVERCLOCK_CORE, 1),
    (ids::ICE_BREAKER, 1),
    (ids::FIREWALL_PLATING, 1),
    (ids::ABLATIVE_PLATING, 1),
    ("scrap_ward", 1),
];

/// A pack with a trader in range and the Items page open.
fn pack_app(seed: u32) -> App {
    let mut app = app_at_a_trading_post(seed, PACK);
    open_hub_screen(&mut app, Mode::Inventory);
    app
}

/// `pack_app` with the scrap ward worn, so the Armor picker offers the two
/// plates and an Unequip row.
fn armored_app(seed: u32) -> App {
    let mut app = pack_app(seed);
    let ward = pack(&app)
        .into_iter()
        .find(|r| r.0.item.as_str() == "scrap_ward")
        .unwrap()
        .0;
    let game = app.game.as_mut().unwrap();
    let player = game.player_entity();
    game.equip(player, &ward).unwrap();
    app
}

fn pack(app: &App) -> Vec<(GearCopy, u32, String)> {
    let game = app.game.as_ref().unwrap();
    game.player_status()
        .inventory
        .into_iter()
        .map(|r| {
            let name = game.copy_name(&r.copy);
            (r.copy, r.qty, name)
        })
        .collect()
}

/// Pack indices whose name contains `filter`.
fn matching(app: &App, filter: &str) -> Vec<usize> {
    pack(app)
        .iter()
        .enumerate()
        .filter(|(_, (_, _, n))| n.to_lowercase().contains(filter))
        .map(|(i, _)| i)
        .collect()
}

const FILTER: &str = "core";

/// Filters the pack and puts the highlight on the second match, returning
/// that match's pack index.
fn on_second_match(app: &mut App) -> usize {
    type_text(app, FILTER);
    let want = matching(app, FILTER);
    assert!(want.len() >= 2 && want.len() < pack(app).len());
    assert_eq!(app.menu_selected, 3, "typing lands on the first match");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 4);
    want[1]
}

#[test]
fn typing_narrows_the_pack_and_leaves_the_slot_rows() {
    let mut app = pack_app(1101);
    type_text(&mut app, FILTER);
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        inventory_visible(game, app.filter_text()),
        matching(&app, FILTER)
    );
    app.handle_key(GameKey::Backspace);
    app.handle_key(GameKey::Backspace);
    app.handle_key(GameKey::Backspace);
    app.handle_key(GameKey::Backspace);
    let game = app.game.as_ref().unwrap();
    assert_eq!(inventory_visible(game, "").len(), pack(&app).len());
    assert_eq!(app.menu_selected, 3);
}

#[test]
fn enter_opens_the_filtered_row_and_esc_comes_back_to_it() {
    let mut app = pack_app(1102);
    let src = on_second_match(&mut app);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::InventoryItemAction);
    assert_eq!(app.pending_inventory_item, Some(pack(&app)[src].0.clone()));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.filter_text(), FILTER);
    assert_eq!(app.menu_selected, 4);
}

#[test]
fn inspect_acts_on_the_filtered_row_and_returns_with_the_filter() {
    let mut app = pack_app(1103);
    let src = on_second_match(&mut app);
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inspect.as_ref().unwrap().copy,
        pack(&app)[src].0
    );
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.filter_text(), FILTER);
    assert_eq!(app.menu_selected, 4);
}

#[test]
fn quick_sell_sells_the_filtered_row() {
    let mut app = pack_app(1104);
    let src = on_second_match(&mut app);
    let before = pack(&app);
    app.handle_key(GameKey::Char('S'));
    let after = pack(&app);
    let sold = &before[src];
    let now = after.iter().find(|r| r.0 == sold.0).map_or(0, |r| r.1);
    assert_eq!(now, sold.1 - 1, "{} lost one", sold.2);
    for (i, row) in before.iter().enumerate().filter(|(i, _)| *i != src) {
        let kept = after.iter().find(|r| r.0 == row.0).map_or(0, |r| r.1);
        assert_eq!(kept, row.1, "row {i} ({}) untouched", row.2);
    }
    assert_eq!(app.filter_text(), FILTER);
}

#[test]
fn capital_letters_never_reach_the_filter() {
    let mut app = pack_app(1105);
    type_text(&mut app, "co");
    app.handle_key(GameKey::Char('U'));
    assert_eq!(app.filter_text(), "co");
    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn esc_clears_the_filter_then_goes_to_the_rail() {
    let mut app = pack_app(1106);
    type_text(&mut app, FILTER);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.filter_text(), "");
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
}

#[test]
fn typing_on_the_rail_does_not_filter() {
    let mut app = pack_app(1107);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    type_text(&mut app, FILTER);
    assert_eq!(app.filter_text(), "");
}

fn swap_rows(app: &App, slot: EquipmentSlot) -> Vec<SwapRow> {
    let game = app.game.as_ref().unwrap();
    equip_swap_rows(game, game.player_entity(), slot)
}

/// Opens the Armor swap picker from the Items page, filtered to the name of
/// the *last* armor row so the filtered position and the source row differ.
fn armor_picker_filtered_to_last(app: &mut App) -> (String, SwapChoice) {
    let rows = swap_rows(app, EquipmentSlot::Armor);
    assert!(rows.len() >= 3, "two plates and Unequip");
    let last = rows
        .iter()
        .rposition(|r| matches!(r.choice, SwapChoice::Equip(_)))
        .unwrap();
    let SwapChoice::Equip(copy) = &rows[last].choice else {
        unreachable!()
    };
    let name = app.game.as_ref().unwrap().copy_name(copy).to_lowercase();
    highlight_row(app, 1);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    type_text(app, &name);
    (name, rows[last].choice.clone())
}

#[test]
fn enter_in_the_picker_equips_the_filtered_row_not_the_first() {
    let mut app = armored_app(1108);
    let (_, want) = armor_picker_filtered_to_last(&mut app);
    let SwapChoice::Equip(copy) = want else {
        unreachable!()
    };
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Inventory);
    let worn = app
        .game
        .as_ref()
        .unwrap()
        .worn(
            app.game.as_ref().unwrap().player_entity(),
            EquipmentSlot::Armor,
        )
        .unwrap();
    assert_eq!(worn.copy.item, copy.item);
}

#[test]
fn the_unequip_row_stays_pinned_under_a_filter() {
    let mut app = armored_app(1109);
    armor_picker_filtered_to_last(&mut app);
    let rows = swap_rows(&app, EquipmentSlot::Armor);
    let visible = equip_swap_visible(&rows, app.filter_text());
    let unequip = rows
        .iter()
        .position(|r| matches!(r.choice, SwapChoice::Unequip))
        .unwrap();
    assert!(visible.contains(&unequip));
    assert_eq!(visible.len(), 2, "the match and Unequip");
}

#[test]
fn inspect_in_the_picker_reads_the_filtered_row() {
    let mut app = armored_app(1110);
    let (_, want) = armor_picker_filtered_to_last(&mut app);
    let SwapChoice::Equip(copy) = want else {
        unreachable!()
    };
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(app.pending_inspect.as_ref().unwrap().copy, copy);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap, "back to the picker");
    assert!(app.filter_applied(), "the picker's own filter survived");
}

#[test]
fn the_pack_filter_survives_a_filtered_picker() {
    let mut app = pack_app(1111);
    type_text(&mut app, "plating");
    assert!(matching(&app, "plating").len() >= 2);
    highlight_row(&mut app, 1);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    assert_eq!(app.filter_text(), "", "the picker starts unfiltered");
    type_text(&mut app, "ablative");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.filter_text(), "");
    assert_eq!(app.mode, Mode::EquipSwap, "Esc cleared first");
    type_text(&mut app, "ablative");
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.filter_text(), "plating");
    assert_eq!(app.menu_selected, 1, "the slot row it was opened from");
}
