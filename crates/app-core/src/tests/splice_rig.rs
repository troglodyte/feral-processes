//! `Mode::SpliceRig`: `[I]` in base space beside a Splice Rig.

use super::support::*;
use crate::*;

/// Load 3 against a cap of 4 at level 1.
const LATTICE: &str = "dermal_lattice";
/// Load 2: on top of the lattice it passes the cap.
const LEDGER: &str = "black_ledger";

/// Implant items and fragments are all the view reads of the pack, which
/// is all these tests spend.
fn pack_count(app: &App, item: &str) -> u32 {
    let v = view(app);
    if item == "core_fragment" {
        return v.fragments;
    }
    v.installable
        .iter()
        .find(|row| row.item.as_str() == item)
        .map_or(0, |row| row.count)
}

fn view(app: &App) -> feral_processes_engine::ImplantView {
    app.game.as_ref().unwrap().implant_view()
}

#[test]
fn i_beside_a_rig_opens_the_screen_and_esc_closes_it() {
    let mut app = app_beside_a_splice_rig_carrying(9301, &[(LATTICE, 1)]);

    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::SpliceRig);
    assert_eq!(app.splice_rig.as_ref().unwrap().view.installable.len(), 1);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.splice_rig.is_none());
}

#[test]
fn i_away_from_a_rig_refuses_and_logs() {
    let mut app = test_app(9302);
    found_the_base(&mut app);
    app.mode = Mode::Playing;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.status_line.as_deref(),
        Some("There is no Splice Rig here.")
    );
}

#[test]
fn installing_within_the_cap_is_immediate_and_consumes_the_item() {
    let mut app = app_beside_a_splice_rig_carrying(9303, &[(LATTICE, 1)]);
    app.handle_key(GameKey::Char('I'));

    app.handle_key(GameKey::Char('I'));

    let v = view(&app);
    assert_eq!(v.installed.len(), 1);
    assert_eq!(v.load, 3);
    assert_eq!(pack_count(&app, LATTICE), 0);
    assert_eq!(
        app.splice_rig.as_ref().unwrap().view,
        v,
        "the screen re-reads"
    );
}

#[test]
fn installing_past_the_cap_asks_first_and_y_commits() {
    let mut app = app_beside_a_splice_rig_carrying(9304, &[(LATTICE, 1), (LEDGER, 1)]);
    app.handle_key(GameKey::Char('I'));
    // Rows are sorted by item id: the ledger, then the lattice.
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('I'));
    assert_eq!(view(&app).load, 3, "the lattice went in under the cap");

    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('I'));

    assert!(app.splice_rig.as_ref().unwrap().confirm.is_some());
    assert_eq!(view(&app).load, 3, "nothing is spent while it asks");
    assert_eq!(pack_count(&app, LEDGER), 1);

    app.handle_key(GameKey::Char('Y'));
    let v = view(&app);
    assert_eq!(v.load, 5);
    assert_eq!(v.overload, 1);
    assert!(app.splice_rig.as_ref().unwrap().confirm.is_none());
    assert_eq!(pack_count(&app, LEDGER), 0);
}

#[test]
fn declining_the_overload_confirm_changes_nothing() {
    let mut app = app_beside_a_splice_rig_carrying(9305, &[(LATTICE, 1), (LEDGER, 1)]);
    app.handle_key(GameKey::Char('I'));
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('I'));
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('I'));
    assert!(app.splice_rig.as_ref().unwrap().confirm.is_some());

    app.handle_key(GameKey::Esc);

    assert!(app.splice_rig.as_ref().unwrap().confirm.is_none());
    assert_eq!(
        app.mode,
        Mode::SpliceRig,
        "Esc backs out of the question only"
    );
    assert_eq!(view(&app).load, 3);
    assert_eq!(pack_count(&app, LEDGER), 1);
}

#[test]
fn r_removes_for_fragments_and_a_short_pack_surfaces_the_refusal() {
    let mut app = app_beside_a_splice_rig_carrying(9306, &[(LATTICE, 1)]);
    app.handle_key(GameKey::Char('I'));
    app.handle_key(GameKey::Char('I'));
    assert_eq!(view(&app).installed.len(), 1);

    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('R'));

    assert_eq!(view(&app).installed.len(), 1, "no fragments, no removal");
    assert!(app.status_line.is_some(), "the engine's refusal is shown");
    assert_eq!(pack_count(&app, LATTICE), 0);
}

#[test]
fn r_with_fragments_returns_the_item() {
    let mut app = app_beside_a_splice_rig_carrying(9307, &[(LATTICE, 1), ("core_fragment", 40)]);
    app.handle_key(GameKey::Char('I'));
    app.handle_key(GameKey::Char('I'));
    assert_eq!(view(&app).installed.len(), 1);

    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('R'));

    assert!(view(&app).installed.is_empty());
    assert_eq!(pack_count(&app, LATTICE), 1);
}

#[test]
fn the_verbs_on_the_wrong_kind_of_row_refuse_without_acting() {
    let mut app = app_beside_a_splice_rig_carrying(9308, &[(LATTICE, 1)]);
    app.handle_key(GameKey::Char('I'));

    app.handle_key(GameKey::Char('R'));

    assert!(app.status_line.is_some());
    assert_eq!(pack_count(&app, LATTICE), 1);
}
