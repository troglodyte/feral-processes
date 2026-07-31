//! Placing, demolishing and upgrading structures through the menus.

use super::support::*;
use crate::*;

#[test]
fn pressing_u_opens_the_upgrade_picker_and_esc_closes_it() {
    let mut app = test_app(230);

    app.handle_key(GameKey::Char('U'));
    assert!(
        app.mode == Mode::Upgrade,
        "'U' should open the upgrade menu"
    );

    app.handle_key(GameKey::Esc);
    assert!(app.mode == Mode::Playing, "Esc should return to play");
}

#[test]
fn the_upgrade_picker_skips_structures_with_no_upgrade_path() {
    let mut app = test_app(231);

    // Home is the first entry in the build menu and declares no upgrade
    // path — same b/Enter/Up sequence the remove-flow test drives.
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Up);
    assert_eq!(structure_count(&mut app), 1, "Home should now be deployed");

    app.handle_key(GameKey::Char('U'));
    assert!(app.mode == Mode::Upgrade);
    app.handle_key(GameKey::Enter);
    assert!(
        app.mode == Mode::Upgrade,
        "with nothing upgradeable nearby the picker has no entry to select, so Enter \
         should leave the player in the menu rather than firing a doomed upgrade"
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

            app.handle_key(GameKey::Char('b'));
            assert!(app.mode == Mode::Build, "'b' should open the build menu");

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

/// Exercises the `R` demolish flow end to end through `App::handle_key`:
/// picking Home moves to a confirmation step instead of demolishing
/// immediately (unlike any other structure — see `Game::remove_structure`
/// for why Home is special), `n` backs out leaving it standing, and `y`
/// actually demolishes it.
#[test]
fn remove_key_on_home_requires_confirmation_before_demolishing() {
    let mut app = test_app(203);

    app.handle_key(GameKey::Char('b'));
    assert!(app.mode == Mode::Build, "'b' should open the build menu");
    app.handle_key(GameKey::Enter);
    assert!(
        app.mode == Mode::BuildDirection,
        "Home is the first entry in the build menu"
    );
    app.handle_key(GameKey::Up);
    assert!(app.mode == Mode::Playing);
    assert_eq!(structure_count(&mut app), 1, "Home should now be deployed");

    app.handle_key(GameKey::Char('R'));
    assert!(
        app.mode == Mode::Remove,
        "'R' should open the demolish menu"
    );
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

    app.handle_key(GameKey::Char('R'));
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

/// `w` posts a program to a node; `W` is the same job done yourself, so it
/// offers the same `can_work` list rather than a second kind of screen.
#[test]
fn working_a_structure_yourself_opens_the_same_structure_list() {
    let mut app = test_app(960);
    app.handle_key(GameKey::Char('W'));
    assert_eq!(app.mode, Mode::WorkStructure);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "Esc should back out to the map");
}
