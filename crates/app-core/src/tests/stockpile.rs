//! `Mode::Stockpile` — `z` zones laid floor with the Excavation box.

use super::support::*;
use crate::*;

fn app_in_the_base(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 3, 3);
    app
}

fn tick(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

fn zoned(app: &App) -> Vec<(i32, i32)> {
    app.game.as_ref().unwrap().stockpile_cells()
}

#[test]
fn z_opens_the_stockpile_cursor_on_the_party() {
    let mut app = app_in_the_base(6201);
    app.excavate_anchor = Some((9, 9));
    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::Stockpile);
    assert_eq!(app.excavate_cursor, Some((3, 3)));
    assert_eq!(app.excavate_anchor, None);
}

#[test]
fn two_spaces_mark_a_stockpile_box_and_spend_no_tick() {
    let mut app = app_in_the_base(6202);
    let before = tick(&app);
    app.handle_key(GameKey::Char('z'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char(' '));
    assert_eq!(zoned(&app), vec![(3, 3), (4, 3)]);
    assert_eq!(app.mode, Mode::Stockpile, "the mode stays open");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(tick(&app), before);
}

#[test]
fn esc_takes_the_anchor_then_the_mode() {
    let mut app = app_in_the_base(6203);
    app.handle_key(GameKey::Char('z'));
    app.handle_key(GameKey::Char(' '));
    assert!(app.excavate_anchor.is_some());
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Stockpile);
    assert_eq!(app.excavate_anchor, None);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.excavate_cursor, None);
}

#[test]
fn z_on_the_zone_surface_refuses() {
    let mut app = test_app(6204);
    found_the_base(&mut app);
    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.status_line.is_some());
}
