//! The transfer picker: one screen, one signed amount per row, both
//! directions in one commit.

use super::support::*;
use crate::*;

/// The item the shelf fixtures stock. Anything with a name would do; naming
/// it once keeps the tests reading as being about quantities.
const ITEM: &str = "core_fragment";
/// The two items the multi-row fixtures stock, in `ItemId` order — which is
/// the order the rows come back in, so the tests can index them.
const ITEMS: [(&str, u32); 2] = [(ITEM, 6), ("power_cell", 2)];

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

fn amounts(app: &App) -> Vec<i64> {
    app.basket_amounts.clone()
}

/// Opens the picker on two mining nodes — a shelf with no Depot beside it,
/// so every row is a take and the put budget is closed. Both sides hold the
/// same stock and the picker pools them, so the rows read 12 and 4.
fn picker(seed: u32) -> App {
    let mut app = app_beside_stocked_machines(seed, &ITEMS);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
    app
}

/// The picker over a single row holding exactly `shelf` and nothing in the
/// pack. Stating the number rather than deriving it is the point: these
/// tests are arithmetic, and a shelf that moved with a content edit would
/// make them assert something else.
fn picker_with_shelf(seed: u32, shelf: u32) -> App {
    let mut app = picker(seed);
    app.basket_rows = vec![TransferRow {
        item: item(ITEM),
        on_shelves: shelf,
        carried: 0,
        can_put: 0,
    }];
    app.basket_amounts = vec![0];
    app.menu_selected = 0;
    app
}

/// The picker beside one Depot holding `filled` of `ITEM`, with `pack` in
/// the player's hands. A Depot's `capacity` is 200, so `filled` is what
/// decides the room.
fn depot_picker(seed: u32, filled: u32, pack: &[(&str, u32)]) -> App {
    let mut app = app_beside_depots(seed, 1, filled, pack);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
    app
}

fn row_of(app: &App, id: &str) -> usize {
    app.basket_rows
        .iter()
        .position(|r| r.item == item(id))
        .unwrap_or_else(|| panic!("no row for {id}"))
}

/// `c` beside a stocked machine opens the window rather than emptying the
/// shelves. Every row starts at zero — nothing is selected by default.
#[test]
fn c_opens_the_transfer_window_with_an_empty_basket() {
    let mut app = app_beside_stocked_machines(970, &[(ITEM, 6)]);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Transfer);
    assert_eq!(app.basket_rows.len(), 1);
    assert_eq!(app.basket_rows[0].on_shelves, 12, "both machines pooled");
    assert_eq!(amounts(&app), vec![0]);
}

/// Opening a screen is not an action, and both halves of that are here.
///
/// The tick half is about the engine: nothing moves, so nothing is charged.
/// The `status_line` half is about `acted`, which is what
/// `after_world_action` reads — returning `true` from a keypress that only
/// opened a window wipes whatever the last refusal was still explaining.
#[test]
fn opening_the_transfer_window_spends_no_turn() {
    let mut app = app_beside_stocked_machines(971, &[(ITEM, 6)]);
    let before = app.game.as_ref().unwrap().current_tick();
    app.status_line = Some("still explaining something".to_string());

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(
        app.status_line.as_deref(),
        Some("still explaining something")
    );
}

/// The refusal stays the engine's: with nothing on either side `c` never
/// opens the window, so there is no empty screen to back out of and no
/// second copy of the sentence in app-core.
#[test]
fn c_with_nothing_on_either_side_opens_no_window() {
    let mut app = app_beside_stocked_machines(972, &[(ITEM, 0)]);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.basket_rows.is_empty());
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .message_history(usize::MAX)
            .iter()
            .any(|l| l.text.contains("nothing")),
        "the engine said why"
    );
}

/// **Left takes out and Right puts in**, which is the arrow moving stock in
/// the direction of the column it heads for: the screen is a table reading
/// `change | you | container`, so Left pulls off the container toward you and
/// Right pushes from you into it. This test is the pin.
#[test]
fn left_takes_out_and_right_puts_in() {
    let mut app = depot_picker(973, 100, &[(ITEM, 5)]);
    let row = row_of(&app, ITEM);
    app.menu_selected = row;

    app.handle_key(GameKey::Left);
    assert_eq!(app.basket_amounts[row], 1, "Left takes off the shelf");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.basket_amounts[row], -1,
        "Right walks back through zero and puts into the Depot"
    );
}

