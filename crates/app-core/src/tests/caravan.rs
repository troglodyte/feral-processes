//! The counter a visiting caravan sets out beside the base's own.

use feral_processes_engine::items::ids;
use feral_processes_engine::save;

use super::support::*;
use crate::app::caravan::{CaravanRow, caravan_row};
use crate::*;

/// The offset arithmetic, which is the part of a two-section screen that can
/// be tested without a trader standing in front of you — and the part that
/// goes wrong. `market_row` and `trade_row` exist for the same reason.
#[test]
fn caravan_row_resolves_both_sections_and_stops_at_the_end() {
    assert_eq!(caravan_row(0, 3, 2), Some(CaravanRow::Offer(0)));
    assert_eq!(caravan_row(2, 3, 2), Some(CaravanRow::Offer(2)));
    assert_eq!(caravan_row(3, 3, 2), Some(CaravanRow::Sell(0)));
    assert_eq!(caravan_row(4, 3, 2), Some(CaravanRow::Sell(1)));
    assert_eq!(caravan_row(5, 3, 2), None, "one past the end");
    assert_eq!(caravan_row(0, 0, 2), Some(CaravanRow::Sell(0)), "no offers");
    assert_eq!(caravan_row(0, 3, 0), Some(CaravanRow::Offer(0)), "no cargo");
    assert_eq!(caravan_row(0, 0, 0), None, "an empty screen picks nothing");
}

/// A base with a Home, an iso Market and cargo, ticked forward until a
/// trader has walked in and set out its stock.
///
/// The clock is *run* rather than the caravan hand-placed: app-core cannot
/// reach the `World` (that is the architectural rule), and a fixture that
/// could would be testing its own idea of a docked trader.
fn app_at_a_caravan(seed: u32) -> Option<App> {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("caravan", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = vec![
        (ItemId::from(ids::CREDITS), 100_000),
        (ItemId::from(ids::CORE_FRAGMENT), 40),
    ];
    data.structures.push(save::StructureSave {
        kind: "market".to_string(),
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    // Founded from the open grid, which is the one build made from outside
    // base space — then straight back in, since a caravan is met in there.
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 1, 1);

    // One whole visit interval is the bound, since exactly one visit falls
    // in each — a run that finds none in that many ticks has found a broken
    // schedule rather than an unlucky seed.
    for _ in 0..3000 {
        if app
            .game
            .as_mut()
            .is_some_and(|g| g.caravan_reach() == CaravanReach::AtCaravan)
        {
            return Some(app);
        }
        app.game.as_mut().unwrap().wait();
    }
    None
}

fn a_caravan() -> App {
    app_at_a_caravan(4)
        .or_else(|| app_at_a_caravan(11))
        .expect("a trader should reach the counter inside two visit windows")
}

/// The row asks `caravan_reach`, never `caravan_view`: this closure runs
/// every frame the menu is open, and the view rolls a whole shelf — gear
/// copies, affixes and all — before it could answer.
#[test]
fn the_base_menu_offers_the_caravan_row_only_while_one_is_docked() {
    let mut app = test_app(7);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    assert!(
        !app.base_menu_rows().iter().any(|r| r.label == "Caravan"),
        "nothing is visiting and the row is offered anyway"
    );

    let mut docked = a_caravan();
    assert!(
        docked.base_menu_rows().iter().any(|r| r.label == "Caravan"),
        "a trader is standing at the counter and the row is missing"
    );

    // Out on the grid there is nothing to take, and the row goes with it.
    // Base space has one door, so the party walks to it first.
    stand_in_base_at(&mut docked, 0, 0);
    docked
        .game
        .as_mut()
        .unwrap()
        .leave_base()
        .expect("standing on the exit cell, the way out is open");
    assert!(
        !docked.base_menu_rows().iter().any(|r| r.label == "Caravan"),
        "the row survived the party walking out of the base"
    );
}

/// The wagon is one screen now, not two — the quantity page it used to open
/// on a cargo row is gone, so Esc has one step to take rather than two.
#[test]
fn esc_backs_out_of_the_caravan_screen_to_where_it_came_from() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    assert_eq!(app.mode, Mode::Caravan);

    // Editing a row must not take the player anywhere. Walked to with the
    // arrows rather than typed: a shelf is deeper than `menu_shortcut`'s 35
    // labels, so the sell rows below it have no key of their own.
    let offers = app
        .game
        .as_mut()
        .unwrap()
        .caravan_view()
        .unwrap()
        .offers
        .len();
    for _ in 0..offers {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Caravan);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::BaseMenu,
        "the screen was opened from the base menu, so Esc goes back to it"
    );
}

/// A cursor past the end of the drawn list must resolve to a row rather
/// than index off it. It gets there on its own: buying a row drops one out
/// of the offers section, and a stale `menu_selected` is what a screen with
/// two dynamically-sized sections is left holding.
#[test]
fn a_cursor_past_the_end_still_resolves_to_a_row() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let before = app.game.as_mut().unwrap().caravan_view().unwrap();
    assert!(
        before.offers.len() >= 2 && !before.sells.is_empty(),
        "the fixture needs both sections to have rows for this to mean anything"
    );

    // Buy the first row, which drops it out of the offers section and
    // leaves `menu_selected` pointing one past where it did.
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Caravan, "one purchase should not close it");
    let after = app.game.as_mut().unwrap().caravan_view().unwrap();
    assert_eq!(
        after.offers.len(),
        before.offers.len() - 1,
        "the bought row is still on the wagon"
    );

    // Well past whatever the list holds now. Asserted on the basket rather
    // than on the mode: a dead key leaves the screen exactly as it was,
    // which is indistinguishable from working.
    app.menu_selected = 999;
    app.handle_key(GameKey::ShiftRight);
    assert!(
        app.caravan_amounts.iter().any(|n| *n > 0),
        "a cursor past the end resolved to nothing, so the key was dead"
    );
}

