//! The `b`/`p` group menus: which rows are on screen, and where Esc goes.

use super::support::*;
use crate::*;

fn labels(rows: &[GroupMenuRow]) -> Vec<&'static str> {
    rows.iter().map(|r| r.label).collect()
}

#[test]
fn b_opens_the_hub() {
    let mut app = test_app(4001);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
}

/// The party pages are deliberately almost all non-surface — managing what you
/// brought with you is exactly what the Stack is for.
#[test]
fn the_party_hub_survives_going_underground() {
    let mut app = app_underground(4005);
    let rows = labels(&party_rows(&mut app));
    assert!(rows.contains(&"Read a manifest"), "{rows:?}");
    open_via_menu(&mut app, 'p', "Perks");
    assert_eq!(app.mode, Mode::Perks);
}

/// Fusion consumes both programs, so one companion is not enough to make the
/// row worth offering — the second picker would be empty.
#[test]
fn fuse_needs_two_programs_to_be_offered() {
    let mut app = app_owning_a_program_and_an_extractor(4006, &[]);
    let rows = labels(&party_rows(&mut app));
    open_via_menu(&mut app, 'p', "Companions");
    assert_eq!(app.mode, Mode::Companion, "one program is enough");
    assert!(
        !rows.contains(&"Fuse two programs"),
        "one program has nothing to fuse with: {rows:?}"
    );
}

/// Fusion is a researched capability (Task 10): even with two programs on
/// hand, the row stays hidden until `program_refactoring` is researched.
#[test]
fn fuse_row_is_hidden_until_fusion_is_researched_then_shown() {
    let mut app = app_owning_distant_programs(4008, 2);
    let rows = labels(&party_rows(&mut app));
    assert!(
        !rows.contains(&"Fuse two programs"),
        "fusion is unresearched on a fresh run: {rows:?}"
    );

    mark_researched(&mut app, "program_refactoring");
    let rows = labels(&party_rows(&mut app));
    assert!(
        rows.contains(&"Fuse two programs"),
        "researching program_refactoring should unlock the row: {rows:?}"
    );
}

#[test]
fn esc_from_a_screen_opened_by_a_hub_tab_returns_to_it() {
    let mut app = test_app(4008);
    stand_in_base(&mut app);
    open_command(&mut app, "Deploy a structure");
    assert_eq!(app.mode, Mode::Build);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BuildDirection);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Build, "Esc walks back up one level");
    assert!(app.hub_at().is_some(), "onto the Deploy tab of the hub");
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail, "the next goes to the rail");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "and out to the map from there");
}

/// A screen reached straight from the map has no menu to return to, and Esc
/// must not invent one.
#[test]
fn esc_from_a_screen_opened_from_the_map_returns_to_the_map() {
    let mut app = test_app(4009);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Completing an action drops to the map rather than back into the menu —
/// and, crucially, clears the origin. Left standing, it would send the next
/// unrelated screen's Esc back to a menu the player left long ago.
#[test]
fn completing_an_action_lands_on_the_map_and_forgets_the_menu() {
    let mut app = test_app(4010);
    stand_in_base(&mut app);
    open_command(&mut app, "Deploy a structure");
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BuildDirection);
    app.handle_key(GameKey::Char('l'));
    // Founding fires the base tutorial; it is not the menu the origin
    // belongs to.
    dismiss_notifications(&mut app);
    assert_eq!(app.mode, Mode::Playing, "placing ends on the map");

    // The stale-origin trap: a completely unrelated screen, opened from the
    // map, must still Esc back to the map.
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the pack was opened from the map, not from a command page"
    );
}

/// Esc out of a group menu itself is a plain exit, not another level up.
#[test]
fn esc_from_a_group_menu_returns_to_the_map() {
    for key in ['b', 'i'] {
        let mut app = test_app(4011);
        app.handle_key(GameKey::Char(key));
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Playing, "{key} menu should close");
    }
}

/// Eleven of the thirteen keys the group menus replaced. Kept as aliases they
/// would have meant the flat surface never actually shrank, and the help
/// screen would have had two systems to document.
///
/// `B` has left this list, reissued as the base report, and `w`, reissued as
/// `b`'s shortcut onto the site pages. `d` has too: it was
/// the manifest picker's key, and is now the direct demolish (`the_demolish_key_*` in
/// `tests/building.rs`). A retired key is free to be *reissued* — what this
/// census forbids is an old binding surviving as a silent alias, which is a
/// different thing from the key being used again deliberately.
#[test]
fn the_retired_map_keys_do_nothing() {
    for key in ['c', 'W', 'G', 'R', 'U', 'T', 'f', 'M', 'v'] {
        let mut app = test_app(4012);
        app.handle_key(GameKey::Char(key));
        assert_eq!(
            app.mode,
            Mode::Playing,
            "{key} was retired and must not still open a screen"
        );
    }
}

