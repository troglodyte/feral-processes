//! The two-page refactor flow: what each key does, and what it spends.

use super::support::*;
use crate::*;

/// Two upgrades so the picker has a row to move off, and one program.
fn app_ready_to_refactor(seed: u32) -> App {
    app_owning_a_program_and_a_compiler_with_cargo(
        seed,
        &[],
        &[("bounds_check", 1), ("inline_cache", 1)],
    )
}

#[test]
fn the_party_menu_opens_the_refactor_picker_and_esc_backs_all_the_way_out() {
    let mut app = app_ready_to_refactor(70);
    open_via_menu(&mut app, 'p', "Refactor a program");
    assert_eq!(app.mode, Mode::Refactor);

    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::RefactorItem);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Refactor, "Esc backs out one page");
    assert!(
        app.pending_refactor_target.is_none(),
        "and forgets which program was picked, or the next visit refactors the old one"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyMenu);
}

/// A refactor is permanent and cannot be undone, so Enter must never resolve
/// to a row the player is not looking at.
///
/// `selected_index`'s Enter arm is `menu_selected.min(len - 1)`, and nothing
/// re-clamps `menu_selected` when the offered list shrinks under it — the
/// mode does not change on a successful refactor, so `handle_key`'s own reset
/// never fires. Spending the last copy of the highlighted row therefore used
/// to leave the highlight pointing past the end, and the *next* Enter spent a
/// different upgrade than the one on screen.
#[test]
fn spending_the_highlighted_upgrade_does_not_leave_the_highlight_on_another_one() {
    let mut app = app_ready_to_refactor(71);
    open_via_menu(&mut app, 'p', "Refactor a program");
    app.handle_key(GameKey::Char('1'));

    // Row 2 of the id-sorted pair is `inline_cache`; `bounds_check` is row 1.
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Enter);

    let held = |app: &mut App, item: &str| {
        app.game
            .as_mut()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|r| r.item.as_str() == item)
            .map(|r| r.qty)
            .unwrap_or(0)
    };
    assert_eq!(
        held(&mut app, "inline_cache"),
        0,
        "the highlighted one went"
    );
    assert_eq!(held(&mut app, "bounds_check"), 1, "the other one did not");

    // One row left. Enter must resolve to it because it is what is drawn,
    // not because the stale index happened to clamp onto it.
    assert_eq!(
        app.menu_selected, 0,
        "the highlight has to follow the list it is drawn against"
    );
}

/// Spending the last upgrade leaves nothing to pick, so the flow must leave
/// entirely. Backing out one page to the program picker looks safe and is
/// not: every program row there opens a second page with no rows on it,
/// which is the blank screen the auto-exit exists to prevent.
#[test]
fn spending_the_last_upgrade_leaves_the_flow_rather_than_the_page() {
    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(72, &[], &[("buffer_extension", 1)]);
    open_via_menu(&mut app, 'p', "Refactor a program");
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));

    assert_ne!(
        app.mode,
        Mode::RefactorItem,
        "there is nothing left to pick on this page"
    );
    assert_ne!(
        app.mode,
        Mode::Refactor,
        "and the program picker only leads back to that empty page"
    );
    assert!(app.pending_refactor_target.is_none());
}

/// The refusal path: the engine's own checks land in the status line, and
/// the item stays in cargo. A bump on a program already current for the zone
/// is the refusal a player will actually meet.
#[test]
fn a_refused_refactor_reports_why_and_spends_nothing() {
    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(73, &[], &[("recompile_kernel", 1)]);
    open_via_menu(&mut app, 'p', "Refactor a program");
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));

    assert!(
        app.status_line.is_some(),
        "a zone-1 program in zone 1 is already current, and the screen has to say so"
    );
    assert_eq!(
        app.mode,
        Mode::RefactorItem,
        "a refusal holds the page so the player can pick something else"
    );
    let kernels = app
        .game
        .as_mut()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.item.as_str() == "recompile_kernel")
        .map(|r| r.qty)
        .unwrap_or(0);
    assert_eq!(kernels, 1, "a refused refactor spends nothing");
}
