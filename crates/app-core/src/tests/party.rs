//! Reordering the battle line from the party screen.

use super::support::*;
use crate::*;

fn roster(app: &mut App) -> Vec<feral_processes_engine::Entity> {
    app.game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .map(|p| p.entity)
        .collect()
}

#[test]
fn the_angle_keys_reorder_the_party_and_the_highlight_follows_the_member() {
    let mut app = app_with_companions_in_the_party(760, 2);
    let before = roster(&mut app);

    open_via_menu(&mut app, 'p', "Companions");
    assert_eq!(app.mode, Mode::Companion);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1, "the second slot is highlighted");

    app.handle_key(GameKey::Char('<'));

    assert_eq!(
        roster(&mut app),
        vec![before[1], before[0]],
        "'<' takes the slot ahead"
    );
    assert_eq!(app.status_line, None, "the move should not be refused");
    assert_eq!(
        app.mode,
        Mode::Companion,
        "reordering leaves the screen open to keep shuffling"
    );
    assert_eq!(
        app.menu_selected, 0,
        "the highlight rides the member it moved, so a second press keeps \
         pushing the same program rather than the one it displaced"
    );

    app.handle_key(GameKey::Char('>'));
    assert_eq!(roster(&mut app), before, "'>' puts it back");
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn the_lead_member_cannot_be_pushed_off_the_front_of_the_line() {
    let mut app = app_with_companions_in_the_party(761, 2);
    let before = roster(&mut app);

    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Char('<'));

    assert!(
        app.status_line.is_some(),
        "the refusal is reported rather than swallowed"
    );
    assert_eq!(roster(&mut app), before, "and the order is untouched");
    assert_eq!(app.menu_selected, 0, "the highlight stays put on a refusal");
}
