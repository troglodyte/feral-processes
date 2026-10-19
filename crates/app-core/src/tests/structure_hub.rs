//! `[c]` as the structure hub: one screen per structure beside the party,
//! opened in one press and cycled with `Tab`.

use super::support::*;
use crate::*;

fn current_name(app: &App) -> String {
    app.structure_hub
        .as_ref()
        .map(|h| h.current().name.clone())
        .unwrap_or_default()
}

fn tick(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

#[test]
fn c_beside_a_depot_opens_on_its_inventory_in_one_press() {
    let mut app = app_beside_depots(2601, 1, 0, &[("power_cell", 3)]);
    let before = tick(&app);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
    assert!(!app.hub_on_rail, "the cursor starts in the page");
    assert_eq!(app.hub_at(), Some((Hub::Structure, 0, 0)));
    assert_eq!(current_name(&app), "Depot");
    assert_eq!(tick(&app), before, "opening the hub spends no turn");
}

#[test]
fn tab_walks_the_structures_clockwise_and_wraps() {
    // Factory east, Depot west: N, E, S, W puts the machine first.
    let mut app = app_between_a_depot_and_a_machine(2602);
    app.handle_key(GameKey::Char('c'));
    let hub = app.structure_hub.clone().expect("hub open");
    assert_eq!(hub.cycle.len(), 2);
    assert_eq!(hub.current().dir, (1, 0));
    let machine = hub.current().entity;
    assert_eq!(app.mode, Mode::Transfer);

    app.handle_key(GameKey::Tab);
    let hub = app.structure_hub.clone().unwrap();
    assert_eq!(hub.current().dir, (-1, 0));
    assert_eq!(
        app.mode,
        Mode::Transfer,
        "the depot opens on its first page"
    );

    app.handle_key(GameKey::Tab);
    assert_eq!(
        app.structure_hub.as_ref().unwrap().current().entity,
        machine
    );
}

#[test]
fn tab_lands_on_the_next_structures_first_page() {
    let mut app = app_between_a_depot_and_a_machine(2603);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::MachineRecipe, "the machine's second page");
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn esc_goes_page_then_rail_then_map() {
    let mut app = app_beside_depots(2604, 1, 0, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::Transfer);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.structure_hub.is_none());
}

#[test]
fn left_and_right_on_inventory_move_stock_not_pages() {
    let mut app = app_beside_depots(2605, 1, 0, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Transfer);
    assert!(!app.hub_on_rail);
    assert!(
        app.basket_amounts.iter().any(|n| *n < 0),
        "Right puts a unit into the depot"
    );
}

#[test]
fn walking_away_closes_the_hub() {
    let mut app = app_beside_depots(2606, 1, 0, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    stand_in_base_at(&mut app, 9, 9);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.structure_hub.is_none());
}

#[test]
fn losing_the_current_structure_steps_to_the_next() {
    let mut app = app_between_a_depot_and_a_machine(2607);
    app.handle_key(GameKey::Char('c'));
    let machine = app.structure_hub.as_ref().unwrap().current().entity;
    app.game
        .as_mut()
        .unwrap()
        .remove_structure(machine)
        .unwrap();
    app.handle_key(GameKey::Up);
    let hub = app.structure_hub.as_ref().expect("the depot remains");
    assert_eq!(hub.cycle.len(), 1);
    assert_eq!(hub.current().name, "Depot");
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn losing_the_last_structure_in_the_cycle_wraps_to_the_first() {
    let mut app = app_beside_a_machine_and_two_depots(2609);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Tab);
    let hub = app.structure_hub.as_ref().unwrap();
    assert_eq!(hub.at, 2, "precondition: on the last of three");
    let last = hub.current().entity;
    app.game.as_mut().unwrap().remove_structure(last).unwrap();
    app.handle_key(GameKey::Up);
    let hub = app.structure_hub.as_ref().expect("two remain");
    assert_eq!(hub.cycle.len(), 2);
    assert_eq!(hub.at, 0, "wrapped, not left on the previous one");
}

#[test]
fn a_page_whose_structure_is_gone_closes_the_hub_instead_of_keeping_the_last_page() {
    let mut app = app_beside_a_teardown_rig_holding(2610, Vec::new());
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::RigTool, "precondition: the Tool page is up");
    let rig = app.structure_hub.as_ref().unwrap().current().entity;
    app.game.as_mut().unwrap().remove_structure(rig).unwrap();

    app.enter_structure_page(0);

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.structure_hub.is_none());
    assert!(app.rig_tool.is_none());
}

#[test]
fn a_basket_left_by_tab_is_discarded() {
    let mut app = app_between_a_depot_and_a_machine(2608);
    give_player_items(&mut app, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Right);
    assert!(
        app.basket_amounts.iter().any(|n| *n != 0),
        "something staged"
    );
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Tab);
    assert!(app.basket_amounts.iter().all(|n| *n == 0));
}
