//! The collect picker — `Mode::Collect` and the basket it builds.

use super::support::*;
use crate::*;

/// The item the fixtures stock. Anything with a name would do; naming it
/// once keeps the tests reading as being about quantities.
const ITEM: &str = "core_fragment";

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

/// `c` beside a stocked machine opens the window rather than emptying the
/// shelves. Every row starts at zero — nothing is selected by default, which
/// is the whole of the change.
#[test]
fn c_opens_the_collect_window_with_an_empty_basket() {
    let mut app = app_beside_stocked_machines(970, &[(ITEM, 6)]);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Collect);
    let offer = app.game.as_ref().unwrap().collectable_adjacent();
    assert_eq!(app.collect_rows, offer);
    assert_eq!(
        app.collect_rows,
        vec![(item(ITEM), 12)],
        "pooled, both sides"
    );
    assert_eq!(app.collect_basket, vec![0]);
}

/// Opening a screen is not an action, and both halves of that are here.
///
/// The tick half is about the engine: nothing is taken, so nothing is
/// charged, and the commit is the first tick this whole flow spends. The
/// `status_line` half is about `acted`, which is what `after_world_action`
/// reads — returning `true` from a keypress that only opened a window wipes
/// whatever the last refusal was still explaining.
#[test]
fn opening_the_collect_window_spends_no_turn() {
    let mut app = app_beside_stocked_machines(971, &[(ITEM, 6)]);
    let before = app.game.as_ref().unwrap().current_tick();
    app.status_line = Some("something the player still needs to read".into());

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert!(
        app.status_line.is_some(),
        "opening a window is not an action, so it clears nothing"
    );
}

/// The refusal is unchanged and stays the engine's: with nothing on offer
/// `c` never opens the window, so there is no empty screen to back out of
/// and no second copy of the sentence in app-core.
#[test]
fn c_with_nothing_adjacent_opens_no_window() {
    let mut app = test_app(972);
    stand_in_base(&mut app);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.collect_rows.is_empty());
    assert!(app.collect_basket.is_empty());
}

/// The two items the multi-row fixtures stock, in `ItemId` order — which is
/// the order the rows come back in, so the tests can index them.
const ITEMS: [(&str, u32); 2] = [("core_fragment", 6), ("power_cell", 2)];

/// Opens the picker on the two-item fixture. Both sides hold the same
/// stock, so the rows read 12 and 4 — different from each other, which is
/// what stops a per-row rule passing against a shared maximum.
fn picker(seed: u32) -> App {
    let mut app = app_beside_stocked_machines(seed, &ITEMS);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Collect);
    assert_eq!(
        app.collect_rows,
        vec![(item("core_fragment"), 12), (item("power_cell"), 4)]
    );
    app
}

/// Up and Down move the cursor and wrap, through `menu_selected` — the
/// field `popup_layout` follows, which is what gives this screen scrolling
/// without a scroll of its own.
#[test]
fn up_and_down_move_the_row_cursor_and_wrap() {
    let mut app = picker(973);

    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 0, "wraps at the bottom");
    app.handle_key(GameKey::Up);
    assert_eq!(app.menu_selected, 1, "and at the top");
}

/// A digit is a quantity here, not a row pick — the whole reason this screen
/// cannot use `selected_index`. It lands on the highlighted row and nowhere
/// else.
#[test]
fn a_digit_sets_the_highlighted_row_alone() {
    let mut app = picker(974);

    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('3'));

    assert_eq!(app.collect_basket, vec![0, 3]);
}

/// Clamping happens as the digit is typed rather than at commit: `5` then
/// `0` against 12 available leaves 12. Ignoring the second digit instead
/// would be silent, and a row that stops responding to a keypress reads as
/// the screen being broken.
#[test]
fn typing_past_what_is_there_clamps_to_it() {
    let mut app = picker(975);

    app.handle_key(GameKey::Char('5'));
    assert_eq!(app.collect_basket[0], 5);
    app.handle_key(GameKey::Char('0'));

    assert_eq!(app.collect_basket[0], 12, "not 50, and not still 5");
}

/// Backspace drops the last digit, and stops at nothing rather than
/// wrapping round.
#[test]
fn backspace_drops_a_digit_and_bottoms_out_at_zero() {
    let mut app = picker(976);

    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.collect_basket[0], 12);

    app.handle_key(GameKey::Backspace);
    assert_eq!(app.collect_basket[0], 1);
    app.handle_key(GameKey::Backspace);
    assert_eq!(app.collect_basket[0], 0);
    app.handle_key(GameKey::Backspace);
    assert_eq!(app.collect_basket[0], 0, "nothing below nothing");
}

