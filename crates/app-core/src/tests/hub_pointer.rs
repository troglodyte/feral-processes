//! The mouse on a hub: every hit is the key it stands for, so each test
//! drives one app with the pointer and a twin with the keys and holds the
//! two to the same place.

use crate::tests::support::*;
use crate::*;

type Place = (Mode, Option<(Hub, usize, usize)>, bool, usize);

fn place(app: &App) -> Place {
    (app.mode, app.hub_at(), app.hub_on_rail, app.menu_selected)
}

fn click(app: &mut App, hit: PointerHit) {
    app.handle_pointer_event(PointerEvent {
        hover: Some(hit),
        press: true,
        ..Default::default()
    });
}

fn keys(app: &mut App, keys: &[GameKey]) {
    for k in keys {
        app.handle_key(*k);
    }
}

fn pair(seed: u32, open: &[GameKey]) -> (App, App) {
    let mut a = test_app(seed);
    let mut b = test_app(seed);
    keys(&mut a, open);
    keys(&mut b, open);
    (a, b)
}

#[test]
fn a_rail_entry_click_is_the_page_turn_and_focuses_the_rail() {
    use GameKey::*;
    let (mut mouse, mut kb) = pair(41200, &[Char('b'), Right]);
    let build = HubPageKey::Named("Build");
    click(&mut mouse, PointerHit::Page(build));
    keys(&mut kb, &[Esc, Down]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(mouse.mode, Mode::Build);
    assert!(mouse.hub_on_rail);
}

#[test]
fn a_click_on_the_held_rail_entry_only_focuses_the_rail() {
    use GameKey::*;
    let (mut mouse, mut kb) = pair(41201, &[Char('b'), Down, Tab, Right]);
    let held = mouse.hub_page().cloned().unwrap();
    click(&mut mouse, PointerHit::Page(held));
    keys(&mut kb, &[Esc]);
    assert_eq!(place(&mouse), place(&kb));
}

#[test]
fn a_tab_click_is_the_tab_key() {
    use GameKey::*;
    let mut open = vec![Char('b')];
    open.extend([Char(']'); 4]);
    let (mut mouse, mut kb) = pair(41202, &open);
    click(&mut mouse, PointerHit::Tab(2));
    keys(&mut kb, &[Tab, Tab]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(mouse.mode, Mode::StaffDiscontent);
    click(&mut mouse, PointerHit::Tab(3));
    keys(&mut kb, &[Tab]);
    assert_eq!(place(&mouse), place(&kb));
}

/// The game menu's first page is a list of toggles: Enter flips one.
fn on_the_options_list(seed: u32) -> (App, App) {
    pair(seed, &[GameKey::Esc, GameKey::Right])
}

#[test]
fn a_row_click_selects_it_and_a_second_click_is_enter() {
    use GameKey::*;
    let (mut mouse, mut kb) = on_the_options_list(41203);
    click(&mut mouse, PointerHit::Row(2));
    keys(&mut kb, &[Down, Down]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(mouse.menu_selected, 2);
    click(&mut mouse, PointerHit::Row(2));
    keys(&mut kb, &[Enter]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(
        format!("{:?}", mouse.profile()),
        format!("{:?}", kb.profile())
    );
}

#[test]
fn a_double_click_on_an_unselected_row_is_select_then_enter() {
    use GameKey::*;
    let (mut mouse, mut kb) = on_the_options_list(41204);
    mouse.handle_pointer_event(PointerEvent {
        hover: Some(PointerHit::Row(1)),
        press: true,
        double: true,
        ..Default::default()
    });
    keys(&mut kb, &[Down, Enter]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(
        format!("{:?}", mouse.profile()),
        format!("{:?}", kb.profile())
    );
}

#[test]
fn a_row_click_with_the_cursor_on_the_rail_enters_the_page_without_enter() {
    use GameKey::*;
    let (mut mouse, mut kb) = pair(41205, &[Esc]);
    click(&mut mouse, PointerHit::Row(0));
    keys(&mut kb, &[Right]);
    assert_eq!(place(&mouse), place(&kb));
    assert!(!mouse.hub_on_rail);
}

#[test]
fn a_keycap_click_is_the_key() {
    use GameKey::*;
    let (mut mouse, mut kb) = pair(41206, &[Char('b')]);
    click(&mut mouse, PointerHit::Key(Char(']')));
    keys(&mut kb, &[Char(']')]);
    assert_eq!(place(&mouse), place(&kb));
    click(&mut mouse, PointerHit::Key(Esc));
    keys(&mut kb, &[Esc]);
    assert_eq!(place(&mouse), place(&kb));
    assert_eq!(mouse.mode, Mode::Playing);
}

#[test]
fn a_more_marker_is_a_page_key() {
    use GameKey::*;
    let (mut mouse, mut kb) = on_the_options_list(41207);
    click(&mut mouse, PointerHit::More(Direction::Down));
    keys(&mut kb, &[PageDown]);
    assert_eq!(place(&mouse), place(&kb));
    click(&mut mouse, PointerHit::More(Direction::Up));
    keys(&mut kb, &[PageUp]);
    assert_eq!(place(&mouse), place(&kb));
}

#[test]
fn the_wheel_is_up_and_down_on_the_hovered_region() {
    use GameKey::*;
    // Over the page: rows, the cursor in the page.
    let (mut mouse, mut kb) = on_the_options_list(41208);
    mouse.handle_pointer_event(PointerEvent {
        hover: Some(PointerHit::Region(Region::Page)),
        wheel: -2,
        ..Default::default()
    });
    keys(&mut kb, &[Down, Down]);
    assert_eq!(place(&mouse), place(&kb));
    mouse.handle_pointer_event(PointerEvent {
        hover: Some(PointerHit::Row(0)),
        wheel: 1,
        ..Default::default()
    });
    keys(&mut kb, &[Up]);
    assert_eq!(place(&mouse), place(&kb));
    // Over the rail: the pages.
    let (mut mouse, mut kb) = pair(41209, &[Esc]);
    mouse.handle_pointer_event(PointerEvent {
        hover: Some(PointerHit::Region(Region::Rail)),
        wheel: -1,
        ..Default::default()
    });
    keys(&mut kb, &[Down]);
    assert_eq!(place(&mouse), place(&kb));
}

#[test]
fn the_wheel_over_nothing_does_nothing() {
    let (mut mouse, kb) = on_the_options_list(41210);
    mouse.handle_pointer_event(PointerEvent {
        hover: None,
        wheel: -3,
        ..Default::default()
    });
    assert_eq!(place(&mouse), place(&kb));
}

#[test]
fn hub_hits_do_nothing_off_a_hub() {
    let mut app = test_app(41211);
    assert_eq!(app.mode, Mode::Playing);
    click(&mut app, PointerHit::Row(1));
    click(&mut app, PointerHit::Tab(1));
    click(&mut app, PointerHit::Page(HubPageKey::Named("Build")));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.menu_selected, 0);
}

#[test]
fn only_a_primary_press_acts_and_hover_is_recorded_always() {
    let (mut app, kb) = on_the_options_list(41212);
    app.handle_pointer_event(PointerEvent {
        hover: Some(PointerHit::Row(3)),
        press: true,
        button: PointerButton::Secondary,
        ..Default::default()
    });
    assert_eq!(place(&app), place(&kb));
    assert!(app.hovered(&PointerHit::Row(3)));
    assert!(!app.hovered(&PointerHit::Row(2)));
    app.handle_pointer_event(PointerEvent::default());
    assert!(!app.hovered(&PointerHit::Row(3)));
}

#[test]
fn a_second_click_changes_what_enter_changes() {
    use GameKey::*;
    // The overview's Enter opens the page its row names.
    let (mut mouse, mut kb) = pair(41213, &[Char('b'), Right]);
    let before = place(&mouse);
    click(&mut mouse, PointerHit::Row(0));
    keys(&mut kb, &[Enter]);
    assert_eq!(place(&mouse), place(&kb));
    assert_ne!(place(&mouse), before, "Enter did something");
}
