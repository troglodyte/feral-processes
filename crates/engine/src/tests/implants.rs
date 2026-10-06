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

fn hook_def(id: &str, hook: ImplantHook) -> ImplantDef {
    ImplantDef {
        hooks: vec![hook],
        ..def(id)
    }
}

fn power_after_a_wait(game: &mut Game) -> f32 {
    let player = game.player_entity();
    let before = game.world.get::<PowerReserve>(player).unwrap().get();
    game.wait();
    before - game.world.get::<PowerReserve>(player).unwrap().get()
}

#[test]
fn upkeep_scales_with_load_and_low_power_mode_reduces_it_too() {
    let mut game = new_game();
    let bare = power_after_a_wait(&mut game);
    assert!(bare > 0.0);

    install(&mut game, &["dermal_lattice", "overclock_spine"]);
    let loaded = power_after_a_wait(&mut game);
    assert!(
        (loaded - bare * crate::implants::drain_factor(7)).abs() < 1e-4,
        "{loaded} vs {bare}"
    );

    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().unlocked = vec![Perk::LowPowerMode; 3];
    let reduced = power_after_a_wait(&mut game);
    assert!(reduced < loaded, "LowPowerMode must cut implant upkeep too");
}

#[test]
fn capture_odds_rise_by_exactly_the_hooks_pct() {
    let mut game = new_game();
    let before = game.player_decompiler_bonuses().capture_boost_pct;
    install(&mut game, &["ghost_handshake"]);
    assert_eq!(
        game.player_decompiler_bonuses().capture_boost_pct,
        before + 10
    );
}

#[test]
fn drop_boost_applies_in_the_stack_and_not_on_the_surface() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !game.equipment_drops_for(s).is_empty())
        .expect("a species with equipment drops");
    let before = game.equipment_drops_for(&species);
    install(&mut game, &["black_ledger"]);
    assert_eq!(
        game.equipment_drops_for(&species),
        before,
        "unchanged on the surface"
    );

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    let in_stack = game.equipment_drops_for(&species);
    assert!(game.is_underground());
    for ((_, base), (_, boosted)) in before.iter().zip(&in_stack) {
        assert!((boosted - base * 1.25).abs() < 1e-6);
    }
}

#[test]
fn xp_boost_raises_player_xp() {
    let mut game = new_game();
    add_def(&mut game, hook_def("xp", ImplantHook::XpBoost(50)));
    install(&mut game, &["xp"]);
    let player = game.player_entity();
    game.award_player_xp(player, 10);
    assert_eq!(game.world.get::<Experience>(player).unwrap().xp, 15);
}

#[test]
fn routine_slots_gain_one_from_overclock_spine() {
    let mut game = new_game();
    let player = game.player_entity();
    let before = game.routine_slots(player);
    install(&mut game, &["overclock_spine"]);
    assert_eq!(game.routine_slots(player), before + 1);
}

fn trace_after_a_cache(game: &mut Game) -> u32 {
    game.world.insert_resource(crate::resources::Trace(0));
    game.raise_trace(crate::tuning::TRACE_PER_CACHE);
    game.trace()
}

#[test]
fn trace_rises_faster_with_a_downside_and_slower_with_a_damp_and_never_stops() {
    let mut game = new_game();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    let plain = trace_after_a_cache(&mut game);

    install(&mut game, &["black_ledger"]);
    assert!(trace_after_a_cache(&mut game) > plain);

    add_def(&mut game, hook_def("damp", ImplantHook::TraceDamp(100)));
    install(&mut game, &["damp"]);
    assert!(trace_after_a_cache(&mut game) >= 1, "a damp never stops it");

    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().unlocked = vec![Perk::Obfuscation; 20];
    assert!(
        trace_after_a_cache(&mut game) >= 1,
        "the obfuscation floor still holds"
    );
}
