//! The `b`/`p` group menus: which rows are on screen, and where Esc goes.

use super::support::*;
use crate::*;

fn labels(rows: &[GroupMenuRow]) -> Vec<&'static str> {
    rows.iter().map(|r| r.label).collect()
}

#[test]
fn b_and_p_open_the_group_menus() {
    let mut app = test_app(4001);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseMenu);

    let mut app = test_app(4001);
    app.handle_key(GameKey::Char('p'));
    assert_eq!(app.mode, Mode::PartyMenu);
}

/// The whole point of clause 2: a row is not offered when the screen behind
/// it has nothing in it. A fresh game has no structures deployed and no
/// programs compiled, so five of the nine base rows have nowhere to go.
#[test]
fn the_base_menu_hides_rows_whose_screen_would_be_empty() {
    let mut app = test_app(4002);
    let rows = labels(&app.base_menu_rows());
    for absent in [
        "Base staff",
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
    let mut app = app_owning_a_program_and_a_compiler(4003, &[]);
    let rows = labels(&app.base_menu_rows());
    assert!(rows.contains(&"Base staff"), "{rows:?}");
    assert!(rows.contains(&"Demolish a structure"), "{rows:?}");
}

/// Clause 1. Every row that reads `App::nearby_*` scans around the player's
/// `Position`, which stays pinned to the surface entrance tile while the
/// party is in the Stack — so without the `surface_only` flag the base menu
/// would list a base four frames overhead and offer to demolish it.
#[test]
fn underground_the_base_menu_drops_its_surface_only_rows() {
    let mut app = app_underground(4004);
    let rows = labels(&app.base_menu_rows());
    for absent in [
        "Deploy a structure",
        "Base staff",
        "Work orders",
        "Work a structure yourself",
        "Upgrade a structure",
        "Demolish a structure",
    ] {
        assert!(
            !rows.contains(&absent),
            "{absent:?} reaches the zone map through a pinned Position: {rows:?}"
        );
    }
    assert!(
        rows.contains(&"Research"),
        "research is not surface-gated: {rows:?}"
    );
    // The chains are a property of the loaded assets, not of anything the
    // player's `Position` can reach — and four frames down is exactly when
    // you want to check what the base upstairs still needs.
    assert!(
        rows.contains(&"Recipes"),
        "recipes are asset data, not a scan around the party: {rows:?}"
    );
}

/// The party menu is deliberately almost all non-surface — managing what you
/// brought with you is exactly what the Stack is for.
#[test]
fn the_party_menu_survives_going_underground() {
    let mut app = app_underground(4005);
    let rows = labels(&app.party_menu_rows());
    assert!(rows.contains(&"Read a manifest"), "{rows:?}");
    assert!(rows.contains(&"Perks"), "{rows:?}");
}

/// Fusion consumes both programs, so one companion is not enough to make the
/// row worth offering — the second picker would be empty.
#[test]
fn fuse_needs_two_programs_to_be_offered() {
    let mut app = app_owning_a_program_and_a_compiler(4006, &[]);
    let rows = labels(&app.party_menu_rows());
    assert!(
        rows.contains(&"Companions"),
        "one program is enough: {rows:?}"
    );
    assert!(
        !rows.contains(&"Fuse two programs"),
        "one program has nothing to fuse with: {rows:?}"
    );
}

/// The reason the handler and the renderer must call the same function.
/// Rows are hidden dynamically, so row 1 of the base menu is whatever
/// survived the filter — not the first entry of the static table.
#[test]
fn a_hidden_row_cannot_be_reached_by_its_table_position() {
    let mut app = app_underground(4007);
    let expected = app.base_menu_rows()[0].target;
    assert_ne!(
        expected,
        Mode::Build,
        "the fixture should have hidden Deploy, or this proves nothing"
    );
    app.mode = Mode::BaseMenu;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode, expected,
        "row 1 must open the first row still on screen"
    );
}

