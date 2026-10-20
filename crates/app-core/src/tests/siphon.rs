//! `Mode::Siphon`: the Siphon page of a Power Siphon's hub.

use super::support::*;
use crate::*;

const SIPHON: &str = "power_siphon";

/// A base with a staff program and a siphon, the party standing beside it.
fn app_beside_a_siphon(seed: u32) -> App {
    let mut app = app_owning_a_program_and_a_station_of("siphon", SIPHON, seed);
    stand_beside_the_extractor(&mut app);
    app
}

fn siphon_beside(game: &Game) -> Entity {
    game.adjacent_structures()
        .into_iter()
        .find(|s| s.pages.contains(&StructurePage::Siphon))
        .expect("a siphon beside the party")
        .entity
}

fn the_siphon(app: &App) -> Entity {
    siphon_beside(app.game.as_ref().unwrap())
}

fn pet_hp(game: &mut Game, program: Entity) -> i32 {
    game.owned_pets()
        .into_iter()
        .find(|p| p.entity == program)
        .expect("the program is still owned")
        .hp
}

#[test]
fn c_beside_a_structure_with_no_siphon_page_opens_no_siphon() {
    let mut app = app_owning_a_program_and_a_station_of("no_siphon", "research_node", 9101);
    stand_beside_the_extractor(&mut app);

    open_structure_page(&mut app);

    assert_ne!(app.mode, Mode::Siphon);
    assert!(app.siphon.is_none());
}

#[test]
fn c_at_an_empty_siphon_opens_the_picker_and_a_pick_holds_the_program() {
    let mut app = app_beside_a_siphon(9102);
    let program = app.game.as_mut().unwrap().base_staff()[0];

    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Siphon);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::Playing);
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.program_role(program), Some(ProgramRole::Siphoned));
    assert_eq!(game.siphon_holder(siphon_beside(game)), Some(program));
}

#[test]
fn c_at_an_occupied_siphon_offers_release_and_confirming_releases() {
    let mut app = app_beside_a_siphon(9103);
    let program = app.game.as_mut().unwrap().base_staff()[0];
    let siphon = the_siphon(&app);
    app.game
        .as_mut()
        .unwrap()
        .siphon_program(program, siphon)
        .unwrap();
    let hp_before = pet_hp(app.game.as_mut().unwrap(), program);

    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Siphon);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::Playing);
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.siphon_holder(siphon), None);
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert!(pet_hp(game, program) < hp_before, "it comes out hurt");
}

#[test]
fn esc_leaves_the_siphon_screen_writing_nothing() {
    let mut app = app_beside_a_siphon(9104);
    let program = app.game.as_mut().unwrap().base_staff()[0];
    let siphon = the_siphon(&app);
    let log_before = app.game.as_ref().unwrap().message_history(usize::MAX).len();

    open_structure_page(&mut app);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.siphon.is_none());
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.siphon_holder(siphon), None);
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert_eq!(game.message_history(usize::MAX).len(), log_before);
}

#[test]
fn a_siphon_screen_leaves_when_the_party_is_no_longer_beside_it() {
    let mut app = app_beside_a_siphon(9106);
    let program = app.game.as_mut().unwrap().base_staff()[0];
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::Siphon);

    // Walked away while the screen was open.
    stand_in_base_at(&mut app, 40, 40);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.siphon.is_none());
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
}

#[test]
fn p_is_unbound_in_the_stack_block() {
    let mut app = app_underground(9105);
    assert_eq!(app.mode, Mode::Playing);
    let status_before = app.status_line.clone();

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.status_line, status_before,
        "no refusal: the key is not bound"
    );
}
