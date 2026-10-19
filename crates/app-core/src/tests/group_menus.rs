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

/// The whole point of clause 2: a row is not offered when the screen behind
/// it has nothing in it. A fresh game has no structures deployed and no
/// programs compiled, so five of the nine base rows have nowhere to go.
#[test]
fn the_command_pages_hide_rows_whose_screen_would_be_empty() {
    let mut app = test_app(4002);
    // Inside the base, which is where every one of these rows is legal at
    // all now — see `the_command_pages_offer_their_base_rows_only_inside_base_space`.
    // Without it the Deploy assertion below would be asking the locale
    // rather than asking whether the screen has anything in it.
    stand_in_base(&mut app);
    let rows = command_labels_all(&mut app);
    for absent in [
        "Work a structure yourself",
        "Upgrade a structure",
        "Demolish a structure",
    ] {
        assert!(
            !rows.contains(&absent),
            "nothing is deployed yet, so {absent:?} leads to an empty screen: {rows:?}"
        );
    }
    assert!(rows.contains(&"Deploy a structure"), "{rows:?}");
    assert!(rows.contains(&"Research"), "{rows:?}");
}

/// A program the player owns makes the row that hands one to the base
/// appear, and only that — the structure-side rows still have nothing to
/// offer.
#[test]
fn a_program_in_range_brings_back_the_rows_that_need_one() {
    let mut app = app_owning_a_program_and_an_extractor(4003, &[]);
    stand_in_base(&mut app);
    let rows = command_labels_all(&mut app);
    assert!(rows.contains(&"Demolish a structure"), "{rows:?}");
}

/// Every row the `base_only` flag carries, and the flag's whole job. Each of
/// them is a `Game::require_base` caller, so base space is the only locale
/// the engine will accept them in — offering them anywhere else advertises a
/// screen whose every action is refused on the far side. The flag used to be
/// `surface_only` and used to ask `is_underground()`, which was the same
/// question only while there were two locales.
const GUARDED: [&str; 4] = [
    "Deploy a structure",
    "Work a structure yourself",
    "Upgrade a structure",
    "Demolish a structure",
];

/// Which of `GUARDED` `app` offers from inside the base, having asserted
/// that walking back out through the anchor takes **every** one of them off
/// screen.
///
/// One fixture on both sides of the door rather than two fixtures, for the
/// reason the engine's own `tests/base_space.rs` gives: the base, the
/// machines and the player's `Position` are identical either side, so where
/// the party is standing is the only thing that can explain a difference.
///
/// Returns what it found so the caller can prove the fixtures between them
/// exercise all four rows. A row no fixture ever makes available inside is a
/// row whose flag has no coverage at all — flipping it to `base_only: false`
/// would leave the suite green, which is exactly the hole this returns a
/// value to close.
fn base_rows_lost_on_leaving(mut app: App) -> Vec<&'static str> {
    assert!(
        app.game.as_ref().is_some_and(|g| g.in_base()),
        "the fixture must start inside the base"
    );
    let offered: Vec<&'static str> = command_labels_all(&mut app)
        .into_iter()
        .filter(|label| GUARDED.contains(label))
        .collect();

    app.game
        .as_mut()
        .unwrap()
        .leave_base()
        .expect("the fixture stands on the exit cell");

    let rows = command_labels_all(&mut app);
    for absent in GUARDED {
        assert!(
            !rows.contains(&absent),
            "{absent:?} is a base action, so it must not be offered on the open grid: {rows:?}"
        );
    }
    // Not a menu that lost every row: the two that are not locale-gated at
    // all have to survive the walk out, or the assertions above would pass
    // against a command page that had simply gone empty.
    assert!(
        rows.contains(&"Research"),
        "research is not locale-gated: {rows:?}"
    );
    assert!(
        rows.contains(&"Recipes"),
        "recipes are asset data, not a scan around the party: {rows:?}"
    );

    offered
}

#[test]
fn the_command_pages_offer_their_base_rows_only_inside_base_space() {
    let mut covered: Vec<&'static str> = Vec::new();

    // Two fixtures because no single one stands up every guarded row: the
    // extractor fixture has something to upgrade and clearance to deploy, the
    // small base has an extractor whose product can be ordered.
    let mut extractor = app_owning_a_program_and_an_extractor(4004, &[]);
    stand_in_base(&mut extractor);
    covered.extend(base_rows_lost_on_leaving(extractor));
    covered.extend(base_rows_lost_on_leaving(
        app_inside_a_small_base_with_programs(4006, false, 1),
    ));

    for row in GUARDED {
        assert!(
            covered.contains(&row),
            "no fixture here ever offers {row:?} inside the base, so its `base_only` flag is untested: {covered:?}"
        );
    }
}

