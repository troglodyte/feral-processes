//! `AbilityEffect::HealOverTime`: a built routine's Heal + Trickle, which
//! restores a share of the Core's heal at the end of each round in both
//! combat models. Its out-of-battle counterpart, the Field Regen Core, is a
//! `FieldBuff` and never touches this path.

use super::support::{battle_with_a_pack_of, test_assets_dir};
use super::tactical::tactical_fight;
use crate::Game;
use crate::abilities::{AbilityDef, AbilityEffect, AbilityTarget};
use crate::components::{Derived, Stats, StatusEffects};
use crate::resources::DifficultyMode;
use crate::tuning::AFFINITY_NEUTRAL;

const PER_ROUND: i32 = 7;
const ROUNDS: u32 = 2;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn hot_def() -> AbilityDef {
    AbilityDef {
        id: "test_hot".into(),
        name: "Test Hot".into(),
        description: "d".into(),
        target: AbilityTarget::OneAlly,
        effect: AbilityEffect::HealOverTime {
            power: PER_ROUND,
            spread: 0,
            rounds: ROUNDS,
        },
        cooldown: 1,
        power_cost: 1.0,
        accuracy: 0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
        shape: None,
        range: None,
        fx: None,
        charge: None,
    }
}

fn hurt_player(game: &mut Game) -> i32 {
    let player = game.player_entity();
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.max_hp = 200;
    stats.hp = 20;
    stats.hp
}

fn hp(game: &Game) -> i32 {
    game.world.get::<Stats>(game.player_entity()).unwrap().hp
}

/// What one round of the routine restores at the player's level, so the
/// assertion tracks `scaled_hp_power` rather than restating its curve.
fn per_round(game: &Game) -> i32 {
    let level = game.ability_user_level(game.player_entity());
    crate::abilities::scaled_hp_power(PER_ROUND, level, AFFINITY_NEUTRAL)
}

/// Runs one end-of-round upkeep in whichever model is open.
type Upkeep = fn(&mut Game);

fn assert_heals_each_round(game: &mut Game, upkeep: Upkeep) {
    let start = hurt_player(game);
    let player = game.player_entity();
    game.use_ability(&hot_def(), player, "TestBot", &[player]);
    assert_eq!(hp(game), start, "nothing is restored the moment it is run");

    // The round it landed in is not one of the rounds it lasts.
    upkeep(game);
    assert_eq!(hp(game), start, "the landing round heals nothing");

    let step = per_round(game);
    for n in 1..=ROUNDS as i32 {
        upkeep(game);
        assert_eq!(hp(game), start + step * n, "round {n} restores its share");
    }
    upkeep(game);
    assert_eq!(
        hp(game),
        start + step * ROUNDS as i32,
        "it stops after the rounds it names"
    );
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_empty(),
        "the regeneration is gone once spent"
    );
}

#[test]
fn a_group_fight_heals_each_round_then_stops() {
    let mut game = game(41001);
    battle_with_a_pack_of(&mut game, 1, 1000);
    assert_heals_each_round(&mut game, |g| {
        let player = g.player_entity();
        g.tick_round_status_effects(player);
    });
}

#[test]
fn a_battle_map_heals_each_round_then_stops() {
    let mut game = game(41002);
    tactical_fight(&mut game, 1, 1000);
    assert_heals_each_round(&mut game, |g| g.tactical_round_upkeep());
}

#[test]
fn a_locked_body_is_not_healed_by_it() {
    let mut game = game(41003);
    battle_with_a_pack_of(&mut game, 1, 1000);
    let start = hurt_player(&mut game);
    let player = game.player_entity();
    game.use_ability(&hot_def(), player, "TestBot", &[player]);
    game.arm_status(player, &"locked".into(), 10, 0);
    game.tick_round_status_effects(player);
    game.tick_round_status_effects(player);
    assert_eq!(hp(&game), start, "restore_hp honours HealBlock");
}

#[test]
fn a_high_status_resist_does_not_shorten_it() {
    let mut game = game(41004);
    battle_with_a_pack_of(&mut game, 1, 1000);
    let player = game.player_entity();
    game.world.get_mut::<Derived>(player).unwrap().status_resist = 90;
    game.use_ability(&hot_def(), player, "TestBot", &[player]);
    let held = &game.world.get::<StatusEffects>(player).unwrap().active;
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].remaining, ROUNDS);
}

#[test]
fn a_routine_with_no_rounds_is_refused_at_load() {
    let mut def = hot_def();
    def.effect = AbilityEffect::HealOverTime {
        power: 5,
        spread: 0,
        rounds: 0,
    };
    assert!(crate::abilities::validate(&def).is_err());
    assert!(crate::abilities::validate(&hot_def()).is_ok());
}
