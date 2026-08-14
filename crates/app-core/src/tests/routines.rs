use super::support::*;
use crate::*;
use feral_processes_engine::abilities::FALLBACK_ABILITY_ID;

#[test]
fn the_party_menu_opens_the_routine_target_picker_and_esc_backs_all_the_way_out() {
    let mut app = test_app(61);
    open_via_menu(&mut app, 'p', "Install a routine");
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Char('1')); // "You"
    assert_eq!(app.mode, Mode::Routines);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyMenu);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn picking_a_filled_slot_uninstalls_and_picking_an_empty_one_opens_the_install_list() {
    let mut app = test_app(62);
    open_via_menu(&mut app, 'p', "Install a routine");
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

/// The reported bug: extraction from a program carrying more than one
/// routine silently did nothing — the program stayed alive and no refusal
/// reached the status line, so there was nothing on screen to explain it.
#[test]
fn extracting_from_a_program_with_two_routines_salvages_the_picked_one() {
    // Neither routine may be one the player already knows — extraction
    // teaches, and refuses what it has nothing to teach. A new game knows
    // decompile, so that is deliberately not one of these two.
    let mut app = app_owning_a_program_and_a_compiler(64, &[FALLBACK_ABILITY_ID, "sandbox"]);
    open_via_menu(&mut app, 'p', "Extract a routine");
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ExtractPick);
    let offered = {
        let program = app.pending_extract_program.unwrap();
        app.game.as_ref().unwrap().extractable_routines(program)
    };
    assert_eq!(offered.len(), 2, "both routines should be on offer");

    let before = app.game.as_mut().unwrap().owned_pets().len();
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.mode, Mode::ExtractConfirm);
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.status_line, None,
        "extraction was refused with: {:?}",
        app.status_line
    );
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before - 1,
        "the program should have been broken down"
    );
}

#[test]
fn the_extract_flow_requires_confirmation_before_the_program_is_destroyed() {
    let mut app = app_owning_a_program_and_a_compiler(63, &[FALLBACK_ABILITY_ID]);
    open_via_menu(&mut app, 'p', "Extract a routine");
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
    // ExtractConfirm -> ExtractPick -> Extract -> PartyMenu -> Playing, four
    // Escs to fully unwind. Asserting each step is what makes the following
    // re-entry meaningful — driven from anywhere but `Playing` it would be
    // swallowed by that mode's own numbered-menu handler and prove nothing.
    assert_eq!(app.mode, Mode::ExtractPick);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Extract);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyMenu);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    open_via_menu(&mut app, 'p', "Extract a routine");
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

/// The etch screen used to be reachable only through *Install a routine* →
/// a holder → an **empty** slot → `[e]`. Every routine slot in the game
/// starts full, so a player who had never popped one out could not reach it
/// at all — and it is where blanks turn into the disks that fill slots.
#[test]
fn the_party_menu_opens_the_etch_screen_and_esc_comes_straight_back() {
    let mut app = test_app(65);
    open_via_menu(&mut app, 'p', "Etch a routine disk");
    assert_eq!(app.mode, Mode::RoutineEtch);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::PartyMenu,
        "Esc returns to the menu that opened it, not into the install flow"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// The `[e]` shortcut is the other way in, and it has to unwind differently:
/// a player who came here mid-install is still holding a slot to fill.
#[test]
fn escaping_the_etch_screen_reached_with_e_lands_back_on_the_install_list() {
    let mut app = test_app(66);
    open_via_menu(&mut app, 'p', "Install a routine");
    app.handle_key(GameKey::Char('1')); // You
    app.handle_key(GameKey::Char('1')); // pop the innate routine out
    app.handle_key(GameKey::Char('1')); // the slot it vacated
    assert_eq!(app.mode, Mode::RoutineInstall);
    app.handle_key(GameKey::Char('e'));
    assert_eq!(app.mode, Mode::RoutineEtch);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineInstall);
    // And the rest of the chain is undisturbed — the `[e]` detour must not
    // consume the origin the party menu left behind.
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Routines);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyMenu);
}
