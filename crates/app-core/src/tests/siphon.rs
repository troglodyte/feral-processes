//! `Mode::Siphon`: `[P]` beside a Power Siphon in base space.

use super::support::*;
use crate::*;

const SIPHON: &str = "power_siphon";

/// A base with a staff program and a siphon, the party standing beside it.
fn app_beside_a_siphon(seed: u32) -> App {
    let mut app = app_owning_a_program_and_a_station_of("siphon", SIPHON, seed);
    stand_beside_the_compiler(&mut app);
    app
}

fn the_siphon(app: &App) -> Entity {
    app.game.as_ref().unwrap().adjacent_siphons()[0]
}

fn pet_hp(game: &mut Game, program: Entity) -> i32 {
    game.owned_pets()
        .into_iter()
        .find(|p| p.entity == program)
        .expect("the program is still owned")
        .hp
}

fn log_has(app: &App, fragment: &str) -> bool {
    app.game
        .as_ref()
        .unwrap()
        .message_history(usize::MAX)
        .iter()
        .any(|l| l.text.contains(fragment))
}

#[test]
fn p_at_no_siphon_refuses_and_logs() {
    let mut app = app_owning_a_program_and_a_station_of("no_siphon", "research_node", 9101);
    stand_beside_the_compiler(&mut app);

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.status_line.as_deref(),
        Some("There is no Power Siphon here.")
    );
    assert!(log_has(&app, "There is no Power Siphon here."));
}

#[test]
fn p_at_an_empty_siphon_opens_the_picker_and_a_pick_holds_the_program() {
    let mut app = app_beside_a_siphon(9102);
    let program = app.game.as_mut().unwrap().base_staff()[0];

    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.mode, Mode::Siphon);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::Playing);
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.program_role(program), Some(ProgramRole::Siphoned));
    assert_eq!(
        game.siphon_holder(game.adjacent_siphons()[0]),
        Some(program)
    );
}

#[test]
fn p_at_an_occupied_siphon_offers_release_and_confirming_releases() {
    let mut app = app_beside_a_siphon(9103);
    let program = app.game.as_mut().unwrap().base_staff()[0];
    let siphon = the_siphon(&app);
    app.game
        .as_mut()
        .unwrap()
        .siphon_program(program, siphon)
        .unwrap();
    let hp_before = pet_hp(app.game.as_mut().unwrap(), program);

    app.handle_key(GameKey::Char('P'));
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

    app.handle_key(GameKey::Char('P'));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.siphon.is_none());
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.siphon_holder(siphon), None);
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert_eq!(game.message_history(usize::MAX).len(), log_before);
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
