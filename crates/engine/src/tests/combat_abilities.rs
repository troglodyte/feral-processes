//! Data-driven abilities: multi-target shapes, cooldowns, and the
//! back-rank kill handling the enemy-side shapes depend on.

use crate::components::*;
use crate::resources::*;
use crate::*;

use super::support::*;

/// Spawns `count` hostile members of one species into a single group and
/// starts a battle against them, so back-rank indices actually exist.
/// Stats are set by hand rather than rolled, because these tests assert on
/// exact HP.
///
/// Placed deep and far on purpose: a group's size ceiling is the local
/// `max_group_size`, which at a zone-1 spawn point is one member — there
/// would be no back rank to test. The hand-set stats are what make the move
/// free, since nothing here reads the distance or zone scaling it implies.
fn battle_with_a_pack_of(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + GROUP_SIZE_STEP_TILES * 7, spawn.y);
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position { x: x + i as i32, y },
                    Stats {
                        hp,
                        max_hp: hp,
                        atk: 0,
                        def: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(game, player, members.clone());
    members
}

#[test]
fn a_back_rank_member_killed_outright_leaves_the_group_and_awards_its_xp() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 3, 20);
    let back = pack[2];

    // Level and XP together, because awarding enough XP to level up wraps
    // `xp` back toward zero — a bare `xp` comparison would read a level-up
    // as "no reward".
    let before = {
        let xp = game.world.get::<Experience>(player).unwrap();
        (xp.level, xp.xp)
    };
    game.apply_damage(back, 20);
    assert!(!game.creature_alive(back), "the back member should be down");

    let ended = game.reap_dead_members(player);
    assert!(!ended, "two members are still standing");

    let members = &game.world.resource::<BattleState>().groups[0].members;
    assert_eq!(
        members.len(),
        2,
        "the dead back member must leave the group"
    );
    assert!(
        !members.contains(&back),
        "a corpse must not stay in the group where it can be promoted to front"
    );
    let after = {
        let xp = game.world.get::<Experience>(player).unwrap();
        (xp.level, xp.xp)
    };
    assert!(
        after > before,
        "a back-rank kill awards XP exactly as a front kill does"
    );
}

#[test]
fn killing_every_member_of_the_only_group_ends_the_battle() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 3, 20);

    for member in &pack {
        game.apply_damage(*member, 20);
    }
    let ended = game.reap_dead_members(player);

    assert!(ended, "clearing every group ends the encounter");
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "a won battle removes BattleState"
    );
}

#[test]
fn reaping_walks_every_index_so_two_deaths_in_one_group_both_resolve() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 4, 20);

    // Front and a middle member, so removal has to survive the indices
    // shifting underneath it.
    game.apply_damage(pack[0], 20);
    game.apply_damage(pack[2], 20);
    game.reap_dead_members(player);

    let members = &game.world.resource::<BattleState>().groups[0].members;
    assert_eq!(members.len(), 2, "both corpses must be cleared in one pass");
    assert_eq!(
        members,
        &vec![pack[1], pack[3]],
        "the survivors keep their relative order"
    );
}

