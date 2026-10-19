//! `Mode::MachineRecipe`: the recipe menu, the Recipe page of a machine's hub.

use super::support::*;
use crate::*;

fn recipe_of(app: &App) -> Option<ItemId> {
    let game = app.game.as_ref().unwrap();
    let machine = game.adjacent_machines()[0];
    game.machine_recipe_view(machine).unwrap().current
}

/// `[c]` opens the machine's hub on its shelf; `]` turns to the recipe.
fn open_recipe(app: &mut App) {
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer, "the shelf is the first page");
    app.handle_key(GameKey::Char(']'));
}

#[test]
fn c_beside_a_machine_opens_its_recipe_menu() {
    let mut app = app_beside_a_machine(9400, "factory", None);

    open_recipe(&mut app);

    assert_eq!(app.mode, Mode::MachineRecipe);
    assert!(app.machine_recipe.is_some());

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::MachineRecipe, "Esc goes to the rail first");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.machine_recipe.is_none(), "and drops it on the way out");
}

#[test]
fn r_beside_a_machine_no_longer_opens_it() {
    let mut app = app_beside_a_machine(9401, "factory", None);

    app.handle_key(GameKey::Char('R'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.machine_recipe.is_none());
}

#[test]
fn picking_a_row_on_an_unset_machine_sets_it_without_asking() {
    let mut app = app_beside_a_machine(9402, "factory", None);
    open_recipe(&mut app);
    let first = app.machine_recipe.as_ref().unwrap().view.rows[0]
        .item
        .clone();

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::MachineRecipe);
    assert_eq!(recipe_of(&app), Some(first));
}

#[test]
fn picking_a_row_on_a_set_machine_asks_first() {
    let mut app = app_beside_a_machine(9403, "factory", Some("bytecode_block"));
    open_recipe(&mut app);
    let other = app
        .machine_recipe
        .as_ref()
        .unwrap()
        .view
        .rows
        .iter()
        .position(|r| !r.current)
        .unwrap();
    let key = char::from_digit(other as u32 + 1, 10).unwrap();

    app.handle_key(GameKey::Char(key));

    assert_eq!(app.mode, Mode::MachineRecipeConfirm);
    assert_eq!(recipe_of(&app), Some(ItemId::from("bytecode_block")));
    let prompt = app
        .machine_recipe
        .as_ref()
        .unwrap()
        .confirm_prompt()
        .unwrap();
    assert!(prompt.starts_with("Switch the Factory to "), "{prompt}");

    app.handle_key(GameKey::Char('y'));

    assert_eq!(app.mode, Mode::MachineRecipe);
    assert_ne!(recipe_of(&app), Some(ItemId::from("bytecode_block")));
}

#[test]
fn declining_the_confirm_leaves_the_recipe() {
    let mut app = app_beside_a_machine(9404, "factory", Some("bytecode_block"));
    open_recipe(&mut app);
    let other = app
        .machine_recipe
        .as_ref()
        .unwrap()
        .view
        .rows
        .iter()
        .position(|r| !r.current)
        .unwrap();
    app.handle_key(GameKey::Char(
        char::from_digit(other as u32 + 1, 10).unwrap(),
    ));
    assert_eq!(app.mode, Mode::MachineRecipeConfirm);

    app.handle_key(GameKey::Char('n'));

    assert_eq!(app.mode, Mode::MachineRecipe);
    assert_eq!(recipe_of(&app), Some(ItemId::from("bytecode_block")));
    assert!(app.machine_recipe.as_ref().unwrap().pending.is_none());
}

#[test]
fn the_menu_shows_only_researched_rows() {
    let mut app = app_beside_a_machine(9405, "factory", None);
    open_recipe(&mut app);

    let items: Vec<&str> = app
        .machine_recipe
        .as_ref()
        .unwrap()
        .view
        .rows
        .iter()
        .map(|r| r.item.as_str())
        .collect();

    assert_eq!(
        items,
        ["bytecode_block", "charge_coil"],
        "a fresh game has researched none of the Factory's gated recipes"
    );
}

#[test]
fn an_uppercase_letter_never_picks_a_row() {
    let mut app = app_beside_a_machine(9406, "factory", None);
    open_recipe(&mut app);

    app.handle_key(GameKey::Char('A'));

    assert_eq!(recipe_of(&app), None);
    assert_eq!(app.mode, Mode::MachineRecipe);
}

#[test]
fn the_screen_closes_when_the_machine_is_demolished() {
    let mut app = app_beside_a_machine(9407, "factory", None);
    open_recipe(&mut app);
    let machine = app.machine_recipe.as_ref().unwrap().machine;
    app.game
        .as_mut()
        .unwrap()
        .remove_structure(machine)
        .unwrap();

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.machine_recipe.is_none());
}
