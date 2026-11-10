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
    app_with_a_trader(seed, |g| {
        g.caravan_reach() == feral_processes_engine::CaravanReach::AtCaravan
    })
}

/// `app_at_a_caravan`'s base, ticked until `ready` holds.
fn app_with_a_trader(seed: u32, ready: fn(&mut Game) -> bool) -> Option<App> {
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
        recipe: None,
        kind: "market".to_string(),
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        battery_charge: 0,
        refit_charge: 0,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    // Founded, then stood on the pocket's floor, since a caravan is met there.
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 1, 1);

    // One whole visit interval is the bound, since exactly one visit falls
    // in each — a run that finds none in that many ticks has found a broken
    // schedule rather than an unlucky seed.
    for _ in 0..3000 {
        if app.game.as_mut().is_some_and(ready) {
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

/// The Trader tab is always on the Caravans page: with no wagon at the
/// counter its keys say so and it stays put, rather than closing the hub
/// under the player.
#[test]
fn the_trader_tab_with_no_wagon_stays_open_and_says_why() {
    let mut app = test_app(7);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    open_hub_screen(&mut app, Mode::Caravan);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.status_line, None,
        "browsing an empty tab is not a refusal"
    );
    for key in [GameKey::Right, GameKey::Enter, GameKey::Char('A')] {
        app.status_line = None;
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Caravan, "{key:?}");
        assert_eq!(
            app.status_line.as_deref(),
            Some("No trader is at the base's counter."),
            "{key:?}"
        );
    }
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

/// A wagon still walking in is not waiting for anyone, wherever the party
/// stands: the tab says it has not arrived, not to walk back.
#[test]
fn the_trader_tab_says_a_wagon_on_its_way_has_not_arrived() {
    let walking = |g: &mut Game| {
        g.caravan_reach() == feral_processes_engine::CaravanReach::NotDocked && !g.caravan_docked()
    };
    let mut app = app_with_a_trader(4, walking)
        .or_else(|| app_with_a_trader(11, walking))
        .expect("a trader sets out inside two visit windows");
    for _ in 0..2 {
        let line = app.trader_absence().expect("no wagon to deal with");
        assert_eq!(line, "The trader hasn't reached the base's counter yet.");
        walk_far_from_the_base(&mut app);
    }
    open_hub_screen(&mut app, Mode::Caravan);
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.status_line.as_deref(),
        Some("The trader hasn't reached the base's counter yet.")
    );
}

/// With no wagon there is no basket for Left to lower, so Left goes back
/// to the rail and nothing is refused.
#[test]
fn left_on_an_empty_trader_tab_goes_to_the_rail() {
    let mut app = test_app(7);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    open_hub_screen(&mut app, Mode::Caravan);
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
    assert_eq!(app.status_line, None);
}

/// Off the base's floor a docked wagon is still there, so the tab says to
/// walk back rather than that nobody came.
#[test]
fn the_trader_tab_away_from_the_base_says_to_walk_back() {
    let mut app = a_caravan();
    walk_far_from_the_base(&mut app);
    assert!(
        app.game.as_mut().unwrap().caravan_view().is_none(),
        "precondition"
    );
    open_hub_screen(&mut app, Mode::Caravan);
    let line = app.trader_absence().expect("no wagon to deal with");
    assert!(line.contains("Walk back to the base"), "{line}");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line.as_deref(), Some(line));
}

/// The basket's arrows are the tab's: Right raises a sell row and Left
/// lowers it again rather than leaving for the rail, and Esc is the way
/// back to the rail.
#[test]
fn the_trader_tab_keeps_its_arrows_and_esc_goes_to_the_rail() {
    let mut app = a_caravan();
    open_hub_screen(&mut app, Mode::Caravan);

    // Walked to with the arrows: the sell rows sit below the whole shelf.
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
    app.handle_key(GameKey::Left);
    assert_eq!(app.mode, Mode::Caravan);
    assert!(!app.hub_on_rail, "Left lowered the row, it did not leave");

    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail, "Esc on the tab goes to the rail");
    assert_eq!(app.mode, Mode::Caravan);
}

/// A cursor past the end of the drawn list must resolve to a row rather
/// than index off it. It gets there on its own: buying a row drops one out
/// of the offers section, and a stale `menu_selected` is what a screen with
/// two dynamically-sized sections is left holding.
#[test]
fn a_cursor_past_the_end_still_resolves_to_a_row() {
    let mut app = a_caravan();
    open_command(&mut app, "Caravan");
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

/// A wagon that rolls away leaves the Trader tab saying so, rather than
/// closing the hub under the player, and takes its basket with it: the hub
/// eats both Esc presses, so it is leaving the tab that empties it.
#[test]
fn a_trader_that_rolls_away_leaves_the_tab_empty() {
    let mut app = a_caravan();
    open_hub_screen(&mut app, Mode::Caravan);
    fill_a_sell_row(&mut app);

    // Wait out the whole stay. The trader packs up and there is nothing to
    // draw.
    for _ in 0..(feral_processes_engine::tuning::CARAVAN_STAY_TICKS + 50) {
        app.game.as_mut().unwrap().wait();
    }
    assert!(app.game.as_mut().unwrap().caravan_view().is_none());
    assert_eq!(app.mode, Mode::Caravan, "the tab stays up");
    // Long enough for the base to be swept at least once, which fires the
    // raid tutorial over whatever the screen had settled on, so each Esc
    // may first clear a notification.
    for _ in 0..2 {
        app.handle_key(GameKey::Esc);
        dismiss_notifications(&mut app);
    }
    if app.mode == Mode::Caravan {
        app.handle_key(GameKey::Esc);
    }

    assert_eq!(app.mode, Mode::Playing, "Esc leaves the hub");
    assert!(app.caravan_amounts.is_empty(), "the basket went with it");
}

/// Esc goes to the rail and Esc again leaves the hub, neither reaching the
/// tab's handler; the basket still does not outlive the tab.
#[test]
fn leaving_the_trader_tab_empties_the_basket() {
    let mut app = a_caravan();
    open_hub_screen(&mut app, Mode::Caravan);
    fill_a_sell_row(&mut app);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Caravan, "the first Esc is the rail's");
    assert!(
        app.caravan_amounts.iter().any(|n| *n > 0),
        "still on the tab"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.caravan_amounts.is_empty(), "the basket was left behind");
}

/// Inspecting a sell row's gear is a look, not a leave: the basket is there
/// on the way back.
#[test]
fn inspecting_gear_from_the_trader_keeps_the_basket() {
    let mut app = a_caravan();
    open_hub_screen(&mut app, Mode::Caravan);
    fill_a_sell_row(&mut app);
    let kept = app.caravan_amounts.clone();
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Caravan);
    assert_eq!(app.caravan_amounts, kept);
}

/// Walks to the first sell row and raises it by one.
fn fill_a_sell_row(app: &mut App) {
    let row = first_sell_row(app);
    for _ in 0..row {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Right);
    assert!(
        app.caravan_amounts.iter().any(|n| *n > 0),
        "the basket holds a row"
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
    open_command(&mut app, "Caravan");
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
    open_command(&mut app, "Caravan");
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
    open_command(&mut app, "Caravan");
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
    open_command(&mut app, "Caravan");
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
    open_command(&mut app, "Caravan");
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
