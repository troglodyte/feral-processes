//! Data-driven abilities: multi-target shapes, cooldowns, and the
//! back-rank kill handling the enemy-side shapes depend on.

use crate::components::*;
use crate::resources::*;
use crate::*;

use super::support::*;
use crate::tuning::{COMPANION_COMMAND_FATIGUE_COST, GROUP_SIZE_STEP_TILES};

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
    let dir = modded_assets_dir("sweeper", &[], &[], &[("test_sweeper.ron", SWEEPER)], &[]);
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
    // Routine slots are level-gated (see `abilities::companion_routine_slots`):
    // a level-1 companion has exactly one, so a level-1 sweeper would only
    // ever install `cascade_overflow` and the other two tests below it would
    // have nothing at their index. Level 6 is the lowest level worth three
    // slots, which is exactly what all three declared abilities need to land.
    game.world.get_mut::<Experience>(sweeper).unwrap().level = 6;
    game.install_innate_routines(sweeper);
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
    let (gx, gy) = multi_group_ground(&game);
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
                        x: gx + i as i32,
                        y: gy,
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

#[test]
fn the_player_has_no_abilities_until_they_research_one() {
    let game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .actor_abilities(game.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(
        ids,
        vec![crate::abilities::DECOMPILE_ABILITY_ID.to_string()],
        "the player starts with only the pre-installed decompile — everything else is \
         what the research is selling"
    );
}

#[test]
fn researching_self_execution_grants_the_player_priority_boost() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "self_exec");

    // The only slot at level 1 already holds decompile; free it for the
    // routine this test is actually about.
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    let item = crate::abilities::routine_item_id("priority_boost");
    game.install_routine(game.player_entity(), &item).unwrap();

    let ids: Vec<String> = game
        .actor_abilities(game.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["priority_boost".to_string()]);
}

/// Two nodes may legitimately name the same ability — a mod branching the
/// tree, say. Research must not then auto-install it twice; it just stacks
/// the routine item, and installing is still a separate, deliberate act.
#[test]
fn an_ability_granted_by_two_nodes_stacks_the_item_rather_than_double_installing() {
    const ALSO_BOOST: &str = r#"(
        id: "also_boost",
        name: "Redundant Routine",
        description: "Grants what self_exec already grants.",
        cost: 12,
        unlocks_abilities: ["priority_boost"],
    )"#;
    let dir = modded_assets_dir(
        "dup_ability",
        &[],
        &[],
        &[],
        &[("also_boost.ron", ALSO_BOOST)],
    );
    let mut game = Game::new(33, DifficultyMode::Forgiving, &dir).unwrap();
    unlock_research_chain(&mut game, "self_exec");
    unlock_research_chain(&mut game, "also_boost");

    let item = crate::abilities::routine_item_id("priority_boost");
    assert_eq!(
        count_item(&game, item.as_str()),
        2,
        "each node deposits its own copy of the routine"
    );

    // The only slot at level 1 already holds decompile; free it for the
    // routine this test is actually about.
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    game.install_routine(game.player_entity(), &item).unwrap();
    let ids: Vec<String> = game
        .actor_abilities(game.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(
        ids,
        vec!["priority_boost".to_string()],
        "installing spends one copy and fills one slot, however many nodes granted it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The player's Special goes through the same resolution path a companion's
/// does — so the cooldown must arm on the player's own entity, and the
/// effect must land.
#[test]
fn a_player_special_applies_its_effect_and_arms_the_players_cooldown() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "runtime_patching");
    let player = game.player_entity();
    // A level-1 player has only one routine slot (see
    // `tuning::PLAYER_ROUTINE_SLOT_BASE`), so both grants need a slot to land in.
    // Levelling up doesn't evict decompile from the first, so it's popped
    // out explicitly to make room.
    set_level(&mut game, player, 10);
    game.uninstall_routine(player, 0).unwrap();
    let priority_boost = crate::abilities::routine_item_id("priority_boost");
    game.install_routine(player, &priority_boost).unwrap();
    let hot_patch_item = crate::abilities::routine_item_id("hot_patch");
    game.install_routine(player, &hot_patch_item).unwrap();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let hot_patch = game
        .actor_abilities(player)
        .iter()
        .position(|a| a.id == "hot_patch")
        .expect("runtime_patching grants hot_patch");
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;

    resolve_round_with(
        &mut game,
        BattleAction::Special {
            ability: hot_patch,
            target: battle::SpecialTarget::Ally { slot: 0 },
        },
    );

    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 1,
        "the player patched themselves, so their Integrity must have gone up"
    );
    assert_eq!(
        game.world
            .get::<AbilityCooldowns>(player)
            .and_then(|c| c.0.get("hot_patch").copied()),
        Some(1),
        "armed on the player's own entity as 1 + 1, less this round's tick"
    );
    assert!(
        game.battle_special_options(0)[hot_patch]
            .unavailable
            .is_some(),
        "and the player's own menu reads it back as still cooling down"
    );
}

