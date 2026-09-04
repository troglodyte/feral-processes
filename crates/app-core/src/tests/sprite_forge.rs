//! `Mode::SpritePicker` — the subject list and its two gates.
//!
//! Both gates are fields read once in `App::new` (or set by
//! `install_sprite_dir`), not a live env lookup, so a test can open them
//! without touching an environment the parallel suite shares — the same
//! reasoning `dev_arena_enabled` records.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use feral_processes_engine::icon::Canvas;

use super::support::test_app;
use crate::*;

/// An app sitting on the main menu with both gates open — the flag and a
/// sprite dir installed.
fn app_with_sprite_forge(seed: u32) -> App {
    let mut app = test_app(seed);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.sprite_forge_flag = true;
    app.install_sprite_dir(PathBuf::from("/does/not/matter"));
    app
}

#[test]
fn neither_gate_open_leaves_the_row_absent_and_the_mode_unreachable() {
    let mut app = test_app(1);
    app.game = None;
    app.mode = Mode::MainMenu;
    // The default `test_app` state, made explicit: no flag, no dir.
    app.sprite_forge_flag = false;

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::MainMenu,
        "the row must not be reachable with neither gate open"
    );
}

#[test]
fn the_flag_alone_is_not_enough_without_a_checkout() {
    let mut app = test_app(2);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.sprite_forge_flag = true;
    // No `install_sprite_dir` call — an installed build's whole story:
    // there is no repo to write art into, so the row must stay off even
    // with the flag set.

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn a_dir_alone_is_not_enough_without_the_flag() {
    let mut app = test_app(3);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.install_sprite_dir(PathBuf::from("/does/not/matter"));
    // `sprite_forge_flag` left false.

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn both_gates_open_d_opens_the_picker_and_esc_returns() {
    let mut app = app_with_sprite_forge(4);
    assert!(app.sprite_forge_enabled());

    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::SpritePicker);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn sprite_subjects_is_every_species_and_structure_plus_player_and_anchor() {
    let app = app_with_sprite_forge(5);

    let subjects = app.sprite_subjects();

    assert_eq!(
        subjects.len(),
        49,
        "17 species + 30 structures + player + anchor, with no shipped \
         sprite_name overlaps to de-duplicate away"
    );
    let names: Vec<&str> = subjects.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"player"),
        "the engine's DEFAULT_PLAYER_SPRITE"
    );
    assert!(
        names.contains(&"anchor"),
        "the name hardcoded at render/base.rs:1379"
    );

    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        names, sorted,
        "sprite_subjects must already be sorted and de-duplicated by name"
    );
}

#[test]
fn sprite_subjects_reads_art_state_off_the_installed_library() {
    let mut app = app_with_sprite_forge(6);
    let mut enabled = HashMap::new();
    enabled.insert("anchor".to_string(), Canvas::new(16));
    let mut disabled = HashSet::new();
    disabled.insert("player".to_string());
    app.install_sprite_library(enabled, disabled);

    let subjects = app.sprite_subjects();
    let by_name = |name: &str| subjects.iter().find(|s| s.name == name).unwrap();

    assert_eq!(by_name("anchor").art, SpriteArt::On);
    assert_eq!(by_name("player").art, SpriteArt::Off);
    let neither = subjects
        .iter()
        .find(|s| s.name != "anchor" && s.name != "player")
        .expect("at least one subject with no installed or disabled art");
    assert_eq!(neither.art, SpriteArt::None);
}