#[test]
fn esc_from_a_screen_opened_by_a_group_menu_returns_to_it() {
    let mut app = test_app(4008);
    app.handle_key(GameKey::Char('b'));
    let deploy = app
        .base_menu_rows()
        .iter()
        .position(|r| r.label == "Deploy a structure")
        .expect("a fresh game can always deploy");
    app.handle_key(GameKey::Char(menu_shortcut(deploy)));
    assert_eq!(app.mode, Mode::Build);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
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
    app.handle_key(GameKey::Char('b'));
    let deploy = app
        .base_menu_rows()
        .iter()
        .position(|r| r.label == "Deploy a structure")
        .expect("a fresh game can always deploy");
    app.handle_key(GameKey::Char(menu_shortcut(deploy)));
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
        "the pack was opened from the map, not from the base menu"
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

/// Twelve of the thirteen keys the group menus replaced. Kept as aliases they
/// would have meant the flat surface never actually shrank, and the help
/// screen would have had two systems to document.
///
/// `d` is the thirteenth and has left this list: it was the manifest picker's
/// key, and is now the direct demolish (`the_demolish_key_*` in
/// `tests/building.rs`). A retired key is free to be *reissued* — what this
/// census forbids is an old binding surviving as a silent alias, which is a
/// different thing from the key being used again deliberately.
#[test]
fn the_retired_map_keys_do_nothing() {
    for key in ['c', 'w', 'W', 'G', 'R', 'U', 'B', 'T', 'f', 'm', 'M', 'v'] {
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
/// moving into the party menu.
#[test]
fn i_opens_the_pack_and_x_inspects() {
    let mut app = test_app(4013);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);

    let mut app = test_app(4013);
    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
}

/// The three party-side actions deliberately left flat, because they are
/// pressed every few turns while walking.
#[test]
fn the_hot_keys_stayed_on_the_map() {
    for (key, expected) in [
        (GameKey::Char('a'), Mode::FieldCast),
        (GameKey::Char('t'), Mode::Trade),
        (GameKey::Char('u'), Mode::Symlink),
    ] {
        let mut app = test_app(4014);
        app.handle_key(key);
        assert_eq!(app.mode, expected, "{key:?} should still be a map key");
    }
}

/// Clause 2 again, and the one row where both halves of it bite: a refactor
/// needs a program to spend an upgrade *on* and an upgrade to spend, and
/// either missing leaves the second page empty. Cargo is the half that
/// changes during a run, so it is the half worth pinning.
#[test]
fn refactor_needs_both_a_program_and_an_upgrade_in_cargo() {
    let mut app = app_owning_a_program_and_a_compiler(4020, &[]);
    let rows = labels(&app.party_menu_rows());
    assert!(
        !rows.contains(&"Refactor a program"),
        "a program with nothing to spend on it has no screen: {rows:?}"
    );

    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(4020, &[], &[("buffer_extension", 1)]);
    let rows = labels(&app.party_menu_rows());
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
        app_owning_a_program_and_a_compiler_with_cargo(4021, &[], &[("buffer_extension", 1)]);
    assert!(labels(&surface.party_menu_rows()).contains(&"Refactor a program"));

    let mut app =
        app_owning_a_program_and_a_compiler_deep(4021, &[], &[("buffer_extension", 1)], true);
    assert!(
        labels(&app.party_menu_rows()).contains(&"Refactor a program"),
        "a refactor reaches no zone-map state through Position, so it is not surface-only"
    );
}

/// Manual posting is gone: the base says *what to make*, and works out who
/// stands where itself. The two rows that used to pick a program and then a
/// structure are replaced by one that queues an order and one that hands a
/// program to the base.
#[test]
fn the_base_menu_offers_work_orders_and_staff_rather_than_manual_posting() {
    let mut app = app_owning_a_program_and_a_compiler(4030, &[]);
    let rows = labels(&app.base_menu_rows());

    assert!(!rows.contains(&"Assign a cronjob"), "{rows:?}");
    assert!(!rows.contains(&"Post a guard"), "{rows:?}");
    assert!(rows.contains(&"Base staff"), "{rows:?}");
    assert!(
        rows.contains(&"Work a structure yourself"),
        "the player is not staff, and that flow is untouched: {rows:?}"
    );
}

/// The Work orders row asks the same question its screen does — a base with
/// nothing orderable would open on an empty list, which is the drift the
/// `available` closure exists to prevent.
#[test]
fn the_work_orders_row_appears_once_something_is_orderable() {
    let mut app = test_app(4031);
    assert!(
        !labels(&app.base_menu_rows()).contains(&"Work orders"),
        "nothing is deployed, so nothing can be ordered"
    );

    // A Mining Node makes Core Fragments out of nothing on a timer, so it
    // is orderable the moment it is standing. An assembler with no feeder
    // beside it deliberately is not — `chain_break` refuses a line that can
    // never be fed, and this row asks the same question the queue does.
    let mut app = app_inside_a_small_base_with_programs(4032, false, 1);
    assert!(
        labels(&app.base_menu_rows()).contains(&"Work orders"),
        "a deployed extractor makes its product orderable: {:?}",
        labels(&app.base_menu_rows())
    );
}
