//! Charge routines in the group model: the shared lifecycle in
//! `game/charge.rs` and its seams with the round, the stun, the flee and the
//! teardown.

use crate::battle::{BattleAction, SpecialTarget};
use crate::components::*;
use crate::game::charge::{AimedVictim, ChargeChoice, charge_choice};
use crate::resources::*;
use crate::*;

use super::support::*;

const ID: &str = "test_charge";
const TARGET_HP: i32 = 10_000;

fn charge_def(power: i32, rounds: u32, cooldown: u32, power_cost: u32) -> AbilityDef {
    ron::from_str(&format!(
        r#"(id: "{ID}", name: "Test Charge", description: "d",
        target: OneEnemyGroupFront, effect: Damage(power: {power}), cooldown: {cooldown},
        power_cost: {power_cost}, charge: Some((rounds: {rounds})))"#
    ))
    .unwrap()
}

/// A player holding the charge routine against one tough hostile, the
/// player's own HP raised out of the way of the hostile's swings.
fn charge_battle(seed: u32, def: AbilityDef) -> (Game, Entity, Entity) {
    let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    charge_battle_in(game, def)
}

fn charge_battle_in(mut game: Game, def: AbilityDef) -> (Game, Entity, Entity) {
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 1, TARGET_HP);
    game.world.resource_mut::<AbilityDb>().insert(def);
    game.world
        .entity_mut(player)
        .insert(Routines(vec![ID.to_string()]));
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.hp = 100_000;
    stats.max_hp = 100_000;
    (game, player, pack[0])
}

fn charge_index(game: &Game) -> usize {
    game.battle_special_options(0)
        .into_iter()
        .find(|o| o.name == "Test Charge")
        .expect("the routine is installed")
        .index
}

fn start_charge_round(game: &mut Game) {
    let ability = charge_index(game);
    resolve_round_with(
        game,
        BattleAction::Special {
            ability,
            target: SpecialTarget::EnemyGroup { group: 0 },
            image: None,
        },
    );
}

fn progress(game: &Game, entity: Entity) -> Option<u32> {
    game.world.get::<Charging>(entity).map(|c| c.progress)
}

fn on_cooldown(game: &Game, entity: Entity) -> bool {
    game.world
        .get::<AbilityCooldowns>(entity)
        .is_some_and(|c| c.0.contains_key(ID))
}

fn hp(game: &Game, entity: Entity) -> i32 {
    game.world.get::<Stats>(entity).unwrap().hp
}

#[test]
fn starting_a_charge_lands_nothing_pays_power_and_leaves_the_cooldown_unarmed() {
    let (mut game, player, wild) = charge_battle(1, charge_def(40, 3, 5, 10));
    let before = game.world.get::<PowerReserve>(player).unwrap().get();

    start_charge_round(&mut game);

    assert_eq!(progress(&game, player), Some(1));
    assert_eq!(
        hp(&game, wild),
        TARGET_HP,
        "nothing lands on the start turn"
    );
    assert!(
        game.world.get::<PowerReserve>(player).unwrap().get() < before,
        "the start pays Power"
    );
    assert!(!on_cooldown(&game, player), "the cooldown arms at the end");
}

#[test]
fn holding_to_full_charge_fires_on_its_own_and_arms_the_cooldown() {
    let (mut game, player, wild) = charge_battle(2, charge_def(400, 3, 5, 0));
    start_charge_round(&mut game);
    resolve_round_with(&mut game, BattleAction::ChargeHold);
    assert_eq!(progress(&game, player), Some(2));
    resolve_round_with(&mut game, BattleAction::ChargeHold);
    assert_eq!(progress(&game, player), Some(3));
    assert_eq!(hp(&game, wild), TARGET_HP, "still nothing has landed");
    assert_eq!(
        game.battle_active_slot(),
        None,
        "at full charge the release is automatic, so nobody is asked"
    );

    for _ in 0..40 {
        if progress(&game, player).is_none() {
            break;
        }
        game.battle_resolve_round();
    }

    assert_eq!(progress(&game, player), None, "the auto-fire ended it");
    assert!(on_cooldown(&game, player));
}

/// Damage the charge deals when released at `progress` of `rounds`, off a
/// forced plain hit so the figure is exact.
fn released_damage(progress: u32, rounds: u32) -> i32 {
    let (mut game, player, wild) = charge_battle(3, charge_def(40, rounds, 0, 0));
    game.world.entity_mut(player).insert(Charging {
        ability: ID.to_string(),
        rounds,
        progress,
        aim: ChargeAim::Group(vec![wild]),
    });
    force_the_next_attack_to_land(&mut game);
    game.charge_turn(player, Some(&BattleAction::ChargeRelease), player);
    TARGET_HP - hp(&game, wild)
}

