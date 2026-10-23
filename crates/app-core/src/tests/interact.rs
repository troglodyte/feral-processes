//! `[c]` as the interact key: it opens the structure hub for what is beside
//! the party, refuses when there is nothing, and the removed map keys stay
//! removed.

use super::support::*;
use crate::*;

fn site_count(app: &mut App) -> usize {
    app.game.as_ref().unwrap().awaiting_program_sites().len()
}

#[test]
fn c_beside_one_depot_opens_the_transfer_screen() {
    let mut app = app_beside_depots(2401, 1, 0, &[("power_cell", 3)]);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn c_with_nothing_beside_the_party_stays_on_the_map() {
    let mut app = app_in_base_with_programs(2402, 2);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.structure_hub.is_none());
}

#[test]
fn c_beside_one_awaiting_site_opens_the_picker_and_commits() {
    let mut app = app_beside_a_rebuild_site(2403, false);
    assert_eq!(site_count(&mut app), 1);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::BuildProgram);
    assert!(matches!(
        app.pending_build,
        Some(PendingBuild::Rebuild { .. })
    ));
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(site_count(&mut app), 0, "the program was committed");
    assert!(app.pending_build.is_none());
}

#[test]
fn escaping_the_rebuild_picker_commits_nothing() {
    let mut app = app_beside_a_rebuild_site(2404, false);
    open_structure_page(&mut app);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_build.is_none());
    assert!(app.structure_hub.is_none());
    assert_eq!(site_count(&mut app), 1);
}

#[test]
fn c_with_a_depot_and_a_site_opens_the_site_east_then_tab_reaches_the_depot() {
    let mut app = app_beside_a_rebuild_site(2405, true);
    open_structure_page(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "the site is east, first clockwise"
    );

    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Transfer, "the Depot is west");
}

#[test]
fn c_with_a_depot_and_a_machine_opens_the_machine_and_its_recipe_is_a_page() {
    let mut app = app_between_a_depot_and_a_machine(2407);
    open_structure_page(&mut app);
    assert_eq!(
        app.mode,
        Mode::Transfer,
        "the Factory is east: its shelf first"
    );

    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::MachineRecipe);
    assert!(app.machine_recipe.is_some());

    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Transfer, "the Depot is west");
}

#[test]
fn c_beside_two_rebuild_sites_opens_one_and_tab_the_other() {
    let mut app = app_beside_two_rebuild_sites(2409);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::BuildProgram);
    let first = app.structure_hub.as_ref().unwrap().current().entity;
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::BuildProgram);
    assert_ne!(app.structure_hub.as_ref().unwrap().current().entity, first);
}

#[test]
fn the_cycle_goes_clockwise_whichever_structure_sorts_first_by_tile() {
    let mut app = app_beside_a_machine_west_and_a_depot_east(2410);
    open_structure_page(&mut app);
    assert_eq!(
        app.mode,
        Mode::Transfer,
        "the Depot is east, first clockwise"
    );
    assert_eq!(app.structure_hub.as_ref().unwrap().current().name, "Depot");

    app.handle_key(GameKey::Tab);
    assert_eq!(app.structure_hub.as_ref().unwrap().current().dir, (-1, 0));
}

#[test]
fn a_set_machine_holding_output_opens_on_its_shelf_with_the_recipe_a_page_on() {
    let mut app = app_beside_a_set_machine_holding_output(2408);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Transfer);
    assert_eq!(app.basket_rows.len(), 1, "the three blocks on its shelf");

    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::MachineRecipe);
}

/// The rig tool, siphon and splice keys moved into the hub as pages.
#[test]
fn f_p_and_i_on_the_map_do_nothing() {
    let mut app = app_beside_depots(2412, 1, 0, &[]);
    for key in ['F', 'P', 'I'] {
        app.handle_key(GameKey::Char(key));
        assert_eq!(app.mode, Mode::Playing, "{key}");
        assert!(app.status_line.is_none(), "{key}");
    }
}