/// Left and Right are the one-at-a-time nudge, saturating at both ends.
///
/// **Left adds and Right removes**, which is the opposite of every other
/// Left/Right in the game and is deliberate — see `handle_collect_key`. This
/// test is the pin: a later hand reading the arena row editor or the manifest
/// pager and "restoring consistency" here fails it.
#[test]
fn left_and_right_step_by_one_and_saturate() {
    let mut app = picker(977);

    app.handle_key(GameKey::Right);
    assert_eq!(app.collect_basket[0], 0, "cannot ask for less than none");

    for _ in 0..3 {
        app.handle_key(GameKey::Left);
    }
    assert_eq!(app.collect_basket[0], 3);
    app.handle_key(GameKey::Right);
    assert_eq!(app.collect_basket[0], 2);

    app.handle_key(GameKey::Down);
    for _ in 0..9 {
        app.handle_key(GameKey::Left);
    }
    assert_eq!(
        app.collect_basket[1], 4,
        "cannot ask for more than is there"
    );
}

/// `[A]` fills every row to *its own* maximum, not to a shared one — which
/// is why the fixture's two rows hold different amounts. `[N]` puts them all
/// back to nothing.
#[test]
fn a_fills_every_row_and_n_clears_them() {
    let mut app = picker(978);

    app.handle_key(GameKey::Char('A'));
    assert_eq!(app.collect_basket, vec![12, 4]);

    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.collect_basket, vec![0, 0]);
}

/// Uppercase is reserved for screen actions across every screen in the game.
/// Lowercase `a` and `n` are not this screen's, and must do nothing at all.
#[test]
fn lowercase_a_and_n_do_nothing() {
    let mut app = picker(979);

    // Asserted after each key rather than after both: `a` filling and `n`
    // clearing cancel out, so a test that presses the pair and looks once
    // passes against a screen that honours them both.
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.collect_basket, vec![0, 0], "lowercase a fills nothing");

    app.handle_key(GameKey::Char('A'));
    app.handle_key(GameKey::Char('n'));
    assert_eq!(
        app.collect_basket,
        vec![12, 4],
        "and lowercase n clears nothing"
    );

    assert_eq!(app.mode, Mode::Collect);
}

/// A player holding a digit key must not overflow `u32` on the way to the
/// clamp.
///
/// The row is written by hand at `u32::MAX`, because on any ordinary shelf
/// the clamp lands after every keypress and the amount never climbs high
/// enough to overflow — a test pressing `9` twelve times against a real
/// buffer passes with the arithmetic written plainly, which is no coverage
/// at all. A ceiling that high is what the saturating form is actually for,
/// and a modded structure with a vast `capacity` is where it would come
/// from.
#[test]
fn holding_a_digit_key_cannot_overflow() {
    let mut app = picker(980);
    app.collect_rows = vec![(item(ITEM), u32::MAX)];
    app.collect_basket = vec![0];

    for _ in 0..12 {
        app.handle_key(GameKey::Char('9'));
    }

    assert_eq!(app.collect_basket[0], u32::MAX);
}

/// Enter takes exactly the basket and nothing else — the whole point of the
/// change is that the rest stays where the base's chains can still pull it.
#[test]
fn enter_takes_exactly_the_basket() {
    let mut app = picker(981);

    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Playing);
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.collectable_adjacent(),
        vec![(item("core_fragment"), 9), (item("power_cell"), 3)],
        "only what was asked for left the shelves"
    );
}

/// One commit is one action, whatever the basket holds. Asserted with two
/// items, since a one-item basket cannot tell one tick from per-item
/// ticking.
#[test]
fn enter_spends_exactly_one_turn_however_big_the_basket() {
    let mut app = picker(982);
    let before = app.game.as_ref().unwrap().current_tick();

    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);

    assert_eq!(app.game.as_ref().unwrap().current_tick(), before + 1);
}

/// Enter on an all-zero basket is the same no-op as Esc: nothing is taken,
/// no turn is spent, and the screen closes.
#[test]
fn enter_on_an_empty_basket_takes_nothing() {
    let mut app = picker(983);
    let before = app.game.as_ref().unwrap().current_tick();
    let offer = app.game.as_ref().unwrap().collectable_adjacent();

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(app.game.as_ref().unwrap().collectable_adjacent(), offer);
}

/// Abandoning the screen takes nothing and costs no turn, the same way a
/// collect that finds nothing costs no turn today.
#[test]
fn esc_takes_nothing_and_spends_no_turn() {
    let mut app = picker(984);
    let before = app.game.as_ref().unwrap().current_tick();
    let offer = app.game.as_ref().unwrap().collectable_adjacent();

    app.handle_key(GameKey::Char('9'));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert_eq!(app.game.as_ref().unwrap().collectable_adjacent(), offer);
}

/// Both exits clear the two fields, so a reopened screen can never show a
/// stale shelf or a basket the player already spent.
#[test]
fn both_exits_leave_no_stale_shelf() {
    for leave in [GameKey::Enter, GameKey::Esc] {
        let mut app = picker(985);
        app.handle_key(GameKey::Char('2'));
        app.handle_key(leave);

        assert!(app.collect_rows.is_empty(), "{leave:?} left rows behind");
        assert!(
            app.collect_basket.is_empty(),
            "{leave:?} left a basket behind"
        );
    }
}
