//! The enemy-strength band: a run-level difficulty knob expressed as a
//! fractional zone step.
//!
//! Every band is at or above the shipped curve — `Standard` is the floor and
//! is exactly the game as it was before the knob existed, which is what
//! leaves every `balance_sim` curve and every field-ramp test gating this
//! feature for free.

use super::support::test_assets_dir;
use crate::components::{Hostile, Position, Stats};
use crate::resources::{DifficultyMode, EnemyStrength, GameRng, ZoneLevel, ZoneSpawnPoint};
use crate::tuning::{DANGER_RAMP_TILES, OPENING_RING_TILES};
use crate::Game;
use bevy_ecs::prelude::*;
use rand::SeedableRng;

/// The tile the field ramp measures from, as `tests/spawning.rs` reads it.
fn danger_origin(game: &Game) -> (i32, i32) {
    let spawn = game.world.resource::<ZoneSpawnPoint>();
    (spawn.x, spawn.y)
}

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Nothing below the shipped curve, and `Standard` is exactly zero steps.
/// The ladder only ever makes the game harder — the whole point of the
/// feature.
#[test]
fn no_band_sits_below_the_shipped_curve() {
    assert_eq!(EnemyStrength::default(), EnemyStrength::Standard);
    assert_eq!(EnemyStrength::Standard.zone_steps(), 0.0);
    for band in EnemyStrength::all() {
        assert!(
            band.zone_steps() >= 0.0,
            "{band:?} is below Standard, which this ladder must never be"
        );
    }
    let steps: Vec<f32> = EnemyStrength::all().iter().map(|b| b.zone_steps()).collect();
    for pair in steps.windows(2) {
        assert!(
            pair[1] > pair[0],
            "the ladder must climb: {pair:?} does not"
        );
    }
}

/// A new run starts on the floor, so an existing player notices nothing.
#[test]
fn a_new_run_starts_at_standard() {
    assert_eq!(game(9301).enemy_strength(), EnemyStrength::Standard);
}

/// `Standard` is byte-identical to the game as shipped: the field ramp is
/// the pure geometry ratio it always was, at every zone and every distance.
#[test]
fn standard_leaves_the_field_ramp_exactly_as_it_was() {
    let mut g = game(9302);
    let (sx, sy) = danger_origin(&g);
    for zone in 1..=8u32 {
        g.world.insert_resource(ZoneLevel(zone));
        let here = ZoneLevel(zone).stat_multiplier() as f32;
        let next = ZoneLevel(zone + 1).stat_multiplier() as f32;
        assert_eq!(
            g.field_stat_mult(sx, sy),
            1.0,
            "zone {zone}'s ring moved at Standard"
        );
        let far = g.field_stat_mult(sx + OPENING_RING_TILES + DANGER_RAMP_TILES, sy);
        assert!(
            (here * far - next).abs() < 1e-4,
            "zone {zone}'s far field moved at Standard"
        );
    }
}

/// The property the whole shape exists for: a band is a **whole number of
/// zone steps**, so playing zone N at `High` is arithmetically the zone N+1
/// fixture `balance_sim` already sweeps. No new balance bound is needed.
#[test]
fn a_band_is_a_number_of_zone_steps_on_the_existing_curve() {
    let mut g = game(9303);
    let (sx, sy) = danger_origin(&g);
    for zone in 1..=8u32 {
        g.world.insert_resource(ZoneLevel(zone));
        let here = ZoneLevel(zone).stat_multiplier() as f32;
        for band in EnemyStrength::all() {
            g.force_enemy_strength(band);
            // Inside the ring, where the distance ramp contributes nothing,
            // the band is the only term.
            let reached = here * g.field_stat_mult(sx, sy);
            let want = here + crate::tuning::ZONE_STAT_STEP as f32 * band.zone_steps();
            assert!(
                (reached - want).abs() < 1e-4,
                "zone {zone} at {band:?} reached x{reached}, not x{want}"
            );
        }
    }
}

/// The band and the distance ramp are the **same addend at the same seam**,
/// so they add rather than compound. At the far field, `High` is exactly two
/// zone steps out — one bought by walking, one by the band.
#[test]
fn the_band_and_the_distance_ramp_add_rather_than_compound() {
    let mut g = game(9304);
    let (sx, sy) = danger_origin(&g);
    g.force_enemy_strength(EnemyStrength::High);
    let far = sx + OPENING_RING_TILES + DANGER_RAMP_TILES;
    for zone in 1..=8u32 {
        g.world.insert_resource(ZoneLevel(zone));
        let here = ZoneLevel(zone).stat_multiplier() as f32;
        let reached = here * g.field_stat_mult(far, sy);
        let want = ZoneLevel(zone + 2).stat_multiplier() as f32;
        assert!(
            (reached - want).abs() < 1e-4,
            "zone {zone}'s far field at High reached x{reached}, not zone {}'s x{want}",
            zone + 2
        );
    }
}

