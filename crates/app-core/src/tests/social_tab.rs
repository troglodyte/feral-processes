//! `Tab` on a manifest: STATS, SOCIAL and TALK, for programs you own.

use super::support::*;
use crate::*;

fn open(app: &mut App, subject: Entity) {
    app.pending_manifest = Some(subject);
    app.manifest_origin = ManifestOrigin::Map;
    app.mode = Mode::Manifest;
}

fn owned_pair(seed: u32) -> (App, Entity, Entity) {
    let mut app = app_owning_distant_programs(seed, 2);
    let subjects = app.manifest_subjects();
    let programs: Vec<Entity> = subjects
        .iter()
        .copied()
        .filter(|&e| app.game.as_ref().unwrap().social(e).is_some())
        .collect();
    assert!(programs.len() >= 2, "fixture must own two programs");
    (app, programs[0], programs[1])
}

#[test]
fn tab_cycles_stats_social_talk_on_an_owned_program() {
    let (mut app, a, _) = owned_pair(7401);
    open(&mut app, a);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Social);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Talk);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn leaving_from_the_talk_tab_resets_it() {
    let (mut app, a, _) = owned_pair(7407);
    open(&mut app, a);
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Talk);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn paging_keeps_the_tab() {
    let (mut app, a, b) = owned_pair(7402);
    open(&mut app, a);
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Right);
    assert_ne!(app.pending_manifest, Some(a));
    assert_eq!(app.manifest_tab, ManifestTab::Social);
    let _ = b;
}

#[test]
fn leaving_the_manifest_resets_the_tab() {
    let (mut app, a, _) = owned_pair(7403);
    open(&mut app, a);
    app.handle_key(GameKey::Tab);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn tab_is_a_no_op_on_the_player() {
    let (mut app, _, _) = owned_pair(7404);
    let player = app.game.as_ref().unwrap().player_entity();
    open(&mut app, player);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn tab_is_a_no_op_on_a_wild_program() {
    let (mut app, _, _) = owned_pair(7405);
    let wild = place_wild_program_east(&mut app, 3);
    open(&mut app, wild);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn watching_from_the_social_tab_resets_it() {
    let mut app = app_owning_distant_programs(7406, 1);
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
        .expect("the fixture owns a staff program");
    open(&mut app, staff);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Social);
    app.handle_key(GameKey::Char('w'));
    assert_eq!(app.watching, Some(staff), "the watch must have started");
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}