/// A game whose species ships the three new multi-target abilities, so the
/// shapes can be exercised without depending on shipped kit assignments.
fn game_with_a_sweeper() -> (Game, Entity) {
    const SWEEPER: &str = r#"(
        id: "test_sweeper",
        name: "Test Sweeper",
        glyph: 's',
        color: Red,
        base_hp: 30,
        base_atk: 10,
        base_def: 2,
        taming_difficulty: 0.5,
        habitats: [OpenGrid],
        base_speed: 10,
        moves: [(name: "Poke", power: 3)],
        abilities: [
            (id: "cascade_overflow"),
            (id: "broadcast_storm"),
            (id: "redundancy_sync"),
        ],
    )"#;
    let dir = modded_assets_dir("sweeper", &[], &[], &[("test_sweeper.ron", SWEEPER)]);
    let mut game = Game::new(31, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let sweeper = game
        .world
        .spawn((
            Creature {
                species: "test_sweeper".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 30,
                max_hp: 30,
                atk: 10,
                def: 2,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.add_companion(sweeper).unwrap();
    (game, sweeper)
}

#[test]
fn a_whole_group_ability_damages_every_member_not_just_the_front() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let pack = battle_with_a_pack_of(&mut game, 3, 50);

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    for (rank, member) in pack.iter().enumerate() {
        let hp = game.world.get::<Stats>(*member).unwrap().hp;
        assert!(
            hp < 50,
            "the member at rank {rank} should have taken damage, still at {hp}"
        );
    }
}

#[test]
fn an_all_enemies_ability_reaches_every_group_including_past_engagement_range() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    // Four distinct species so `group_pack` yields four groups — more than
    // ENGAGED_GROUPS, which is the point.
    let species: Vec<String> = game
        .species_defs()
        .into_iter()
        .take(4)
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(species.len(), 4, "the shipped set must supply four species");
    let enemies: Vec<Entity> = species
        .iter()
        .enumerate()
        .map(|(i, id)| {
            game.world
                .spawn((
                    Creature {
                        species: id.clone(),
                    },
                    Hostile,
                    Position {
                        x: 5 + i as i32,
                        y: 5,
                    },
                    Stats {
                        hp: 50,
                        max_hp: 50,
                        atk: 0,
                        def: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(&mut game, player, enemies.clone());
    assert_eq!(game.living_group_count(), 4, "four species, four groups");

    companion_uses_special(
        &mut game,
        sweeper,
        1, // broadcast_storm
        battle::SpecialTarget::AllEnemies,
    );

    for (group, enemy) in enemies.iter().enumerate() {
        let hp = game.world.get::<Stats>(*enemy).unwrap().hp;
        assert!(hp < 50, "group {group} should have been hit, still at {hp}");
    }
}

#[test]
fn a_whole_party_heal_raises_every_living_member_and_skips_the_downed() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    let downed = spawn_tamed(&mut game, 20, 5);
    game.add_companion(downed).unwrap();
    battle_with_a_pack_of(&mut game, 1, 200);

    for (entity, hp) in [(player, 10), (sweeper, 10), (downed, 0)] {
        game.world.get_mut::<Stats>(entity).unwrap().hp = hp;
    }

    companion_uses_special(&mut game, sweeper, 2, battle::SpecialTarget::WholeParty);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 10,
        "the player is part of the party"
    );
    assert!(
        game.world.get::<Stats>(sweeper).unwrap().hp > 10,
        "the caster heals itself too"
    );
    assert_eq!(
        game.world.get::<Stats>(downed).unwrap().hp,
        0,
        "a heal spent on a downed member would be wasted"
    );
}

#[test]
fn an_ability_on_cooldown_is_offered_but_refused() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 200);
    let slot = 1;

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow, cooldown 2
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    let options = game.battle_special_options(slot);
    assert!(
        options[0].unavailable.is_some(),
        "an ability just spent must render greyed, not silently fail"
    );
    assert!(
        game.battle_set_action(
            slot,
            BattleAction::Special {
                ability: 0,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            }
        )
        .is_err(),
        "planning a cooling ability must be refused, not burn the round"
    );
}

#[test]
fn a_cooldown_expires_after_its_declared_rounds() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 500);

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cooldown 2
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    assert!(game.battle_special_options(1)[0].unavailable.is_some());

    for _ in 0..2 {
        resolve_round_with(&mut game, BattleAction::Defend);
    }

    assert!(
        game.battle_special_options(1)[0].unavailable.is_none(),
        "a 2-round cooldown must be clear two rounds later"
    );
}

#[test]
fn cooldowns_do_not_survive_the_battle_that_set_them() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 1, 1);

    companion_uses_special(
        &mut game,
        sweeper,
        0,
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "the one 1-HP enemy should have died, ending the fight"
    );
    let cooldowns = game.world.get::<AbilityCooldowns>(sweeper);
    assert!(
        cooldowns.is_none_or(|c| c.0.values().all(|&r| r == 0)),
        "cooldowns are scoped to one intrusion, like every other combat status"
    );
}

#[test]
fn a_costly_ability_charges_its_own_fatigue_not_the_flat_command_cost() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);

    let before = game.world.get::<Needs>(player).unwrap().fatigue;
    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow declares fatigue_cost 8.0
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    let spent = before - game.world.get::<Needs>(player).unwrap().fatigue;

    assert!(
        spent > COMPANION_COMMAND_FATIGUE_COST,
        "cascade_overflow's 8.0 must cost more than the flat 5.0 default, spent {spent}"
    );
}

#[test]
fn an_ability_costing_more_fatigue_than_you_have_is_unavailable() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Needs>(player).unwrap().fatigue = 1.0;

    let options = game.battle_special_options(1);
    assert!(
        options[1].unavailable.is_some(),
        "broadcast_storm costs 15.0 Fatigue and must be refused at 1.0"
    );
    let _ = sweeper;
}
