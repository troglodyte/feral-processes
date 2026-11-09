//! The one nav rule (ADR 0045): arrows move the cursor, `Tab`/`Shift+Tab`
//! switch the highlighted page's tabs from the rail or the page, `{ }` jump
//! rail sections, and `Left` from the page's leftmost position returns to the
//! rail. The structure hub keeps its own `Tab` (the structure cycle) until
//! phase 3 folds it into `i`.

use super::sites_hub::app_with_far_sites;
use super::support::*;
use crate::*;

fn base_page(app: &mut App, title: &'static str) {
    app.handle_key(GameKey::Char('b'));
    app.turn_hub_to(Hub::Base, &HubPageKey::Named(title));
}

fn tab_modes(app: &App) -> Vec<Mode> {
    let (hub, page, _) = app.hub_at().expect("a hub page is showing");
    hub.kind().pages(app)[page].modes().collect()
}

/// Presses `key` `n` times and records the mode after each press.
fn walk(app: &mut App, key: GameKey, n: usize) -> Vec<Mode> {
    (0..n)
        .map(|_| {
            app.handle_key(key);
            app.mode
        })
        .collect()
}

#[test]
fn tab_cycles_the_pages_tabs_from_the_rail_and_wraps() {
    let mut app = test_app(43001);
    base_page(&mut app, "Staff");
    assert!(app.hub_on_rail);
    let tabs = tab_modes(&app);
    assert_eq!(tabs.len(), 5);
    let forward = walk(&mut app, GameKey::Tab, 6);
    assert_eq!(
        forward,
        [tabs[1], tabs[2], tabs[3], tabs[4], tabs[0], tabs[1]]
    );
    assert!(app.hub_on_rail, "Tab stays in its half");
}

#[test]
fn shift_tab_cycles_backwards_in_the_page_and_wraps() {
    let mut app = test_app(43002);
    base_page(&mut app, "Production");
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail);
    let tabs = tab_modes(&app);
    let back = walk(&mut app, GameKey::ShiftTab, 6);
    assert_eq!(back, [tabs[4], tabs[3], tabs[2], tabs[1], tabs[0], tabs[4]]);
    assert!(!app.hub_on_rail, "Shift+Tab stays in its half");
}

#[test]
fn tab_is_a_no_op_on_a_one_tab_page() {
    let mut app = test_app(43003);
    base_page(&mut app, "Stock");
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::ShiftTab);
    assert_eq!(app.mode, Mode::BaseStock);
}

#[test]
fn right_in_a_page_no_longer_changes_tabs() {
    let mut app = test_app(43004);
    base_page(&mut app, "Production");
    app.handle_key(GameKey::Right);
    let first = app.mode;
    for _ in 0..4 {
        app.handle_key(GameKey::Right);
        assert_eq!(app.mode, first);
    }
}

#[test]
fn left_from_the_leftmost_position_returns_to_the_rail_on_every_tab() {
    let mut app = test_app(43005);
    base_page(&mut app, "Build orders");
    app.handle_key(GameKey::Right);
    for _ in 0..tab_modes(&app).len() {
        let shown = app.mode;
        app.handle_key(GameKey::Left);
        assert!(app.hub_on_rail, "Left on {shown:?}");
        assert_eq!(app.mode, shown, "Left leaves the tab alone");
        app.handle_key(GameKey::Right);
        assert!(!app.hub_on_rail);
        app.handle_key(GameKey::Tab);
    }
}

#[test]
fn site_tabs_cycle_with_tab_and_shift_tab() {
    let mut app = app_with_far_sites(43006);
    app.handle_key(GameKey::Char('w'));
    let tabs = tab_modes(&app);
    assert_eq!(tabs.len(), 5);
    assert_eq!(walk(&mut app, GameKey::Tab, 5)[4], tabs[0]);
    assert_eq!(walk(&mut app, GameKey::ShiftTab, 1), [tabs[4]]);
    assert!(app.hub_on_rail);
}

