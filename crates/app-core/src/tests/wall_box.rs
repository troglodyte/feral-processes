//! `Mode::WallBox` — picking the Wall from the build menu paints a box of
//! wall requests with the Excavation plan's cursor.

use super::support::*;
use crate::*;

fn app_in_the_base(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 3, 1);
    app
}

/// Opens the build menu and picks `id` out of its category.
fn pick_structure(app: &mut App, id: &str) {
    open_command(app, "Deploy a structure");
    let menu = app.build_menu().expect("the build menu is open");
    let category = menu
        .defs
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("{id} is buildable"))
        .category();
    let row = menu
        .rows
        .iter()
        .position(|r| *r == BuildRow::Category(category))
        .expect("the structure's category is listed");
    pick_row(app, row);
    let menu = app.build_menu().expect("the category is open");
    let row = menu
        .rows
        .iter()
        .position(|r| matches!(r, BuildRow::Structure(i) if menu.defs[*i].id == id))
        .expect("the structure is listed in its category");
    pick_row(app, row);
}

fn filed_walls(app: &App) -> usize {
    app.game
        .as_ref()
        .unwrap()
        .build_order_report()
        .iter()
        .filter(|o| o.structure == "Wall")
        .count()
}

#[test]
fn picking_the_wall_opens_the_box_cursor_on_the_party() {
    let mut app = app_in_the_base(6301);
    app.excavate_anchor = Some((9, 9));

    pick_structure(&mut app, "wall");

    assert_eq!(app.mode, Mode::WallBox);
    assert_eq!(app.excavate_cursor, Some((3, 1)));
    assert_eq!(app.excavate_anchor, None);
}

#[test]
fn two_spaces_file_a_row_of_walls_and_spend_no_tick() {
    let mut app = app_in_the_base(6302);
    let before = app.game.as_ref().unwrap().current_tick();
    pick_structure(&mut app, "wall");

    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char(' '));

    // Three cells, less the party's own.
    assert_eq!(filed_walls(&app), 2);
    assert_eq!(app.mode, Mode::WallBox, "the mode stays open");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

#[test]
fn a_door_still_asks_for_a_direction() {
    let mut app = app_in_the_base(6303);

    pick_structure(&mut app, "door");

    assert_eq!(app.mode, Mode::BuildDirection);
}
