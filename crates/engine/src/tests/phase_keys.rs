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

/// The Points screen's "after" figure is what the spend then produces, keys
/// included.
#[test]
fn the_points_preview_matches_the_spend_with_keys_held() {
    use crate::attributes::AttributeId;
    use crate::components::StatPoints;
    let mut game = new_game();
    let mut db = game.world.resource::<PhaseKeyDb>().clone();
    let pct = StatPct {
        max_hp: 40,
        atk: 40,
        mitigation: 40,
        decompiler: 40,
        max_power: 40,
    };
    db.set(keyed(1, pct_effect(pct)));
    db.set(keyed(10, pct_effect(pct)));
    game.world.insert_resource(db);
    let player = game.player_entity();
    game.grant_phase_key(1);
    game.grant_phase_key(10);
    game.world.get_mut::<StatPoints>(player).unwrap().0 = 3;
    let spend = [
        (AttributeId::from("parity"), 2),
        (AttributeId::from("analysis"), 1),
    ];

    let bonus = game.stat_bonus(player);
    let mut attrs = game.attributes_of(player);
    let db = game.attribute_db();
    for (id, points) in &spend {
        let base = db.get(id).unwrap().base;
        attrs.set(id, attrs.get(id).unwrap_or(base) + *points as i32);
    }
    let preview = bonus.apply(crate::progression::derive(
        &game.derived_base(player),
        &attrs,
        &db,
    ));
    game.spend_stat_points(StatOwner::Player, &spend).unwrap();
    let actual = stats(&game);
    assert_eq!(preview.max_hp, actual.max_hp);
    assert_eq!(preview.atk, actual.atk);
    assert_eq!(preview.mitigation, actual.mitigation);
    assert_eq!(
        preview.decompiler,
        game.world.get::<Decompiler>(player).unwrap().skill
    );
    assert_eq!(
        preview.max_power,
        game.world
            .get::<crate::components::Derived>(player)
            .unwrap()
            .max_power
    );
}

#[test]
fn the_count_ignores_stray_high_bits() {
    let keys = PhaseKeys {
        held: 0xFC00 | 0b100,
        ..PhaseKeys::default()
    };
    assert_eq!(keys.count(), 1);
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

// ---- drops, gate, warp, achievements ----

fn guardian_species(game: &Game) -> String {
    game.species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("a boss species ships in assets/species")
        .id
}

fn corpse(game: &mut Game, species: &str) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id()
}

fn stand_underground(game: &mut Game) {
    game.world.insert_resource(Locale::Stack {
        depth: 1,
        frames: 6,
        x: 1,
        y: 1,
        facing: crate::stack::Dir::North,
        entrance: (0, 0),
    });
}

/// One guardian kill in the Stack. Returns whether the zone's key is held
/// afterwards.
fn kill_guardian(game: &mut Game) -> bool {
    stand_underground(game);
    let species = guardian_species(game);
    let wild = corpse(game, &species);
    game.award_loot(wild, 0.0);
    let zone = game.world.resource::<ZoneLevel>().0;
    game.world
        .get::<PhaseKeys>(game.player_entity())
        .unwrap()
        .holds(zone)
}

fn keys(game: &Game) -> PhaseKeys {
    *game.world.get::<PhaseKeys>(game.player_entity()).unwrap()
}

#[test]
fn the_roll_is_a_pure_function_of_its_inputs() {
    use crate::game::phase_keys::phase_key_roll;
    for seed in [0, 1, 4471, u32::MAX] {
        for zone in 1..=10 {
            for kill in 0..4 {
                assert_eq!(
                    phase_key_roll(seed, zone, kill),
                    phase_key_roll(seed, zone, kill)
                );
            }
        }
    }
    let hits = (0..2000).filter(|&seed| phase_key_roll(seed, 3, 0)).count();
    assert!(
        (500..800).contains(&hits),
        "about a third of 2000 seeds should drop, got {hits}"
    );
}

#[test]
fn the_third_eligible_kill_always_drops() {
    for seed in 0..40 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut kills = 0;
        while !kill_guardian(&mut game) {
            kills += 1;
            assert!(
                kills < crate::tuning::PHASE_KEY_GUARANTEE_KILLS,
                "seed {seed}"
            );
        }
    }
}

#[test]
fn a_miss_is_counted_and_a_held_key_is_not_rolled_again() {
    let mut game = new_game();
    let seed = game.world.resource::<crate::base_grid::BaseGrid>().seed();
    let first_drops = crate::game::phase_keys::phase_key_roll(seed, 1, 0);
    let held = kill_guardian(&mut game);
    assert_eq!(held, first_drops);
    assert_eq!(keys(&game).misses, u32::from(!first_drops));
    if held {
        let before = keys(&game);
        kill_guardian(&mut game);
        assert_eq!(keys(&game), before, "a held key is ineligible");
    }
}

#[test]
fn a_surface_boss_never_drops_a_key() {
    let mut game = new_game();
    let species = guardian_species(&game);
    for _ in 0..5 {
        let wild = corpse(&mut game, &species);
        game.award_loot(wild, 0.0);
    }
    assert_eq!(keys(&game), PhaseKeys::default());
}

#[test]
fn no_key_drops_past_zone_ten() {
    let mut game = new_game();
    game.world.resource_mut::<ZoneLevel>().0 = 11;
    for _ in 0..5 {
        kill_guardian(&mut game);
    }
    assert_eq!(keys(&game), PhaseKeys::default());
}

