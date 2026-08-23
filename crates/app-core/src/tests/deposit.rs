//! The deposit picker — `Mode::Deposit`, and the shared budget that is the
//! one thing it does not inherit from the collect screen.

use super::support::*;
use crate::*;

const ITEM: &str = "core_fragment";
const OTHER: &str = "cache_grain";

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

/// `P` beside a Depot opens the window rather than dumping the pack, and
/// every row starts at zero.
#[test]
fn p_opens_the_deposit_window_with_an_empty_basket() {
    let mut app = app_beside_depots(4101, 1, 0, &[(ITEM, 6)]);

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.mode, Mode::Deposit);
    assert_eq!(app.basket_rows, vec![(item(ITEM), 6)]);
    assert_eq!(app.basket_amounts, vec![0]);
}

/// Opening a screen is not an action: no tick, and no wiping of whatever the
/// last refusal was still explaining.
#[test]
fn opening_the_deposit_window_spends_no_turn() {
    let mut app = app_beside_depots(4102, 1, 0, &[(ITEM, 6)]);
    let before = app.game.as_ref().unwrap().current_tick();
    app.status_line = Some("standing".to_string());

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(app.status_line.as_deref(), Some("standing"));
}

/// **The shared budget.** A Depot with less room than the pack holds means
/// the rows spend one ceiling between them: filling the first lowers what the
/// second may reach, and nothing the player can press gets past the total.
///
/// Driven through the real keys on purpose. This is a key-handling invariant
/// — `basket_available` is only ever reached through them — so pressing them
/// is the honest test.
#[test]
fn filling_one_row_lowers_what_the_next_may_reach() {
    // 200 capacity, 195 already in it: five units of room against a pack of
    // twenty.
    let mut app = app_beside_depots(4103, 1, 195, &[(ITEM, 10), (OTHER, 10)]);
    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.basket_rows.len(), 2, "both rows are on offer");

    // The first row can reach the whole budget but no further, though it
    // holds ten.
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[0], 5, "the budget, not the ten held");

    // And now the second row has nothing left to reach.
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[1], 0, "the budget is already spent");

    // Give three back and the second row can take exactly those three.
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.basket_amounts[0], 2);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[1], 3, "exactly what was freed");
    assert_eq!(app.basket_amounts.iter().sum::<u32>(), 5, "never past it");
}

/// A row being edited keeps its own units. Counting the highlighted row
/// against its own budget would make every key a no-op the moment the basket
/// filled — it could be lowered but never raised again.
#[test]
fn the_highlighted_row_does_not_spend_its_own_budget() {
    let mut app = app_beside_depots(4104, 1, 195, &[(ITEM, 10)]);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[0], 5);
    // Down to two and back up to the ceiling: the second half is what fails
    // if the row is charged for what it is already holding.
    app.handle_key(GameKey::CtrlRight);
    app.handle_key(GameKey::CtrlRight);
    assert!(app.basket_amounts[0] < 5, "came down");
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[0], 5, "and can go back up");
}

/// `[A]` fills against the shared budget rather than handing every row its
/// own full quantity. Zipping straight across the rows would blow past it.
#[test]
fn take_all_stops_at_the_shared_budget() {
    let mut app = app_beside_depots(4105, 1, 195, &[(ITEM, 10), (OTHER, 10)]);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::Char('A'));

    assert_eq!(
        app.basket_amounts.iter().sum::<u32>(),
        5,
        "the Depot's room, not the pack's twenty"
    );
}

/// Left adds and Right removes — inverted against every other Left/Right in
/// the game, and the inversion is the specification. Shared with the collect
/// screen through `app/basket.rs`, and pinned here too because a "fix" to one
/// screen's table would now silently move both.
#[test]
fn left_adds_and_right_removes_on_the_deposit_screen_too() {
    let mut app = app_beside_depots(4106, 1, 0, &[(ITEM, 4)]);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);
    assert_eq!(app.basket_amounts[0], 2);
    app.handle_key(GameKey::Right);
    assert_eq!(app.basket_amounts[0], 1);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.basket_amounts[0], 0, "nothing below nothing");
}

/// The modifiers reach this screen. Shift is a target and is idempotent under
/// key repeat; Ctrl is a step that halves the gap and closes the last unit
/// rather than stranding it.
#[test]
fn the_modifiers_reach_the_deposit_screen() {
    let mut app = app_beside_depots(4107, 1, 0, &[(ITEM, 8)]);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(app.basket_amounts[0], 4, "half of eight");
    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(app.basket_amounts[0], 6);

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[0], 8);
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[0], 8, "idempotent under key repeat");

    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.basket_amounts[0], 0);
}

/// Digits set a quantity and clamp at the budget rather than overflowing.
#[test]
fn digits_clamp_to_the_budget() {
    let mut app = app_beside_depots(4108, 1, 195, &[(ITEM, 10)]);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::Char('9'));
    assert_eq!(app.basket_amounts[0], 5, "the room, not the nine typed");
}

/// Enter sends exactly the non-zero rows, and the goods end up where the base
/// can see them.
#[test]
fn enter_puts_exactly_the_basket_into_the_depot() {
    let mut app = app_beside_depots(4109, 1, 0, &[(ITEM, 9)]);
    app.handle_key(GameKey::Char('P'));
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);

    app.handle_key(GameKey::Enter);

    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.collectable_adjacent(),
        vec![(item(ITEM), 3)],
        "three units, now the base's"
    );
    assert_eq!(app.mode, Mode::Playing);
}

/// Both exits clear all three fields, so a reopened screen shows no stale
/// pack and no stale ceiling.
#[test]
fn both_exits_leave_no_stale_pack() {
    for exit in [GameKey::Esc, GameKey::Enter] {
        let mut app = app_beside_depots(4110, 1, 0, &[(ITEM, 5)]);
        app.handle_key(GameKey::Char('P'));
        assert!(!app.basket_rows.is_empty());

        app.handle_key(exit);

        assert!(app.basket_rows.is_empty(), "{exit:?} left rows");
        assert!(app.basket_amounts.is_empty(), "{exit:?} left amounts");
        assert_eq!(app.basket_room, None, "{exit:?} left a ceiling");
    }
}

/// An all-zero basket never reaches the engine: no tick, and nothing moved.
#[test]
fn an_all_zero_basket_spends_no_turn() {
    let mut app = app_beside_depots(4111, 1, 0, &[(ITEM, 5)]);
    app.handle_key(GameKey::Char('P'));
    let before = app.game.as_ref().unwrap().current_tick();

    app.handle_key(GameKey::Enter);

    let game = app.game.as_ref().unwrap();
    assert_eq!(game.current_tick(), before);
    assert!(game.collectable_adjacent().is_empty(), "nothing moved");
}

/// `P` with nothing to put away routes straight back through the engine, so
/// the engine speaks its own refusal rather than app-core keeping a copy of
/// the sentence.
///
/// The claim is about the **log**, not `status_line`. A `status_line` set in
/// that branch would be wiped by `after_world_action` before anything could
/// read it — the branch reports having acted — so asserting it was `None`
/// passed against a deliberate copy of the sentence and proved nothing.
#[test]
fn p_with_an_empty_pack_lets_the_engine_speak() {
    let mut app = app_beside_depots(4112, 1, 0, &[]);

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.mode, Mode::Playing, "no window opened");
    let log = app.game.as_ref().unwrap().message_history(64);
    assert!(
        log.iter()
            .any(|line| line.text.contains("nothing to put away")),
        "the engine's own refusal is what the player sees: {log:?}"
    );
}