#[test]
fn releasing_early_fires_at_k_over_n_of_full_power() {
    let atk = {
        let (game, player, _) = charge_battle(3, charge_def(40, 4, 0, 0));
        game.effective_atk(player)
    };
    let half = released_damage(2, 4);
    let full = released_damage(4, 4);
    assert!(half > atk, "the half charge still lands");
    assert_eq!(
        full - atk,
        2 * (half - atk),
        "two of four turns is half the full hit"
    );
}

#[test]
fn a_stun_cancels_the_charge_where_it_lands_and_keeps_power_spent() {
    let (mut game, player, _) = charge_battle(4, charge_def(40, 3, 5, 10));
    start_charge_round(&mut game);
    let spent = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(progress(&game, player).is_some());

    game.arm_status(player, &StatusId::from("stun"), 1, 0);

    assert_eq!(progress(&game, player), None);
    assert!(on_cooldown(&game, player), "a cancel starts the cooldown");
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        spent,
        "and refunds nothing"
    );
}

#[test]
fn a_gone_aim_retargets_the_hit_to_the_front_group() {
    let (mut game, player, first) = charge_battle(5, charge_def(400, 2, 0, 0));
    let second = battle_with_a_pack_of(&mut game, 1, TARGET_HP)[0];
    let species = game.world.get::<Creature>(second).unwrap().species.clone();
    let groups = vec![
        crate::battle::EnemyGroup {
            species: species.clone(),
            members: vec![first],
        },
        crate::battle::EnemyGroup {
            species,
            members: vec![second],
        },
    ];
    insert_battle_with_groups(&mut game, player, groups);
    game.world.entity_mut(player).insert(Charging {
        ability: ID.to_string(),
        rounds: 2,
        progress: 2,
        aim: ChargeAim::Group(vec![second]),
    });
    game.apply_damage(second, TARGET_HP);
    game.reap_dead_members(player);
    force_the_next_attack_to_land(&mut game);

    game.charge_turn(player, None, player);

    assert!(hp(&game, first) < TARGET_HP, "the surviving group takes it");
}

#[test]
fn a_failed_flee_cancels_the_charge_and_arms_its_cooldown() {
    // Only a failed attempt leaves the fight standing to be inspected; a
    // success tears everything down and would pass for the wrong reason.
    let found = (0..64).any(|seed| {
        let (mut game, player, wild) = charge_battle(seed, charge_def(40, 3, 5, 0));
        // Outgunned, so the escape roll is not close to certain.
        {
            let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
            stats.hp = 1_000_000_000;
            stats.max_hp = 1_000_000_000;
        }
        start_charge_round(&mut game);
        if game.battle_flee() {
            return false;
        }
        assert_eq!(progress(&game, player), None);
        assert!(on_cooldown(&game, player));
        true
    });
    assert!(found, "no seed in 0..64 failed the flee");
}

#[test]
fn teardown_clears_charging_on_every_side() {
    let (mut game, player, wild) = charge_battle(7, charge_def(40, 3, 5, 0));
    for who in [player, wild] {
        game.world.entity_mut(who).insert(Charging {
            ability: ID.to_string(),
            rounds: 3,
            progress: 1,
            aim: ChargeAim::Group(vec![player]),
        });
    }

    game.clear_battle_status_effects(player, Some(wild));

    assert_eq!(progress(&game, player), None);
    assert_eq!(progress(&game, wild), None);
}

#[test]
fn a_charging_slot_offers_only_hold_and_release_and_refuses_other_plans_for_them() {
    let (mut game, _, _) = charge_battle(8, charge_def(40, 3, 5, 0));
    assert!(
        game.battle_set_action(0, BattleAction::ChargeHold).is_err(),
        "nothing is charging yet"
    );
    start_charge_round(&mut game);

    let rows = game.battle_action_options(0);
    let kinds: Vec<_> = rows.iter().map(|o| (o.kind, o.key)).collect();
    assert_eq!(
        kinds,
        vec![
            (crate::battle::ActionKind::ChargeHold, 'h'),
            (crate::battle::ActionKind::ChargeRelease, 'x'),
        ]
    );
    assert!(game.battle_set_action(0, BattleAction::ChargeHold).is_ok());
}

