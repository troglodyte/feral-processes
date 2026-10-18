//! `[c]` as the interact key: one thing beside the party acts at once,
//! several ask which side, nothing is refused as before.

use super::support::*;
use crate::*;

fn site_count(app: &mut App) -> usize {
    app.game.as_ref().unwrap().awaiting_program_sites().len()
}

#[test]
fn c_beside_one_depot_opens_the_transfer_screen() {
    let mut app = app_beside_depots(2401, 1, 0, &[("power_cell", 3)]);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer);
}

#[test]
fn c_with_nothing_beside_the_party_stays_on_the_map() {
    let mut app = app_in_base_with_programs(2402, 2);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn c_beside_one_awaiting_site_opens_the_picker_and_commits() {
    let mut app = app_beside_a_rebuild_site(2403, false);
    assert_eq!(site_count(&mut app), 1);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::BuildProgram);
    assert!(matches!(
        app.pending_build,
        Some(PendingBuild::Rebuild { .. })
    ));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(site_count(&mut app), 0, "the program was committed");
    assert!(app.pending_build.is_none());
}

#[test]
fn escaping_the_rebuild_picker_commits_nothing() {
    let mut app = app_beside_a_rebuild_site(2404, false);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_build.is_none());
    assert_eq!(site_count(&mut app), 1);
}

#[test]
fn c_with_a_depot_and_a_site_asks_which_side_and_routes_each() {
    let mut app = app_beside_a_rebuild_site(2405, true);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);

    app.handle_key(GameKey::Left);
    assert_eq!(app.mode, Mode::Transfer, "the Depot is west");
    app.handle_key(GameKey::Esc);

    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::BuildProgram, "the site is east");
}

#[test]
fn a_side_with_nothing_is_refused_and_esc_backs_out() {
    let mut app = app_beside_a_rebuild_site(2406, true);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.status_line.is_some());

    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn c_with_a_depot_and_a_machine_asks_which_side_and_the_machine_opens_its_recipe() {
    let mut app = app_between_a_depot_and_a_machine(2407);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);

    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::MachineRecipe, "the Factory is east");
    assert!(app.machine_recipe.is_some());
    app.handle_key(GameKey::Esc);

    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Left);
    assert_eq!(app.mode, Mode::Transfer, "the Depot is west");
}

#[test]
fn on_a_shared_side_the_arrow_opens_the_machines_recipe_and_c_again_moves_cargo() {
    let mut app = app_beside_a_set_machine_holding_output(2408);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(
        app.mode,
        Mode::InteractDirection,
        "cargo and a recipe, both east"
    );

    app.handle_key(GameKey::Right);
    assert_eq!(
        app.mode,
        Mode::MachineRecipe,
        "the machine wins its own side"
    );
    app.handle_key(GameKey::Esc);

    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Transfer, "c again is the cargo");
}

#[test]
fn c_again_with_no_cargo_beside_the_party_is_refused() {
    let mut app = app_beside_two_rebuild_sites(2409);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);

    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.status_line.as_deref(), Some("No cargo to move here."));
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .message_history(usize::MAX)
            .iter()
            .any(|l| l.text == "No cargo to move here."),
        "the refusal went through App::refuse, so the log has it too"
    );
}

/// The transfer is aimed at the Depot even when a machine sorts before it,
/// so the arrow toward a visible Depot is never refused.
#[test]
fn the_arrow_toward_a_depot_opens_the_transfer_when_the_machine_sorts_first() {
    let mut app = app_beside_a_machine_west_and_a_depot_east(2410);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::InteractDirection);

    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Transfer, "the Depot is east");
    app.handle_key(GameKey::Esc);

    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Left);
    assert_eq!(app.mode, Mode::MachineRecipe, "the Factory is west");
}
