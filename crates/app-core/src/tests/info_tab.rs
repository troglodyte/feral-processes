//! Which pane of the HUD's info column is open — see `InfoTab` and the
//! digits that pick one.

use super::support::*;
use crate::*;

#[test]
fn the_column_opens_on_base() {
    let app = test_app(180);
    assert_eq!(app.info_tab, InfoTab::Base);
}

#[test]
fn the_digits_pick_a_pane() {
    let mut app = test_app(181);
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.info_tab, InfoTab::Crew);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.info_tab, InfoTab::Pack);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(
        app.info_tab,
        InfoTab::Pack,
        "a repeat is a no-op, not a cycle"
    );
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.info_tab, InfoTab::Base);
}

/// The whole reason these arms `return` rather than falling through:
/// changing which pane you are reading is not an action.
#[test]
fn a_digit_costs_no_turn() {
    let mut app = test_app(182);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

/// **The load-bearing one.** `handle_stack_key` ends in `_ => {}`, so a key
/// the Stack path never sees is a swallowed keypress with no refusal and
/// nothing in the log — which is exactly how `r` (rest) shipped broken
/// underground. The column is drawn in both locales, so its keys have to
/// work in both.
#[test]
fn the_digits_work_underground() {
    let mut app = app_underground(183);
    assert!(app.game.as_ref().unwrap().is_underground());

    app.handle_key(GameKey::Char('3'));

    assert_eq!(app.info_tab, InfoTab::Pack);
    assert!(
        app.status_line.is_none(),
        "the key was refused rather than acted on: {:?}",
        app.status_line
    );
}

/// The row the renderer draws and the key that picks it must not disagree —
/// `LogFilter`'s `the_header_order_is_the_cycle_order`, one screen along.
#[test]
fn the_tab_order_is_the_digit_order() {
    for (i, tab) in InfoTab::ALL.iter().enumerate() {
        let mut app = test_app(184 + i as u32);
        let digit = char::from_digit(i as u32 + 1, 10).expect("a tab per digit");
        app.handle_key(GameKey::Char(digit));
        assert_eq!(app.info_tab, *tab, "{digit} did not open {}", tab.label());
    }
}
