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

/// A structure the current zone caps at Mk1 is still offered, not filtered
/// out. Hiding it would take the whole Upgrade row out of the base menu for
/// the entirety of zone 1 — see `app/group_menu.rs`, which drops a row whose
/// screen would be empty — and a player who has never breached would never
/// learn that upgrading exists. The row explains itself instead, which is
/// what `EntityView::ceiling` is carried for.
#[test]
fn a_structure_at_its_zone_ceiling_is_still_listed_with_the_ceiling_shown() {
    let mut app = app_owning_a_program_and_a_compiler(232, &[]);

    let listed = app.upgradeable_structures();
    let compiler = listed
        .iter()
        .find(|e| e.label.contains("Compiler"))
        .expect("a zone-capped structure must still be offered, not hidden");

    assert_eq!(compiler.tier, Some(1));
    assert_eq!(
        compiler.ceiling,
        Some(1),
        "zone 1 caps the Compiler at Mk1 even though its def allows Mk5"
    );
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

/// A companion's `Position` is the tile it was captured on and is never
/// written again, so the posting picker must offer the whole roster rather
/// than a window onto the map. It used to scan `MENU_SCAN_RADIUS`, which hid
/// every program tamed further out than that — and because `base_menu_rows`
/// drops a row whose first screen would be empty, a player whose only
/// program was tamed that far away lost the Cronjob row with it.
#[test]
fn the_posting_picker_offers_programs_parked_far_from_the_player() {
    // The fixture parks both of these beyond `MENU_SCAN_RADIUS`.
    let mut app = app_owning_distant_programs(741, 2);
    let roster = app.game.as_mut().unwrap().owned_pets().len();
    assert!(
        roster >= 2,
        "fixture should hand the player at least the two distant programs"
    );

    let offered = app.nearby_programs();

    assert_eq!(
        offered.len(),
        roster,
        "the picker must offer every program the player owns, wherever it was tamed"
    );
    assert!(
        offered.iter().all(|v| v.is_tamed),
        "and still list only tamed programs"
    );
}

/// `d` + a direction demolishes what is on the neighbouring tile, without
/// going through the base menu's picker at all.
#[test]
fn the_demolish_key_removes_the_structure_next_to_you() {
    let mut app = app_inside_a_small_base(240, false);
    assert_eq!(
        structure_count(&mut app),
        2,
        "precondition: Home and a node"
    );

    app.handle_key(GameKey::Char('d'));
    assert_eq!(
        app.mode,
        Mode::RemoveDirection,
        "the key opens a direction prompt rather than a list"
    );

    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        1,
        "the node east of the player should be gone"
    );
}

/// Home cascades to the whole base, so the hotkey routes it into the same
/// warning the menu does rather than taking the base down on one keypress.
#[test]
fn the_demolish_key_still_asks_before_taking_down_home() {
    let mut app = app_inside_a_small_base(241, false);

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Up);
    assert_eq!(
        app.mode,
        Mode::RemoveConfirm,
        "Home must reach the confirmation screen, not be demolished outright"
    );
    assert_eq!(structure_count(&mut app), 2, "nothing is gone yet");

    app.handle_key(GameKey::Char('y'));
    assert_eq!(
        structure_count(&mut app),
        0,
        "confirming takes Home and the cascade with it"
    );
}

#[test]
fn the_demolish_key_says_when_there_is_nothing_that_way() {
    let mut app = app_inside_a_small_base(242, false);

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Down);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(structure_count(&mut app), 2, "nothing was demolished");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("Nothing to demolish")),
        "an empty neighbour has to say so: {:?}",
        app.status_line
    );
}

/// `Position` is pinned to the surface entrance tile underground, so a
/// direction key down there would aim at the base overhead. Refused at the
/// keypress, matching the `surface_only` flag the menu's Demolish row carries.
#[test]
fn the_demolish_key_is_refused_underground() {
    let mut app = app_inside_a_small_base(243, true);
    assert!(
        app.game.as_ref().is_some_and(|g| g.is_underground()),
        "precondition: the fixture really went down"
    );

    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the direction prompt must not even open down here"
    );
    assert!(
        app.status_line.is_some(),
        "a refused key says why rather than doing nothing"
    );
}
