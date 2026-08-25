//! The `[I]` inspect page, and the seven screens it opens from.
//!
//! One page rather than a second describe screen, so the mechanics a
//! routine carries are stated in exactly one place — see
//! `Game::gear_detail`. What has to be held here is the other half: the
//! page is reachable from every list that names a piece of gear, and each
//! of them has to get its own highlight back.

use feral_processes_engine::items::ids;

use super::support::*;
use crate::*;

/// The swap picker is the screen the feature was asked for: it lists
/// candidate weapons and, before this, said nothing at all about the
/// routine one of them grants.
#[test]
fn inspect_opens_from_the_swap_picker_and_esc_returns_to_it() {
    let mut app = app_wearing_weapon(940, None, &[("crash_handler", 1)], 1);
    app.mode = Mode::Inventory;
    app.menu_selected = 0;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap, "row 1 is the weapon slot");

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    let inspect = app.pending_inspect.clone().expect("the page has a subject");
    assert_eq!(inspect.copy, gear(&ItemId::from("crash_handler"), 0));
    assert_eq!(inspect.from, Mode::EquipSwap);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::EquipSwap,
        "Esc goes back to the picker it was opened from, not out to cargo"
    );
    assert_eq!(app.menu_selected, 0, "and lands on the row it left");
}

/// Straight off the cargo list, without the item-action page in between —
/// the list already prints `Grants: …` under a row, so the question is
/// asked there.
#[test]
fn inspect_opens_straight_from_the_cargo_list() {
    let mut app = app_wearing_weapon(941, None, &[("crash_handler", 1)], 1);
    app.mode = Mode::Inventory;
    // The three equipment slots lead, so the first cargo row is 4.
    app.menu_selected = 3;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inspect.as_ref().map(|i| i.copy.item.clone()),
        Some(ItemId::from("crash_handler"))
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
}

/// An equipment slot row names what is *worn*, which is the copy the
/// player is asking about when the highlight is on it.
#[test]
fn inspect_on_a_slot_row_reads_what_is_worn() {
    let mut app = app_wearing_weapon(942, Some(("crash_handler", 1)), &[], 1);
    app.mode = Mode::Inventory;
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inspect.as_ref().map(|i| i.copy.item.clone()),
        Some(ItemId::from("crash_handler")),
        "the slot row is about the weapon in it"
    );
}

/// An empty slot has nothing to inspect, and saying so beats opening a
/// page about no item.
#[test]
fn inspect_on_an_empty_slot_says_so() {
    let mut app = app_wearing_weapon(943, None, &[], 1);
    app.mode = Mode::Inventory;
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::Inventory, "no page for an empty slot");
    assert!(
        app.status_line.is_some(),
        "and a silent no-op reads as broken"
    );
}

/// A program's gear is measured for the *program*: the accuracy the page
/// quotes and the level every granted magnitude is scaled at are the
/// wearer's, and the wearer here is not the player.
#[test]
fn inspect_from_a_programs_slots_measures_it_for_that_program() {
    let mut app = app_with_companions_and_cargo(944, 1, &[("crash_handler", 1)]);
    let program = app.game.as_mut().unwrap().owned_pets()[0].entity;
    app.pending_equip_program = Some(program);
    app.mode = Mode::CompanionEquip;
    app.menu_selected = 0;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);

    app.handle_key(GameKey::Char('I'));

    let inspect = app.pending_inspect.clone().expect("the page has a subject");
    assert_eq!(
        inspect.wearer,
        Some(program),
        "the picker was opened for a program, so the page is about that program"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::EquipSwap);
}

/// The row that empties a slot names no item, so there is nothing to open
/// a page about.
#[test]
fn the_unequip_row_has_nothing_to_inspect() {
    let mut app = app_wearing_weapon(945, Some(("crash_handler", 1)), &[], 1);
    app.mode = Mode::Inventory;
    app.menu_selected = 0;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    // Nothing in cargo fits, so the only row is the unequip one.
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::EquipSwap, "the picker stays open");
    assert!(app.pending_inspect.is_none());
}

/// A trader's shelves name gear the player does not own yet, which is
/// exactly when "what does this grant" is worth asking.
#[test]
fn inspect_opens_from_a_traders_shelf() {
    let mut app = app_at_a_trading_post(946, &[("crash_handler", 1)]);
    app.mode = Mode::Trade;
    app.menu_selected = 0;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TradeAction);
    // The player's own cargo is sold first, so row 1 is the Crash Handler.
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inspect.as_ref().map(|i| i.copy.item.clone()),
        Some(ItemId::from("crash_handler"))
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::TradeAction);
}

/// `[d]` from the item-action page lands on the same page, so there is one
/// screen and not two. Its Esc still steps back to the actions.
#[test]
fn describe_and_inspect_are_the_same_page() {
    let mut app = test_app(947);
    app.pending_inventory_item = Some(gear(&ItemId::from(ids::CORE_FRAGMENT), 0));
    app.mode = Mode::InventoryItemAction;

    app.handle_key(GameKey::Char('d'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    let inspect = app.pending_inspect.clone().expect("the page has a subject");
    assert_eq!(inspect.copy.item, ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(inspect.from, Mode::InventoryItemAction);
}

/// A program's slot page answers `[I]` about the program, not the player —
/// the same three rows the inventory leads with, read for a different body.
#[test]
fn inspect_on_a_programs_slot_row_reads_what_that_program_wears() {
    let mut app = app_with_companions_and_cargo(948, 1, &[("crash_handler", 1)]);
    let program = app.game.as_mut().unwrap().owned_pets()[0].entity;
    app.pending_equip_program = Some(program);
    app.mode = Mode::CompanionEquip;
    app.menu_selected = 0;
    // Put the weapon on the program, so the slot row has something in it.
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::CompanionEquip);

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::ItemDescribe);
    let inspect = app.pending_inspect.clone().expect("the page has a subject");
    assert_eq!(inspect.copy.item, ItemId::from("crash_handler"));
    assert_eq!(inspect.wearer, Some(program), "priced for the program");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::CompanionEquip);
}

/// The rating rides the **view**, not a second `Game::copy_power` call in
/// the renderer. `GearDetailView`'s promise is that the page is one call,
/// and a renderer reaching past it for one figure is how each of the four
/// hand-rolled `copy_bonus` chains started.
///
/// And it is the **absolute** figure, not one measured for the wearer: the
/// same copy inspected from a program's slots rates exactly what it rates
/// from the player's cargo, which is what makes one number mean one thing on
/// every screen.
#[test]
fn the_inspect_view_carries_the_copys_absolute_rating() {
    let mut app = app_with_companions_and_cargo(945, 1, &[("shim_blade", 1)]);
    let game = app.game.as_mut().unwrap();
    let copy = feral_processes_engine::items::GearCopy::plain("shim_blade".into());
    let program = game.owned_pets()[0].entity;

    let rated = game.copy_power(&copy).expect("a weapon is rated");
    for wearer in [game.player_entity(), program] {
        let worn = game
            .gear_detail(&copy, wearer)
            .worn
            .expect("a weapon has a slot");
        assert_eq!(worn.power, Some(rated));
    }
}
