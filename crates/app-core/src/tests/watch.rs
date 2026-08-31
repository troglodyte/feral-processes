//! Watching a program work: `w` on its manifest, and the four things that
//! hand the camera back.

use super::support::*;
use crate::*;

/// A base with the party inside it and one owned program on the staff,
/// drifted onto base floor.
///
/// The ticks are load-bearing. A program you have just beaten carries the
/// surface tile it was beaten on; `drift_idle_staff` gives it a base-space
/// cell through `entry_tile` on the next tick it runs, and until then its
/// `Position` is a surface coordinate aliasing into base space. Waiting is
/// what a player does anyway — the world runs at two ticks a second while
/// `Mode::Playing` is up and untouched.
fn watching_app(seed: u32) -> (App, Entity) {
    let mut app = app_owning_distant_programs(seed, 1);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    for _ in 0..4 {
        app.handle_key(GameKey::Char('.'));
    }
    let staff = app
        .game
        .as_ref()
        .unwrap()
        .base_staff()
        .first()
        .copied()
        .expect("the fixture owns a program and it is not in the party");
    (app, staff)
}

/// Opens `entity`'s manifest the way the roster does, without walking the
/// menus to get there.
fn open_manifest(app: &mut App, entity: Entity, origin: ManifestOrigin) {
    app.pending_manifest = Some(entity);
    app.manifest_origin = origin;
    app.mode = Mode::Manifest;
}

#[test]
fn w_on_a_staff_manifest_starts_watching_and_returns_to_the_map() {
    let (mut app, staff) = watching_app(7);
    assert!(
        app.game.as_ref().unwrap().watch_position(staff).is_some(),
        "fixture must own a watchable program or this proves nothing"
    );
    open_manifest(&mut app, staff, ManifestOrigin::Roster);

    app.handle_key(GameKey::Char('w'));

    assert_eq!(app.watching, Some(staff));
    assert_eq!(
        app.mode,
        Mode::Playing,
        "watching happens on the map, so `w` drops the whole sheet rather \
         than backing into the roster it was opened from"
    );
}

/// The roster reaches every program you own, including the ones whose tile
/// is the one they were beaten on. Offering `w` there and having it park the
/// camera on a stale surface coordinate is the failure this refuses.
#[test]
fn w_on_a_program_the_sim_never_walks_is_refused() {
    let (mut app, staff) = watching_app(7);
    app.game
        .as_mut()
        .unwrap()
        .add_companion(staff)
        .expect("a fresh party has room");
    open_manifest(&mut app, staff, ManifestOrigin::Roster);

    app.handle_key(GameKey::Char('w'));

    assert_eq!(app.watching, None);
    assert_eq!(
        app.mode,
        Mode::Manifest,
        "a refused key leaves the screen it was pressed on standing"
    );
    assert!(
        app.status_line.is_some(),
        "a refusal is a sentence, never a swallowed keypress"
    );
}

#[test]
fn esc_on_the_map_hands_the_camera_back() {
    let (mut app, staff) = watching_app(7);
    open_manifest(&mut app, staff, ManifestOrigin::Roster);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.watching, Some(staff));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.watching, None);
    assert_eq!(app.mode, Mode::Playing);
}

/// Walking while the camera is somewhere else is walking blind, so a step
/// takes the camera with it. The step still happens — this is a release, not
/// a swallowed key.
#[test]
fn a_step_hands_the_camera_back_and_still_steps() {
    let (mut app, staff) = watching_app(7);
    open_manifest(&mut app, staff, ManifestOrigin::Roster);
    app.handle_key(GameKey::Char('w'));
    let before = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Right);

    assert_eq!(app.watching, None);
    assert_ne!(
        app.game.as_ref().unwrap().base_pos().unwrap(),
        before,
        "the movement key must still move — releasing is not swallowing"
    );
}

/// The camera reads `watch_position` every frame, so the release has to come
/// from the same call rather than from a list of things that might end a
/// watch. Leaving base space is one of them and stands for all of them.
#[test]
fn the_camera_lets_go_when_the_program_stops_being_watchable() {
    let (mut app, staff) = watching_app(7);
    open_manifest(&mut app, staff, ManifestOrigin::Roster);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.watching, Some(staff));

    app.game
        .as_mut()
        .unwrap()
        .leave_base()
        .expect("the party can always step out");

    assert_eq!(
        app.watch_center(),
        None,
        "nothing to watch from the zone surface"
    );
    assert_eq!(
        app.watching, None,
        "and the read is what clears the field, so the next frame does not \
         ask again"
    );
}
