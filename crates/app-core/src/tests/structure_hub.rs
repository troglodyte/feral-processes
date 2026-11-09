//! `[c]` as `i`'s BESIDE YOU section: one entry per structure beside the
//! party, opened on the first and walked with `]`, `Tab` on its own pages.

use super::support::*;
use crate::*;

fn tick(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

/// The page index of the held entry on `i`'s rail.
fn page(app: &App) -> usize {
    app.hub_at().map(|(_, p, _)| p).expect("the hub is open")
}

#[test]
fn c_beside_a_depot_opens_on_its_inventory_with_the_cursor_on_the_rail() {
    let mut app = app_beside_depots(2601, 1, 0, &[("power_cell", 3)]);
    let before = tick(&app);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
    assert!(app.hub_on_rail, "c opens on the rail, as b and i do");
    assert_eq!(app.hub_at(), Some((Hub::Inventory, 3, 0)));
    assert_eq!(held_beside(&app).name, "Depot");
    assert_eq!(tick(&app), before, "opening the hub spends no turn");
}

#[test]
fn a_structures_pages_are_its_tabs_and_tab_walks_them() {
    let mut app = app_between_a_depot_and_a_machine(2609);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::MachineRecipe, "Tab is the next page");
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::ShiftTab);
    assert_eq!(app.mode, Mode::Transfer);
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail, "Right enters the page");
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "Left off a one-screen page is the rail");
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn brackets_walk_the_structures_clockwise() {
    // Factory east, Depot west: N, E, S, W puts the machine first.
    let mut app = app_between_a_depot_and_a_machine(2602);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(held_beside(&app).dir, (1, 0));
    let machine = held_beside(&app).entity;
    assert_eq!(app.mode, Mode::Transfer);

    app.handle_key(GameKey::Char(']'));
    assert_eq!(held_beside(&app).dir, (-1, 0));
    assert_eq!(app.mode, Mode::Transfer, "the depot opens on its first tab");

    app.handle_key(GameKey::Char('['));
    assert_eq!(held_beside(&app).entity, machine);
}

#[test]
fn turning_to_a_structure_without_the_tab_opens_its_first() {
    let mut app = app_between_a_depot_and_a_machine(2603);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::MachineRecipe, "the machine's second tab");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Transfer, "the depot has no recipe tab");
}

#[test]
fn turning_to_a_structure_with_the_tab_keeps_it() {
    let mut app = app_beside_a_machine_and_two_depots(2614);
    app.handle_key(GameKey::Char('c'));
    while !held_beside(&app).name.starts_with("Depot") {
        app.handle_key(GameKey::Char(']'));
    }
    while app.mode != Mode::DepotFilter {
        app.handle_key(GameKey::Tab);
    }
    let first = held_beside(&app).entity;
    app.handle_key(GameKey::Char(']'));
    assert_ne!(held_beside(&app).entity, first);
    assert!(held_beside(&app).name.starts_with("Depot"));
    assert_eq!(app.mode, Mode::DepotFilter, "the tab came along");
}

#[test]
fn two_structures_of_a_name_say_which_side_they_are_on() {
    let app = app_beside_a_machine_and_two_depots(2615);
    let titles: Vec<String> = Hub::Inventory
        .kind()
        .pages(&app)
        .into_iter()
        .filter(|p| matches!(p.key, HubPageKey::Structure(_)))
        .map(|p| p.title)
        .collect();
    assert_eq!(titles.len(), 3);
    assert_eq!(
        titles.iter().filter(|t| t.starts_with("Depot (")).count(),
        2
    );
    assert!(titles.iter().any(|t| t == "Factory"), "{titles:?}");
}

#[test]
fn esc_goes_page_then_rail_then_map() {
    let mut app = app_beside_depots(2604, 1, 0, &[("power_cell", 3)]);
    open_structure_page(&mut app);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::Transfer);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.held_structure().is_none());
}

#[test]
fn plus_and_minus_on_inventory_move_stock_and_left_is_the_rail() {
    let mut app = app_beside_depots(2605, 1, 0, &[("power_cell", 3)]);
    open_structure_page(&mut app);
    app.handle_key(GameKey::Char('+'));
    app.handle_key(GameKey::Char('+'));
    assert!(
        app.basket_amounts.contains(&-2),
        "+ puts a unit into the depot"
    );
    app.handle_key(GameKey::Char('-'));
    assert!(app.basket_amounts.contains(&-1), "- takes one back");
    app.handle_key(GameKey::Char('='));
    assert!(app.basket_amounts.contains(&-2), "= is + without Shift");
    let staged = app.basket_amounts.clone();
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "Left goes to the rail");
    assert_eq!(app.basket_amounts, staged, "and moves no stock");
}

