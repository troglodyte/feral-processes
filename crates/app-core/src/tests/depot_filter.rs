//! The Depot filter screen: `[F]` out of the transfer picker, an arrow per
//! row, and the two keys that write the whole list.

use super::support::*;
use crate::*;

const ITEM: &str = "core_fragment";

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

/// The picker beside `depots` Depots, then `[F]` into the filter screen.
fn filter_screen(seed: u32, depots: i32) -> App {
    let mut app = app_beside_depots(seed, depots, 6, &[(ITEM, 4)]);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer, "the picker opens first");
    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.mode, Mode::DepotFilter);
    app
}

fn row_of(app: &App, id: &str) -> usize {
    let screen = app.depot_filter.as_ref().expect("the screen is open");
    screen
        .view
        .rows
        .iter()
        .position(|r| r.item == item(id))
        .unwrap_or_else(|| panic!("no row for {id}"))
}

fn allowed(app: &App, id: &str) -> bool {
    let screen = app.depot_filter.as_ref().expect("the screen is open");
    screen.view.rows[row_of(app, id)].allowed
}

/// Left denies and Right allows, the arrow moving the row toward the column
/// it points at — the transfer picker's own convention, since that is the
/// screen this one is opened from.
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

/// Up to four Depots can be orthogonally adjacent, so the screen has to
/// say which one it is editing and offer a way to the next.
#[test]
fn tab_walks_to_the_next_adjacent_depot() {
    let mut app = filter_screen(3304, 2);
    let first = app.depot_filter.as_ref().unwrap().depot;
    assert_eq!(app.depot_filter.as_ref().unwrap().of, 2);

    app.handle_key(GameKey::Tab);
    let second = app.depot_filter.as_ref().unwrap().depot;
    assert_ne!(first, second, "Tab moves to the other Depot");

    app.handle_key(GameKey::Tab);
    assert_eq!(
        app.depot_filter.as_ref().unwrap().depot,
        first,
        "and wraps back round"
    );
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
    assert!(
        allowed(&app, ITEM),
        "the second shelf never heard the instruction"
    );
}

/// With one Depot beside you, Tab is a no-op rather than a way out.
#[test]
fn tab_with_one_depot_stays_put() {
    let mut app = filter_screen(3306, 1);
    let only = app.depot_filter.as_ref().unwrap().depot;

    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::DepotFilter);
    assert_eq!(app.depot_filter.as_ref().unwrap().depot, only);
}

/// Esc goes back where it came from, on a freshly taken offer: the filter
/// just edited is what `TransferRow::can_put` is derived from.
#[test]
fn esc_returns_to_the_picker_with_a_zeroed_basket() {
    let mut app = filter_screen(3307, 1);
    app.handle_key(GameKey::Esc);

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
    app.handle_key(GameKey::Esc);

    let row = app
        .basket_rows
        .iter()
        .find_map(|r| r.item().filter(|r| r.item == item(ITEM)))
        .expect("the row stays, so the pack is not hidden");
    assert_eq!(row.carried, 4);
    assert_eq!(row.can_put, 0, "and nothing may be put where it is refused");
}

/// A Mining Node has a shelf but no filter, so the key that opens this
/// screen has to say so rather than opening an empty one.
#[test]
fn the_key_is_refused_where_there_is_no_depot() {
    let mut app = app_beside_stocked_machines(3309, &[(ITEM, 6)]);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);

    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.mode, Mode::Transfer, "the picker stays open");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|l| l.contains("Depot")),
        "and the refusal says why: {:?}",
        app.status_line
    );
}

/// The case the picker's empty-offer arm exists for: a Depot built a
/// moment ago, nothing on it and nothing in the pack, is exactly the one
/// worth setting up — and `[F]` is only reachable from inside the picker.
#[test]
fn an_empty_depot_and_an_empty_pack_still_open_the_picker() {
    let mut app = app_beside_depots(3310, 1, 0, &[]);
    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Transfer);
    assert!(
        app.basket_rows.is_empty(),
        "nothing to move, and that is fine"
    );

    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.mode, Mode::DepotFilter);
}
