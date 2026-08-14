//! The contracts screen: its base-menu row, its two stacked sections, and
//! the three verbs.

use super::support::*;
use crate::app::contracts::{ContractScreenRow, contract_row};
use crate::*;

fn base_labels(app: &mut App) -> Vec<&'static str> {
    app.base_menu_rows().iter().map(|r| r.label).collect()
}

#[test]
fn the_row_is_hidden_with_no_broker_and_nothing_in_hand() {
    let mut app = test_app(3101);
    assert!(
        !base_labels(&mut app).contains(&"Contracts"),
        "a row that opens an empty screen is what `group_rows` exists to stop"
    );
}

#[test]
fn a_broker_in_range_shows_the_row() {
    let mut app = app_at_a_contract_broker(3102, false);
    assert!(base_labels(&mut app).contains(&"Contracts"));
}

#[test]
fn a_contract_in_hand_shows_the_row_underground() {
    let mut app = app_at_a_contract_broker(3103, false);
    open_via_menu(&mut app, 'b', "Contracts");
    app.handle_key(GameKey::Char('1'));
    assert!(
        !app.game.as_ref().unwrap().active_contracts().is_empty(),
        "the fixture needs a contract in hand: {:?}",
        app.status_line
    );

    let mut app = app_at_a_contract_broker(3104, true);
    assert!(
        app.game.as_mut().unwrap().contract_board().is_none(),
        "no board four frames down"
    );
    assert!(
        !base_labels(&mut app).contains(&"Contracts"),
        "underground with nothing in hand there is still nothing to show"
    );
}

#[test]
fn the_screen_opens_from_the_base_menu_and_esc_walks_back() {
    let mut app = app_at_a_contract_broker(3105, false);
    open_via_menu(&mut app, 'b', "Contracts");
    assert_eq!(app.mode, Mode::Contracts);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn a_row_number_resolves_against_the_right_section() {
    assert_eq!(contract_row(0, 2, 3), Some(ContractScreenRow::Active(0)));
    assert_eq!(contract_row(1, 2, 3), Some(ContractScreenRow::Active(1)));
    assert_eq!(contract_row(2, 2, 3), Some(ContractScreenRow::Offer(0)));
    assert_eq!(contract_row(4, 2, 3), Some(ContractScreenRow::Offer(2)));
    assert_eq!(contract_row(5, 2, 3), None, "past the end");
    assert_eq!(contract_row(0, 0, 1), Some(ContractScreenRow::Offer(0)));
    assert_eq!(contract_row(0, 0, 0), None);
}

#[test]
fn picking_an_offer_accepts_it_and_the_screen_stays_open() {
    let mut app = app_at_a_contract_broker(3106, false);
    open_via_menu(&mut app, 'b', "Contracts");
    let offered = app.game.as_mut().unwrap().contract_board().unwrap();
    assert!(!offered.is_empty());

    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::Contracts,
        "the screen stays open so several can be taken in one visit"
    );
    let held = app.game.as_ref().unwrap().active_contracts();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].id, offered[0].id);
    assert!(
        !app.game
            .as_mut()
            .unwrap()
            .contract_board()
            .unwrap()
            .iter()
            .any(|row| row.id == held[0].id),
        "an accepted contract drops off the offers, so the row it vacated \
         cannot be picked twice"
    );
}

#[test]
fn abandoning_the_highlighted_contract_drops_it() {
    let mut app = app_at_a_contract_broker(3107, false);
    open_via_menu(&mut app, 'b', "Contracts");
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.game.as_ref().unwrap().active_contracts().len(), 1);

    // The active section leads the list, so row 0 is the contract just taken.
    app.handle_key(GameKey::Char('A'));
    assert!(
        app.game.as_ref().unwrap().active_contracts().is_empty(),
        "status: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Contracts);
}

#[test]
fn a_refusal_reports_why_and_leaves_the_screen_open() {
    let mut app = app_at_a_contract_broker(3108, false);
    open_via_menu(&mut app, 'b', "Contracts");
    // Take everything the board offers, then try for one more.
    for _ in 0..MAX_ACTIVE_CONTRACTS + 1 {
        let offers = app.game.as_mut().unwrap().contract_board().unwrap().len();
        if offers == 0 {
            break;
        }
        let held = app.game.as_ref().unwrap().active_contracts().len();
        app.handle_key(GameKey::Char(
            char::from_digit(held as u32 + 1, 10).unwrap(),
        ));
    }
    assert_eq!(
        app.game.as_ref().unwrap().active_contracts().len(),
        MAX_ACTIVE_CONTRACTS.min(3),
        "status: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Contracts);
}