/// Buying the wagon out entirely closes the screen — a screen left open on
/// a counter with nothing on it is one the player has to press Esc to escape
/// for no reason. `handle_stack_market_key` closes on the same rule.
#[test]
fn a_trader_that_rolls_away_closes_the_screen() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    assert_eq!(app.mode, Mode::Caravan);

    // Wait out the whole stay. The trader packs up and there is nothing to
    // draw, so the next keypress on the screen drops back to the map.
    for _ in 0..(feral_processes_engine::tuning::CARAVAN_STAY_TICKS + 50) {
        app.game.as_mut().unwrap().wait();
    }
    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the wagon has gone and the screen is still open on it"
    );
}

/// Sets the cursor on the first sell row and returns its index.
fn first_sell_row(app: &mut App) -> usize {
    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    assert!(!view.sells.is_empty(), "the fixture needs cargo to sell");
    view.offers.len()
}

/// **The sell ceiling is per row and static** — the mirror of
/// `App::take_available`, and the half of the transfer picker's asymmetry
/// this side inherits. Nothing pending anywhere may move it, its own row
/// included: the shelf it counts is the pack, and the pack does not change
/// until the basket commits.
#[test]
fn a_sell_rows_ceiling_is_static() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let row = first_sell_row(&mut app);
    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    let before = app.caravan_sell_available(&view, row);
    assert!(before > 0, "the fixture needs cargo the wagon will take");

    // Something pending on both sides of the basket.
    app.menu_selected = 0;
    app.handle_key(GameKey::ShiftRight);
    app.menu_selected = row;
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(
        app.caravan_amounts[row], before,
        "the row filled to its stack"
    );

    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    assert_eq!(
        app.caravan_sell_available(&view, row),
        before,
        "a pending amount moved the shelf it was measured against"
    );
    assert_eq!(
        app.caravan_sell_available(&view, 0),
        0,
        "an offer row has no shelf of the player's to count"
    );
}

/// **The buy side is one budget.** A pending buy lowers what the *others*
/// can reach — and never what the highlighted row can, or the row could be
/// lowered and never raised again.
#[test]
fn a_pending_buy_lowers_the_other_rows_budget_but_not_its_own() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    assert!(view.offers.len() >= 2, "the fixture needs two offers");
    let budget_before = app.caravan_budget(&view, 1);

    app.menu_selected = 0;
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.caravan_amounts[0], 1, "an offer row is all or nothing");

    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    let price = view.offers[0].unit_cost * view.offers[0].qty;
    assert_eq!(
        app.caravan_budget(&view, 1),
        budget_before - price,
        "a pending buy has to come off what the rest of the basket can reach"
    );
    assert_eq!(
        app.caravan_budget(&view, 0),
        budget_before,
        "...and never off its own row, or it could not be raised again"
    );

    // Which is the property that matters: lowered and raised, at the ceiling.
    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.caravan_amounts[0], 0);
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.caravan_amounts[0], 1, "the row went dead once lowered");
}

/// Enter commits and **leaves the screen open**: a wagon is a place you shop
/// at, not a form you submit. The amounts clear, because they have been
/// spent.
#[test]
fn enter_commits_the_basket_and_stays_on_the_wagon() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let before = app.game.as_mut().unwrap().caravan_view().unwrap();

    app.menu_selected = 0;
    app.handle_key(GameKey::ShiftRight);
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Caravan, "the wagon closed on a commit");
    assert!(
        app.caravan_amounts.iter().all(|n| *n == 0),
        "a committed basket must not still be holding what it spent"
    );
    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .caravan_view()
            .unwrap()
            .offers
            .len(),
        before.offers.len() - 1,
        "the row was bought"
    );
}

/// **The modifier fold.** `Mode::Caravan` has to be named in the one
/// condition at the top of `App::handle_key`, or its four modified arrows are
/// folded to bare `Left`/`Right` before this handler ever sees them — Shift
/// becomes a step of one and nothing anywhere fails.
///
/// Read on the *left* modifier from a full row, because a step of one and a
/// jump to the end differ by exactly one press only there.
#[test]
fn shift_left_empties_a_sell_row_in_one_press() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let row = first_sell_row(&mut app);
    app.menu_selected = row;

    app.handle_key(GameKey::ShiftRight);
    let filled = app.caravan_amounts[row];
    assert!(filled > 1, "the fixture needs a stack deeper than one");

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(
        app.caravan_amounts[row], 0,
        "Shift+Left stepped by one, so the modifier never reached this screen"
    );
}

/// `[A]` fills the **sell** rows only. The take side is the one with a
/// per-row ceiling, and here that is the sell side — filling the offers
/// would spend the whole purse on one keypress, on a screen with no buyback.
#[test]
fn a_fills_the_sell_rows_and_leaves_the_offers_alone() {
    let mut app = a_caravan();
    open_via_menu(&mut app, 'b', "Caravan");
    let view = app.game.as_mut().unwrap().caravan_view().unwrap();
    let offers = view.offers.len();

    app.handle_key(GameKey::Char('A'));

    assert!(
        app.caravan_amounts[..offers].iter().all(|n| *n == 0),
        "[A] spent the purse on the wagon's stock"
    );
    assert!(
        app.caravan_amounts[offers..].iter().any(|n| *n > 0),
        "[A] filled nothing at all"
    );
    for (i, n) in app.caravan_amounts[offers..].iter().enumerate() {
        assert_eq!(*n, view.sells[i].held, "a cargo row is filled to the stack");
    }
}
