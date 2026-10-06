//! Implant state and its save path. Effects are tested where they are read.

use super::support::*;
use crate::components::Implants;
use crate::implants::{ImplantDb, ImplantId, load_of};
use crate::*;

fn temp_save(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("feral_implants_{tag}_{}.sav", std::process::id()))
}

fn ids(list: &[&str]) -> Vec<ImplantId> {
    list.iter().map(|s| ImplantId::from(*s)).collect()
}

#[test]
fn a_new_player_has_no_implants() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(
        game.world
            .get::<Implants>(player)
            .unwrap()
            .installed
            .is_empty()
    );
}

#[test]
fn implants_survive_a_save_load_round_trip_in_order() {
    // Through `Game::save`/`Game::load`, not RON alone: the field is
    // `#[serde(default)]`, so only the real path can show it defaulting away.
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = ids(&["ripper_fibers", "dead_mans_switch"]);
    game.world.get_mut::<Implants>(player).unwrap().installed = held.clone();

    let path = temp_save("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .world
        .get::<Implants>(loaded.player_entity())
        .unwrap();
    assert_eq!(restored.installed, held);
}

#[test]
fn a_save_without_the_implants_key_loads_with_none() {
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Implants>(player)
        .unwrap()
        .installed
        .clear();
    let path = temp_save("old");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| l.trim() != "implants: [],")
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(
        stripped.len(),
        text.len(),
        "the key was written, then removed"
    );
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let restored = loaded
        .world
        .get::<Implants>(loaded.player_entity())
        .unwrap();
    assert!(restored.installed.is_empty());
}

#[test]
fn a_save_naming_a_missing_implant_keeps_the_id_and_contributes_nothing() {
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Implants>(player).unwrap().installed = ids(&["no_such_implant"]);
    let path = temp_save("missing");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .world
        .get::<Implants>(loaded.player_entity())
        .unwrap();
    assert_eq!(restored.installed, ids(&["no_such_implant"]));
    let db = loaded.world.resource::<ImplantDb>();
    assert_eq!(load_of(restored, db), 0);
}

#[test]
fn the_shipped_implants_load_into_the_game() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.world.resource::<ImplantDb>().is_empty());
}
