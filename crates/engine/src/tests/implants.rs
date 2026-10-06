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

// ---- effects at their seams (plan P2) ----

use crate::implants::{ImplantDef, ImplantDownside, ImplantHook, ImplantSignature, ImplantStats};

fn new_game() -> Game {
    Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// What P3's `install_implant` will do to the world, minus its checks.
fn install(game: &mut Game, list: &[&str]) {
    let player = game.player_entity();
    game.world.get_mut::<Implants>(player).unwrap().installed = ids(list);
    game.recompute_derived(player);
}

fn def(id: &str) -> ImplantDef {
    ImplantDef {
        id: ImplantId::from(id),
        name: id.to_string(),
        description: String::new(),
        load: 1,
        stats: ImplantStats::default(),
        hooks: Vec::new(),
        signature: None,
        downside: None,
    }
}

fn add_def(game: &mut Game, def: ImplantDef) {
    game.world.resource_mut::<ImplantDb>().insert(def);
}

fn snapshot(game: &Game) -> String {
    let player = game.player_entity();
    format!(
        "{:?} {:?} {:?} {}",
        game.world.get::<Stats>(player).unwrap(),
        game.world
            .get::<crate::components::Derived>(player)
            .unwrap(),
        game.world.get::<Decompiler>(player).unwrap().skill,
        game.world.get::<PowerReserve>(player).unwrap().get(),
    )
}

#[test]
fn stats_install_and_remove_restores_every_field() {
    let mut game = new_game();
    add_def(
        &mut game,
        ImplantDef {
            stats: ImplantStats {
                max_hp: 7,
                atk: 3,
                mitigation: 2,
                max_power: 25.0,
                crit: 0.05,
                status_resist: 4,
                decompiler: 2,
                accuracy: 0,
                evasion: 0,
            },
            ..def("all_stats")
        },
    );
    let player = game.player_entity();
    let before = snapshot(&game);
    let base = *game.world.get::<Stats>(player).unwrap();
    let base_derived = *game
        .world
        .get::<crate::components::Derived>(player)
        .unwrap();

    install(&mut game, &["all_stats"]);
    let stats = *game.world.get::<Stats>(player).unwrap();
    let derived = *game
        .world
        .get::<crate::components::Derived>(player)
        .unwrap();
    assert_eq!(stats.max_hp, base.max_hp + 7);
    assert_eq!(stats.atk, base.atk + 3);
    assert_eq!(stats.mitigation, base.mitigation + 2);
    assert_eq!(derived.max_power, base_derived.max_power + 25.0);
    assert!((derived.crit - base_derived.crit - 0.05).abs() < 1e-9);
    assert_eq!(derived.status_resist, base_derived.status_resist + 4);
    assert_eq!(game.world.get::<Decompiler>(player).unwrap().skill, 2);

    install(&mut game, &[]);
    assert_eq!(snapshot(&game), before);
}

#[test]
fn dermal_lattice_lowers_evasion_in_the_profile_and_the_manifest() {
    let mut game = new_game();
    let player = game.player_entity();
    let swing = crate::battle::Swing::default();
    let evasion = |g: &Game| g.combatant_profile(player, swing).evasion;
    let shown = |g: &Game| g.manifest(player).unwrap().evasion;
    let (profile_before, shown_before) = (evasion(&game), shown(&game));

    install(&mut game, &["dermal_lattice"]);
    assert!(evasion(&game) < profile_before);
    assert!(shown(&game) < shown_before);
}

#[test]
fn accuracy_comes_from_implants_in_the_profile() {
    let mut game = new_game();
    add_def(
        &mut game,
        ImplantDef {
            stats: ImplantStats {
                accuracy: 5,
                ..Default::default()
            },
            ..def("aim")
        },
    );
    let player = game.player_entity();
    let swing = crate::battle::Swing::default();
    let before = game.combatant_profile(player, swing).accuracy;
    let shown_before = game.manifest(player).unwrap().accuracy;
    install(&mut game, &["aim"]);
    assert!(game.combatant_profile(player, swing).accuracy > before);
    assert!(game.manifest(player).unwrap().accuracy > shown_before);
}

#[test]
fn a_missing_def_contributes_nothing() {
    let mut game = new_game();
    let before = snapshot(&game);
    install(&mut game, &["no_such_implant"]);
    assert_eq!(snapshot(&game), before);
}