/// The knob reaches the Stack too — one ratio, applied where `trace_stat_mult`
/// already composes.
#[test]
fn the_band_reaches_underground() {
    let mut g = game(9305);
    super::support::descend(&mut g);
    let standard = g.stack_depth_multiplier();
    g.force_enemy_strength(EnemyStrength::Critical);
    let raised = g.stack_depth_multiplier();
    assert!(
        raised > standard,
        "a Stack spawn ignored the band: x{standard} then x{raised}"
    );
}

/// What the player actually feels: the same seeded spawn on the same tile
/// comes out stronger.
#[test]
fn a_higher_band_spawns_a_stronger_body() {
    let mut g = game(9306);
    let (sx, sy) = danger_origin(&g);
    let spawn = |g: &mut Game| -> (i32, i32) {
        g.world.insert_resource(GameRng(rand::rngs::StdRng::seed_from_u64(7717)));
        let mult = g.field_stat_mult(sx, sy);
        let e = g
            .spawn_wild_creature_scaled("glitch", sx, sy, mult, false)
            .expect("glitch ships in the test assets");
        let s = *g.world.get::<Stats>(e).expect("a spawn carries Stats");
        g.world.despawn(e);
        (s.max_hp, s.atk)
    };
    let standard = spawn(&mut g);
    g.force_enemy_strength(EnemyStrength::Critical);
    let critical = spawn(&mut g);
    assert!(
        critical.0 > standard.0 && critical.1 > standard.1,
        "Critical spawned {critical:?} against Standard's {standard:?}"
    );
}

/// Changing the band re-stocks the ground already walked, the three calls a
/// breach makes — so the knob is felt now rather than at the next spawn.
#[test]
fn changing_the_band_restocks_the_local_wild() {
    let mut g = game(9307);
    let before: Vec<Entity> = g
        .world
        .query_filtered::<Entity, With<Hostile>>()
        .iter(&g.world)
        .collect();
    assert!(
        !before.is_empty(),
        "the fixture has to start with wild bodies for this to mean anything"
    );

    g.set_enemy_strength(EnemyStrength::Critical)
        .expect("nothing refuses this on the open map");

    let after: Vec<Entity> = g
        .world
        .query_filtered::<Entity, With<Hostile>>()
        .iter(&g.world)
        .collect();
    assert!(
        before.iter().any(|e| !after.contains(e)),
        "not one local body was replaced; the re-stock did not run"
    );
    assert!(
        !after.is_empty(),
        "the re-stock cleared the ground and never re-populated it"
    );
}

/// `clear_local_wild` despawns bodies, and a body in `BattleState::groups`
/// is exactly what `end_battle` exists to stop anyone deleting mid-fight.
#[test]
fn the_band_is_refused_during_a_fight() {
    let mut g = game(9308);
    let player = g.player_entity();
    let enemy = {
        let pos = *g.world.get::<Position>(player).unwrap();
        g.spawn_wild_creature_scaled("glitch", pos.x + 3, pos.y, 1.0, false)
            .expect("glitch ships in the test assets")
    };
    super::support::insert_battle(&mut g, player, vec![enemy]);

    let refusal = g.set_enemy_strength(EnemyStrength::High);

    assert!(refusal.is_err(), "the band changed mid-fight");
    assert_eq!(
        g.enemy_strength(),
        EnemyStrength::Standard,
        "a refused change still moved the band"
    );
}

/// The band is the run's, so it has to come back with the run. A real
/// save→load, not a RON round trip: a `#[serde(skip)]` survives that one.
#[test]
fn the_band_survives_a_save_and_load() {
    let mut g = game(9309);
    g.force_enemy_strength(EnemyStrength::Severe);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_difficulty_{}.bin",
        std::process::id()
    ));
    g.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.enemy_strength(), EnemyStrength::Severe);
}

/// The console's one gesture. Wraps back to the floor rather than sticking
/// at the top, so one row can reach every band.
#[test]
fn the_ladder_cycles_back_to_standard() {
    let mut band = EnemyStrength::Standard;
    for _ in 0..EnemyStrength::all().len() - 1 {
        band = band.next();
        assert_ne!(band, EnemyStrength::Standard);
    }
    assert_eq!(
        band.next(),
        EnemyStrength::Standard,
        "the top band must wrap to the floor"
    );
}
