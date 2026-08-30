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
    // Past onboarding: a run still running the chain always has a mission in
    // hand, which is the case the test below covers.
    skip_tutorial(&mut app);
    assert!(
        !base_labels(&mut app).contains(&"Contracts"),
        "a row that opens an empty screen is what `group_rows` exists to stop"
    );
}

/// The other half, and new with the onboarding chain: a fresh run holds its
/// first mission before it has built anything, so the row is there from the
/// first keypress. Without it the chain is handed out somewhere the player
/// has no way to reach.
#[test]
fn the_row_is_shown_from_the_first_tick_by_the_onboarding_mission() {
    let mut app = test_app(3122);
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .active_contracts()
            .iter()
            .any(|row| row.tutorial),
        "a new run holds the chain's first mission"
    );
    assert!(base_labels(&mut app).contains(&"Contracts"));
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
        app.game.as_mut().unwrap().contract_board().is_some(),
        "the board is the sector's bulletin, and four frames down the party \
         is still in the sector"
    );
    assert!(
        base_labels(&mut app).contains(&"Contracts"),
        "mission status is the question worth answering underground"
    );
}

/// The row is what makes the screen reachable at all, so "you can check your
/// missions from anywhere" is really a claim about this list.
#[test]
fn the_row_is_listed_across_the_map_from_the_base() {
    let mut app = app_at_a_contract_broker(3120, false);
    walk_far_from_the_base(&mut app);
    assert_eq!(
        app.broker_reach(),
        BrokerReach::OffBase,
        "the fixture has to actually leave the slab"
    );
    assert!(
        base_labels(&mut app).contains(&"Contracts"),
        "the board is readable from anywhere, so the row that opens it is too"
    );
}

/// The refusal a player will actually hit: they read the board out in the
/// field and press a number. The wording has to send them home rather than
/// claim the contract does not exist.
#[test]
fn taking_an_offer_away_from_the_base_says_where_to_go() {
    let mut app = app_at_a_contract_broker(3121, false);
    open_via_menu(&mut app, 'b', "Contracts");
    let offered = app.contract_sections().1;
    assert!(!offered.is_empty(), "the fixture needs a board");

    walk_far_from_the_base(&mut app);
    assert_eq!(
        app.contract_sections().1.len(),
        offered.len(),
        "the offers are still on screen out here"
    );

    app.handle_key(GameKey::Char('1'));
    assert!(app.game.as_ref().unwrap().active_contracts().is_empty());
    let said = app.status_line.clone().expect("a refusal is reported");
    assert!(
        said.contains("base"),
        "the refusal has to name the errand, not just decline: {said:?}"
    );
    assert_eq!(app.mode, Mode::Contracts, "the screen stays open");
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

/// `[A]` on an onboarding mission refuses with a sentence, on both surfaces.
/// A silent no-op reads as the key being broken, and the engine's own
/// refusal is a bare `false` that cannot reach the log.
#[test]
fn giving_back_an_onboarding_mission_is_refused_with_a_sentence() {
    // Not `app_at_a_contract_broker`, which skips the chain: this is about a
    // run still running it, and it needs no Broker — the mission is in hand
    // from tick 0.
    let mut app = test_app(3123);
    open_via_menu(&mut app, 'b', "Contracts");
    let held = app.game.as_ref().unwrap().active_contracts();
    assert_eq!(held.len(), 1, "one onboarding mission in hand");
    assert!(held[0].tutorial);

    app.handle_key(GameKey::Char('A'));

    let line = app.status_line.clone().expect("a refusal has a sentence");
    let lowered = line.to_lowercase();
    assert!(
        lowered.contains("onboarding") || lowered.contains("finish"),
        "the sentence says why: {line:?}"
    );
    // Both surfaces: `App::refuse` writes the log too, and asserting only on
    // `status_line` would pass against a bare `self.status_line = ...`.
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .message_history(50)
            .iter()
            .any(|entry| entry.text.contains("cannot be given back")),
        "the refusal reaches the log the player scrolls back through — and on \
         a fragment the hand-out line does not share, or the ONBOARDING: \
         line would answer this on its own"
    );
    assert_eq!(
        app.game
            .as_ref()
            .unwrap()
            .active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        1,
        "and it is still in hand"
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
