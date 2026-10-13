//! The base report hub: `BASE_REPORT_PAGES`, the `[`/`]`/`Tab`/`Esc` nav
//! that runs ahead of every hub screen's own handler, and the three doors
//! in (`B`, `N`, the base-menu row).

use feral_processes_engine::alerts::AlertKind;

use crate::tests::alerts::{alert, seeded};
use crate::tests::support::*;
use crate::*;

fn page_modes() -> Vec<Mode> {
    BASE_REPORT_PAGES
        .iter()
        .map(|p| p.screens[0])
        .collect::<Vec<_>>()
}

#[test]
fn the_rail_is_overview_production_alerts() {
    assert_eq!(
        page_modes(),
        vec![Mode::BaseOverview, Mode::BaseOutput, Mode::Alerts]
    );
    assert_eq!(base_page_of(Mode::BaseOutput), Some((1, 0)));
    assert_eq!(base_page_of(Mode::Playing), None);
}

#[test]
fn b_opens_overview_and_esc_returns_to_the_map() {
    let mut app = test_app(30001);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn right_bracket_wraps_last_to_first_and_left_bracket_first_to_last() {
    let mut app = test_app(30002);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Alerts, "[ wraps first -> last");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview, "] wraps last -> first");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOutput);
}

#[test]
fn esc_from_a_later_page_leaves_the_hub_rather_than_stepping_back() {
    let mut app = test_app(30003);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Alerts);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn esc_returns_to_the_base_menu_when_the_row_opened_the_hub() {
    let mut app = test_app(30004);
    open_via_menu(&mut app, 'b', "Base report");
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu);
}

#[test]
fn tab_is_consumed_by_the_hub_and_keeps_a_single_screen_page() {
    let mut app = test_app(30005);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::BaseOverview);
    assert!(!app.log_expanded, "Tab must not reach the map's log toggle");
}

#[test]
fn a_digit_on_alerts_stays_on_alerts() {
    let alerts = (0..5)
        .map(|i| {
            alert(
                AlertKind::DepotsFull,
                &format!("s{i}"),
                &format!("alert-{i}"),
            )
        })
        .collect();
    let mut app = seeded(30006, alerts);
    app.handle_key(GameKey::Char('N'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(
        app.menu_selected, 1,
        "the highlight is Down's, not the digit's page jump"
    );
}

#[test]
fn b_reopens_the_last_page() {
    let mut app = test_app(30007);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::BaseOutput);
}

#[test]
fn the_base_menu_row_reopens_the_last_page_too() {
    let mut app = test_app(30008);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Esc);
    open_via_menu(&mut app, 'b', "Base report");
    assert_eq!(app.mode, Mode::Alerts);
}

#[test]
fn n_lands_on_alerts_with_everything_marked_read() {
    let mut app = seeded(
        30009,
        vec![alert(
            AlertKind::DepotsFull,
            "depots",
            "Every Depot is full.",
        )],
    );
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 1);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_base_output_row_is_gone_and_the_report_row_is_offered() {
    let mut app = test_app(30010);
    app.handle_key(GameKey::Char('b'));
    let labels: Vec<_> = app.base_menu_rows().iter().map(|r| r.label).collect();
    assert!(!labels.contains(&"Base output"), "{labels:?}");
    assert!(labels.contains(&"Base report"), "{labels:?}");
}

#[test]
fn enter_on_an_overview_row_jumps_to_that_pages_first_screen() {
    let mut app = test_app(30011);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::BaseOutput,
        "row 0 is the page after Overview"
    );

    let mut app = test_app(30013);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Alerts);
}

/// The nav layer must let a digit through to the screen under it — Alerts
/// discards its row pick, so only a screen that acts on one can catch a nav
/// that swallows digits.
#[test]
fn a_digit_on_the_overview_reaches_its_handler() {
    let mut app = test_app(30014);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.mode, Mode::Alerts);
}

#[test]
fn the_screens_inside_the_hub_cost_no_game_time() {
    let mut app = test_app(30012);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('B'));
    for _ in 0..4 {
        app.handle_key(GameKey::Char(']'));
        app.handle_key(GameKey::Tab);
    }
    app.handle_key(GameKey::Esc);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}
