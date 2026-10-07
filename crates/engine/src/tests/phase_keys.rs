//! Phase Keys: the database, the held state, the effects and the save path.

use super::support::*;
use crate::components::{Decompiler, PhaseKeys};
use crate::implants::{ImplantHook, ImplantSignature, ImplantStats};
use crate::phase_keys::{
    KeyEffect, PhaseKeyDb, PhaseKeyDef, StatPct, apply_key_pct, key_scaled_power,
};
use crate::*;

fn new_game() -> Game {
    Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn db_from(scratch: &str, files: &[(&str, &str)]) -> (PhaseKeyDb, Vec<String>) {
    let dir = scratch_assets_dir(scratch);
    std::fs::create_dir_all(&*dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    PhaseKeyDb::load_dir(&dir).unwrap()
}

fn key_text(zone: u32, name: &str) -> String {
    format!("(zone: {zone}, name: \"{name}\")")
}

fn keyed(zone: u32, effect: KeyEffect) -> PhaseKeyDef {
    PhaseKeyDef {
        zone,
        name: format!("Test {zone}"),
        flavour: String::new(),
        effect,
    }
}

fn pct_effect(pct: StatPct) -> KeyEffect {
    KeyEffect {
        stat_pct: pct,
        ..KeyEffect::default()
    }
}

fn stats(game: &Game) -> Stats {
    *game.world.get::<Stats>(game.player_entity()).unwrap()
}

// ---- the database ----

#[test]
fn a_missing_directory_is_all_fallbacks_and_silent() {
    let dir = scratch_assets_dir("pk_missing");
    let (db, warnings) = PhaseKeyDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(db.all().len() as u32, crate::tuning::PHASE_KEY_COUNT);
    assert_eq!(db.get(3).unwrap().name, "Phase Key 3");
    assert_eq!(db.get(3).unwrap().effect, KeyEffect::default());
}

#[test]
fn an_existing_directory_fills_every_gap_with_a_warning() {
    let (db, warnings) = db_from("pk_gaps", &[("a.ron", &key_text(2, "Two"))]);
    assert_eq!(db.get(2).unwrap().name, "Two");
    assert_eq!(db.get(1).unwrap().name, "Phase Key 1");
    assert_eq!(db.get(10).unwrap().name, "Phase Key 10");
    assert_eq!(warnings.len(), 9);
}

#[test]
fn malformed_out_of_range_and_negative_files_are_skipped_with_a_warning() {
    let (db, warnings) = db_from(
        "pk_bad",
        &[
            ("a.ron", "(zone: oops"),
            ("b.ron", &key_text(0, "Zero")),
            ("c.ron", &key_text(11, "Eleven")),
            (
                "d.ron",
                "(zone: 4, name: \"Neg\", effect: (stat_pct: (atk: -5)))",
            ),
        ],
    );
    for zone in 1..=10 {
        assert_eq!(db.get(zone).unwrap().name, format!("Phase Key {zone}"));
    }
    for needle in ["invalid", "zone 0", "zone 11", "negative"] {
        assert!(
            warnings.iter().any(|w| w.contains(needle)),
            "{needle}: {warnings:?}"
        );
    }
}

#[test]
fn a_duplicate_zone_keeps_the_first_file_by_name() {
    let (db, warnings) = db_from(
        "pk_dup",
        &[
            ("b.ron", &key_text(5, "Second")),
            ("a.ron", &key_text(5, "First")),
        ],
    );
    assert_eq!(db.get(5).unwrap().name, "First");
    assert!(warnings.iter().any(|w| w.contains("duplicate")));
}

#[test]
fn the_shipped_keys_fill_all_ten_slots_with_no_fallback() {
    let (db, warnings) = PhaseKeyDb::load_dir(&test_assets_dir().join("phase_keys")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    for zone in 1..=crate::tuning::PHASE_KEY_COUNT {
        let def = db.get(zone).unwrap();
        assert_eq!(def.zone, zone);
        assert!(
            !def.name.starts_with("Phase Key "),
            "zone {zone} is a fallback"
        );
        assert!(!def.flavour.is_empty(), "zone {zone} has no flavour");
        assert!(!def.effect.summary().is_empty(), "zone {zone} does nothing");
    }
    let game = new_game();
    assert_eq!(
        game.world.resource::<PhaseKeyDb>().get(1).unwrap().name,
        db.get(1).unwrap().name
    );
}

#[test]
fn a_summary_names_each_part_of_an_effect() {
    let effect = KeyEffect {
        stat_pct: StatPct {
            max_hp: 5,
            ..StatPct::default()
        },
        stats: ImplantStats {
            accuracy: 5,
            crit: 0.03,
            ..ImplantStats::default()
        },
        hooks: vec![ImplantHook::XpBoost(10)],
        signature: Some(ImplantSignature::DeadMansSwitch),
    };
    assert_eq!(
        effect.summary(),
        "+5% Integrity, +5 accuracy, +3% crit, +10% XP, Dead Man's Switch"
    );
}

// ---- the formula ----

#[test]
fn a_percent_adds_at_least_one_and_zero_adds_nothing() {
    assert_eq!(apply_key_pct(200, 5), 210);
    assert_eq!(apply_key_pct(10, 5), 11, "5% of 10 is 0, floored to +1");
    assert_eq!(apply_key_pct(0, 10), 1);
    assert_eq!(apply_key_pct(200, 0), 200);
    assert_eq!(key_scaled_power(100.0, 10), 110.0);
    assert_eq!(key_scaled_power(100.0, 0), 100.0);
}

// ---- held state and derived effects ----

#[test]
fn a_new_player_holds_nothing() {
    let game = new_game();
    let view = game.phase_keys();
    assert_eq!(view.slots.len(), 10);
    assert_eq!(view.held_count, 0);
    assert!(!view.story_complete);
    assert!(view.slots.iter().all(|slot| slot.held.is_none()));
}

#[test]
fn granting_a_key_shows_it_in_the_view_and_is_idempotent() {
    let mut game = new_game();
    assert!(game.grant_phase_key(3));
    assert!(!game.grant_phase_key(3));
    assert!(!game.grant_phase_key(0));
    assert!(!game.grant_phase_key(11));
    let view = game.phase_keys();
    assert_eq!(view.held_count, 1);
    let held = view.slots[2].held.as_ref().expect("zone 3 is held");
    assert_eq!(held.name, "Seam Lens");
    assert!(held.effect.contains("accuracy"));
    assert!(view.slots[0].held.is_none());
}

#[test]
fn percents_sum_before_they_apply_and_are_derived_not_baked() {
    let mut game = new_game();
    let mut db = game.world.resource::<PhaseKeyDb>().clone();
    db.set(keyed(
        1,
        pct_effect(StatPct {
            max_hp: 5,
            atk: 100,
            ..StatPct::default()
        }),
    ));
    db.set(keyed(
        2,
        pct_effect(StatPct {
            max_hp: 10,
            ..StatPct::default()
        }),
    ));
    game.world.insert_resource(db);
    let player = game.player_entity();
    game.recompute_derived(player);
    let before = stats(&game);

    game.grant_phase_key(1);
    game.grant_phase_key(2);
    let after = stats(&game);
    assert_eq!(after.max_hp, apply_key_pct(before.max_hp, 15));
    assert_eq!(after.atk, apply_key_pct(before.atk, 100));
    assert_eq!(after.mitigation, before.mitigation);

    // Derived, not baked: dropping the bits and re-deriving returns the
    // original figures, and doing it twice does not compound.
    game.recompute_derived(player);
    assert_eq!(stats(&game), after);
    game.world.get_mut::<PhaseKeys>(player).unwrap().held = 0;
    game.recompute_derived(player);
    assert_eq!(stats(&game), before);
}

#[test]
fn a_small_stat_still_gains_one() {
    let mut game = new_game();
    let mut db = game.world.resource::<PhaseKeyDb>().clone();
    db.set(keyed(
        1,
        pct_effect(StatPct {
            mitigation: 1,
            ..StatPct::default()
        }),
    ));
    game.world.insert_resource(db);
    let before = stats(&game).mitigation;
    game.grant_phase_key(1);
    assert_eq!(stats(&game).mitigation, before + 1);
}

#[test]
fn decompiler_and_power_percents_apply() {
    let mut game = new_game();
    let mut db = game.world.resource::<PhaseKeyDb>().clone();
    db.set(keyed(
        1,
        pct_effect(StatPct {
            decompiler: 50,
            max_power: 50,
            ..StatPct::default()
        }),
    ));
    game.world.insert_resource(db);
    let player = game.player_entity();
    let skill = game.world.get::<Decompiler>(player).unwrap().skill;
    let power = game
        .world
        .get::<crate::components::Derived>(player)
        .unwrap()
        .max_power;
    game.grant_phase_key(1);
    assert_eq!(
        game.world.get::<Decompiler>(player).unwrap().skill,
        apply_key_pct(skill, 50)
    );
    assert_eq!(
        game.world
            .get::<crate::components::Derived>(player)
            .unwrap()
            .max_power,
        key_scaled_power(power, 50)
    );
}

#[test]
fn flat_stats_ride_the_implant_readers() {
    let mut game = new_game();
    let player = game.player_entity();
    let (acc, eva) = game.hit_bonus(player);
    let crit = game
        .world
        .get::<crate::components::Derived>(player)
        .unwrap()
        .crit;
    let mut db = game.world.resource::<PhaseKeyDb>().clone();
    db.set(keyed(
        1,
        KeyEffect {
            stats: ImplantStats {
                accuracy: 5,
                evasion: 4,
                crit: 0.03,
                ..ImplantStats::default()
            },
            ..KeyEffect::default()
        },
    ));
    game.world.insert_resource(db);
    game.grant_phase_key(1);
    assert_eq!(game.hit_bonus(player), (acc + 5, eva + 4));
    let now = game
        .world
        .get::<crate::components::Derived>(player)
        .unwrap()
        .crit;
    assert!((now - crit - 0.03).abs() < 1e-9);
}

#[test]
fn hooks_are_summed_with_the_implants() {
    let mut game = new_game();
    let player = game.player_entity();
    let before = game.routine_slots(player);
    game.grant_phase_key(7);
    assert_eq!(game.routine_slots(player), before + 1);
    let xp_before = game.world.get::<Experience>(player).unwrap().xp;
    game.grant_phase_key(4);
    game.award_player_xp(player, 10);
    assert_eq!(
        game.world.get::<Experience>(player).unwrap().xp - xp_before,
        11
    );
}

fn open_battle(game: &mut Game) {
    let wild = spawn_wild_on_player_tile(game);
    let groups = game.group_pack(vec![wild]);
    game.begin_battle(groups);
}

#[test]
fn the_switch_key_saves_once_and_does_not_double_with_the_implant() {
    let mut game = new_game();
    game.grant_phase_key(10);
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::Implants>(player)
        .unwrap()
        .installed = vec![crate::implants::ImplantId::from("dead_mans_switch")];
    open_battle(&mut game);
    let power = |g: &Game| g.world.get::<PowerReserve>(player).unwrap().get();
    let before = power(&game);

    game.apply_damage(player, 10_000);
    assert_eq!(stats(&game).hp, 1);
    assert_eq!(before - power(&game), crate::tuning::DEAD_MANS_SWITCH_POWER);
    game.apply_damage(player, 10_000);
    assert_eq!(stats(&game).hp, 0, "it fires once per battle");
}

#[test]
fn the_switch_key_alone_saves_without_an_implant() {
    let mut game = new_game();
    game.grant_phase_key(10);
    open_battle(&mut game);
    let player = game.player_entity();
    game.apply_damage(player, 10_000);
    assert_eq!(stats(&game).hp, 1);
}

#[test]
fn a_creature_never_feels_the_player_keys() {
    let mut game = new_game();
    game.grant_phase_key(1);
    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(game.key_defs(wild).is_empty());
    assert_eq!(game.key_pct(wild), StatPct::default());
}

// ---- save ----

fn temp_save(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("feral_phase_keys_{tag}_{}.sav", std::process::id()))
}

#[test]
fn held_misses_and_the_story_flag_survive_a_save_and_load() {
    // Through `Game::save`/`Game::load`: the field is `#[serde(default)]`, so
    // only the real path shows it defaulting away.
    let mut game = new_game();
    game.grant_phase_key(2);
    game.grant_phase_key(9);
    let player = game.player_entity();
    {
        let mut keys = game.world.get_mut::<PhaseKeys>(player).unwrap();
        keys.misses = 2;
        keys.story_complete = true;
    }
    let before = stats(&game);
    let path = temp_save("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = *loaded
        .world
        .get::<PhaseKeys>(loaded.player_entity())
        .unwrap();
    assert_eq!(restored.held, (1 << 1) | (1 << 8));
    assert_eq!(restored.misses, 2);
    assert!(restored.story_complete);
    assert_eq!(
        stats(&loaded).max_hp,
        before.max_hp,
        "effects re-derive on load"
    );
    assert_eq!(stats(&loaded).atk, before.atk);
}
