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

/// The wielded row, if any — `PetInfo::wielded` is what the `(WEP)` tag is
/// drawn from, so app-core can assert the state without a renderer.
fn wielded_row(app: &mut App) -> Option<feral_processes_engine::Entity> {
    app.game
        .as_mut()
        .unwrap()
        .owned_pets()
        .into_iter()
        .find(|p| p.wielded)
        .map(|p| p.entity)
}

#[test]
fn the_hidden_key_wields_the_highlighted_program() {
    let mut app = app_with_companions_in_the_party(770, 2);
    let before = roster(&mut app);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);

    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None, "the wield should not be refused");
    assert_eq!(
        wielded_row(&mut app),
        Some(before[1]),
        "the highlighted program goes in hand"
    );
    assert_eq!(
        app.mode,
        Mode::Companion,
        "the screen stays open the way reordering does"
    );
}

#[test]
fn the_hidden_key_unwields_when_pressed_on_the_program_already_in_hand() {
    let mut app = app_with_companions_in_the_party(771, 2);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Char('W'));
    let wielded = wielded_row(&mut app).expect("the first press wields");

    // Wielding stands the program down from the party, so `owned_pets`
    // reorders — find the row it moved to rather than assuming it stayed.
    let row = app
        .game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .position(|p| p.entity == wielded)
        .unwrap();
    app.menu_selected = row;
    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None);
    assert_eq!(wielded_row(&mut app), None, "the second press puts it down");
}

#[test]
fn the_hidden_key_is_ignored_on_an_empty_roster() {
    let mut app = test_app(772);
    // Set directly: with no programs the party menu hides the row entirely,
    // so the screen is unreachable through it — which is the point. The key
    // still must not panic if the screen is somehow open on nothing.
    app.mode = Mode::Companion;

    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None, "no roster, nothing to say about it");
    assert_eq!(app.mode, Mode::Companion);
}
