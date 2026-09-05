//! A town's job board — Phase 5's app-core half.
//!
//! `tests/contracts.rs`'s screen one vendor over, so what is worth asserting
//! here is only what differs: the `[J]` door and its reach check, the held
//! section being **this town's jobs alone**, and every engine door being
//! called with the town's key rather than `None`.

use super::support::*;
use crate::*;

/// At a town's counter, on `Mode::SettlementBoard`.
fn at_a_board(seed: u32) -> (App, feral_processes_engine::settlements::SettlementKey) {
    let mut app = test_app(seed);
    let (key, _) = place_settlement_east_of_player(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Settlement);
    app.handle_key(GameKey::Char('J'));
    (app, key)
}

#[test]
fn j_opens_the_board_from_the_settlement_page() {
    let (app, key) = at_a_board(970);
    assert_eq!(app.mode, Mode::SettlementBoard);
    assert_eq!(app.pending_settlement, Some(key));
}

#[test]
fn esc_returns_to_the_settlement_page_rather_than_the_map() {
    let (mut app, key) = at_a_board(971);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Settlement);
    assert_eq!(app.pending_settlement, Some(key));
}

/// `[M]`'s rule: `x` opens the hub page from `EXAMINE_RANGE_TILES` away
/// while the board is Chebyshev 1, so a town read from across the map has to
/// be refused with a sentence rather than opening an empty board.
///
/// The town is placed out of reach directly, through the same save round
/// trip `place_settlement_east_of_player` uses — moving the player instead
/// would depend on the seed leaving somewhere to walk to.
#[test]
fn reading_a_board_from_across_the_map_is_refused_with_a_sentence() {
    let mut app = test_app(972);
    let key = place_settlement_far_from_player(&mut app);
    // The hub page opens from a distance; the board does not.
    app.pending_settlement = Some(key);
    app.mode = Mode::Settlement;

    app.handle_key(GameKey::Char('J'));

    assert_eq!(app.mode, Mode::Settlement);
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|line| line.contains("walk over there")),
        "{:?}",
        app.status_line
    );
}

/// A row number resolves against the two sections, `contract_row`'s own
/// test one vendor over — and taking an offer files it under this town.
#[test]
fn taking_an_offer_files_it_under_the_town_that_posted_it() {
    let (mut app, key) = at_a_board(973);
    let (held_before, offers) = app.settlement_board_sections();
    assert!(!offers.is_empty(), "the town posted nothing to take");
    let id = offers[0].id.clone();

    app.handle_key(GameKey::Char('1'));

    let (held_after, _) = app.settlement_board_sections();
    assert_eq!(held_after.len(), held_before.len() + 1);
    let taken = held_after
        .iter()
        .find(|row| row.id == id)
        .expect("not held");
    assert_eq!(taken.issuer, Some(key));
}

/// The whole reason the held section is filtered: a job signed at the Broker
/// cannot be delivered here, so a row for it would be a key that only ever
/// refuses.
#[test]
fn the_held_section_lists_this_towns_jobs_and_nothing_else() {
    let (mut app, _) = at_a_board(974);
    let (held, _) = app.settlement_board_sections();
    // A new run is already holding its first onboarding mission, which is
    // the Broker's — so this is a real filter and not a vacuous one.
    let all = app.game.as_mut().unwrap().active_contracts();
    assert!(
        all.iter().any(|row| row.issuer.is_none()),
        "the run has to be holding a non-town job, or this proves nothing"
    );
    assert!(held.iter().all(|row| row.issuer.is_some()));
}

/// `[A]` on a row that is not a held one says so rather than doing nothing.
#[test]
fn giving_back_with_nothing_highlighted_says_what_to_highlight() {
    let (mut app, _) = at_a_board(975);
    app.menu_selected = 0;
    let (held, _) = app.settlement_board_sections();
    if !held.is_empty() {
        return;
    }
    app.handle_key(GameKey::Char('A'));
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|line| line.contains("Highlight")),
        "{:?}",
        app.status_line
    );
}