#[test]
fn the_roster_tags_the_charger_with_progress_and_aim() {
    let (mut game, _, _) = charge_battle(9, charge_def(40, 3, 5, 0));
    start_charge_round(&mut game);

    let view = game.battle_view().unwrap();
    let tag = view.party[0].charge.clone().unwrap();
    assert_eq!((tag.k, tag.n, tag.target.as_str()), (1, 3, "A"));
    assert!(view.groups[0].charge.is_none());
}

#[test]
fn an_all_attack_does_not_strand_a_charging_slot() {
    let (mut game, player, _) = charge_battle(10, charge_def(400, 4, 5, 0));
    start_charge_round(&mut game);

    assert!(game.battle_auto_round());

    assert_eq!(
        progress(&game, player),
        Some(2),
        "the AI held a charge that kills nothing"
    );
}

fn wild_charger(seed: u32, player_hp: i32) -> (Game, Entity, Entity) {
    let (mut game, player, wild) = charge_battle(seed, charge_def(3000, 3, 5, 0));
    game.world
        .entity_mut(wild)
        .insert(Routines(vec![ID.to_string()]));
    game.world.get_mut::<Stats>(player).unwrap().hp = player_hp;
    (game, player, wild)
}

#[test]
fn a_hostile_winds_up_instead_of_swinging() {
    let (mut game, player, wild) = wild_charger(11, 100_000);

    resolve_round_with(&mut game, BattleAction::Defend);

    // Its first turn is the start, so it has not hit anyone yet.
    assert_eq!(progress(&game, wild), Some(1));
    assert_eq!(hp(&game, player), 100_000);
    let tag = game.battle_view().unwrap().groups[0]
        .charge
        .clone()
        .unwrap();
    assert_eq!((tag.k, tag.n, tag.target.as_str()), (1, 3, "you"));
}

#[test]
fn a_hostile_releases_early_when_the_hit_would_kill_and_otherwise_holds() {
    let (mut game, player, wild) = wild_charger(12, 100_000);
    resolve_round_with(&mut game, BattleAction::Defend);
    resolve_round_with(&mut game, BattleAction::Defend);
    assert_eq!(
        progress(&game, wild),
        Some(2),
        "nothing to kill, so it holds"
    );

    game.world.get_mut::<Stats>(player).unwrap().hp = 5;
    game.world.get_mut::<Charging>(wild).unwrap().aim = ChargeAim::Group(vec![player]);
    resolve_round_with(&mut game, BattleAction::Defend);

    assert_eq!(
        progress(&game, wild),
        None,
        "a kill is in reach, so it fires"
    );
    assert!(on_cooldown(&game, wild));
}

#[test]
fn a_hostile_cancels_when_nothing_is_left_in_its_aim() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 1);
    enlist(&mut game, companion);
    let (mut game, player, wild) = charge_battle_in(game, charge_def(3000, 3, 5, 0));
    game.world
        .entity_mut(wild)
        .insert(Routines(vec![ID.to_string()]));
    game.world.entity_mut(wild).insert(Charging {
        ability: ID.to_string(),
        rounds: 3,
        progress: 1,
        aim: ChargeAim::Group(vec![companion]),
    });
    game.apply_damage(companion, 10);

    game.charge_turn(wild, None, player);

    assert_eq!(progress(&game, wild), None);
    assert!(on_cooldown(&game, wild));
    assert_eq!(hp(&game, player), 100_000);
}

fn victim(hp: i32) -> AimedVictim {
    let combatant = |range| battle::Combatant {
        accuracy: 100.0,
        evasion: 0.0,
        atk: 0,
        range,
        crit: 0.0,
        fumble: 0.0,
    };
    AimedVictim {
        hp,
        attacker: combatant(battle::DamageRange::centred(100, 0)),
        defender: combatant(battle::DamageRange::centred(0, 0)),
        mitigation: 0,
        deflection: 0,
    }
}

#[test]
fn the_ai_releases_on_a_kill_cancels_on_an_empty_aim_and_otherwise_holds() {
    assert_eq!(charge_choice(2, 4, &[]), ChargeChoice::Cancel);
    assert_eq!(charge_choice(2, 4, &[victim(1000)]), ChargeChoice::Hold);
    assert_eq!(charge_choice(2, 4, &[victim(20)]), ChargeChoice::Release);
    assert_eq!(
        charge_choice(1, 4, &[victim(1000), victim(20)]),
        ChargeChoice::Release,
        "one killable body in the aim is enough"
    );
    assert_eq!(
        charge_choice(1, 4, &[victim(80)]),
        ChargeChoice::Hold,
        "a quarter of the hit does not reach 80"
    );
}
