//! `y` puts a thing from the pack on the floor and `o` takes it back.

use feral_processes_engine::save;

use super::support::*;
use crate::*;

fn app_in_the_base(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 3, 3);
    app
}

fn floor_count(app: &App) -> usize {
    app.game.as_ref().unwrap().floor_items().len()
}

#[test]
fn y_opens_the_put_down_picker_and_a_row_puts_it_down() {
    let mut app = app_in_the_base(6101);
    give_player_items(&mut app, &[("core_fragment", 2)]);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::PutDown);
    assert_eq!(app.menu_selected, 0);
    let rows = app.put_down_rows();
    assert!(!rows.is_empty());

    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::Playing, "{:?}", app.status_line);
    assert_eq!(floor_count(&app), 1);
}

#[test]
fn esc_leaves_the_picker_without_putting_anything_down() {
    let mut app = app_in_the_base(6102);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::PutDown);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(floor_count(&app), 0);
}

/// Through a save round trip, as `give_player_items` does: the pack has no
/// public way to be emptied that would not also fill the floor.
fn empty_the_pack(app: &mut App) {
    let path = scratch_path("floor_hands_empty_pack", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory.clear();
    data.player.gear_copies.clear();
    data.player.downed_programs.clear();
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn y_with_an_empty_pack_refuses() {
    let mut app = app_in_the_base(6103);
    empty_the_pack(&mut app);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.status_line.as_deref(),
        Some("You're carrying nothing to put down.")
    );
}

#[test]
fn o_picks_up_underfoot() {
    let mut app = app_in_the_base(6104);
    app.handle_key(GameKey::Char('y'));
    pick_row(&mut app, 0);
    assert_eq!(floor_count(&app), 1);
    app.handle_key(GameKey::Char('o'));
    assert_eq!(floor_count(&app), 0);
}

#[test]
fn o_with_nothing_underfoot_refuses() {
    let mut app = app_in_the_base(6105);
    app.handle_key(GameKey::Char('o'));
    assert_eq!(
        app.status_line.as_deref(),
        Some("There's nothing here to pick up.")
    );
}

#[test]
fn y_on_the_zone_surface_refuses() {
    let mut app = test_app(6106);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.status_line.as_deref(),
        Some("Nothing to put down out here — the floor is through the anchor.")
    );
}