/// Commanding an ability spends the *player's* Fatigue, which is what keeps
/// a top-tier routine a budget decision rather than a free extra action.
///
/// Measured against a control round rather than against the raw cost: a
/// round of any kind drains a little Fatigue on its own, so the ability's
/// price is the difference between a Special round and a Defend one.
#[test]
fn a_player_special_spends_the_players_fatigue_once() {
    fn round_cost(action: BattleAction) -> f32 {
        let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "kernel_privileges");
        let player = game.player_entity();
        // The only slot at level 1 already holds decompile; free it for the
        // routine this test is actually about.
        game.uninstall_routine(player, 0).unwrap();
        let item = crate::abilities::routine_item_id("null_route");
        game.install_routine(player, &item).unwrap();
        let enemy = spawn_wild_on_player_tile(&mut game);
        insert_battle(&mut game, player, vec![enemy]);

        let before = game.world.get::<Needs>(player).unwrap().fatigue;
        resolve_round_with(&mut game, action);
        before - game.world.get::<Needs>(player).unwrap().fatigue
    }

    let mut probe = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut probe, "kernel_privileges");
    probe.uninstall_routine(probe.player_entity(), 0).unwrap();
    let item = crate::abilities::routine_item_id("null_route");
    probe.install_routine(probe.player_entity(), &item).unwrap();
    let abilities = probe.actor_abilities(probe.player_entity());
    let index = abilities
        .iter()
        .position(|a| a.id == "null_route")
        .expect("kernel_privileges grants null_route");
    let cost = abilities[index].fatigue_cost;
    assert!(
        cost > 0.0,
        "null_route is the first researched routine that costs Fatigue"
    );

    let idle = round_cost(BattleAction::Defend);
    let special = round_cost(BattleAction::Special {
        ability: index,
        target: battle::SpecialTarget::AllEnemies,
    });

    assert_eq!(special - idle, cost, "charged exactly once, and to you");
}

