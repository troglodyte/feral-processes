//! The two-page Develop flow: picking a program, then spending on it.

use super::support::*;
use crate::*;

/// A program to develop and three Privilege Rings to spend on it.
fn app_ready_to_develop(seed: u32) -> App {
    app_owning_a_program_and_a_compiler_with_cargo(seed, &[], &[("privilege_ring", 3)])
}

#[test]
fn the_party_menu_opens_the_develop_picker_and_esc_backs_all_the_way_out() {
    let mut app = app_ready_to_develop(80);
    open_via_menu(&mut app, 'p', "Develop a program");
    assert_eq!(app.mode, Mode::Develop);

    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::DevelopProgram);
    assert!(app.pending_develop_target.is_some());

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Develop, "Esc backs out one page");
    assert!(
        app.pending_develop_target.is_none(),
        "and forgets which program was picked, or the next visit develops the old one"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyMenu);
}

/// Through `App::party_menu_rows` rather than a bespoke predicate: rows are
/// hidden dynamically and that table is the only source of them.
#[test]
fn the_develop_row_is_hidden_with_no_programs_to_develop() {
    let mut none = test_app(4080);
    assert!(
        none.game.as_mut().unwrap().owned_pets().is_empty(),
        "test premise: a fresh run owns no programs yet"
    );
    let rows: Vec<_> = none
        .party_menu_rows()
        .iter()
        .map(|r| r.label)
        .collect::<Vec<_>>();
    assert!(
        !rows.contains(&"Develop a program"),
        "nothing to develop is no screen: {rows:?}"
    );

    let mut app = app_ready_to_develop(4080);
    let rows: Vec<_> = app.party_menu_rows().iter().map(|r| r.label).collect();
    assert!(
        rows.contains(&"Develop a program"),
        "one program is enough — the rings can be gone and the ladder still reads: {rows:?}"
    );
}

/// Opening a ring works four frames down, like a refactor: the screen reaches
/// no zone-map state through `Position`.
#[test]
fn opening_a_ring_from_the_screen_spends_the_rings_and_holds_the_page() {
    let mut app = app_ready_to_develop(81);
    open_via_menu(&mut app, 'p', "Develop a program");
    app.handle_key(GameKey::Char('1'));
    let target = app.pending_develop_target.unwrap();

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::DevelopProgram, "the page holds");
    let game = app.game.as_mut().unwrap();
    assert_eq!(
        game.owned_pets()
            .iter()
            .find(|p| p.entity == target)
            .map(|p| p.ring),
        Some(1)
    );
    assert_eq!(game.privilege_rings_held(), 2);
}

#[test]
fn a_refused_ring_lands_in_the_status_line_and_the_page_holds() {
    let mut app = app_owning_a_program_and_a_compiler_with_cargo(82, &[], &[]);
    open_via_menu(&mut app, 'p', "Develop a program");
    app.handle_key(GameKey::Char('1'));

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::DevelopProgram, "a refusal never backs out");
    let status = app.status_line.clone().expect("the refusal has to be said");
    assert!(
        status.contains("Privilege Ring"),
        "and it has to name what is missing, got: {status}"
    );
}