/// The same rows are off screen four frames down, where they were already
/// hidden before the base moved out of phase — so the flag's rename cannot
/// have quietly re-opened the Stack.
#[test]
fn the_command_pages_drop_their_base_rows_underground_too() {
    for mut app in [
        app_owning_a_program_and_an_extractor_deep(4005, &[], &[], true),
        app_inside_a_small_base_with_programs(4007, true, 1),
    ] {
        assert!(
            app.game.as_ref().is_some_and(|g| g.is_underground()),
            "precondition: the fixture really went down"
        );
        let rows = command_labels_all(&mut app);
        for absent in GUARDED {
            assert!(
                !rows.contains(&absent),
                "{absent:?} reaches the base through a pinned Position: {rows:?}"
            );
        }
        // The chains are a property of the loaded assets, not of anything the
        // player's `Position` can reach — and four frames down is exactly when
        // you want to check what the base back home still needs.
        assert!(
            rows.contains(&"Recipes"),
            "recipes are asset data, not a scan around the party: {rows:?}"
        );
    }
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
fn esc_from_a_screen_opened_by_a_group_menu_returns_to_it() {
    let mut app = test_app(4008);
    stand_in_base(&mut app);
    open_command(&mut app, "Deploy a structure");
    assert_eq!(app.mode, Mode::Build);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseBuild, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseBuild, "the first goes to the rail");
    assert!(app.hub_on_rail);
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
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::BuildDirection);
    app.handle_key(GameKey::Char('l'));
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
    for key in ['b', 'p'] {
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
/// `B` has left this list, reissued as the base report. `d` has too: it was
/// the manifest picker's key, and is now the direct demolish (`the_demolish_key_*` in
/// `tests/building.rs`). A retired key is free to be *reissued* — what this
/// census forbids is an old binding surviving as a silent alias, which is a
/// different thing from the key being used again deliberately.
#[test]
fn the_retired_map_keys_do_nothing() {
    for key in ['c', 'w', 'W', 'G', 'R', 'U', 'T', 'f', 'm', 'M', 'v'] {
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
/// trader is a base-space `Structure` now, so it only opens on a base.
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

/// `t` closes the same hole `d`'s `in_base()` guard and the group menu's
/// `base_only` rows already closed: every trader `Game::view_entities` can now
/// find is a `Structure`, and a `Structure` is never found outside base space
/// — so the open grid used to open `Mode::Trade` onto a list that could only
/// ever come back empty. Noted rather than fixed in
/// `seam:require-surface-used-to-mean-not-in-the-stack-and-ten-of` when the
/// other two keys moved.
#[test]
fn t_opens_the_trader_list_only_from_the_base() {
    let mut app = test_app(4015);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(
        app.mode,
        Mode::Playing,
        "no base exists yet, so there is nowhere for the list to open onto"
    );
    assert!(
        app.status_line.is_some(),
        "the refusal has to say something, not fail silently"
    );

    found_the_base(&mut app);
    stand_in_base(&mut app);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(
        app.mode,
        Mode::Trade,
        "standing in the base, the same key opens the trader list"
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

/// Manual posting is gone: the base says *what to make*, and works out who
/// stands where itself. The two rows that used to pick a program and then a
/// structure are replaced by one that queues an order and one that hands a
/// program to the base.
#[test]
fn the_build_page_offers_work_orders_and_staff_rather_than_manual_posting() {
    let mut app = app_owning_a_program_and_an_extractor(4030, &[]);
    stand_in_base(&mut app);
    let rows = command_labels_all(&mut app);

    assert!(!rows.contains(&"Assign a cronjob"), "{rows:?}");
    assert!(!rows.contains(&"Post a guard"), "{rows:?}");
    assert!(!rows.contains(&"Base staff"), "{rows:?}");
    assert!(!rows.contains(&"Work orders"), "{rows:?}");
    assert!(
        rows.contains(&"Work a structure yourself"),
        "the player is not staff, and that flow is untouched: {rows:?}"
    );
}

/// `Mode::PinSubject`'s row: hidden until a `studies` structure stands, and
/// offered once one does — `Game::study_station` is the closure's own
/// question, so a mod naming a second `studies` structure needs no change
/// here.
#[test]
fn the_study_row_is_hidden_with_no_station_and_shown_with_one() {
    let mut app = app_owning_a_program_and_an_extractor(883, &[]);
    stand_in_base(&mut app);
    assert!(
        !command_labels_all(&mut app).contains(&"Study a program"),
        "an Extractor alone declares no `studies` pen"
    );

    let mut app = app_owning_a_program_and_a_research_station(884);
    stand_in_base(&mut app);
    assert!(
        command_labels_all(&mut app).contains(&"Study a program"),
        "a standing Research Station must offer the row"
    );
}

/// `Mode::PinPrisoner`'s row: hidden until a Holding Cell stands.
#[test]
fn the_hold_row_is_hidden_with_no_cell_and_shown_with_one() {
    let mut app = app_owning_a_program_and_an_extractor(887, &[]);
    stand_in_base(&mut app);
    assert!(!command_labels_all(&mut app).contains(&"Hold a program"));

    let mut app = app_beside_a_holding_cell(888, Vec::new(), 0);
    stand_in_base(&mut app);
    assert!(command_labels_all(&mut app).contains(&"Hold a program"));
}