/// The player's installed routines are carried in `data.player.routines`,
/// a separate save path from a companion's `Routines` component — this
/// pins that the player's own path round-trips too.
#[test]
fn a_save_round_trip_preserves_the_players_abilities() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "runtime_patching");
    let player = game.player_entity();
    // A level-1 player has only one routine slot (see
    // `tuning::PLAYER_ROUTINE_SLOT_BASE`), so both grants need a slot to land in.
    // Levelling up doesn't evict decompile from the first, so it's popped
    // out explicitly to make room.
    set_level(&mut game, player, 10);
    game.uninstall_routine(player, 0).unwrap();
    let priority_boost = crate::abilities::routine_item_id("priority_boost");
    game.install_routine(player, &priority_boost).unwrap();
    let hot_patch = crate::abilities::routine_item_id("hot_patch");
    game.install_routine(player, &hot_patch).unwrap();
    let before: Vec<String> = game
        .actor_abilities(player)
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(before.len(), 2, "priority_boost and hot_patch");

    let path = std::env::temp_dir().join(format!(
        "feral_player_routines_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let after: Vec<String> = loaded
        .actor_abilities(loaded.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(after, before);
}

/// The fallback exists so a companion's menu is never empty. It must not
/// leak onto the player, or the first research node would sell something
/// already owned.
#[test]
fn the_companion_fallback_does_not_leak_onto_the_player() {
    let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 20, 5);

    assert!(
        !game.actor_abilities(companion).is_empty(),
        "a companion always resolves at least the fallback"
    );
    assert_eq!(
        game.actor_abilities(game.player_entity())
            .into_iter()
            .map(|a| a.id)
            .collect::<Vec<_>>(),
        vec![crate::abilities::DECOMPILE_ABILITY_ID.to_string()],
        "the player gets decompile, not the companion fallback"
    );
}

/// Drain heals the user for its fraction of the damage it actually dealt —
/// not of its authored power, which DEF has already eaten into.
#[test]
fn drain_heals_the_user_for_a_fraction_of_the_damage_it_dealt() {
    let mut game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 200;
        stats.hp = 50;
        stats.atk = 10;
    }
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            heal_fraction: 0.5,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    let dealt = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;
    assert!(dealt > 0, "the drain must actually land damage");
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        50 + dealt / 2,
        "the user is healed for half of what it dealt"
    );
}

#[test]
fn drain_never_heals_the_user_past_its_maximum() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 60;
        stats.hp = 59;
        stats.atk = 40;
    }

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            heal_fraction: 1.0,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        60,
        "a full-lifesteal drain caps at max Integrity rather than overhealing"
    );
}

#[test]
fn cleanse_clears_an_active_status_and_is_silent_on_a_clean_target() {
    let mut game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 3,
        power: 4,
    });

    let ability = crate::abilities::AbilityDef {
        id: "test_cleanse".into(),
        name: "Test Cleanse".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeParty,
        effect: crate::abilities::AbilityEffect::Cleanse,
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[player]);
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_none(),
        "cleanse must clear the condition"
    );

    let lines_before = game.world.resource::<MessageLog>().lines.len();
    game.use_ability(&ability, player, "You", &[player]);
    assert_eq!(
        game.world.resource::<MessageLog>().lines.len(),
        lines_before,
        "a cleanse with nothing to clear logs nothing — one line per party member every time would drown the log"
    );
}

/// A sap is a negative-power `Buff` aimed at the enemy side. No `Sap`
/// variant exists, deliberately — `effective_atk` adds the buff bonus
/// unconditionally, so a negative power already subtracts.
#[test]
fn a_negative_power_buff_saps_effective_attack() {
    let mut game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<Stats>(enemies[0]).unwrap().atk = 20;
    let before = game.effective_atk(enemies[0]);

    let ability = crate::abilities::AbilityDef {
        id: "test_sap".into(),
        name: "Test Sap".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeEnemyGroup,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power: -6,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.effective_atk(enemies[0]),
        before - 6,
        "a negative buff power subtracts, which is the whole sap mechanic"
    );
}

/// `CombatBuff` holds one `active` slot and `is_defending` identifies the
/// Defend stance by an exact power match, so a sap landing on a bracing
/// member cancels its stance. Documented cost of the single-slot design,
/// pinned here rather than special-cased.
#[test]
fn a_sap_landing_on_a_bracing_member_cancels_its_defend_stance() {
    let mut game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.begin_defend(player);
    assert!(game.is_defending(player), "fixture: the player is bracing");

    let ability = crate::abilities::AbilityDef {
        id: "test_sap".into(),
        name: "Test Sap".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeEnemyGroup,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Def,
            power: -4,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "Enemy", &[player]);

    assert!(
        !game.is_defending(player),
        "one buff slot means a sap overwrites the stance — the documented cost, not a bug"
    );
}