/// Both arrows saturate at their own end rather than running past it.
#[test]
fn the_arrows_saturate_at_both_ends() {
    let mut app = picker_with_shelf(974, 2);

    for _ in 0..4 {
        app.handle_key(GameKey::Left);
    }
    assert_eq!(amounts(&app), vec![2], "nothing more is on the shelf");
    for _ in 0..6 {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(amounts(&app), vec![0], "and nothing in the pack to put in");
}

/// Shift is a *target* and so idempotent under key repeat; Ctrl is a *step*
/// that halves the gap and **terminates** — pinned on a gap of one at both
/// ends, which is where a step rounded down goes dead with the row neither
/// full nor empty.
#[test]
fn shift_is_a_target_and_ctrl_is_a_step_that_terminates() {
    let mut app = depot_picker(975, 199, &[("power_cell", 8)]);
    let row = row_of(&app, "power_cell");
    app.menu_selected = row;

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[row], 0, "nothing of it on the shelf");
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.basket_amounts[row], -1, "the Depot has room for one");
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.basket_amounts[row], -1, "and a target is idempotent");

    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(
        app.basket_amounts[row], 0,
        "a gap of one closes rather than stranding the last unit"
    );

    let mut app = picker_with_shelf(976, 8);
    // On 8 the ceiling and the floor agree on every step but the final one,
    // so a step rounded down sticks at 7 forever.
    for expected in [4, 6, 7, 8, 8] {
        app.handle_key(GameKey::CtrlLeft);
        assert_eq!(amounts(&app), vec![expected]);
    }
    for expected in [4, 2, 1, 0, 0] {
        app.handle_key(GameKey::CtrlRight);
        assert_eq!(amounts(&app), vec![expected]);
    }
}

/// The put budget is one shared room. Filling one row lowers what the next
/// may reach, while the highlighted row keeps its own amount as it is edited
/// — counting itself would make every key a no-op the moment the basket
/// reached the budget.
#[test]
fn the_put_budget_is_shared_but_never_counts_the_row_being_edited() {
    let mut app = depot_picker(977, 197, &[(ITEM, 5), ("power_cell", 5)]);
    let first = row_of(&app, ITEM);
    let second = row_of(&app, "power_cell");

    assert_eq!(app.put_available(first), 3, "the Depot's whole room");
    app.menu_selected = first;
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.basket_amounts[first], -3);

    assert_eq!(app.put_available(second), 0, "the room is spent");
    assert_eq!(
        app.put_available(first),
        3,
        "the row being edited does not spend its own budget"
    );
    app.handle_key(GameKey::Left);
    assert_eq!(app.basket_amounts[first], -2, "so it can be lowered again");
}

/// **The reported case.** A Depot at exactly `capacity` leaves every put at
/// zero while the takes are untouched — and a take already set on another
/// row deliberately does not credit the budget, since a take may come off a
/// machine that is not a Depot.
#[test]
fn a_full_depot_closes_every_put_and_touches_no_take() {
    let mut app = depot_picker(978, 200, &[("power_cell", 5)]);
    let shelf = row_of(&app, ITEM);
    let pack = row_of(&app, "power_cell");

    assert_eq!(app.take_available(shelf), 200, "the shelf is untouched");
    assert_eq!(app.put_available(pack), 0);

    app.menu_selected = shelf;
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.basket_amounts[shelf], 200, "take everything on it");
    assert_eq!(
        app.put_available(pack),
        0,
        "a pending take does not credit the Depot's room"
    );
}

/// The two states that must not collapse: no Depot beside you at all, and a
/// Depot with nothing left.
#[test]
fn basket_room_tells_no_depot_apart_from_a_full_one() {
    let app = picker(979);
    assert_eq!(app.basket_room, None, "a Mining Node has no room");

    let app = depot_picker(980, 200, &[(ITEM, 1)]);
    assert_eq!(app.basket_room, Some(0), "a Depot with nothing left");
}

/// A digit is a quantity here, not a row pick — the whole reason this screen
/// cannot use `selected_index`. It lands on the highlighted row alone,
/// accumulates in that row's current sign, and clamps as it is typed.
#[test]
fn digits_type_into_the_rows_current_sign() {
    let mut app = depot_picker(981, 100, &[(ITEM, 40)]);
    let row = row_of(&app, ITEM);
    app.menu_selected = row;

    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.basket_amounts[row], 3, "a row at zero types a take");

    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.basket_amounts[row], -1);
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.basket_amounts[row], -12, "and keeps the sign it is in");

    app.handle_key(GameKey::Backspace);
    assert_eq!(app.basket_amounts[row], -1, "Backspace keeps it too");

    app.handle_key(GameKey::Char('9'));
    app.handle_key(GameKey::Char('9'));
    assert_eq!(
        app.basket_amounts[row], -40,
        "clamped as it is typed, to what the pack holds"
    );
}

/// A player holding a digit key must not overflow on the way to the clamp.
///
/// The row is written by hand at `u32::MAX`, because on any ordinary shelf
/// the clamp lands after every keypress and the amount never climbs high
/// enough — a test pressing `9` twelve times against a real buffer passes
/// with the arithmetic written plainly, which is no coverage at all.
#[test]
fn holding_a_digit_key_cannot_overflow() {
    let mut app = picker_with_shelf(982, u32::MAX);

    for _ in 0..24 {
        app.handle_key(GameKey::Char('9'));
    }

    assert_eq!(amounts(&app), vec![u32::MAX as i64]);
}