/// `i` moved from inspect to the pack, so inspect took `x` — freed by perks
/// moving into the party hub.
#[test]
fn i_opens_the_pack_and_x_inspects() {
    let mut app = test_app(4013);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);

    let mut app = test_app(4013);
    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
}

/// The party-side action deliberately left flat, because it is pressed
/// every few turns while walking. `t` is tested separately below — every
/// trader is a field `Structure`, so it opens anywhere on the field.
#[test]
fn the_hot_keys_stayed_on_the_map() {
    let mut app = test_app(4014);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(
        app.mode,
        Mode::FieldRoutine,
        "'a' should still be a map key"
    );
}

/// Symlink is a routine now, and `u` is not a second door to it. It was a
/// key of its own that opened a picker of teleport-capable structures and
/// charged Power Cells — reachable on turn one, before the run had earned
/// anything. The way home is `routine/symlink` researched, etched, installed
/// and run from the `a` list like every other field routine.
///
/// The letter itself was free from then until the compass took it, which is
/// what this now holds: `u` opens the destination picker and nothing that
/// moves the party or spends anything.
#[test]
fn u_opens_the_compass_and_never_a_way_home() {
    let mut app = test_app(4015);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('u'));
    assert_eq!(app.mode, Mode::Compass);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "the compass reads; symlink lives in the field-routine list and costs"
    );
}

/// `t` opens the trader list wherever the party is on the field, and says so
/// underground, where only a live stall sells anything.
#[test]
fn t_opens_the_trader_list_on_the_field_and_refuses_underground() {
    let mut app = test_app(4015);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(app.mode, Mode::Trade);

    let mut stack = app_underground(4016);
    stack.handle_key(GameKey::Char('t'));
    assert_eq!(stack.mode, Mode::Playing);
    assert!(
        stack.status_line.is_some(),
        "the refusal has to say something, not fail silently"
    );
}

/// Clause 2 again, and the one row where both halves of it bite: a refactor
/// needs a program to spend an upgrade *on* and an upgrade to spend, and
/// either missing leaves the second page empty. Cargo is the half that
/// changes during a run, so it is the half worth pinning.
#[test]
fn refactor_needs_both_a_program_and_an_upgrade_in_cargo() {
    let mut app = app_owning_a_program_and_an_extractor(4020, &[]);
    let rows = labels(&party_rows(&mut app));
    assert!(
        !rows.contains(&"Refactor a program"),
        "a program with nothing to spend on it has no screen: {rows:?}"
    );

    let mut app =
        app_owning_a_program_and_an_extractor_with_cargo(4020, &[], &[("buffer_extension", 1)]);
    let rows = labels(&party_rows(&mut app));
    assert!(
        rows.contains(&"Refactor a program"),
        "one upgrade in cargo is enough to open the row: {rows:?}"
    );
}

/// A refactor reaches no zone-map state through `Position`, so unlike
/// building or trading it is not `surface_only` — and managing what you
/// brought down with you is exactly what the Stack is for.
#[test]
fn refactoring_works_underground() {
    let mut surface =
        app_owning_a_program_and_an_extractor_with_cargo(4021, &[], &[("buffer_extension", 1)]);
    assert!(labels(&party_rows(&mut surface)).contains(&"Refactor a program"));

    let mut app =
        app_owning_a_program_and_an_extractor_deep(4021, &[], &[("buffer_extension", 1)], true);
    assert!(
        labels(&party_rows(&mut app)).contains(&"Refactor a program"),
        "a refactor reaches no zone-map state through Position, so it is not surface-only"
    );
}

/// The parked row is an index into a list that can shrink while the child
/// screen is open; Esc must land on a row that still exists. Ported from the
/// deleted BASE Structures page to the party's Programs page, whose rows hide
/// dynamically too (Develop wants a program). No other test covers the clamp
/// in `handle_key`'s mode-change tail.
#[test]
fn esc_clamps_the_parked_row_when_the_page_shrank_under_it() {
    let mut app = app_inside_a_small_base_with_programs(4010, false, 1);
    open_hub_screen(&mut app, Mode::PartyPrograms);
    let rows = labels(&app.command_rows(Mode::PartyPrograms).unwrap());
    let develop = rows
        .iter()
        .position(|l| *l == "Develop a program")
        .expect("a program is enough to develop");
    assert!(develop > 0, "precondition: the parked row is not the first");
    pick_row(&mut app, develop);
    assert_eq!(app.mode, Mode::Develop);

    // Every program leaves the party, so the page keeps only "Read a
    // manifest" (the player is still a subject).
    let path = scratch_path("parked_row_shrinks", 4010);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.creatures.retain(|c| !c.tamed);
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &test_assets_dir()).ok();
    let _ = std::fs::remove_file(&path);
    let after = app.command_rows(Mode::PartyPrograms).unwrap().len();
    assert!(
        after <= develop,
        "precondition: the page shrank under the parked row ({after} rows)"
    );

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::PartyPrograms);
    assert!(
        app.menu_selected < after.max(1),
        "highlight {} is past the {after} rows the page now has",
        app.menu_selected
    );
}
