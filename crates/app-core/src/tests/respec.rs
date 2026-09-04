//! Opening and confirming a respec from the two screens that spend points.
//!
//! What app-core owns here is the routing: which key opens the confirm, where
//! each answer lands, and that `y` actually reaches the engine. What a respec
//! *does* is `tests::respec` in the engine — there is no public way to hand a
//! run perk points from here, and `Game::world` is private by design.

use super::support::*;
use crate::*;

/// Standing on the perk screen of an ordinary run.
fn app_on_the_perk_screen(seed: u32) -> App {
    let mut app = app_owning_a_program_and_a_compiler_with_cargo(seed, &[], &[]);
    open_via_menu(&mut app, 'p', "Perks");
    assert_eq!(app.mode, Mode::Perks);
    app
}

/// Standing on one program's talent ladder.
fn app_on_the_talent_ladder(seed: u32) -> App {
    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(seed, &[], &[("privilege_ring", 3)]);
    open_via_menu(&mut app, 'p', "Develop a program");
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::DevelopProgram);
    app
}

#[test]
fn shift_x_opens_the_perk_confirm() {
    let mut app = app_on_the_perk_screen(70);
    app.handle_key(GameKey::Char('X'));
    assert_eq!(app.mode, Mode::RespecPerksConfirm);
}

#[test]
fn y_reaches_the_engine_and_returns_to_the_picker() {
    let mut app = app_on_the_perk_screen(71);
    app.handle_key(GameKey::Char('X'));

    app.handle_key(GameKey::Char('y'));

    assert_eq!(app.mode, Mode::Perks, "the commit returns to the picker");
    // A fresh run has bought no perks, so the engine refuses — and the
    // refusal landing on `status_line` is the proof the key called through
    // rather than merely changing mode.
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("no perks")),
        "expected the engine's refusal, got {:?}",
        app.status_line
    );
}

#[test]
fn esc_backs_out_of_the_perk_confirm() {
    let mut app = app_on_the_perk_screen(72);
    app.handle_key(GameKey::Char('X'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Perks);
    assert!(
        app.status_line.is_none(),
        "backing out is not a refusal and says nothing"
    );
}

/// The reason the key is uppercase: `selected_index` labels rows past the
/// digits with lowercase letters, and the perk picker has eighteen rows, so a
/// lowercase binding would pick a perk *and* open the wipe on one keypress.
#[test]
fn lowercase_x_is_a_row_label_and_never_the_wipe() {
    let mut app = app_on_the_perk_screen(73);

    app.handle_key(GameKey::Char('x'));

    assert_eq!(app.mode, Mode::Perks);
}

#[test]
fn shift_x_opens_the_talent_confirm_and_esc_returns_to_the_ladder() {
    let mut app = app_on_the_talent_ladder(74);

    app.handle_key(GameKey::Char('X'));
    assert_eq!(app.mode, Mode::RespecTalentsConfirm);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::DevelopProgram,
        "a declined confirmation leaves the player on the ladder they opened it from"
    );
    assert!(app.pending_develop_target.is_some());
}

#[test]
fn a_refused_talent_commit_never_backs_out() {
    let mut app = app_on_the_talent_ladder(75);
    app.handle_key(GameKey::Char('X'));

    app.handle_key(GameKey::Char('y'));

    assert_eq!(app.mode, Mode::DevelopProgram);
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("no talents")),
        "expected the engine's refusal, got {:?}",
        app.status_line
    );
}

/// `X` must not have displaced the kernel ring already bound to `R` there.
#[test]
fn r_still_opens_a_kernel_ring_on_the_talent_ladder() {
    let mut app = app_on_the_talent_ladder(76);

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::DevelopProgram, "the ladder holds");
    assert_ne!(
        app.mode,
        Mode::RespecTalentsConfirm,
        "`r` is the ring, never the wipe"
    );
}
