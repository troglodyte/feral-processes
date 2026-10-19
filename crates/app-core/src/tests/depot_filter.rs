//! The Depot filter screen: the Filter page of a Depot's hub, an arrow per
//! row, and the two keys that write the whole list.

use super::support::*;
use crate::*;

const ITEM: &str = "core_fragment";

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

/// The hub beside `depots` Depots, turned from Inventory to Filter.
fn filter_screen(seed: u32, depots: i32) -> App {
    let mut app = app_beside_depots(seed, depots, 6, &[(ITEM, 4)]);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer, "the Inventory page opens first");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::DepotFilter);
    app
}

fn row_of(app: &App, id: &str) -> usize {
    let screen = app.depot_filter.as_ref().expect("the screen is open");
    screen
        .view
        .rows
        .iter()
        .position(|r| r.items == [item(id)])
        .unwrap_or_else(|| panic!("no row for {id}"))
}

fn allowed(app: &App, id: &str) -> bool {
    let screen = app.depot_filter.as_ref().expect("the screen is open");
    screen.view.rows[row_of(app, id)].allowed
}

/// Left denies and Right allows, the arrow moving the row toward the column
/// it points at — the transfer picker's own convention, the page beside it.
#[test]
fn left_denies_the_selected_row_and_right_allows_it_again() {
    let mut app = filter_screen(3300, 1);
    app.menu_selected = row_of(&app, ITEM);
    assert!(allowed(&app, ITEM), "everything starts allowed");

    app.handle_key(GameKey::Left);
    assert!(!allowed(&app, ITEM), "Left denies");

    app.handle_key(GameKey::Right);
    assert!(allowed(&app, ITEM), "Right allows again");
}

/// Pressing the same arrow twice must not toggle back — an arrow names a
/// state, not a flip, so a held key settles rather than flickering.
#[test]
fn an_arrow_is_a_state_and_not_a_toggle() {
    let mut app = filter_screen(3301, 1);
    app.menu_selected = row_of(&app, ITEM);

    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);
    assert!(!allowed(&app, ITEM));

    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert!(allowed(&app, ITEM));
}

/// `[D]` then `[A]`, the reason the screen is usable with sixty rows.
#[test]
fn deny_all_and_allow_all_write_every_row() {
    let mut app = filter_screen(3302, 1);

    app.handle_key(GameKey::Char('D'));
    let screen = app.depot_filter.as_ref().unwrap();
    assert!(
        screen.view.rows.iter().all(|r| !r.allowed),
        "deny-all closes every row"
    );

    app.handle_key(GameKey::Char('A'));
    let screen = app.depot_filter.as_ref().unwrap();
    assert!(screen.view.rows.iter().all(|r| r.allowed));
}

/// Lowercase picks a row everywhere else in the game, so the two screen
/// actions must be deaf to it.
#[test]
fn the_lowercase_keys_do_nothing() {
    let mut app = filter_screen(3303, 1);

    app.handle_key(GameKey::Char('d'));
    let screen = app.depot_filter.as_ref().unwrap();
    assert!(screen.view.rows.iter().all(|r| r.allowed), "d is not D");
}

/// A filter is per Depot, which is the whole point of sorting: what one
/// shelf refuses must say nothing about the other.
#[test]
fn a_denial_lands_on_one_depot_alone() {
    let mut app = filter_screen(3305, 2);
    app.menu_selected = row_of(&app, ITEM);
    app.handle_key(GameKey::Left);
    assert!(!allowed(&app, ITEM));

    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::DepotFilter);
    assert!(
        allowed(&app, ITEM),
        "the second shelf never heard the instruction"
    );
}

/// Going back to Inventory re-reads the offer, so the basket is a fresh,
/// zeroed one rather than the one built against the old filter.
#[test]
fn turning_back_to_inventory_comes_up_zeroed() {
    let mut app = filter_screen(3307, 1);
    app.handle_key(GameKey::Char('['));

    assert_eq!(app.mode, Mode::Transfer);
    assert!(app.depot_filter.is_none());
    assert!(
        app.basket_amounts.iter().all(|n| *n == 0),
        "the basket comes back at zero"
    );
    assert_eq!(
        app.basket_amounts.len(),
        app.basket_rows.len(),
        "and the two lists are written together"
    );
}

/// Denying the only item the pack holds must close its put on the picker
/// behind — the two screens read one rule.
#[test]
fn a_denial_closes_the_put_on_the_picker_behind_it() {
    let mut app = filter_screen(3308, 1);
    app.menu_selected = row_of(&app, ITEM);
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Char('['));

    let row = app
        .basket_rows
        .iter()
        .find_map(|r| r.item().filter(|r| r.item == item(ITEM)))
        .expect("the row stays, so the pack is not hidden");
    assert_eq!(row.carried, 4);
    assert_eq!(row.can_put, 0, "and nothing may be put where it is refused");
}

/// An Extractor has a shelf but no filter, so its hub has no such page.
#[test]
fn a_machine_has_no_filter_page() {
    let mut app = app_beside_stocked_machines(3309, &[(ITEM, 6)]);
    app.handle_key(GameKey::Char('c'));
    for _ in 0..4 {
        assert_ne!(app.mode, Mode::DepotFilter);
        app.handle_key(GameKey::Char(']'));
    }
}

/// The case the picker's empty-offer arm exists for: a Depot built a
/// moment ago, nothing on it and nothing in the pack, is exactly the one
/// worth setting up.
#[test]
fn an_empty_depot_and_an_empty_pack_still_open_the_picker() {
    let mut app = app_beside_depots(3310, 1, 0, &[]);
    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Transfer);
    assert!(
        app.basket_rows.is_empty(),
        "nothing to move, and that is fine"
    );

    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::DepotFilter);
}
