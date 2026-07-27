use super::support::*;
use crate::*;

#[test]
fn m_opens_the_routine_target_picker_and_esc_backs_all_the_way_out() {
    let mut app = test_app(61);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Char('1')); // "You"
    assert_eq!(app.mode, Mode::Routines);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn picking_a_filled_slot_uninstalls_and_picking_an_empty_one_opens_the_install_list() {
    let mut app = test_app(62);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('1')); // You — slot 1 holds decompile
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Routines, "uninstalling stays on the panel");
    let game = app.game.as_mut().unwrap();
    // `routine_holders()[0]` is the player by construction (see
    // `Game::routine_holders`) — `Game::player_entity` is `pub(crate)`, so
    // this is the entity-facing route a real caller has too.
    let player = game.routine_holders()[0].entity;
    assert!(
        game.routine_view(player)[0].ability.is_none(),
        "decompile should have been popped out"
    );

    app.handle_key(GameKey::Char('1')); // now an empty slot
    assert_eq!(app.mode, Mode::RoutineInstall);
}

#[test]
fn the_extract_flow_requires_confirmation_before_the_program_is_destroyed() {
    let mut app = app_owning_a_program_and_a_compiler(63);
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.mode, Mode::Extract);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ExtractPick);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ExtractConfirm);

    let before = app.game.as_mut().unwrap().owned_pets().len();
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before,
        "backing out must not destroy the program"
    );
    // Esc backs out one page at a time (same as `TradeProgramConfirm`):
    // ExtractConfirm -> ExtractPick -> Extract -> Playing, three Escs to
    // fully unwind. Asserting each step is what makes the following `M`
    // meaningful — pressed from anywhere but `Playing` it would be
    // swallowed by that mode's own numbered-menu handler and prove nothing.
    assert_eq!(app.mode, Mode::ExtractPick);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Extract);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('M'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before - 1,
        "confirming consumes it"
    );
}
