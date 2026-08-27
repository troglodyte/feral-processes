//! The two-page Develop flow: picking a program, then spending on it.

use super::support::*;
use crate::*;
use feral_processes_engine::Entity;
use feral_processes_engine::tuning::TALENT_START_LEVEL;

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

/// A program two levels past the base cap, so it has two points to spend.
fn app_ready_to_spend(seed: u32) -> App {
    let mut app = app_owning_a_developed_program(seed, TALENT_START_LEVEL + 2, 1);
    open_via_menu(&mut app, 'p', "Develop a program");
    app.handle_key(GameKey::Char('1'));
    app
}

fn takeable(app: &mut App, target: Entity) -> Vec<String> {
    app.game
        .as_mut()
        .unwrap()
        .talent_options(target)
        .into_iter()
        .filter(|o| o.takeable)
        .map(|o| o.id.to_string())
        .collect()
}

#[test]
fn picking_a_takeable_node_buys_it() {
    let mut app = app_ready_to_spend(83);
    let target = app.pending_develop_target.unwrap();
    let offered = takeable(&mut app, target);
    assert_eq!(offered.len(), 2, "tier 1 offers a decision");

    app.handle_key(GameKey::Char('1'));

    let game = app.game.as_mut().unwrap();
    assert_eq!(
        game.talent_options(target)
            .into_iter()
            .filter(|o| o.taken)
            .map(|o| o.id.to_string())
            .collect::<Vec<_>>(),
        vec![offered[0].clone()],
        "the row the player was looking at is the one bought"
    );
    assert_eq!(game.owned_pets()[0].talents, 1);
}

#[test]
fn picking_a_node_with_no_points_lands_in_the_status_line_and_holds_the_page() {
    let mut app = app_owning_a_developed_program(84, TALENT_START_LEVEL, 1);
    open_via_menu(&mut app, 'p', "Develop a program");
    app.handle_key(GameKey::Char('1'));
    let target = app.pending_develop_target.unwrap();
    assert!(
        takeable(&mut app, target).is_empty(),
        "test premise: a capped program has earned nothing to spend"
    );

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::DevelopProgram, "the page holds");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("talent point")),
        "a player pressing a key on a greyed row has to be told why: {:?}",
        app.status_line
    );
}

/// Both verbs on one page. Opening a ring and spending the point it earns are
/// the same decision loop, and a later refactor must not quietly split them.
#[test]
fn the_ring_and_the_ladder_are_reachable_from_the_same_page() {
    let mut app = app_ready_to_spend(85);
    let target = app.pending_develop_target.unwrap();

    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::DevelopProgram);
    assert_eq!(app.game.as_mut().unwrap().owned_pets()[0].ring, 2);

    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::DevelopProgram);
    assert_eq!(app.game.as_mut().unwrap().owned_pets()[0].talents, 1);
    assert!(
        !takeable(&mut app, target).is_empty(),
        "one point left, so tier 2 is on offer without leaving the page"
    );
}
