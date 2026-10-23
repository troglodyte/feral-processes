//! Type-to-filter on a Stack market. Filtering to the *last* row of a
//! section puts its view index well off position 0, so a handler that
//! indexed the unfiltered shelf would buy or sell the wrong row.

use super::super::stack_market::app_at_a_market;
use super::super::support::*;
use crate::*;
use feral_processes_engine::StackMarketView;

fn open(credits: u32) -> App {
    let mut app = app_at_a_market(credits);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(app.mode, Mode::StackMarket);
    app
}

fn view(app: &mut App) -> StackMarketView {
    app.game.as_mut().unwrap().stack_market().unwrap()
}

/// The leading run of a row name that the filter can be typed from (the
/// shelf names carry symbols the filter never takes).
fn typeable(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == ' ')
        .collect()
}

#[test]
fn enter_buys_the_filtered_offer_and_keeps_the_filter() {
    let mut app = open(100_000);
    let v = view(&mut app);
    let last = v.offers.last().unwrap().clone();
    let name = typeable(&last.name);
    assert!(
        v.offers.len() > 1,
        "{:?}",
        v.offers.iter().map(|o| &o.name).collect::<Vec<_>>()
    );
    type_text(&mut app, &name);
    let vis = market_visible(&view(&mut app), app.filter_text());
    assert!(vis.offers.contains(&(v.offers.len() - 1)));
    assert!(vis.offers.len() < v.offers.len(), "a real subset");
    // Highlight the last visible offer, whose view index is the last one.
    let pos = vis.offers.iter().position(|&i| i == v.offers.len() - 1);
    app.menu_selected = pos.unwrap();
    let want = vis.offers[app.menu_selected];

    app.handle_key(GameKey::Enter);

    let after = view(&mut app);
    let gone: Vec<usize> = v
        .offers
        .iter()
        .filter(|o| !after.offers.iter().any(|a| a.index == o.index))
        .map(|o| o.index)
        .collect();
    assert_eq!(gone, vec![v.offers[want].index], "only the filtered row");
    assert_eq!(app.filter_text(), name);
}

#[test]
fn shift_sell_and_inspect_use_the_filtered_cargo_row() {
    let mut app = open(0);
    let v = view(&mut app);
    assert!(v.sells.len() >= 2, "the player starts with several stacks");
    let last = v.sells.last().unwrap().clone();
    type_text(&mut app, &typeable(&last.name));
    let vis = market_visible(&view(&mut app), app.filter_text());
    let offers = vis.offers.len();
    let pos = offers
        + vis
            .sells
            .iter()
            .position(|&i| i == v.sells.len() - 1)
            .unwrap();
    app.menu_selected = pos;

    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert_eq!(app.pending_inspect.as_ref().unwrap().copy, last.copy);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StackMarket);
    assert_eq!(app.menu_selected, pos, "filter and highlight come back");

    app.handle_key(GameKey::Char('S'));
    let after = view(&mut app);
    assert!(
        !after.sells.iter().any(|r| r.copy == last.copy),
        "the whole filtered stack went"
    );
    assert_eq!(after.sells.len(), v.sells.len() - 1, "and only that one");
}

#[test]
fn a_filter_with_no_rows_does_nothing() {
    let mut app = open(100_000);
    let before = view(&mut app);
    type_text(&mut app, "qqq");
    for key in [GameKey::Enter, GameKey::Char('S'), GameKey::Char('I')] {
        app.handle_key(key);
    }
    assert_eq!(app.mode, Mode::StackMarket);
    assert_eq!(view(&mut app), before);
}

#[test]
fn esc_clears_the_filter_then_walks_on() {
    let mut app = open(0);
    type_text(&mut app, "q");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::StackMarket);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_highlight_stays_on_a_row_after_the_last_match_is_sold() {
    let mut app = open(0);
    let v = view(&mut app);
    let last = v.sells.last().unwrap().clone();
    // One letter, so several rows match and the vanished one is not the
    // only row left to clamp onto.
    type_text(&mut app, &typeable(&last.name)[..1]);
    let vis = market_visible(&view(&mut app), app.filter_text());
    assert!(vis.total() >= 2 && vis.sells.contains(&(v.sells.len() - 1)));
    app.menu_selected = vis.total() - 1;

    app.handle_key(GameKey::Char('S'));

    let vis = market_visible(&view(&mut app), app.filter_text());
    assert!(
        app.menu_selected < vis.total().max(1),
        "highlight {} past {} rows",
        app.menu_selected,
        vis.total()
    );
}
