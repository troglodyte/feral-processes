//! Type-to-filter on the trade screen. Sections filter separately, and each
//! test puts the highlight past the first match so a handler indexing an
//! unfiltered section would act on the wrong source row.

use super::super::support::*;
use crate::*;
use feral_processes_engine::items::ids;

const PACK: &[(&str, u32)] = &[
    (ids::CREDITS, 50),
    (ids::CORE_FRAGMENT, 5),
    (ids::POWER_CELL, 2),
    (ids::OVERCLOCK_CORE, 3),
    (ids::ICE_BREAKER, 1),
];

fn trade_app(seed: u32) -> App {
    let mut app = app_at_a_trading_post(seed, PACK);
    app.mode = Mode::Trade;
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::TradeAction);
    app
}

fn vis(app: &mut App) -> TradeVisible {
    let structure = app.pending_trade_structure.unwrap();
    let filter = app.filter_text().to_owned();
    trade_visible(app.game.as_mut().unwrap(), structure, &filter)
}

fn held(app: &App, item: &str) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item.as_str() == item)
        .map_or(0, |r| r.qty)
}

/// "ice" leaves ICE Breaker in two sections: the sell row (source 2 of the
/// pack) and the buy row (source 1 of the shelf), which unfiltered are
/// neither the first row nor next to each other.
const FILTER: &str = "ice";

fn filtered(seed: u32) -> App {
    let mut app = trade_app(seed);
    type_text(&mut app, FILTER);
    app
}

#[test]
fn each_section_filters_separately() {
    let mut app = filtered(1301);
    let v = vis(&mut app);
    assert_eq!((v.sells, v.buys), (vec![2], vec![1]));
    assert!(v.buybacks.is_empty() && v.programs.is_empty());
    assert_eq!(app.menu_selected, 0);
}

#[test]
fn enter_picks_the_filtered_row_of_its_own_section() {
    let mut app = filtered(1302);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TradeQuantity);
    let Some(TradeChoice::Sell(copy)) = app.pending_trade_choice.clone() else {
        panic!("first visible row is the sale");
    };
    assert_eq!(copy.item.as_str(), ids::ICE_BREAKER);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::TradeAction);
    assert_eq!(app.filter_text(), FILTER);

    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    let Some(TradeChoice::Buy(item)) = app.pending_trade_choice.clone() else {
        panic!("second visible row is the purchase");
    };
    assert_eq!(item.as_str(), ids::ICE_BREAKER);
}

#[test]
fn quick_sell_and_buy_use_the_filtered_row() {
    let mut app = filtered(1303);
    let credits = held(&app, ids::CREDITS);
    app.handle_key(GameKey::Char('S'));
    assert_eq!(held(&app, ids::ICE_BREAKER), 0, "sold the filtered row");
    assert_eq!(held(&app, ids::POWER_CELL), 2, "and nothing above it");
    assert!(held(&app, ids::CREDITS) > credits);
    assert_eq!(app.filter_text(), FILTER);

    // The pack row is gone, so the buy row is now the only one.
    let v = vis(&mut app);
    assert!(v.sells.is_empty());
    assert_eq!(v.buybacks, vec![0], "the sale is on the buyback shelf");
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('B'));
    assert_eq!(held(&app, ids::ICE_BREAKER), 1, "bought the filtered row");
    assert_eq!(held(&app, ids::POWER_CELL), 2);
}

#[test]
fn inspect_opens_the_filtered_row_and_returns_with_the_filter() {
    let mut app = filtered(1304);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(
        app.pending_inspect.as_ref().unwrap().copy.item.as_str(),
        ids::ICE_BREAKER
    );
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TradeAction);
    assert_eq!(app.filter_text(), FILTER);
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn a_filter_with_no_rows_makes_every_action_do_nothing() {
    let mut app = trade_app(1305);
    type_text(&mut app, "qqq");
    assert_eq!(vis(&mut app).total(), 0);
    let before = (held(&app, ids::CREDITS), held(&app, ids::ICE_BREAKER));
    for key in [GameKey::Char('S'), GameKey::Char('B'), GameKey::Enter] {
        app.handle_key(key);
    }
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::TradeAction);
    assert_eq!(
        before,
        (held(&app, ids::CREDITS), held(&app, ids::ICE_BREAKER))
    );
}

#[test]
fn the_quantity_page_round_trip_keeps_the_filter() {
    let mut app = filtered(1306);
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TradeAction);
    assert_eq!(app.filter_text(), FILTER);
}

#[test]
fn esc_clears_the_filter_before_leaving_the_screen() {
    let mut app = filtered(1307);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::TradeAction);
    assert!(!app.filter_applied());
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Trade);
}

#[test]
fn a_filtered_trade_screen_inside_a_filtered_inventory_gives_the_pack_back_intact() {
    let mut app = app_at_trading_posts(1308, PACK, 2);
    open_hub_screen(&mut app, Mode::Inventory);
    type_text(&mut app, "core");
    app.handle_key(GameKey::Down);
    let row = app.menu_selected;
    // Two posts in range: `S` opens the trader picker with the sale pending.
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::Trade);
    // Take the pending sale away so the post's own list opens, as the trader
    // picker does when there is no sale to carry.
    app.pending_trade_choice = None;
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::TradeAction);
    type_text(&mut app, FILTER);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TradeQuantity);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(held(&app, ids::ICE_BREAKER), 0, "the nested sale landed");
    assert_eq!(app.filter_text(), "core");
    assert_eq!(app.menu_selected, row);
}