#[test]
fn braces_jump_to_the_first_page_of_the_next_and_previous_section() {
    let mut app = app_with_far_sites(43007);
    app.handle_key(GameKey::Char('b'));
    let pages = Hub::Base.kind().pages(&app);
    let starts: Vec<usize> = (0..pages.len())
        .filter(|&i| pages[i].section.is_some())
        .collect();
    assert!(
        starts.len() >= 4,
        "base, towns, outposts and traffic: {starts:?}"
    );
    let at = |app: &App| app.hub_at().map(|(_, p, _)| p).unwrap();
    assert_eq!(at(&app), starts[0]);
    app.handle_key(GameKey::Char('}'));
    assert_eq!(at(&app), starts[1]);
    app.handle_key(GameKey::Char('}'));
    let towns = starts[2];
    assert_eq!(pages[towns].section, Some("TOWNS"));
    assert_eq!(at(&app), towns);
    // From inside a section too: Alder, then Birch, then the next section.
    app.handle_key(GameKey::Down);
    assert_eq!(at(&app), towns + 1);
    app.handle_key(GameKey::Char('}'));
    assert_eq!(at(&app), starts[3]);
    for _ in 3..starts.len() {
        app.handle_key(GameKey::Char('}'));
    }
    assert_eq!(at(&app), starts[0], "wraps past the last section");
    app.handle_key(GameKey::Char('{'));
    assert_eq!(at(&app), *starts.last().unwrap(), "wraps before the first");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('{'));
    assert_eq!(at(&app), starts[starts.len() - 2], "works from the page");
    assert!(!app.hub_on_rail);
}

#[test]
fn braces_do_nothing_on_a_hub_with_no_sections() {
    let mut app = test_app(43008);
    open_hub_screen(&mut app, Mode::PhaseKeys);
    let before = app.hub_at();
    app.handle_key(GameKey::Char('}'));
    app.handle_key(GameKey::Char('{'));
    assert_eq!(app.hub_at(), before);
    assert_eq!(app.mode, Mode::PhaseKeys);
}

#[test]
fn braces_yield_to_an_applied_filter() {
    let mut app = test_app(43009);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Inventory);
    assert!(!app.screen_claims(GameKey::Char('{')));
    app.handle_key(GameKey::Char('a'));
    assert!(app.filter_applied());
    assert!(app.screen_claims(GameKey::Char('{')));
    assert!(app.screen_claims(GameKey::Char('}')));
}

#[test]
fn the_duty_table_keeps_left_and_right_for_its_columns() {
    let mut app = app_inside_a_small_base_with_programs(43010, false, 2);
    app.handle_key(GameKey::Char('b'));
    app.turn_hub_to(Hub::Base, &HubPageKey::Named("Staff"));
    while app.mode != Mode::BaseStaff {
        app.handle_key(GameKey::Tab);
    }
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::BaseStaff);
    let width = app.base_staff_table().columns.len();
    for col in 1..width {
        app.handle_key(GameKey::Right);
        assert_eq!(app.work_column, col);
        assert_eq!(app.mode, Mode::BaseStaff);
    }
    app.handle_key(GameKey::Right);
    assert_eq!(app.work_column, width - 1, "no tab beyond the last column");
    assert_eq!(app.mode, Mode::BaseStaff);
    for _ in 1..width {
        app.handle_key(GameKey::Left);
        assert!(!app.hub_on_rail);
    }
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "off the first column it goes to the rail");
}

#[test]
fn a_level_row_keeps_left_and_right() {
    let mut app = test_app(43011);
    app.handle_key(GameKey::Esc);
    app.turn_hub_to(Hub::GameMenu, &HubPageKey::Named("Audio"));
    app.handle_key(GameKey::Right);
    let before = app.profile.volume_percent;
    app.handle_key(GameKey::Right);
    assert_ne!(app.profile.volume_percent, before);
    assert_eq!(app.mode, Mode::AudioOptions);
    assert!(!app.hub_on_rail);
}

/// Phase 1 leaves `c`'s own Tab (the structure cycle) alone, and Shift+Tab is
/// its inverse so the rule is not half-adopted there.
#[test]
fn the_structure_hub_cycles_structures_with_tab_and_shift_tab() {
    let mut app = app_between_a_depot_and_a_machine(43012);
    app.handle_key(GameKey::Char('c'));
    let first = app.structure_hub.as_ref().unwrap().current().entity;
    app.handle_key(GameKey::Tab);
    let second = app.structure_hub.as_ref().unwrap().current().entity;
    assert_ne!(first, second);
    app.handle_key(GameKey::ShiftTab);
    assert_eq!(app.structure_hub.as_ref().unwrap().current().entity, first);
}
