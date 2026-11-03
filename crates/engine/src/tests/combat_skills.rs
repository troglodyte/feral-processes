//! What a combat skill level is worth at each door, and what fighting
//! practises (ADR 0040). Skills draw no RNG, and a body without `Skills`
//! is exactly today's.

use super::support::*;
use crate::abilities::AbilityDef;
use crate::game::combat_skills as cs;
use crate::skills::{Skill, Skills};
use crate::tuning::*;
use crate::*;
use bevy_ecs::entity::Entity;

fn new_game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn fighter(game: &mut Game, levels: &[(Skill, u32)]) -> Entity {
    let e = spawn_tamed(game, 10_000, 3);
    game.world.entity_mut(e).insert(Skills::default());
    for (s, l) in levels {
        set_skill_level(game, e, *s, *l);
    }
    e
}

/// A tamed body with no `Skills` at all: the fixture seeds species knacks.
fn bare(game: &mut Game) -> Entity {
    let e = spawn_tamed(game, 10_000, 3);
    game.world.entity_mut(e).remove::<Skills>();
    e
}

fn dummy(game: &mut Game) -> Entity {
    let e = game.spawn_wild_creature("worm", 1, 1).unwrap();
    let mut stats = game.world.get_mut::<Stats>(e).unwrap();
    stats.hp = 1_000_000;
    stats.max_hp = 1_000_000;
    e
}

/// Total practice ever earned, so a level crossed mid-test still counts.
fn practice(game: &Game, e: Entity, s: Skill) -> u32 {
    let t = game.world.get::<Skills>(e).unwrap().track(s);
    (0..t.level)
        .map(|l| crate::skills::practice_to_next(l).unwrap())
        .sum::<u32>()
        + t.practice
}

fn swing(game: &Game, e: Entity) -> battle::Swing {
    battle::Swing::plain(game.natural_range_of(e))
}

fn routine(game: &Game, id: &str) -> AbilityDef {
    game.world
        .resource::<crate::abilities::AbilityDb>()
        .get(id)
        .unwrap()
        .clone()
}

#[test]
fn every_effect_is_identity_at_ten_and_symmetric_about_it() {
    assert_eq!(cs::aim_scale(10), 1.0);
    assert_eq!(cs::force_scale(10), 1.0);
    assert_eq!(cs::defence_points(10), 0);
    assert_eq!(cs::routine_price_scale(10), 1.0);
    assert!((cs::aim_scale(0) - 0.9).abs() < 1e-6 && (cs::aim_scale(20) - 1.1).abs() < 1e-6);
    assert!((cs::force_scale(0) - 0.9).abs() < 1e-6 && (cs::force_scale(20) - 1.1).abs() < 1e-6);
    assert_eq!(cs::defence_points(0), -10);
    assert_eq!(cs::defence_points(20), 10);
    assert!((cs::routine_price_scale(0) - 1.1).abs() < 1e-6);
    assert!((cs::routine_price_scale(20) - 0.9).abs() < 1e-6);
}

#[test]
fn aim_scales_the_profile_and_a_body_without_skills_is_untouched() {
    let mut game = new_game(40001);
    let plain = bare(&mut game);
    let neutral = fighter(&mut game, &[(Skill::Aim, 10)]);
    let low = fighter(&mut game, &[(Skill::Aim, 0)]);
    let high = fighter(&mut game, &[(Skill::Aim, 20)]);
    let acc = |g: &Game, e| g.combatant_profile(e, swing(g, e)).accuracy;
    let base = acc(&game, plain);
    assert!(base > 0.0);
    assert_eq!(acc(&game, neutral), base);
    assert!((acc(&game, low) / base - 0.9).abs() < 1e-5);
    assert!((acc(&game, high) / base - 1.1).abs() < 1e-5);
}

#[test]
fn force_sets_the_combatants_damage_scale() {
    let mut game = new_game(40002);
    let plain = bare(&mut game);
    let low = fighter(&mut game, &[(Skill::Force, 0)]);
    let high = fighter(&mut game, &[(Skill::Force, 20)]);
    let scale = |g: &Game, e| g.combatant_profile(e, swing(g, e)).damage_scale;
    assert_eq!(scale(&game, plain), 1.0);
    assert!((scale(&game, low) - 0.9).abs() < 1e-6);
    assert!((scale(&game, high) - 1.1).abs() < 1e-6);
}

