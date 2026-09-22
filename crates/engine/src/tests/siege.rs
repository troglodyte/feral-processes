//! The siege clock — `resources::SiegePressure`'s accrual, its three holds
//! and (Task 5) the dev console's two rows.
//!
//! `raids.rs`'s clock tests are the model throughout: a siege's meter is
//! `RaidPressure`'s shape, built the same way, for the reasons recorded
//! there.

use super::support::*;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, SIEGE_MIN_ZONE,
    SIEGE_PRESSURE_JITTER_PERCENT, SIEGE_PRESSURE_PER_ZONE, SIEGE_PRESSURE_THRESHOLD,
    SIEGE_PRESSURE_WARN_PERCENT,
};
use crate::*;

/// Establishes `base_is_established()` with the minimum staff and
/// structures, and (unless `raidable` is false for every one of them)
/// leaves exactly the first structure carrying `Durability` — a legal
/// siege target — so a test can isolate the "nothing standing" hold by
/// passing `false`.
fn establish_base(game: &mut Game, raidable: bool) {
    for _ in 0..BASE_ESTABLISHED_STAFF {
        spawn_tamed(game, 10, 3);
    }
    for i in 0..BASE_ESTABLISHED_STRUCTURES {
        let mut entity = game.world.spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position {
                x: 10 + i as i32,
                y: 10,
            },
        ));
        if raidable && i == 0 {
            entity.insert(Durability { hp: 30, max_hp: 30 });
        }
    }
}

/// Ticks `game.siege_check()` until the pressure resets to zero (a siege
/// fired) or `max` ticks have passed, and says which.
fn ticks_to_fire(game: &mut Game, max: u32) -> Option<u32> {
    for t in 1..=max {
        game.siege_check();
        if game.siege_pressure() == 0 {
            return Some(t);
        }
    }
    None
}

/// The widest number of ticks the jitter can make an interval take to fire,
/// at `zone`'s accrual rate — the bound every hold test ticks past to prove
/// the clock has certainly reached its target.
fn latest_possible_tick(zone: u32) -> u32 {
    let accrual = SIEGE_PRESSURE_PER_ZONE * zone;
    (SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100).div_ceil(accrual)
}

/// The opening sector's clock never starts, and it starts from zero on the
/// breach into sector 2 rather than arriving with banked pressure —
/// `the_opening_sector_builds_no_raid_pressure`'s exact shape.
#[test]
fn the_opening_sector_builds_no_siege_pressure() {
    assert_eq!(
        SIEGE_MIN_ZONE, 2,
        "sieges are exempt in the opening sector and nowhere else"
    );

    let mut game = Game::new(900, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 1);
    for _ in 0..5_000 {
        game.siege_check();
    }
    assert_eq!(
        game.siege_pressure(),
        0,
        "the opening sector's siege clock never starts"
    );

    set_zone(&mut game, 2);
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        SIEGE_PRESSURE_PER_ZONE * 2,
        "and it starts from zero on the breach into sector 2"
    );
}

/// The first hold: no base. A tick that decides a siege happens but has no
/// base to besiege holds its pressure rather than spending it, and the very
/// next tick a base exists is the tick the held siege fires.
#[test]
fn a_tick_with_no_base_holds_the_clock_until_one_exists() {
    let mut game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "a run with no base holds its clock rather than spending it, but the clock read {}",
        game.siege_pressure()
    );

    establish_base(&mut game, true);
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick the base is established is the tick the held siege fires"
    );
}

/// The second hold: nothing standing to besiege. An established base with
/// every structure stripped of `Durability` holds its clock exactly as a
/// base that does not exist yet does, and the tick something raidable
/// stands is the tick the held siege fires.
#[test]
fn a_tick_with_nothing_to_besiege_holds_the_clock() {
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, false);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "an established base with nothing raidable standing holds its clock, but the clock read {}",
        game.siege_pressure()
    );

    let target: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Structure>>();
        query.iter(&game.world).collect()
    };
    game.world
        .entity_mut(target[0])
        .insert(Durability { hp: 30, max_hp: 30 });
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick something is standing to besiege is the tick the held siege fires"
    );
}

/// The third hold: another fight already running. A siege must not open —
/// or, before Task 8, resolve — on top of one, and it fires the tick the
/// other fight ends.
#[test]
fn a_tick_with_another_fight_running_holds_the_clock() {
    let mut game = Game::new(903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, true);

    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "a siege must not fire on top of another fight, but the clock read {}",
        game.siege_pressure()
    );

    game.world.remove_resource::<BattleState>();
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick the other fight ends is the tick the held siege fires"
    );
}

