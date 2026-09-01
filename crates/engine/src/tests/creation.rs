//! `CharacterChoice` and `Game::new_with` — the foundation the rest of the
//! character-creation feature builds on. `new_and_new_with_default_produce_
//! the_same_player` is the load-bearing one: it is what protects the
//! ~1,600 `Game::new` call sites across the suite from a regression here.

use super::support::*;
use crate::achievements::MainStat;
use crate::tuning;
use crate::*;

fn stats_at(index: MainStat, points: u32) -> [u32; 4] {
    let mut stats = [0u32; 4];
    let i = MainStat::all().iter().position(|s| *s == index).unwrap();
    stats[i] = points;
    stats
}

#[test]
fn new_and_new_with_default_produce_the_same_player() {
    let seed = 90_001;
    let a = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let b = Game::new_with(
        seed,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &CharacterChoice::default(),
    )
    .unwrap();

    let pa = a.player_entity();
    let pb = b.player_entity();

    let sa = a.world.get::<Stats>(pa).unwrap();
    let sb = b.world.get::<Stats>(pb).unwrap();
    assert_eq!(sa.hp, sb.hp);
    assert_eq!(sa.max_hp, sb.max_hp);
    assert_eq!(sa.atk, sb.atk);
    assert_eq!(sa.mitigation, sb.mitigation);

    let ga = a.world.get::<Glyph>(pa).unwrap();
    let gb = b.world.get::<Glyph>(pb).unwrap();
    assert_eq!(ga.ch, gb.ch);
    assert_eq!(ga.color, gb.color);

    assert_eq!(
        a.world.get::<Inventory>(pa).unwrap().items,
        b.world.get::<Inventory>(pb).unwrap().items
    );
    assert_eq!(
        a.world.get::<Routines>(pa).unwrap().0,
        b.world.get::<Routines>(pb).unwrap().0
    );
}

#[test]
fn creation_points_are_additive_over_the_baseline() {
    let points = tuning::CREATION_STAT_POINTS; // Integrity costs 1, so this fits exactly.
    let choice = CharacterChoice {
        stats: stats_at(MainStat::Integrity, points),
        ..CharacterChoice::default()
    };
    assert_eq!(
        choice.cost(),
        Some(points * tuning::CREATION_COST_INTEGRITY)
    );

    let game = Game::new_with(
        90_002,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();

    let expected_max_hp =
        tuning::PLAYER_BASE_STATS.max_hp + (points * tuning::CREATION_GAIN_INTEGRITY) as i32;
    assert_eq!(stats.max_hp, expected_max_hp);
    // A run must not start damaged — `MainStat::Integrity`'s own trap.
    assert_eq!(stats.hp, stats.max_hp);
}

#[test]
fn mitigation_costs_more_than_a_point() {
    let pool = tuning::CREATION_STAT_POINTS;
    // "Spending the whole pool" on an axis priced above 1 buys only as many
    // whole units as the pool covers — the remainder is unspendable.
    let units = pool / tuning::CREATION_COST_DEF;
    let choice = CharacterChoice {
        stats: stats_at(MainStat::Def, units),
        ..CharacterChoice::default()
    };
    assert!(choice.cost().is_some());

    let game = Game::new_with(
        90_003,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();
    let gained = stats.mitigation - tuning::PLAYER_BASE_STATS.mitigation;

    assert_eq!(gained, (pool / tuning::CREATION_COST_DEF) as i32);
    assert_ne!(
        gained, pool as i32,
        "Def costs more than a point per point of mitigation"
    );
}

#[test]
fn an_overspent_choice_is_refused() {
    // One point over the pool at Atk's 1-for-1 rate — cheapest possible
    // overspend.
    let overspent = CharacterChoice {
        stats: stats_at(MainStat::Atk, tuning::CREATION_STAT_POINTS + 1),
        ..CharacterChoice::default()
    };
    assert_eq!(overspent.cost(), None);

    let game = Game::new_with(
        90_004,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &overspent,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();
    assert_eq!(
        stats.atk,
        tuning::PLAYER_BASE_STATS.atk,
        "an overspent choice must fall back to no spend, not a clamped one"
    );
}