fn combatant(damage_scale: f64) -> battle::Combatant {
    battle::Combatant {
        accuracy: 1000.0,
        evasion: 1.0,
        atk: 10,
        range: battle::DamageRange { min: 10, max: 10 },
        crit: 0.0,
        fumble: 0.0,
        damage_scale,
    }
}

#[test]
fn a_landed_hit_and_the_forecast_both_carry_the_damage_scale() {
    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    // The hit chance is clamped below 1, so keep rolling until one lands.
    let mut hit = |scale| loop {
        let dmg =
            battle::resolve_attack(combatant(scale), combatant(1.0), &mut rng).damage_to_defender();
        if dmg > 0 {
            break dmg;
        }
    };
    assert_eq!(hit(1.0), 20);
    assert_eq!(hit(1.1), 22);
    assert_eq!(hit(0.9), 18);
    let e = |s| battle::expected_damage(combatant(s), combatant(1.0));
    assert!(e(1.1) > e(1.0) && e(1.0) > e(0.9));
}

#[test]
fn defence_adds_its_points_before_the_clamp() {
    let mut game = new_game(40003);
    let plain = bare(&mut game);
    let low = fighter(&mut game, &[(Skill::Defence, 0)]);
    let high = fighter(&mut game, &[(Skill::Defence, 20)]);
    for e in [plain, low, high] {
        game.world.get_mut::<Stats>(e).unwrap().mitigation = 30;
    }
    assert_eq!(game.effective_mitigation(plain), 30);
    assert_eq!(game.effective_mitigation(low), 20);
    assert_eq!(game.effective_mitigation(high), 40);
    game.world.get_mut::<Stats>(high).unwrap().mitigation = MAX_MITIGATION_PERCENT;
    assert_eq!(game.effective_mitigation(high), MAX_MITIGATION_PERCENT);
}

#[test]
fn routines_reprice_and_the_refusal_the_charge_and_the_page_agree() {
    let mut game = new_game(40004);
    let def = routine(&game, "null_route");
    let neutral = crate::abilities::routine_power_cost(&def, None);
    assert!(neutral > 0.0);
    assert_eq!(
        crate::abilities::routine_power_cost(&def, Some(10)),
        neutral
    );
    let low = crate::abilities::routine_power_cost(&def, Some(0));
    let high = crate::abilities::routine_power_cost(&def, Some(20));
    assert!(low > neutral && high < neutral);

    let e = fighter(&mut game, &[(Skill::Routines, 20)]);
    assert_eq!(game.routine_cost(e, &def), high);
    // The refusal quotes the same figure.
    *game.world.get_mut::<PowerReserve>(e).unwrap() = PowerReserve::new(high - 0.5, POWER_MAX);
    match game.ability_unavailable(e, &def) {
        Some(crate::game::combat::RoutineRefusal::Power { cost }) => assert_eq!(cost, high),
        other => panic!("expected a Power refusal, got {other:?}"),
    }
    // The page quotes it too.
    install_routine_for_test(&mut game, e, "null_route");
    let detail = game.routine_detail(&"null_route".to_string(), e).unwrap();
    assert_eq!(detail.power_cost, high);
    // The charge takes it.
    *game.world.get_mut::<PowerReserve>(e).unwrap() = PowerReserve::new(50.0, POWER_MAX);
    let before = game.world.get::<PowerReserve>(e).unwrap().get();
    game.spend_power(e, game.routine_cost(e, &def));
    let spent = before - game.world.get::<PowerReserve>(e).unwrap().get();
    assert!((spent - high).abs() < 1e-4);
}

#[test]
fn a_miss_practises_aim_not_force_and_a_hit_practises_both() {
    let mut game = new_game(40005);
    let target = dummy(&mut game);

    // Aimed far off, so a landed hit is rare and a miss is the common case.
    let off = fighter(&mut game, &[]);
    let mut s = swing(&game, off);
    s.accuracy = -1_000_000;
    let (mut rolls, mut landed) = (0, 0);
    for _ in 0..40 {
        let out = game.resolve_and_apply_attack(off, target, s);
        rolls += 1;
        landed += u32::from(out.damage_to_defender() > 0);
    }
    assert!(
        landed < rolls,
        "some rolls must miss for this to mean anything"
    );
    assert_eq!(
        practice(&game, off, Skill::Aim),
        rolls * AIM_PRACTICE_PER_ROLL
    );
    assert_eq!(
        practice(&game, off, Skill::Force),
        landed * FORCE_PRACTICE_PER_HIT
    );

    let on = fighter(&mut game, &[]);
    let mut s = swing(&game, on);
    s.accuracy = 1_000_000;
    let out = game.resolve_and_apply_attack(on, target, s);
    assert!(out.damage_to_defender() > 0);
    assert_eq!(practice(&game, on, Skill::Aim), AIM_PRACTICE_PER_ROLL);
    assert_eq!(practice(&game, on, Skill::Force), FORCE_PRACTICE_PER_HIT);
}

