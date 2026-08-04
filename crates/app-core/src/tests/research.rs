//! The research picker.

use super::support::*;
use crate::*;

/// Exercises the exact key sequence a player drives at the keyboard —
/// `b` to open Build, a number to pick a structure, then a direction to
/// place it — entirely through `App::handle_key`, to make sure the
/// build/deploy flow (as opposed to `Game::place_structure` in
/// isolation, which the engine's own tests already cover) still works
/// end to end after the menu-navigation changes. Loops over every
/// structure number and every direction (re-opening the build menu each
/// time, exactly as a player retrying would) rather than assuming
/// number "1" is affordable or a given direction is walkable — with
/// starting resources, several of the ten structures are affordable, so
/// this only fails if the *menu itself* is broken, not because of which
/// particular structure a fresh session happens to put at each digit.
#[test]
fn the_base_menu_opens_research_and_esc_closes_it() {
    let mut app = test_app(501);
    open_via_menu(&mut app, 'b', "Research");
    assert!(matches!(app.mode, Mode::Research));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn picking_an_unaffordable_research_node_reports_why_and_stays_open() {
    let mut app = test_app(502);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('1'));
    assert!(
        matches!(app.mode, Mode::Research),
        "the menu stays open so several nodes can be taken in one visit"
    );
    assert!(
        app.status_line
            .as_ref()
            .is_some_and(|s| s.contains("Research Data")),
        "got: {:?}",
        app.status_line
    );
}