/// `[A]` writes the take ceiling over **every** row, clearing a give the
/// player had set on a row with nothing on the shelf. That is what "take
/// everything" means on one axis; it is a decision, not an oversight.
/// `[N]` puts them all back to nothing.
#[test]
fn take_everything_overwrites_a_pending_give() {
    let mut app = depot_picker(983, 100, &[("power_cell", 5)]);
    let pack = row_of(&app, "power_cell");
    let shelf = row_of(&app, ITEM);
    app.menu_selected = pack;
    app.handle_key(GameKey::ShiftRight);
    assert!(app.basket_amounts[pack] < 0, "a give is pending");

    app.handle_key(GameKey::Char('A'));
    assert_eq!(app.basket_amounts[pack], 0, "nothing of it on the shelf");
    assert_eq!(app.basket_amounts[shelf], 100);

    app.handle_key(GameKey::Char('N'));
    assert!(app.basket_amounts.iter().all(|n| *n == 0));
}

/// Uppercase is reserved for screen actions across every screen in the game.
/// Lowercase `a` and `n` are not this screen's, and must do nothing at all.
#[test]
fn lowercase_a_and_n_do_nothing() {
    let mut app = picker(984);

    // Asserted after each key rather than after both: `a` filling and `n`
    // clearing cancel out, so a test that presses the pair and looks once
    // passes against a screen that honours them both.
    app.handle_key(GameKey::Char('a'));
    assert_eq!(amounts(&app), vec![0, 0], "lowercase a fills nothing");

    app.handle_key(GameKey::Char('A'));
    app.handle_key(GameKey::Char('n'));
    assert_eq!(amounts(&app), vec![12, 4], "and lowercase n clears nothing");
    assert_eq!(app.mode, Mode::Transfer);
}

/// Up and Down move the cursor and wrap, through `menu_selected` — the field
/// `popup_layout` follows, which is what gives this screen scrolling without
/// a scroll of its own.
#[test]
fn up_and_down_move_the_row_cursor_and_wrap() {
    let mut app = picker(985);

    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 0, "and wraps");
    app.handle_key(GameKey::Up);
    assert_eq!(app.menu_selected, 1);
}

/// Enter commits both halves in one action, and leaves.
#[test]
fn enter_moves_both_halves_and_spends_one_turn() {
    let mut app = depot_picker(986, 100, &[("power_cell", 5)]);
    let shelf = row_of(&app, ITEM);
    let pack = row_of(&app, "power_cell");
    let before = app.game.as_ref().unwrap().current_tick();

    app.menu_selected = shelf;
    app.handle_key(GameKey::Char('4'));
    app.menu_selected = pack;
    app.handle_key(GameKey::ShiftRight);
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Playing);
    let game = app.game.as_ref().unwrap();
    assert_eq!(game.current_tick(), before + 1, "one commit, one turn");
    let offer = game.transfer_offer();
    let shelf_now = offer.iter().find(|r| r.item == item(ITEM)).unwrap();
    assert_eq!(shelf_now.on_shelves, 96, "only what was asked for left");
    let put = offer.iter().find(|r| r.item == item("power_cell")).unwrap();
    assert_eq!(put.on_shelves, 5, "and the pack's cargo is in the Depot");
    assert_eq!(put.carried, 0);
    assert_eq!(put.can_put, 0);
}

/// Enter on an all-zero basket is the same no-op as Esc: nothing moves, no
/// turn is spent, and the screen closes.
#[test]
fn enter_on_an_empty_basket_moves_nothing() {
    let mut app = picker(987);
    let before = app.game.as_ref().unwrap().current_tick();
    let offer = app.game.as_ref().unwrap().transfer_offer();

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(app.game.as_ref().unwrap().transfer_offer(), offer);
}

/// Abandoning the screen moves nothing and costs no turn.
#[test]
fn esc_moves_nothing_and_spends_no_turn() {
    let mut app = picker(988);
    let before = app.game.as_ref().unwrap().current_tick();
    let offer = app.game.as_ref().unwrap().transfer_offer();

    app.handle_key(GameKey::Char('9'));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(app.game.as_ref().unwrap().transfer_offer(), offer);
}

/// Every exit clears all three fields, so a reopened screen can never show a
/// stale shelf, a stale pack or a room figure from the last one.
#[test]
fn both_exits_leave_nothing_stale() {
    for leave in [GameKey::Enter, GameKey::Esc] {
        let mut app = depot_picker(989, 100, &[(ITEM, 2)]);
        app.handle_key(GameKey::Char('2'));
        app.handle_key(leave);

        assert!(app.basket_rows.is_empty(), "{leave:?} left rows behind");
        assert!(
            app.basket_amounts.is_empty(),
            "{leave:?} left a basket behind"
        );
        assert_eq!(app.basket_room, None, "{leave:?} left a room figure");
    }
}
