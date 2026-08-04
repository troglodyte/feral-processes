//! Placing, demolishing and upgrading structures through the menus.

use super::support::*;
use crate::*;

#[test]
fn the_upgrade_picker_opens_from_the_base_menu_and_esc_backs_into_it() {
    // A Compiler, not a Home: the row is hidden unless something nearby
    // actually declares an upgrade path (see `App::upgradeable_structures`).
    let mut app = app_owning_a_program_and_a_compiler(230, &[]);

    open_via_menu(&mut app, 'b', "Upgrade a structure");
    assert_eq!(app.mode, Mode::Upgrade);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "and out to the map from there");
}

/// Home is the first entry in the build menu and declares no upgrade path,
/// which makes it the fixture for "deployed, but nothing to upgrade".
fn deploy_home(app: &mut App) {
    open_via_menu(app, 'b', "Deploy a structure");
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Up);
    assert_eq!(structure_count(app), 1, "Home should now be deployed");
}

/// Home declares no upgrade path, so a base consisting only of one leaves
/// nothing to upgrade — and the base menu now says so by not offering the
/// row at all, rather than opening a picker with no entries in it.
#[test]
fn a_structure_with_no_upgrade_path_hides_the_upgrade_row() {
    let mut app = test_app(231);
    deploy_home(&mut app);

    app.handle_key(GameKey::Char('b'));
    let rows: Vec<_> = app.base_menu_rows().iter().map(|r| r.label).collect();
    assert!(
        rows.contains(&"Demolish a structure"),
        "Home is deployed, so the rows that take any structure are live: {rows:?}"
    );
    assert!(
        !rows.contains(&"Upgrade a structure"),
        "Home has no upgrade path, so the row leads nowhere: {rows:?}"
    );
}

fn structure_count(app: &mut App) -> usize {
    app.game
        .as_mut()
        .unwrap()
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .count()
}

#[test]
fn build_menu_number_key_reaches_the_direction_picker_and_can_place_a_structure() {
    let mut app = test_app(101);
    assert!(app.game.is_some(), "test game should have loaded");
    assert!(app.mode == Mode::Playing);

    let structure_count_in_menu = app.game.as_mut().unwrap().buildable_structure_defs().len();
    let mut placed = false;
    // Navigate with Down + Enter rather than a digit key, both to
    // exercise the new arrow-navigation path and because a menu with
    // more than 9 rows can't be reached by a single digit at all.
    'outer: for n in 0..structure_count_in_menu {
        for dir in [GameKey::Up, GameKey::Down, GameKey::Left, GameKey::Right] {
            let before = structure_count(&mut app);

            open_via_menu(&mut app, 'b', "Deploy a structure");
            assert!(app.mode == Mode::Build, "the base menu should reach Deploy");

            for _ in 0..n {
                app.handle_key(GameKey::Down);
            }
            app.handle_key(GameKey::Enter);
            assert!(
                app.mode == Mode::BuildDirection,
                "picking structure {n} via Down+Enter should move to the direction picker"
            );

            app.handle_key(dir);
            assert!(
                app.mode == Mode::Playing,
                "the direction picker should return to Playing either way"
            );

            if structure_count(&mut app) > before {
                placed = true;
                break 'outer;
            }
        }
    }
    assert!(
        placed,
        "should have been able to place at least one of the {structure_count_in_menu} structures \
         in at least one of the four directions"
    );
}

/// Exercises the demolish flow end to end through `App::handle_key`:
/// picking Home moves to a confirmation step instead of demolishing
/// immediately (unlike any other structure — see `Game::remove_structure`
/// for why Home is special), `n` backs out leaving it standing, and `y`
/// actually demolishes it.
#[test]
fn remove_key_on_home_requires_confirmation_before_demolishing() {
    let mut app = test_app(203);

    deploy_home(&mut app);

    open_via_menu(&mut app, 'b', "Demolish a structure");
    assert_eq!(app.mode, Mode::Remove);
    app.handle_key(GameKey::Enter);
    assert!(
        app.mode == Mode::RemoveConfirm,
        "picking Home should require confirmation instead of demolishing immediately"
    );
    assert_eq!(
        structure_count(&mut app),
        1,
        "Home shouldn't be removed yet"
    );

    app.handle_key(GameKey::Char('n'));
    assert!(app.mode == Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        1,
        "declining the warning should leave Home in place"
    );

    open_via_menu(&mut app, 'b', "Demolish a structure");
    app.handle_key(GameKey::Enter);
    assert!(app.mode == Mode::RemoveConfirm);
    app.handle_key(GameKey::Char('y'));
    assert!(app.mode == Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        0,
        "confirming should demolish Home"
    );
}

/// A cronjob posts a program to a node; working it yourself is the same job,
/// so it offers the same `App::workable_structures` list rather than a second
/// kind of screen.
#[test]
fn working_a_structure_yourself_opens_the_same_structure_list() {
    let mut app = app_owning_a_program_and_a_compiler(960, &[]);
    open_via_menu(&mut app, 'b', "Work a structure yourself");
    assert_eq!(app.mode, Mode::WorkStructure);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc backs into the base menu");
}