#[test]
fn the_defender_practises_defence_on_every_roll_at_it() {
    let mut game = new_game(40006);
    let attacker = dummy(&mut game);
    let defender = fighter(&mut game, &[]);
    let s = swing(&game, attacker);
    for _ in 0..3 {
        game.resolve_and_apply_attack(attacker, defender, s);
    }
    assert_eq!(
        practice(&game, defender, Skill::Defence),
        3 * DEFENCE_PRACTICE_PER_ROLL
    );
    assert_eq!(practice(&game, defender, Skill::Aim), 0);
}

#[test]
fn a_paid_charge_practises_routines_and_a_free_one_does_not() {
    let mut game = new_game(40007);
    let e = fighter(&mut game, &[]);
    *game.world.get_mut::<PowerReserve>(e).unwrap() = PowerReserve::new(50.0, POWER_MAX);
    game.spend_power(e, 0.0);
    assert_eq!(practice(&game, e, Skill::Routines), 0);
    // The charge precedes the effect at every site, so a fizzle keeps it and
    // counts: this is the one practice site.
    game.spend_power(e, 2.0);
    assert_eq!(
        practice(&game, e, Skill::Routines),
        ROUTINES_PRACTICE_PER_CHARGE
    );
}

#[test]
fn a_refused_routine_practises_nothing() {
    let mut game = new_game(40008);
    let e = fighter(&mut game, &[]);
    let def = routine(&game, "null_route");
    *game.world.get_mut::<PowerReserve>(e).unwrap() = PowerReserve::new(0.0, POWER_MAX);
    assert!(game.ability_unavailable(e, &def).is_some());
    assert_eq!(practice(&game, e, Skill::Routines), 0);
}

#[test]
fn running_a_routine_through_use_ability_never_practises_routines() {
    // `use_ability` is the path a passive, the wielded proc and a hostile
    // share, and none of them pays: only `spend_power` practises.
    let mut game = new_game(40009);
    let e = fighter(&mut game, &[]);
    let def = routine(&game, "hot_patch");
    game.use_ability(&def, e, "Test", &[e]);
    assert_eq!(practice(&game, e, Skill::Routines), 0);
}

#[test]
fn a_wild_body_practises_nothing_and_gains_no_skills() {
    let mut game = new_game(40011);
    let wild = dummy(&mut game);
    let other = dummy(&mut game);
    let s = swing(&game, wild);
    game.resolve_and_apply_attack(wild, other, s);
    assert!(game.world.get::<Skills>(wild).is_none());
    assert!(game.world.get::<Skills>(other).is_none());
}

#[test]
fn a_level_gained_in_a_fight_is_announced() {
    let mut game = new_game(40012);
    let target = dummy(&mut game);
    let e = fighter(&mut game, &[]);
    let s = swing(&game, e);
    let rolls = crate::skills::practice_to_next(0).unwrap() / AIM_PRACTICE_PER_ROLL + 1;
    for _ in 0..rolls {
        game.resolve_and_apply_attack(e, target, s);
    }
    assert!(game.world.get::<Skills>(e).unwrap().level(Skill::Aim) >= 1);
    assert!(
        game.world
            .resource::<crate::resources::PendingSkillLevels>()
            .0
            .iter()
            .any(|(who, skill, _)| *who == e && *skill == Skill::Aim)
    );
}

#[test]
fn skills_take_no_rng_draws() {
    use crate::resources::GameRng;
    let run = |skilled: bool| {
        let mut game = new_game(40013);
        let target = dummy(&mut game);
        let e = bare(&mut game);
        if skilled {
            game.world.entity_mut(e).insert(Skills::default());
            for s in Skill::COMBAT {
                set_skill_level(&mut game, e, s, 10);
            }
        }
        let s = swing(&game, e);
        // Few enough rolls that no level is crossed, so every figure stays 1.0.
        let outcomes: Vec<_> = (0..15)
            .map(|_| game.resolve_and_apply_attack(e, target, s))
            .collect();
        let next: u64 = game.world.resource_mut::<GameRng>().0.random();
        (outcomes, next)
    };
    assert_eq!(run(true), run(false));
}