#[test]
fn a_drop_raises_the_screen_and_the_log_line() {
    let mut game = new_game();
    while !kill_guardian(&mut game) {}
    let mut screens = Vec::new();
    while let Some(n) = game
        .world
        .resource_mut::<crate::resources::Notifications>()
        .pop()
    {
        screens.push(n);
    }
    let screen = screens
        .iter()
        .find(|n| n.title == "Phase Key Recovered")
        .expect("the drop should queue its modal screen");
    assert!(screen.body.contains("Origin Vector"), "{}", screen.body);
    assert!(
        screen.detail.is_some(),
        "the effect line rides as the detail"
    );
    assert!(
        game.message_log(60)
            .iter()
            .any(|l| l.text.contains("Origin Vector")),
        "the drop should write a log line naming the key"
    );
}

#[test]
fn breaching_clears_the_miss_counter() {
    let mut game = new_game();
    let player = game.player_entity();
    game.world.get_mut::<PhaseKeys>(player).unwrap().misses = 2;
    game.enter_next_zone();
    assert_eq!(keys(&game).misses, 0);
}

#[test]
fn the_same_seed_gives_the_same_outcomes() {
    let run = || {
        let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut outcomes = Vec::new();
        for zone in 1..=4 {
            for _ in 0..3 {
                outcomes.push((zone, keys(&game).misses, kill_guardian(&mut game)));
            }
            game.enter_next_zone();
        }
        outcomes
    };
    assert_eq!(run(), run());
}

#[test]
fn the_roll_leaves_the_rng_stream_alone() {
    let mut draws = Vec::new();
    for pre_held in [false, true] {
        let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        if pre_held {
            game.grant_phase_key(1);
        }
        kill_guardian(&mut game);
        let next: u64 = game.world.resource_mut::<GameRng>().0.random();
        draws.push(next);
    }
    assert_eq!(
        draws[0], draws[1],
        "rolling for a key must not consume a draw from the shared stream"
    );
}

fn portal_game() -> Game {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(crate::items::ids::PORTAL_FRAGMENT), 200);
    stand_in_base(&mut game);
    game
}

#[test]
fn the_gate_is_open_past_zone_ten_and_for_a_held_key() {
    let mut game = new_game();
    assert!(game.phase_key_gate(1).is_err());
    assert!(game.phase_key_gate(11).is_ok());
    game.grant_phase_key(1);
    assert!(game.phase_key_gate(1).is_ok());
    assert!(game.phase_key_gate(2).is_err());
}

#[test]
fn a_portal_cannot_be_built_without_the_key() {
    let mut game = portal_game();
    let err = game.place_structure("portal", 1, 0, None).unwrap_err();
    assert!(err.contains("Zone 1 Phase Key"), "{err}");
    assert!(find_structure_by_kind(&mut game, "portal").is_none());

    game.grant_phase_key(1);
    place_now(&mut game, "portal", 1, 0).unwrap();
}

#[test]
fn a_portal_builds_freely_in_zone_eleven() {
    let mut game = portal_game();
    game.world.resource_mut::<ZoneLevel>().0 = 11;
    place_now(&mut game, "portal", 1, 0).unwrap();
}

#[test]
fn stepping_onto_a_standing_portal_without_the_key_is_refused() {
    let mut game = portal_game();
    game.grant_phase_key(1);
    place_now(&mut game, "portal", 1, 0).unwrap();
    // An old save can hold a portal the build gate never saw.
    let player = game.player_entity();
    game.world.get_mut::<PhaseKeys>(player).unwrap().held = 0;
    let tick = game.world.resource::<GameClock>().tick;

    game.move_player(1, 0);

    assert_eq!(game.world.resource::<ZoneLevel>().0, 1, "no breach");
    assert!(
        find_structure_by_kind(&mut game, "portal").is_some(),
        "the refused portal is not consumed"
    );
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        tick,
        "a refusal is free"
    );
    assert!(
        game.message_log(20)
            .iter()
            .any(|l| l.text.contains("Zone 1 Phase Key")),
        "the refusal says why"
    );
}

#[test]
fn warping_grants_the_key_of_every_zone_left_behind() {
    let mut game = new_game();
    game.warp_to_zone(4).unwrap();
    let held = keys(&game);
    assert_eq!(held.zones().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(held.misses, 0);
}

#[test]
fn the_phase_key_achievements_fire() {
    use crate::achievements::{AchievementDb, AchievementId, Profile};
    let mut game = new_game();
    let earned = |game: &Game, id: &str| {
        game.world
            .resource::<Profile>()
            .contains(&AchievementId::from(id))
    };
    assert!(
        game.world
            .resource::<AchievementDb>()
            .get(&AchievementId::from("phase_key_3"))
            .is_some()
    );
    game.tick();
    assert!(!earned(&game, "phase_key_3"));

    game.grant_phase_key(3);
    game.tick();
    assert!(earned(&game, "phase_key_3"));
    assert!(!earned(&game, "phase_key_1"));
    assert!(!earned(&game, "phase_keys_all"));

    for zone in 1..=10 {
        game.grant_phase_key(zone);
    }
    game.tick();
    assert!(earned(&game, "phase_key_10"));
    assert!(earned(&game, "phase_keys_all"));
    assert!(!earned(&game, "basin_escaped"));

    let player = game.player_entity();
    game.world
        .get_mut::<PhaseKeys>(player)
        .unwrap()
        .story_complete = true;
    game.tick();
    assert!(earned(&game, "basin_escaped"));
}

#[test]
fn every_shipped_key_has_an_achievement_that_pays_nothing() {
    use crate::achievements::{AchievementDb, AchievementId, Reward, Trigger};
    let game = new_game();
    let db = game.world.resource::<AchievementDb>();
    for zone in 1..=10u32 {
        let def = db
            .get(&AchievementId::from(format!("phase_key_{zone}").as_str()))
            .unwrap_or_else(|| panic!("no achievement for zone {zone}"));
        assert_eq!(def.trigger, Trigger::PhaseKeyFound(zone));
        assert_eq!(def.reward, Reward::None);
    }
}
