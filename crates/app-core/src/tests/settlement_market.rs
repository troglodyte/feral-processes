//! `Mode::SettlementMarket` — the shelf behind a town's hub page, opened
//! with `[M]` and edited as one basket.
//!
//! `tests/caravan.rs`'s questions, one vendor over. What is *not* asked
//! here is anything about prices or delivery: those are
//! `Game::commit_settlement_basket`'s, tested in the engine against the
//! real shelf. This file asks only what app-core owns — the mode, the
//! basket, the ceilings and the fold.

use feral_processes_engine::items::ids;
use feral_processes_engine::save;
use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::views::SettlementMarketView;

use super::support::*;
use crate::*;

/// A town east of the player, a purse, and a stack the town will take —
/// `app_at_a_caravan`'s cargo exactly, since `settlement_sell_rows` takes
/// every inventory row that is neither the currency nor banked.
///
/// Two save round trips rather than one: `place_settlement_east_of_player`
/// owns the placement and is not worth a second parameter for this.
fn app_at_a_settlement(seed: u32) -> (App, SettlementKey) {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let (key, _) = place_settlement_east_of_player(&mut app);

    let path = scratch_path("settlement_market", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = vec![
        (ItemId::from(ids::CREDITS), 100_000),
        (ItemId::from(ids::CORE_FRAGMENT), 40),
    ];
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    (app, key)
}

/// At the shelf, basket empty.
fn app_at_the_market(seed: u32) -> (App, SettlementKey) {
    let (mut app, key) = app_at_a_settlement(seed);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Settlement, "the bump should open the hub");
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.mode, Mode::SettlementMarket);
    (app, key)
}

fn view(app: &mut App, key: SettlementKey) -> SettlementMarketView {
    app.game
        .as_mut()
        .unwrap()
        .settlement_view(key)
        .expect("the party is standing beside the town")
}

fn credits(app: &mut App, key: SettlementKey) -> u32 {
    view(app, key).credits
}

/// The first sell row, which the fixture's cargo guarantees exists.
fn first_sell_row(app: &mut App, key: SettlementKey) -> usize {
    let v = view(app, key);
    assert!(
        !v.sells.is_empty(),
        "the fixture's cargo should be sellable"
    );
    v.offers.len()
}

#[test]
fn m_opens_the_market_from_the_hub_page() {
    let (app, _) = app_at_the_market(970);
    assert!(
        app.settlement_amounts.is_empty(),
        "the basket opens empty; the first keypress sizes it"
    );
}

/// Esc goes back one page rather than to the map — the difference from
/// `Mode::Caravan`, which is reached off the base menu and closes to
/// `Mode::Playing`.
#[test]
fn esc_returns_to_the_settlement_page_and_clears_the_basket() {
    let (mut app, key) = app_at_the_market(971);
    let row = first_sell_row(&mut app, key);
    app.menu_selected = row;
    app.handle_key(GameKey::Right);
    assert_eq!(app.settlement_amounts[row], 1, "Right should raise a row");

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Settlement);
    assert_eq!(
        app.pending_settlement,
        Some(key),
        "backing out of the shelf does not leave the town"
    );
    assert!(app.settlement_amounts.is_empty(), "a stale basket survived");
}

/// The commit is `Game::commit_settlement_basket` — the shared
/// `commerce::settle_basket` core — and the screen stays open after it.
#[test]
fn enter_commits_the_basket_through_the_shared_core() {
    let (mut app, key) = app_at_the_market(972);
    let row = first_sell_row(&mut app, key);
    let before = credits(&mut app, key);
    let unit = view(&mut app, key).sells[0].unit_price;
    assert!(unit > 0, "the town should pay something for cargo");
    app.menu_selected = row;
    app.handle_key(GameKey::Right);

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::SettlementMarket, "the shelf stays open");
    assert_eq!(
        credits(&mut app, key),
        before + unit,
        "one unit sold should be one unit paid for"
    );
    assert!(
        app.settlement_amounts.is_empty(),
        "a committed basket is cleared"
    );
    assert!(
        app.status_line.is_some(),
        "the commit should say what it did"
    );
}

/// The refusal lands before anything is spent — `settle_basket`'s rule,
/// asserted from the screen that can produce an empty basket at will.
#[test]
fn an_empty_basket_spends_nothing() {
    let (mut app, key) = app_at_the_market(973);
    let before = credits(&mut app, key);
    let held = view(&mut app, key).sells[0].held;

    app.handle_key(GameKey::Enter);

    assert_eq!(credits(&mut app, key), before, "an empty basket charged");
    assert_eq!(
        view(&mut app, key).sells[0].held,
        held,
        "cargo moved anyway"
    );
    assert!(
        app.status_line.is_some(),
        "a refusal is one sentence on two surfaces"
    );
}

/// The fold at `app/input.rs:184`. Omitted from it, `ShiftRight` arrives as
/// a bare `Right` and raises the row by one instead of filling it — which
/// is invisible against a row whose ceiling happens to be one, so the
/// assertion is made on a sell row, whose ceiling is the whole held stack.
#[test]
fn shift_and_ctrl_reach_the_market_unfolded() {
    let (mut app, key) = app_at_the_market(974);
    let row = first_sell_row(&mut app, key);
    let held = view(&mut app, key).sells[0].held;
    assert!(held > 2, "the fixture's stack must be big enough to halve");
    app.menu_selected = row;

    app.handle_key(GameKey::ShiftRight);
    assert_eq!(
        app.settlement_amounts[row], held,
        "ShiftRight is a target: the whole stack"
    );

    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(
        app.settlement_amounts[row],
        held.div_ceil(2),
        "CtrlLeft halves the gap to zero"
    );

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.settlement_amounts[row], 0, "ShiftLeft empties the row");
}

/// One budget across every offer row, the caravan's rule: a pending sale
/// funds a buy in the same basket, and every *other* pending buy is spent
/// before this row can reach for it.
#[test]
fn the_buy_budget_counts_the_baskets_own_sales_in() {
    let (mut app, key) = app_at_the_market(975);
    let v = view(&mut app, key);
    assert!(
        !v.offers.is_empty(),
        "the shelf should have something on it"
    );
    let row = v.offers.len();
    let unit = v.sells[0].unit_price;
    let before = app.settlement_budget(&v, 0);
    app.menu_selected = row;
    app.handle_key(GameKey::Right);

    let v = view(&mut app, key);
    assert_eq!(
        app.settlement_budget(&v, 0),
        before + unit,
        "a pending sale should raise what the offer rows may reach"
    );
}

/// The refuse-service consequence at the screen: a hostile town leaves the
/// mode standing and answers with a refusal, rather than dropping the
/// player back to the map as `close_if_settlement_gone` would.
///
/// Standing is set through a save round trip — the only door app-core has
/// onto an engine resource, `place_settlement_east_of_player`'s own reason.
#[test]
fn a_hostile_town_keeps_the_screen_open_and_refuses_the_basket() {
    let (mut app, key) = app_at_the_market(9_411);

    let assets_dir = test_assets_dir();
    let path = scratch_path("settlement_hostile", 9_411);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.standings.0.insert(
        key,
        feral_processes_engine::settlements::Relation {
            standing: feral_processes_engine::tuning::SETTLEMENT_MIN_STANDING,
            trade_credits: 0,
        },
    );
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::SettlementMarket,
        "a shut counter is a page the player stands in front of, not a closed screen"
    );
    let refusal = app.status_line.clone().unwrap_or_default();
    assert!(refusal.contains("won't trade"), "{refusal:?}");
}