#[test]
fn a_turn_keeps_the_cursor_where_it_was() {
    let mut app = app_between_a_depot_and_a_machine(2610);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Char(']'));
    assert!(app.hub_on_rail, "a step from the rail stays on the rail");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('['));
    assert!(!app.hub_on_rail, "a step from the page stays in the page");
}

#[test]
fn walking_away_drops_the_entry_and_the_hold_falls_to_you() {
    let mut app = app_beside_depots(2606, 1, 0, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    stand_in_base_at(&mut app, 9, 9);
    app.settle_hub();
    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("You")));
    assert_eq!(app.mode, Mode::Inventory);
    assert!(app.held_structure().is_none());
}

#[test]
fn losing_the_current_structure_steps_to_the_next() {
    let mut app = app_between_a_depot_and_a_machine(2607);
    app.handle_key(GameKey::Char('c'));
    let machine = held_beside(&app).entity;
    app.game
        .as_mut()
        .unwrap()
        .remove_structure(machine)
        .unwrap();
    app.settle_hub();
    assert_eq!(held_beside(&app).name, "Depot", "the depot remains");
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn losing_the_last_entry_of_the_section_steps_to_the_previous_one() {
    let mut app = app_beside_a_machine_and_two_depots(2609);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Char(']'));
    assert_eq!(page(&app), 5, "precondition: on the last of three");
    let middle = Hub::Inventory.kind().pages(&app)[4].key.clone();
    let last = held_beside(&app).entity;
    app.game.as_mut().unwrap().remove_structure(last).unwrap();
    app.settle_hub();
    assert_eq!(app.hub_page(), Some(&middle));
}

#[test]
fn a_page_whose_structure_is_gone_leaves_for_you_instead_of_keeping_the_last_page() {
    let mut app = app_beside_a_teardown_rig_holding(2610, Vec::new());
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::RigTool, "precondition: the Tool page is up");
    let rig = held_beside(&app).entity;
    app.game.as_mut().unwrap().remove_structure(rig).unwrap();

    app.enter_structure_page(rig, StructurePage::Tool);

    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("You")));
    assert!(app.held_structure().is_none());
    assert!(app.rig_tool.is_none());
}

/// Opens the hub, removes the structure under it and re-enters its `page`.
fn hub_page_after_removal(mut app: App, page: StructurePage) -> App {
    app.handle_key(GameKey::Char('c'));
    let here = held_beside(&app);
    assert!(here.pages.contains(&page), "the structure has that page");
    app.game
        .as_mut()
        .unwrap()
        .remove_structure(here.entity)
        .unwrap();
    app.enter_structure_page(here.entity, page);
    app
}

#[test]
fn a_filter_page_whose_depot_is_gone_leaves_for_you() {
    let app = hub_page_after_removal(app_beside_depots(2611, 1, 0, &[]), StructurePage::Filter);
    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("You")));
    assert!(app.held_structure().is_none());
}

#[test]
fn a_recipe_page_whose_machine_is_gone_leaves_for_you() {
    let app = hub_page_after_removal(
        app_beside_a_machine(2612, "factory", None),
        StructurePage::Recipe,
    );
    assert_eq!(app.hub_page(), Some(&HubPageKey::Named("You")));
    assert!(app.held_structure().is_none());
    assert!(app.machine_recipe.is_none());
}

#[test]
fn a_basket_left_by_a_turn_is_discarded() {
    let mut app = app_between_a_depot_and_a_machine(2608);
    give_player_items(&mut app, &[("power_cell", 3)]);
    open_structure_page(&mut app);
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Char('+'));
    assert!(
        app.basket_amounts.iter().any(|n| *n != 0),
        "something staged"
    );
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Char(']'));
    assert!(app.basket_amounts.iter().all(|n| *n == 0));
}

#[test]
fn i_beside_a_structure_opens_on_it_and_elsewhere_on_its_last_page() {
    let mut app = app_between_a_depot_and_a_machine(2616);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Transfer);
    assert!(app.hub_on_rail);
    let first = app
        .held_structure()
        .expect("opened on the structure beside");
    assert_eq!(held_beside(&app).dir, (1, 0), "the first clockwise, as c");
    // Leave on another page: beside a structure `i` still opens on it.
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.held_structure(), Some(first));

    // With nothing beside, the page left on is the one reopened.
    let mut away = test_app(2617);
    away.handle_key(GameKey::Char('i'));
    away.handle_key(GameKey::Down);
    away.handle_key(GameKey::Esc);
    away.handle_key(GameKey::Char('i'));
    assert_eq!(away.hub_page(), Some(&HubPageKey::Named("Roster")));
}

#[test]
fn c_with_nothing_beside_refuses_as_it_always_has() {
    let mut app = test_app(2618);
    let before = tick(&app);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.held_structure().is_none());
    assert_eq!(tick(&app), before);
}
