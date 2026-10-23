//! `[D]` on a manifest: the second page about the body the sheet is
//! showing.

use super::support::*;
use crate::*;

/// Opens the player's own manifest without walking the menus to get there
/// — `tests/watch.rs`'s `open_manifest`, inlined because this module needs
/// it twice and wants the player rather than a staff program.
fn open_player_manifest(app: &mut App) -> Option<Entity> {
    let player = app.game.as_ref().unwrap().player_entity();
    app.pending_manifest = Some(player);
    app.manifest_origin = ManifestOrigin::Map;
    app.mode = Mode::Manifest;
    Some(player)
}

/// `[D]` on a manifest opens the dossier for the body the sheet is showing,
/// and `Esc` comes back to it — not to the list the manifest was opened
/// from, which is `leave_manifest`'s job and stays its job.
#[test]
fn d_opens_the_dossier_and_esc_returns_to_the_manifest() {
    let mut app = test_app(7301);
    let subject = open_player_manifest(&mut app);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::Dossier);
    assert_eq!(app.pending_manifest, subject, "the subject must not move");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, subject);
}

/// Lowercase `d` is not it. On the map `d` is a verb already, so the dossier
/// takes the uppercase key and a lowercase `d` is inert here.
#[test]
fn lowercase_d_does_nothing_in_the_dossier() {
    let mut app = app_owning_distant_programs(7302, 1);
    let me = open_player_manifest(&mut app);
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::Manifest, "never the dossier");
    assert_eq!(app.pending_manifest, me, "nothing moved");
}

/// Paging and jumping from the dossier move the subject without leaving the
/// page, so the dossier always describes whoever the rail would highlight.
#[test]
fn the_dossier_pages_and_stays_the_dossier() {
    let mut app = app_owning_distant_programs(7304, 2);
    let subjects = app.manifest_subjects();
    open_player_manifest(&mut app);
    app.handle_key(GameKey::Char('D'));
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::Dossier);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
    app.handle_key(GameKey::Up);
    assert_eq!(app.pending_manifest, Some(subjects[0]));
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Dossier);
    assert_eq!(app.pending_manifest, Some(subjects[2]), "Up wraps");
    for c in ['3', 'b'] {
        app.handle_key(GameKey::Char(c));
        assert_eq!(app.pending_manifest, Some(subjects[2]), "[{c}] jumped");
        assert_eq!(app.mode, Mode::Dossier);
    }
}

/// The dossier is a page of the manifest, and the manifest is where the
/// roster's row is parked — so a round trip through `[D]` must leave that
/// parked row exactly where it was, or Esc out of the sheet lands on the
/// wrong program.
#[test]
fn a_trip_through_the_dossier_keeps_the_manifests_parked_row() {
    let mut app = test_app(7303);
    open_player_manifest(&mut app);
    app.menu_selected = 3;
    app.handle_key(GameKey::Char('D'));
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.menu_selected, 3,
        "the roster row parked on the manifest was lost"
    );
}