/// The interval is drawn once, not once a tick.
///
/// A stable `next_at` across two ticks would pass equally well against an
/// implementation that redraws the same interval and discards the roll, so
/// this compares the shared `GameRng` stream rather than only the field:
/// the drawn value must be the stream's *first* draw, and two ticks must
/// leave the stream exactly one draw further along.
#[test]
fn the_interval_is_drawn_once_not_once_a_tick() {
    let seed = 904;
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    game.siege_check();
    let after_first = game
        .world
        .resource::<crate::resources::SiegePressure>()
        .next_at;
    game.siege_check();
    let after_second = game
        .world
        .resource::<crate::resources::SiegePressure>()
        .next_at;
    assert_eq!(
        after_first, after_second,
        "the interval must not be redrawn on the second tick"
    );
    let target = after_first.expect("the first tick must draw an interval");

    // A reference game built from the same seed, asked to make exactly one
    // draw of the same shape `draw_siege_interval` makes. If `game` drew
    // more than once, or drew and discarded, the two streams diverge here.
    let mut reference = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let low = SIEGE_PRESSURE_THRESHOLD * (100 - SIEGE_PRESSURE_JITTER_PERCENT) / 100;
    let high = SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100;
    let one_draw: u32 = reference
        .world
        .resource_mut::<GameRng>()
        .0
        .random_range(low..=high);
    assert_eq!(
        one_draw, target,
        "the clock's one draw must be the stream's first"
    );

    let after_game: u64 = game.world.resource_mut::<GameRng>().0.random();
    let after_reference: u64 = reference.world.resource_mut::<GameRng>().0.random();
    assert_eq!(
        after_game, after_reference,
        "two ticks must cost exactly the one draw the reference also made"
    );
}

/// A siege in sector 6 comes at three times the rate of one in sector 2 —
/// asserted on the tick counts each takes to fire, not on the constants,
/// since the two games share a seed and so draw the identical target.
///
/// `ticks2 = ceil(target / 2)` and `ticks6 = ceil(target / 6)` can disagree
/// with an exact `3 * ticks6` by the rounding each `ceil` introduces; both
/// lie in `[target / 2, target / 2 + 3)`, which bounds the gap between them
/// at 2 ticks whatever the drawn target turns out to be.
#[test]
fn a_deeper_sector_sieges_three_times_as_fast() {
    let seed = 905;
    let max = latest_possible_tick(2) + 1;

    let mut zone2 = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut zone2, 2);
    establish_base(&mut zone2, true);
    let ticks2 = ticks_to_fire(&mut zone2, max).expect("sector 2 must fire within its own bound");

    let mut zone6 = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut zone6, 6);
    establish_base(&mut zone6, true);
    let ticks6 =
        ticks_to_fire(&mut zone6, max).expect("sector 6 must fire well within sector 2's bound");

    assert!(
        ticks2.abs_diff(3 * ticks6) <= 2,
        "sector 6 should fire in about a third of sector 2's ticks: {ticks2} vs 3x{ticks6}"
    );
}

/// The dev console's trigger fires the real thing — `stage_siege` — and
/// leaves the clock untouched, `dev_force_raid`'s shape exactly.
#[test]
fn forcing_a_siege_stages_one_without_touching_the_clock() {
    let mut game = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = *game.world.resource::<crate::resources::SiegePressure>();

    game.dev_force_siege();

    assert_eq!(
        *game.world.resource::<crate::resources::SiegePressure>(),
        before,
        "forcing a siege must not touch the clock, matching the sweep's own dev trigger"
    );
    let entry = game
        .message_log(20)
        .into_iter()
        .find(|e| e.text.contains("siege is forming"));
    assert!(entry.is_some(), "the forced fire must be the real one");
}

/// Winding the clock lands exactly on the drawn interval's own warn point
/// and leaves `warned` clear — `winding_the_clock_reaches_the_warning_and_not_the_sweep`'s
/// shape, one press working whatever the jitter rolled.
#[test]
fn winding_the_siege_clock_reaches_the_warn_point() {
    let mut game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.dev_wind_siege_clock();

    let pressure = *game.world.resource::<crate::resources::SiegePressure>();
    let target = pressure
        .next_at
        .expect("winding draws the interval it winds to");
    assert_eq!(
        pressure.level,
        target * SIEGE_PRESSURE_WARN_PERCENT / 100,
        "the clock must land on the drawn interval's own warn point"
    );
    assert!(!pressure.warned, "and leave the line for the next cycle");
}
