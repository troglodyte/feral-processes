//! Type-to-filter on lists. Phase 1: the dev Give item picker and the hub's
//! Esc rule. The rule that matters throughout is that the highlighted row
//! and the row an action uses are the same source row.

use super::support::*;
use crate::*;

const FILTER: &str = "ar";

fn picker_app(seed: u32) -> App {
    let mut app = test_app(seed);
    app.enable_dev_console_for_test();
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    app.menu_selected = App::dev_console_rows()
        .iter()
        .position(|r| r.action == DevAction::GiveItem)
        .unwrap();
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::DevItemPicker);
    app
}

/// Source rows (id, name, held) whose name contains `FILTER`.
fn expected(app: &App) -> Vec<(feral_processes_engine::items::ItemId, String, u32)> {
    app.game
        .as_ref()
        .unwrap()
        .dev_plain_items()
        .into_iter()
        .filter(|(_, n, _)| n.to_lowercase().contains(FILTER))
        .collect()
}

fn held(app: &App, id: &feral_processes_engine::items::ItemId) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .dev_plain_items()
        .into_iter()
        .find(|(i, _, _)| i == id)
        .unwrap()
        .2
}

#[test]
fn typing_narrows_the_picker_and_backspace_widens_it() {
    let mut app = picker_app(41);
    let total = app.dev_item_rows().len();
    type_text(&mut app, FILTER);
    let want = expected(&app);
    assert!(want.len() >= 2 && want.len() < total, "needs a real subset");
    let names: Vec<String> = app.dev_item_rows().into_iter().map(|r| r.0).collect();
    assert_eq!(names, want.iter().map(|r| r.1.clone()).collect::<Vec<_>>());
    assert_eq!(app.filter_text(), FILTER);

    app.handle_key(GameKey::Backspace);
    assert_eq!(app.filter_text(), "a");
    app.handle_key(GameKey::Backspace);
    app.handle_key(GameKey::Backspace);
    assert_eq!(app.dev_item_rows().len(), total);
}

#[test]
fn capitals_are_not_typed() {
    let mut app = picker_app(42);
    app.handle_key(GameKey::Char('A'));
    assert_eq!(app.filter_text(), "");
}

#[test]
fn enter_gives_the_highlighted_match_and_keeps_filter_and_row() {
    let mut app = picker_app(43);
    type_text(&mut app, FILTER);
    let want = expected(&app);
    app.handle_key(GameKey::Down);
    let (id, _, before) = want[1].clone();
    let first_before = held(&app, &want[0].0);

    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Enter);

    assert_eq!(held(&app, &id), before + 20, "gave the second match twice");
    assert_eq!(held(&app, &want[0].0), first_before);
    assert_eq!(app.filter_text(), FILTER);
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn a_filter_change_puts_the_highlight_on_the_first_match() {
    let mut app = picker_app(44);
    type_text(&mut app, "a");
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.menu_selected, 0);
}

#[test]
fn no_match_gives_nothing() {
    let mut app = picker_app(45);
    type_text(&mut app, "qqqqq");
    assert!(app.dev_item_rows().is_empty());
    let before = app.game.as_ref().unwrap().dev_plain_items();
    app.handle_key(GameKey::Enter);
    assert_eq!(app.game.as_ref().unwrap().dev_plain_items(), before);
    assert_eq!(app.mode, Mode::DevItemPicker);
}

#[test]
fn esc_clears_the_filter_first_then_closes() {
    let mut app = picker_app(46);
    type_text(&mut app, FILTER);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::DevItemPicker);
    assert!(!app.filter_applied());
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::DevConsole);
}

#[test]
fn leaving_the_picker_forgets_the_filter() {
    let mut app = picker_app(47);
    let total = app.dev_item_rows().len();
    type_text(&mut app, FILTER);
    app.handle_key(GameKey::Backspace);
    app.handle_key(GameKey::Backspace);
    type_text(&mut app, FILTER);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::DevItemPicker);
    assert_eq!(app.dev_item_rows().len(), total);
}

/// The hub claims Esc for a filter before it moves focus to the rail, and a
/// page turn is leaving the screen. The Items page is the hub screen that
/// filters.
mod hub {
    use super::*;

    fn planted(seed: u32) -> App {
        let mut app = test_app(seed);
        open_hub_screen(&mut app, Mode::Inventory);
        type_text(&mut app, "x");
        app
    }

    #[test]
    fn esc_with_a_filter_is_not_taken_by_the_rail() {
        let mut app = planted(51);
        app.handle_key(GameKey::Esc);
        assert!(!app.hub_on_rail);
    }

    #[test]
    fn the_rail_round_trip_keeps_the_filter() {
        let mut app = planted(52);
        app.handle_key(GameKey::Left);
        assert!(app.hub_on_rail);
        app.handle_key(GameKey::Right);
        assert!(!app.hub_on_rail);
        assert_eq!(app.filter_text(), "x");
    }

    #[test]
    fn a_page_turn_forgets_it() {
        let mut app = planted(53);
        app.handle_key(GameKey::Char(']'));
        app.handle_key(GameKey::Char('['));
        assert_eq!(app.mode, Mode::Inventory);
        assert_eq!(app.filter_text(), "");
    }
}

mod inventory;
mod market;
mod trade;
