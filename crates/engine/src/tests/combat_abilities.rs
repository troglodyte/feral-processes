//! Data-driven abilities: multi-target shapes, cooldowns, and the
//! back-rank kill handling the enemy-side shapes depend on.

use crate::components::*;
use crate::resources::*;
use crate::*;

use super::support::*;
use crate::tuning::{
    AFFINITY_MAX, AFFINITY_NEUTRAL, AFFINITY_PERK_BONUS_PER_LEVEL,
    AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED, HUNGER_DECAY_PER_TICK,
};

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
        base_mitigation: 2,
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
    let dir = modded_assets_dir(
        "sweeper",
        &[],
        &[],
        &[("test_sweeper.ron", SWEEPER)],
        &[],
        &[],
    );
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
                mitigation: 2,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
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
    // **A sweep, because each recipient now rolls its own attack.** One cast
    // lands on three members and any of the three can miss, so a single cast
    // cannot show that every rank is *reachable*. Summing a fixed set of
    // `GameRng` seeds can, is deterministic, and still fails outright if a
    // rank is never touched — which is the thing that would break.
    let mut dealt = [0i32; 3];
    for seed in 0..16u64 {
        let (mut game, sweeper) = game_with_a_sweeper();
        let pack = battle_with_a_pack_of(&mut game, 3, 50);
        reseed_rng(&mut game, seed);

        companion_uses_special(
            &mut game,
            sweeper,
            0, // cascade_overflow
            battle::SpecialTarget::EnemyGroup { group: 0 },
        );

        for (rank, member) in pack.iter().enumerate() {
            // A member reaped mid-sweep has no `Stats` left — the idiom for
            // "this entity is gone" — and took the whole 50 to get there.
            let hp = game.world.get::<Stats>(*member).map(|s| s.hp).unwrap_or(0);
            dealt[rank] += 50 - hp;
        }
    }

    for (rank, total) in dealt.iter().enumerate() {
        assert!(
            *total > 0,
            "the member at rank {rank} was never reached across the whole sweep"
        );
    }
}

/// A sweeper facing four distinct species, so `group_pack` yields four groups
/// — more than `ENGAGED_GROUPS`, which is the point of the test below.
///
/// Extracted so the sweep can rebuild it per `GameRng` seed.
fn a_sweeper_against_four_groups() -> (Game, Entity, Vec<Entity>) {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    let species: Vec<String> = game
        .species_defs()
        .into_iter()
        .take(4)
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(species.len(), 4, "the shipped set must supply four species");
    let (gx, gy) = multi_group_ground(&mut game);
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
                    // Deep enough to survive the sweep: this asserts on
                    // reach, and a group that dies outright is despawned and
                    // has no Integrity left to read.
                    Stats {
                        hp: 500,
                        max_hp: 500,
                        atk: 0,
                        mitigation: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(&mut game, player, enemies.clone());
    assert_eq!(game.living_group_count(), 4, "four species, four groups");
    (game, sweeper, enemies)
}

#[test]
fn an_all_enemies_ability_reaches_every_group_including_past_engagement_range() {
    // A sweep, for the reason
    // `a_whole_group_ability_damages_every_member_not_just_the_front` gives:
    // each group's front rolls its own attack, so one cast cannot show that
    // all four are *reachable*, only that they happened to be hit.
    let mut dealt = [0i32; 4];
    for seed in 0..16u64 {
        let (mut game, sweeper, enemies) = a_sweeper_against_four_groups();
        reseed_rng(&mut game, seed);
        companion_uses_special(
            &mut game,
            sweeper,
            1, // broadcast_storm
            battle::SpecialTarget::AllEnemies,
        );
        for (group, enemy) in enemies.iter().enumerate() {
            let hp = game.world.get::<Stats>(*enemy).map(|s| s.hp).unwrap_or(0);
            dealt[group] += 500 - hp;
        }
    }

    for (group, total) in dealt.iter().enumerate() {
        assert!(
            *total > 0,
            "group {group} was never reached across the whole sweep"
        );
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

    // The cast has to land for the kill that ends the fight — this test is
    // about what teardown does to cooldowns, not about the to-hit roll.
    force_the_next_attack_to_land(&mut game);
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

/// The bug this closes: a companion's routine came out of *your* meter, so
/// the party's own kit was rationed against a pool only the player had.
/// Whatever a Special costs, the caster is who pays it.
#[test]
fn a_companions_special_charges_the_player_no_power() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);

    let before = game.world.get::<PowerReserve>(player).unwrap().get();
    companion_uses_special(
        &mut game,
        sweeper,
        0,
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    let spent = before - game.world.get::<PowerReserve>(player).unwrap().get();

    assert!(
        spent <= HUNGER_DECAY_PER_TICK + 1e-4,
        "a commanded routine must take nothing off the player; the round's own \
         one tick of drain is the only movement allowed, spent {spent}"
    );
}

#[test]
fn a_player_out_of_power_can_still_command_a_companions_routine() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(0.0);

    let options = game.battle_special_options(1);
    assert_eq!(
        options[1].unavailable, None,
        "the caster's reserve is what gates a Special, and the caster here is \
         the companion"
    );
    let _ = sweeper;
}

/// The cooldown is the only price a Special has, so the picker has to name
/// it: a player choosing between two ready routines otherwise can't tell the
/// one they can repeat next round from the one that locks itself away for
/// five. It travels on the option for the same reason the reason does —
/// neither renderer gets to author it.
#[test]
fn a_special_option_carries_the_cooldown_it_would_arm() {
    let (mut game, _) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 500);

    let options = game.battle_special_options(1);
    assert_eq!(
        options[0].cooldown, 2,
        "cascade_overflow declares cooldown 2"
    );
    assert_eq!(
        options[1].cooldown, 4,
        "broadcast_storm declares cooldown 4"
    );
}

/// Power rides on the party slot because the roster shows it as a per-member
/// column, and **every member now has its own number to show** — a companion
/// pays for its own Specials out of its own reserve.
///
/// `None` survives as the shape for a roster member holding no reserve at
/// all, which after `Game::roster_parts` means a door that skipped it.
#[test]
fn the_battle_view_carries_a_reserve_for_every_party_member() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(62.0);

    let view = game.battle_view().expect("the pack opened a battle");
    assert_eq!(
        view.party[0].power,
        Some(62.0),
        "slot 0 is the player, the only party member with a reserve to show"
    );
    assert_eq!(
        view.party[1].entity, sweeper,
        "the sweeper should be the second party slot"
    );
    assert_eq!(
        view.party[1].power,
        Some(POWER_MAX),
        "a companion shows its own reserve, not the player's 62"
    );
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
    give_disks(&mut game, 1);
    let player = game.player_entity();
    fit_routine(&mut game, player, "priority_boost");

    let ids: Vec<String> = game
        .actor_abilities(game.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["priority_boost".to_string()]);
}

/// Two nodes may legitimately name the same ability — a mod branching the
/// tree, say. Knowledge is a set, so the second node teaches nothing new,
/// and installing is still a separate, deliberate act that costs a disk.
#[test]
fn an_ability_granted_by_two_nodes_is_learned_once() {
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
        &[],
    );
    let mut game = Game::new(33, DifficultyMode::Forgiving, &dir).unwrap();
    unlock_research_chain(&mut game, "self_exec");
    unlock_research_chain(&mut game, "also_boost");

    assert_eq!(
        game.etchable_routines()
            .iter()
            .filter(|r| r.ability == "priority_boost")
            .count(),
        1,
        "the second node teaches nothing the first didn't"
    );

    // The only slot at level 1 already holds decompile; free it for the
    // routine this test is actually about.
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    give_disks(&mut game, 2);
    let player = game.player_entity();
    fit_routine(&mut game, player, "priority_boost");
    let ids: Vec<String> = game
        .actor_abilities(game.player_entity())
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(
        ids,
        vec!["priority_boost".to_string()],
        "installing fills one slot, however many nodes granted it"
    );
    assert_eq!(
        game.blank_disks_held(),
        1,
        "one install burns exactly one disk"
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
    give_disks(&mut game, 2);
    fit_routine(&mut game, player, "priority_boost");
    fit_routine(&mut game, player, "hot_patch");
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let hot_patch = game
        .actor_abilities(player)
        .iter()
        .position(|a| a.id == "hot_patch")
        .expect("runtime_patching grants hot_patch");
    // Not 1: initiative is a roll (`roll_initiative`), and wild spawns now
    // draw from the same `GameRng` (see `Game::roll_wild_routine`), so which
    // side goes first in this round is no longer pinned by this seed alone.
    // `set_level` only advances `Experience.level` — it doesn't grow `Stats`
    // — so the player's DEF is still the base 2, and the fixed atk:0 enemy's
    // strongest move (Cross-Reference, power 9) can land up to 7. 20 clears
    // that with room, either order.
    game.world.get_mut::<Stats>(player).unwrap().hp = 20;

    resolve_round_with(
        &mut game,
        BattleAction::Special {
            ability: hot_patch,
            target: battle::SpecialTarget::Ally { slot: 0 },
        },
    );

    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 15,
        "the player patched themselves, so their Integrity must have gone up \
         past what a single worst-case enemy hit could leave it at"
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

/// A routine is priced in a cooldown **and** a Power cost, and the player is
/// not exempt — `null_route` is the deepest researched routine in the shipped
/// tree and it charges what its file says.
///
/// Measured against a control round rather than against zero: a round of any
/// kind drains a little Power on its own, so what the *ability* costs is the
/// difference between a Special round and a Defend one. That difference used
/// to be nothing, which is what pricing routine calls in Power changed.
#[test]
fn a_player_special_spends_its_authored_power_cost() {
    fn round_cost(action: BattleAction) -> f32 {
        let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "kernel_privileges");
        let player = game.player_entity();
        // The only slot at level 1 already holds decompile; free it for the
        // routine this test is actually about.
        game.uninstall_routine(player, 0).unwrap();
        give_disks(&mut game, 1);
        fit_routine(&mut game, player, "null_route");
        let enemy = spawn_wild_on_player_tile(&mut game);
        insert_battle(&mut game, player, vec![enemy]);

        // Start off the cap: a round's own drain is meant to cancel between
        // the two measurements, and only does when neither is clamped.
        *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
        let before = game.world.get::<PowerReserve>(player).unwrap().get();
        resolve_round_with(&mut game, action);
        before - game.world.get::<PowerReserve>(player).unwrap().get()
    }

    let mut probe = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut probe, "kernel_privileges");
    probe.uninstall_routine(probe.player_entity(), 0).unwrap();
    give_disks(&mut probe, 1);
    let probe_player = probe.player_entity();
    fit_routine(&mut probe, probe_player, "null_route");
    let abilities = probe.actor_abilities(probe.player_entity());
    let index = abilities
        .iter()
        .position(|a| a.id == "null_route")
        .expect("kernel_privileges grants null_route");
    let cost = crate::abilities::routine_power_cost(&abilities[index]);
    assert!(
        cost > 0.0,
        "null_route has to declare a cost to be about this"
    );

    let idle = round_cost(BattleAction::Defend);
    let special = round_cost(BattleAction::Special {
        ability: index,
        target: battle::SpecialTarget::AllEnemies,
    });

    assert!(
        (special - idle - cost).abs() < 1e-4,
        "a Special should cost bracing plus its authored {cost}; bracing cost \
         {idle} and the Special cost {special}"
    );
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
    give_disks(&mut game, 2);
    fit_routine(&mut game, player, "priority_boost");
    fit_routine(&mut game, player, "hot_patch");
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
            spread: 0,
            heal_fraction: 0.5,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    force_the_next_attack_to_land(&mut game);
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
            spread: 0,
            heal_fraction: 1.0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        60,
        "a full-lifesteal drain caps at max Integrity rather than overhealing"
    );
}

/// The heal line reports HP actually restored, not the figure the ability
/// rolled — a patch on a full-health target reads "for 0 HP" rather than
/// claiming an amount the target's ceiling swallowed.
#[test]
fn a_heal_logs_what_it_actually_restored_not_what_it_rolled() {
    let mut game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100;
        stats.hp = 97;
    }

    let ability = crate::abilities::AbilityDef {
        id: "test_patch".into(),
        name: "Test Patch".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal {
            power: 20,
            spread: 0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        100,
        "the heal itself still caps at max Integrity"
    );
    assert!(
        game.message_log(usize::MAX)
            .into_iter()
            .any(|e| e.text.contains("patches you for 3 HP")),
        "the log must name the 3 points that landed, not the 20 that were rolled: {:?}",
        game.message_log(usize::MAX)
    );
}

#[test]
fn a_heal_on_a_full_health_target_logs_zero() {
    let mut game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100;
        stats.hp = 100;
    }

    let ability = crate::abilities::AbilityDef {
        id: "test_patch".into(),
        name: "Test Patch".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal {
            power: 20,
            spread: 0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert!(
        game.message_log(usize::MAX)
            .into_iter()
            .any(|e| e.text.contains("patches you for 0 HP")),
        "a wasted heal must say so: {:?}",
        game.message_log(usize::MAX)
    );
}

/// Drain's line has the same obligation as a plain heal: the restore figure
/// is what the user's ceiling let in, not the fraction it computed.
#[test]
fn drain_logs_what_it_actually_restored() {
    let mut game = Game::new(4106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
            spread: 0,
            heal_fraction: 1.0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert!(
        game.message_log(usize::MAX)
            .into_iter()
            .any(|e| e.text.contains("restoring 1.")),
        "a full-lifesteal drain one point from max restores exactly 1: {:?}",
        game.message_log(usize::MAX)
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
        landed_this_round: false,
    });

    let ability = crate::abilities::AbilityDef {
        id: "test_cleanse".into(),
        name: "Test Cleanse".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeParty,
        effect: crate::abilities::AbilityEffect::Cleanse,
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
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
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.effective_atk(enemies[0]),
        before + crate::abilities::scaled_stat_power(-6, 1, crate::tuning::AFFINITY_NEUTRAL),
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
            kind: BuffKind::Mitigation,
            power: -4,
            duration: 3,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "Enemy", &[player]);

    assert!(
        !game.is_defending(player),
        "one buff slot means a sap overwrites the stance — the documented cost, not a bug"
    );
}

/// A heal stores the scaled figure at the moment it is applied, so nothing
/// downstream has to re-scale.
#[test]
fn a_heal_scales_with_the_users_level() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 400;
        stats.hp = 100;
    }
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_heal".into(),
        name: "Test Heal".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal {
            power: 8,
            spread: 0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        100 + crate::abilities::scaled_hp_power(8, 20, crate::tuning::AFFINITY_NEUTRAL),
        "an 8-point patch at level 20 is 32, not 8"
    );
}

/// A routine's heal is a band, not a fixed figure — the same shape
/// `AbilityEffect::Damage`'s `spread` already gives damage. Rolled through
/// `battle::DamageRange` rather than a second formula, so the low end is
/// floored at 0 and the band scales with the caster exactly as a damage
/// band does.
#[test]
fn a_heal_rolls_a_band_rather_than_a_fixed_amount() {
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 4000;
        stats.hp = 100;
    }

    let ability = crate::abilities::AbilityDef {
        id: "test_heal_band".into(),
        name: "Test Heal Band".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal {
            power: 20,
            spread: 8,
        },
        cooldown: 0,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..60 {
        game.world.get_mut::<Stats>(player).unwrap().hp = 100;
        game.use_ability(&ability, player, "You", &[player]);
        seen.insert(game.world.get::<Stats>(player).unwrap().hp - 100);
    }

    let band = crate::abilities::scaled_range(
        crate::battle::DamageRange::centred(20, 8),
        game.world.get::<Experience>(player).unwrap().level,
        AFFINITY_NEUTRAL,
    );
    assert!(
        seen.len() > 1,
        "a spread heal must not restore one fixed figure every time; saw {seen:?}"
    );
    assert!(
        seen.iter().all(|&r| r >= band.min && r <= band.max),
        "every roll must land inside the scaled band {band:?}; saw {seen:?}"
    );
}

#[test]
fn a_buff_stores_the_scaled_power_so_the_tick_needs_no_change() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_buff".into(),
        name: "Test Buff".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power: 3,
            duration: 3,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert_eq!(
        game.world
            .get::<CombatBuff>(player)
            .unwrap()
            .active
            .unwrap()
            .power,
        crate::abilities::scaled_stat_power(3, 20, crate::tuning::AFFINITY_NEUTRAL),
        "the scaled figure is stored, not recomputed at read time"
    );
}

#[test]
fn a_bleed_debuffs_per_round_damage_scales_with_the_users_level() {
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 400);
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_bleed".into(),
        name: "Test Bleed".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Debuff {
            kind: StatusKind::Bleed,
            power: 2,
            duration: 3,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.world
            .get::<StatusEffects>(enemies[0])
            .unwrap()
            .active
            .unwrap()
            .power,
        crate::abilities::scaled_hp_power(2, 20, crate::tuning::AFFINITY_NEUTRAL),
        "bleed is flat damage per round, so it needs scaling as much as a heal does"
    );
}

/// A landed hit is the band's roll plus ATK, and ATK was once held to carry
/// the whole progression. It cannot: `ATK_PER_LEVEL` is 1 against
/// `HP_PER_LEVEL`'s 12, so an unscaled authored power falls further behind
/// its target's Integrity every single level. A damage magnitude is measured
/// in HP and has to grow on the HP curve.
#[test]
fn ability_damage_scales_with_the_users_level() {
    let mut game = Game::new(4204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Stats>(player).unwrap().atk = 10;

    let ability = crate::abilities::AbilityDef {
        id: "test_hit".into(),
        name: "Test Hit".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Damage {
            power: 6,
            spread: 0,
            status: None,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };

    // Both casts are forced to land. These tests are about what levelling
    // does to a hit's *size*; letting either one miss would make them about
    // the to-hit roll instead, and a missed level-1 cast reads as infinite
    // scaling rather than none.
    game.world.get_mut::<Experience>(player).unwrap().level = 1;
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&ability, player, "You", &[enemies[0]]);
    let at_level_1 = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;

    game.world.get_mut::<Experience>(player).unwrap().level = 20;
    let before = game.world.get::<Stats>(enemies[1]).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&ability, player, "You", &[enemies[1]]);
    let at_level_20 = before - game.world.get::<Stats>(enemies[1]).unwrap().hp;

    assert!(
        at_level_20 > at_level_1 * 3,
        "a level-20 hit should dwarf a level-1 one: {at_level_1} vs {at_level_20}"
    );
}

/// Drain's damage half is a damage magnitude like any other, so it scales
/// the same way. Its `heal_fraction` still doesn't — that rides the damage
/// it already dealt.
#[test]
fn drain_scales_with_the_users_level() {
    let mut game = Game::new(4205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Stats>(player).unwrap().atk = 10;

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            spread: 0,
            heal_fraction: 0.5,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };

    // Both casts are forced to land. These tests are about what levelling
    // does to a hit's *size*; letting either one miss would make them about
    // the to-hit roll instead, and a missed level-1 cast reads as infinite
    // scaling rather than none.
    game.world.get_mut::<Experience>(player).unwrap().level = 1;
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&ability, player, "You", &[enemies[0]]);
    let at_level_1 = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;

    game.world.get_mut::<Experience>(player).unwrap().level = 20;
    let before = game.world.get::<Stats>(enemies[1]).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&ability, player, "You", &[enemies[1]]);
    let at_level_20 = before - game.world.get::<Stats>(enemies[1]).unwrap().hp;

    assert!(
        at_level_20 > at_level_1 * 3,
        "a level-20 drain should dwarf a level-1 one: {at_level_1} vs {at_level_20}"
    );
}

/// The pin for the whole HP-magnitude retune, in the terms the retune was
/// asked for: a mid-run player with the Damage affinity perk five levels
/// deep, spending the heaviest shipped single-target routine, against a
/// program with the Integrity a mid-zone one actually has.
///
/// The level here is 5 and used to be 10. It is the *same point in a run*:
/// `HP_PER_LEVEL`'s `K = 2` halved the level count and doubled
/// `ABILITY_HP_SCALE_PER_LEVEL` to match, so `ability_hp_scale(5)` is now
/// exactly the 5.0x `ability_hp_scale(10)` used to be — and the band below
/// is unchanged, which is the evidence that rebase was power-neutral rather
/// than a stealth buff.
///
/// `balance_sim` cannot hold this — it models no abilities at all — so this
/// is the only gate on ability magnitudes. A number that moves here means
/// routine damage was retuned, which is the signal, not a broken test.
#[test]
fn a_perked_mid_run_kernel_panic_lands_in_the_intended_band() {
    let mut game = Game::new(4207, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 400);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.atk = 16;
    }
    // 0 rather than the 9 points of subtractive absorption this used to
    // author. Mitigation is a percentage cut applied after the routine's own
    // arithmetic, so leaving it in would make this gate move whenever armour
    // is retuned — and the band below staying put across that change is the
    // evidence that ability magnitudes did *not* move with the combat model.
    game.world.get_mut::<Stats>(enemies[0]).unwrap().mitigation = 0;
    game.world.get_mut::<Experience>(player).unwrap().level = 5;
    for _ in 0..5 {
        game.world
            .get_mut::<Perks>(player)
            .unwrap()
            .unlocked
            .push(Perk::DamageAffinity);
    }

    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    game.use_ability(
        &ability(&game, "kernel_panic"),
        player,
        "You",
        &[enemies[0]],
    );
    let dealt = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;

    // The routine rolls a band now, so the *balance* claim is about where the
    // band is centred — a single roll can sit anywhere inside it. Both halves
    // are asserted: the centre is the gate, and containment is the evidence
    // that this cast actually used that band rather than some other one.
    let def = ability(&game, "kernel_panic");
    let crate::abilities::AbilityEffect::Damage { power, spread, .. } = def.effect else {
        panic!("kernel_panic is a Damage ability");
    };
    let atk = game.effective_atk(player);
    let band = crate::abilities::scaled_range(
        crate::battle::DamageRange::centred(power, spread),
        5,
        game.ability_affinity(player, &def.effect),
    );
    let centre = band.mean() + atk as f64;
    assert!(
        (140.0..=165.0).contains(&centre),
        "a perked mid-run Packet Shred Single should centre near 150 against 400 \
         Integrity, got {centre}"
    );
    assert!(
        (band.min + atk..=band.max + atk).contains(&dealt),
        "the cast should land inside its own band {band:?} plus {atk} ATK, got {dealt}"
    );
}

/// Wild programs have no `Experience` — they scale by zone and distance —
/// so a hostile carrier reads the current `ZoneLevel` instead.
#[test]
fn a_hostile_scales_its_routine_off_the_zone_level() {
    let mut game = Game::new(4206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.resource_mut::<ZoneLevel>().0 = 7;

    assert_eq!(
        game.ability_user_level(enemies[0]),
        7,
        "a wild program has no level, so the zone is what its routine scales from"
    );
    assert_eq!(
        game.ability_user_level(player),
        game.world.get::<Experience>(player).unwrap().level,
        "the player scales off their own level"
    );
}

/// `use_ability` was made side-agnostic (it resolves recipients from either
/// side via `ability_recipients`) without making its log kind side-aware:
/// the `Damage`/`Drain` arms hardcoded `MessageKind::PartyDamage`, so a
/// hostile carrier's routine damage rendered in the party's own styling —
/// the same bold-white the log deliberately reserves for the player's hits.
#[test]
fn a_hostile_routines_damage_line_logs_as_enemy_special_not_party_damage() {
    let mut game = Game::new(9201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    let kernel_panic = ability(&game, "kernel_panic");

    game.use_ability(&kernel_panic, enemies[0], "Crawler", &[player]);

    let kinds: Vec<MessageKind> = game
        .world
        .resource::<MessageLog>()
        .lines
        .iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        kinds.contains(&MessageKind::EnemySpecial),
        "a hostile's routine damage should log EnemySpecial: {kinds:?}"
    );
    assert!(
        !kinds.contains(&MessageKind::PartyDamage),
        "and never the party's own damage styling: {kinds:?}"
    );
}

/// A heal is the one good thing that happens to the party mid-fight without
/// being a gain (`Loot`) or a level, so it earns its own kind rather than
/// sitting in `Info`'s dim chatter beside "you have no X".
///
/// The side split is the same one `hit_kind` makes two lines above: a
/// hostile mending its own group is the party's bad news, so it stays on
/// `EnemySpecial` and only the party's heal reads as good.
#[test]
fn a_heal_logs_by_side_the_partys_as_heal_and_a_hostiles_as_enemy_special() {
    let mut game = Game::new(9202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100;
        stats.hp = 40;
    }
    let patch = crate::abilities::AbilityDef {
        id: "test_patch".into(),
        name: "Test Patch".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal {
            power: 20,
            spread: 0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };

    game.use_ability(&patch, player, "You", &[player]);
    let kinds = |game: &Game| -> Vec<MessageKind> {
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .filter(|l| l.text.contains("patches"))
            .map(|l| l.kind)
            .collect()
    };
    assert_eq!(
        kinds(&game),
        vec![MessageKind::Heal],
        "the party's own heal is the line this kind exists for"
    );

    game.use_ability(&patch, enemies[0], "Crawler", &[enemies[0]]);
    assert_eq!(
        kinds(&game),
        vec![MessageKind::Heal, MessageKind::EnemySpecial],
        "a hostile mending itself is not good news and must not read as it"
    );
}

/// A drain's line is a hit that also restores, and it reads as the party's
/// good news for the same reason a patch does — the Integrity coming back is
/// the half the player is watching for, and it is the only half a plain
/// `Attack` cannot also produce.
///
/// The side split is `heal_kind`'s, so a hostile siphoning off the party
/// stays `EnemySpecial` by construction rather than by a second branch.
#[test]
fn a_drain_logs_by_side_the_partys_as_heal_and_a_hostiles_as_enemy_special() {
    let mut game = Game::new(9203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100;
        stats.hp = 40;
        stats.atk = 40;
    }
    let siphon = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            spread: 0,
            heal_fraction: 1.0,
        },
        cooldown: 1,
        power_cost: 0.0,
        wild_weight: 0,
        exclusive: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
    };
    let kinds = |game: &Game| -> Vec<MessageKind> {
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .filter(|l| l.text.contains("siphons"))
            .map(|l| l.kind)
            .collect()
    };

    force_the_next_attack_to_land(&mut game);
    game.use_ability(&siphon, player, "You", &[enemies[0]]);
    assert_eq!(
        kinds(&game),
        vec![MessageKind::Heal],
        "the party's own drain restores Integrity and reads as good news"
    );

    force_the_next_attack_to_land(&mut game);
    game.use_ability(&siphon, enemies[0], "Crawler", &[player]);
    assert_eq!(
        kinds(&game),
        vec![MessageKind::Heal, MessageKind::EnemySpecial],
        "a hostile siphoning off the party is not good news and must not read as it"
    );
}

/// A `test_medic` (support::TWO_ABILITY_SPECIES) with a heal affinity —
/// same species, same `hot_patch`, one number different.
const HEALER_WITH_AFFINITY: &str = r#"(
    id: "test_medic",
    name: "Test Medic",
    glyph: 'm',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [(id: "hot_patch")],
    affinities: (heal: 1.5),
)"#;

#[test]
fn a_species_heal_affinity_scales_the_heal_it_casts() {
    let dir = super::support::modded_assets_dir(
        "heal_affinity_battle",
        &[],
        &[],
        &[("test_medic.ron", HEALER_WITH_AFFINITY)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    // Only `Creature` (for species lookup) and `Experience` (for level)
    // matter here — cast directly via `use_ability` rather than through a
    // full battle round, so a wild enemy's counterattack can't land on the
    // player in the same round and make the net HP delta disagree with the
    // heal actually applied.
    let medic = game
        .world
        .spawn((
            Creature {
                species: "test_medic".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                mitigation: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
        ))
        .id();
    let hot_patch = ability(&game, "hot_patch");

    // Wound the player so a heal has room to land, then have the medic
    // cast hot_patch (Heal(power: 8)) on them directly.
    let before = 20;
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 200;
        stats.hp = before;
    }
    game.use_ability(&hot_patch, medic, "Test Medic", &[player]);

    let healed = game.world.get::<Stats>(player).unwrap().hp - before;
    // hot_patch heals from a band; the medic is level 1. Affinity scales both
    // ends, so what is pinned is that the *affinity-scaled* band is the one
    // rolled from.
    let crate::abilities::AbilityEffect::Heal { power, spread } = hot_patch.effect else {
        panic!("hot_patch is a Heal ability");
    };
    let band =
        crate::abilities::scaled_range(crate::battle::DamageRange::centred(power, spread), 1, 1.5);
    let neutral = crate::abilities::scaled_range(
        crate::battle::DamageRange::centred(power, spread),
        1,
        AFFINITY_NEUTRAL,
    );
    assert!(
        (band.min..=band.max).contains(&healed),
        "heal affinity should scale the heal: {healed} is outside {band:?}"
    );
    assert!(
        band.min > neutral.min && band.max > neutral.max,
        "the fixture must actually differ from neutral, or this proves nothing"
    );
}

/// A species with a damage affinity, for the `Damage` arm of `use_ability` —
/// the one that feeds its scaled band into `battle::resolve_attack`
/// rather than standing on that figure alone.
const STRIKER_WITH_AFFINITY: &str = r#"(
    id: "test_striker",
    name: "Test Striker",
    glyph: 's',
    color: Red,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [(id: "kernel_panic")],
    affinities: (damage: 1.5),
)"#;

#[test]
fn a_species_damage_affinity_scales_the_damage_it_deals() {
    let dir = super::support::modded_assets_dir(
        "damage_affinity_battle",
        &[],
        &[],
        &[("test_striker.ron", STRIKER_WITH_AFFINITY)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let striker = game
        .world
        .spawn((
            Creature {
                species: "test_striker".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                mitigation: 1,
            },
            Experience::default(),
        ))
        .id();
    let target = game
        .world
        .spawn((
            Hostile,
            Position { x: 4, y: 3 },
            Stats {
                hp: 200,
                max_hp: 200,
                atk: 0,
                // 0, not 3: mitigation is a percentage cut applied after the
                // ability's own arithmetic, and this test is about that
                // arithmetic. Leaving it in would fold two formulas into one
                // expected value.
                mitigation: 0,
            },
        ))
        .id();
    let kernel_panic = ability(&game, "kernel_panic");

    game.use_ability(&kernel_panic, striker, "Test Striker", &[target]);

    let taken = 200 - game.world.get::<Stats>(target).unwrap().hp;
    // kernel_panic is Damage(power: 16); the striker is level 1. The target
    // is authored at 0 mitigation so this measures the affinity scaling
    // alone — mitigation is a percentage cut applied afterwards and has its
    // own tests.
    // A landed hit is the band's roll plus the attacker's flat ATK. The
    // ability rolls a band, so what is pinned is that *the affinity-scaled
    // band* is the one fed to the roll — the guard below is what makes
    // landing inside it mean something.
    let crate::abilities::AbilityEffect::Damage { power, spread, .. } = kernel_panic.effect else {
        panic!("kernel_panic is a Damage ability");
    };
    let atk = game.effective_atk(striker);
    let band =
        crate::abilities::scaled_range(crate::battle::DamageRange::centred(power, spread), 1, 1.5);
    let neutral = crate::abilities::scaled_range(
        crate::battle::DamageRange::centred(power, spread),
        1,
        AFFINITY_NEUTRAL,
    );
    assert!(
        (band.min + atk..=band.max + atk).contains(&taken),
        "damage affinity should scale the authored power fed to the damage band: \
         {taken} is outside {band:?} plus {atk} ATK"
    );
    assert!(
        band.min > neutral.min && band.max > neutral.max,
        "the fixture must actually differ from neutral ({band:?} vs {neutral:?}), \
         or this proves nothing"
    );
}

/// A species with a drain affinity, for the `Drain` arm — and the one place
/// `heal_fraction` must NOT be scaled a second time, since it already
/// multiplies damage that affinity has scaled once.
const DRAINER_WITH_AFFINITY: &str = r#"(
    id: "test_drainer",
    name: "Test Drainer",
    glyph: 'd',
    color: Magenta,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [(id: "siphon_cycles")],
    affinities: (drain: 1.5),
)"#;

#[test]
fn a_species_drain_affinity_scales_the_damage_but_not_the_heal_fraction() {
    let dir = super::support::modded_assets_dir(
        "drain_affinity_battle",
        &[],
        &[],
        &[("test_drainer.ron", DRAINER_WITH_AFFINITY)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let drainer = game
        .world
        .spawn((
            Creature {
                species: "test_drainer".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 50,
                max_hp: 200,
                atk: 5,
                mitigation: 1,
            },
            Experience::default(),
        ))
        .id();
    let target = game
        .world
        .spawn((
            Hostile,
            Position { x: 4, y: 3 },
            Stats {
                hp: 200,
                max_hp: 200,
                atk: 0,
                // 0, not 3: mitigation is a percentage cut applied after the
                // ability's own arithmetic, and this test is about that
                // arithmetic. Leaving it in would fold two formulas into one
                // expected value.
                mitigation: 0,
            },
        ))
        .id();
    let siphon_cycles = ability(&game, "siphon_cycles");

    game.use_ability(&siphon_cycles, drainer, "Test Drainer", &[target]);

    let taken = 200 - game.world.get::<Stats>(target).unwrap().hp;
    // siphon_cycles is Drain(power: 10, heal_fraction: 0.5); level and
    // affinity scale the authored power, same as Damage.
    // See the Damage twin above: the band is what affinity scales, and the
    // attacker's flat ATK is added to whatever it rolls.
    let crate::abilities::AbilityEffect::Drain { power, spread, .. } = siphon_cycles.effect else {
        panic!("siphon_cycles is a Drain ability");
    };
    let atk = game.effective_atk(drainer);
    let band =
        crate::abilities::scaled_range(crate::battle::DamageRange::centred(power, spread), 1, 1.5);
    let neutral = crate::abilities::scaled_range(
        crate::battle::DamageRange::centred(power, spread),
        1,
        AFFINITY_NEUTRAL,
    );
    assert!(
        (band.min + atk..=band.max + atk).contains(&taken),
        "drain affinity should scale the authored power fed to the damage band: \
         {taken} is outside {band:?} plus {atk} ATK"
    );
    assert!(
        band.min > neutral.min && band.max > neutral.max,
        "the fixture must actually differ from neutral, or this proves nothing"
    );

    let restored = game.world.get::<Stats>(drainer).unwrap().hp - 50;
    // Off the damage actually dealt (already affinity-scaled), times the
    // authored heal_fraction — never affinity again, or this double-dips.
    // Read off `taken` rather than a recomputed figure, which is what the
    // engine itself does and so survives the roll.
    let expected_restored = (taken as f32 * 0.5).round() as i32;
    assert_eq!(
        restored, expected_restored,
        "heal_fraction must apply once, to damage already scaled by affinity"
    );
    let double_dipped = (taken as f32 * 0.5 * 1.5).round() as i32;
    assert_ne!(
        restored, double_dipped,
        "a regression that re-applies affinity to heal_fraction must fail this"
    );
}

#[test]
fn a_player_affinity_perk_scales_the_players_own_ability() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let effect = AbilityEffect::Heal {
        power: 8,
        spread: 0,
    };
    let before = game.ability_affinity(player, &effect);

    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();
    game.unlock_perk(Perk::HealAffinity).unwrap();

    assert_eq!(before, AFFINITY_NEUTRAL);
    assert_eq!(
        game.ability_affinity(player, &effect),
        AFFINITY_NEUTRAL + 2.0 * AFFINITY_PERK_BONUS_PER_LEVEL
    );
}

/// `Damage`/`Drain` use a different (higher) per-level rate than `Heal` —
/// see `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`'s doc. Same shape as the test
/// above, but for `DamageAffinity`, to prove `AffinityKind::perk_bonus_per_level`
/// actually dispatches rather than both categories silently sharing one rate.
#[test]
fn a_damage_affinity_perk_uses_the_flat_rate_not_the_level_scaled_one() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let effect = AbilityEffect::Damage {
        power: 10,
        spread: 0,
        status: None,
    };
    let before = game.ability_affinity(player, &effect);

    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::DamageAffinity).unwrap();
    game.unlock_perk(Perk::DamageAffinity).unwrap();

    assert_eq!(before, AFFINITY_NEUTRAL);
    assert_eq!(
        game.ability_affinity(player, &effect),
        AFFINITY_NEUTRAL + 2.0 * AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED,
        "Damage must scale by the flat rate, not AFFINITY_PERK_BONUS_PER_LEVEL"
    );
}

/// Both per-level rates need their own overshoot: `HealAffinity` (0.05,
/// crosses `AFFINITY_MAX` at 20 levels) and `DamageAffinity` (0.15, crosses
/// at 7) hit the ceiling at very different points, and a fixture that only
/// exercises one rate says nothing about whether the other is clamped too.
#[test]
fn a_player_affinity_perk_is_clamped_at_affinity_max() {
    for (perk, rate, levels, effect) in [
        (
            Perk::HealAffinity,
            AFFINITY_PERK_BONUS_PER_LEVEL,
            25u32,
            AbilityEffect::Heal {
                power: 8,
                spread: 0,
            },
        ),
        (
            Perk::DamageAffinity,
            AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED,
            8u32,
            AbilityEffect::Damage {
                power: 10,
                spread: 0,
                status: None,
            },
        ),
    ] {
        let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let cost = game.world.resource::<PerkDb>().get(perk).unwrap().cost;
        {
            let mut perks = game.world.get_mut::<Perks>(player).unwrap();
            perks.points = levels * cost;
        }
        for _ in 0..levels {
            game.unlock_perk(perk).unwrap();
        }

        // A few levels past the crossing point confirms the clamp, not
        // just a formula that happens to land on the ceiling.
        let uncapped = AFFINITY_NEUTRAL + levels as f32 * rate;
        assert!(
            uncapped > AFFINITY_MAX,
            "{perk:?}: the fixture must actually overshoot the ceiling, or this proves nothing"
        );
        assert_eq!(
            game.ability_affinity(player, &effect),
            AFFINITY_MAX,
            "{perk:?}: a player's perk affinity must not exceed the species ceiling"
        );
    }
}

#[test]
fn a_player_affinity_perk_does_not_scale_a_companions_ability() {
    // The scoping decision, asserted directly: the perk is the player's
    // own, and a companion answers to its species instead.
    let (mut game, medic) = super::support::game_with_two_ability_companion();
    let player = game.player_entity();
    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();

    let effect = AbilityEffect::Heal {
        power: 8,
        spread: 0,
    };
    assert!(game.ability_affinity(player, &effect) > AFFINITY_NEUTRAL);
    assert_eq!(
        game.ability_affinity(medic, &effect),
        AFFINITY_NEUTRAL,
        "the player's perk must not reach a companion's cast"
    );
}

#[test]
fn an_effect_with_no_category_takes_no_multiplier() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();
    // Cleanse has no magnitude; a perk must not invent one for it.
    assert_eq!(
        game.ability_affinity(player, &AbilityEffect::Cleanse),
        AFFINITY_NEUTRAL
    );
}

#[test]
fn a_wild_carrier_gets_its_species_damage_affinity() {
    const BITER: &str = r#"(
    id: "test_biter",
    name: "Test Biter",
    glyph: 'b',
    color: Red,
    base_hp: 40,
    base_atk: 6,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (damage: 2.0),
)"#;
    let dir = super::support::modded_assets_dir(
        "wild_damage_affinity",
        &[],
        &[],
        &[("test_biter.ron", BITER)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    // Resolve through the same entry point battle uses, on a wild entity,
    // rather than asserting a damage total that pack composition and
    // initiative both move.
    let biter = super::support::spawn_wild_without_routine(&mut game, "test_biter", 3, 3);
    let effect = AbilityEffect::Damage {
        power: 6,
        spread: 0,
        status: None,
    };
    assert_eq!(game.ability_affinity(biter, &effect), 2.0);
}

/// A `FieldBuff` effect is field-only — it has no in-battle resolution, so
/// `battle_special_options` must never offer it, and the offer that survives
/// filtering must keep the index it holds in `actor_abilities`, since
/// `battle_set_action` resolves that index straight back against the
/// unfiltered list.
#[test]
fn a_field_only_ability_never_appears_in_the_battle_picker_and_indices_survive_filtering() {
    let dir = super::support::modded_assets_dir(
        "field_only_picker",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_regen.ron", super::support::FIELD_ONLY_ABILITY)],
    );
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 1, 200);
    game.world.entity_mut(player).insert(Routines(vec![
        "test_field_regen".to_string(),
        crate::abilities::DECOMPILE_ABILITY_ID.to_string(),
    ]));

    let options = game.battle_special_options(0);
    assert!(
        options.iter().all(|o| o.name != "Test Field Regen"),
        "a field-only ability must never appear in the in-battle picker: {options:?}"
    );
    assert_eq!(
        options.len(),
        1,
        "only decompile should remain once the field-only row is filtered"
    );
    assert!(
        options[0].name.to_lowercase().contains("decompile"),
        "decompile is actor_abilities()[1]; the surviving row must keep that index rather \
         than being renumbered to 0, or battle_set_action would resolve the wrong ability"
    );
    assert_eq!(options[0].index, 1);
}

/// A normal ability sitting *after* a field-only one in `Routines`, so its
/// stable `actor_abilities` index (1) never lines up with its position in
/// the filtered `battle_special_options` list (0). Regression for
/// `battle_set_action` indexing that filtered list positionally instead of
/// resolving by `SpecialOption::index` the way app-core's own consumers do
/// (`battle_target_title`, `handle_battle_special_key`).
const HEAL_AFTER_FIELD_ONLY: &str = r#"(
    id: "test_heal_after_field",
    name: "Test Heal After Field",
    description: "d",
    target: OneAlly,
    effect: Heal(power: 5),
)"#;

#[test]
fn committing_a_special_that_sits_behind_a_field_only_ability_resolves_the_right_one() {
    let dir = super::support::modded_assets_dir(
        "field_only_before_normal",
        &[],
        &[],
        &[],
        &[],
        &[
            ("test_field_regen.ron", super::support::FIELD_ONLY_ABILITY),
            ("test_heal_after_field.ron", HEAL_AFTER_FIELD_ONLY),
        ],
    );
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 1, 200);
    // Field-only entry first, so its stable index (0) is filtered out of the
    // menu and the normal ability behind it keeps stable index 1 while
    // sitting at menu position 0 — the exact misalignment the fix closes.
    game.world.entity_mut(player).insert(Routines(vec![
        "test_field_regen".to_string(),
        "test_heal_after_field".to_string(),
    ]));
    // Comfortably above anything the hostile's single swing this round can
    // deal, so the player is guaranteed to still be alive for their own
    // turn regardless of which side initiative favours — the point of this
    // test is whether the Special resolves to the right ability, not a race
    // against the enemy's hit.
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 100;
        stats.max_hp = 100;
    }

    let options = game.battle_special_options(0);
    assert_eq!(
        options.len(),
        1,
        "the field-only entry must not be offered: {options:?}"
    );
    let ability_index = options[0].index;
    assert_eq!(
        ability_index, 1,
        "the surviving option must still name its true actor_abilities position"
    );

    game.battle_set_action(
        0,
        BattleAction::Special {
            ability: ability_index,
            target: battle::SpecialTarget::Ally { slot: 0 },
        },
    )
    .expect(
        "committing the one ability the picker actually offered must succeed, not report \
         \"no such ability\"",
    );
    game.battle_resolve_round();

    // Read off the log line the heal itself writes rather than the player's
    // final HP: the hostile's own swing lands in the same round and could
    // otherwise mask a real heal behind a bigger hit, making the assertion
    // race the enemy's stats instead of checking what actually ran.
    let healed = game
        .message_log(usize::MAX)
        .into_iter()
        .any(|e| e.text.contains("patches"));
    assert!(
        healed,
        "the committed Special must have run the heal behind the filtered entry, not \
         silently resolved to nothing or to the wrong ability"
    );
}

/// The gate and the plan must never disagree — a routine the picker offers
/// has to be one the cast can pay for. `ability_unavailable` is the one seam
/// both read, and this asserts both halves rather than trusting that.
#[test]
fn an_empty_reserve_greys_a_special_and_refuses_the_same_plan() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 500);
    *game.world.get_mut::<PowerReserve>(sweeper).unwrap() = PowerReserve::new(0.0);

    let options = game.battle_special_options(1);
    // Index 0 is cascade_overflow — see `game_with_a_sweeper`'s kit order.
    let costed = &options[0];
    assert!(
        costed.unavailable.is_some(),
        "an empty reserve must grey the row with a reason"
    );

    assert!(
        game.battle_set_action(
            1,
            BattleAction::Special {
                ability: costed.index,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .is_err(),
        "and the same plan must be refused, or a greyed row is planable"
    );
}

/// **The caster pays.** This is the assertion that "every companion tracks
/// their power level" actually shipped: a companion's Special draws on the
/// companion's own reserve and leaves the player's alone.
#[test]
fn a_casting_companion_pays_from_its_own_reserve_not_the_players() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);

    let cost = {
        let db = game.world.resource::<crate::abilities::AbilityDb>();
        crate::abilities::routine_power_cost(db.get("cascade_overflow").unwrap())
    };
    assert!(cost > 0.0, "the fixture routine has to cost something");

    let player_before = game.world.get::<PowerReserve>(player).unwrap().get();
    let companion_before = game.world.get::<PowerReserve>(sweeper).unwrap().get();

    companion_uses_special(
        &mut game,
        sweeper,
        0,
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    let companion_spent = companion_before - game.world.get::<PowerReserve>(sweeper).unwrap().get();
    let player_spent = player_before - game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        (companion_spent - cost).abs() < 1e-4,
        "the companion should have paid {cost}, paid {companion_spent}"
    );
    assert!(
        player_spent <= HUNGER_DECAY_PER_TICK + 1e-4,
        "the player pays only the round's own drain, paid {player_spent}"
    );
}

/// Hostiles get no reserve, deliberately: `Game::choose_wild_action`'s
/// policy weights were trained against today's action distribution, and a
/// Power constraint would change which moves are available in which rounds.
/// A missing reserve must therefore leave a hostile's casting untouched —
/// `spend_power` is a no-op rather than a branch.
#[test]
fn a_hostile_with_no_reserve_casts_normally() {
    let (mut game, _) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);
    let hostiles: Vec<Entity> = {
        let mut q = game.world.query_filtered::<Entity, With<Hostile>>();
        q.iter(&game.world).collect()
    };
    assert!(!hostiles.is_empty(), "the pack has to exist");
    assert!(
        hostiles
            .iter()
            .all(|&h| game.world.get::<PowerReserve>(h).is_none()),
        "hostiles hold no reserve, which is what this test is about"
    );

    let before = game.world.get::<Stats>(player).unwrap().hp;
    for _ in 0..6 {
        if !game.has_active_battle() {
            break;
        }
        resolve_round_with(&mut game, BattleAction::Defend);
    }

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < before,
        "a reserve-less hostile must still be able to act"
    );
}

/// A high-level ability must not become deterministic: the spread scales
/// with the centre, proportionally. Delete the scaling of `spread` and this
/// fails — the band collapses to a point as the caster levels.
#[test]
fn an_abilitys_spread_scales_with_its_centre() {
    use crate::abilities::scaled_range;
    use crate::battle::DamageRange;
    let base = DamageRange::centred(10, 4);
    let low = scaled_range(base, 1, 1.0);
    let high = scaled_range(base, 40, 1.0);
    let low_width = low.max - low.min;
    let high_width = high.max - high.min;
    assert!(high.mean() > low.mean(), "the centre must scale");
    assert!(
        high_width > low_width,
        "the spread must scale with it, not stay put"
    );
}

/// `species::basic_attack_ability` converts a move to an ability, and the
/// centre-and-spread pair is what survives that losslessly — a `(min, max)`
/// pair would round on an odd width. Nothing in combat names `MoveDef`, so
/// this conversion is where a move's range has to arrive intact.
#[test]
fn converting_a_move_to_an_ability_keeps_its_range_exactly() {
    use crate::species::{MoveDef, basic_attack_ability};
    let mv = MoveDef {
        name: "Fray".into(),
        power: 9,
        spread: 4,
        effect: None,
        ranged: false,
    };
    let ability = basic_attack_ability(&"scrapper".into(), 0, &mv);
    assert_eq!(ability.attack_parts().0, mv.range());
}

/// A weapon **overrides** a natural attack rather than adding to it. A
/// companion still rolls a species move each turn for its name and its
/// status rider, but an equipped weapon supplies the damage range.
#[test]
fn an_equipped_weapon_replaces_the_natural_attack_range() {
    let mut game = Game::new(4801, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 30, 5);
    let natural = crate::battle::DamageRange { min: 6, max: 10 };
    assert_eq!(game.attack_range(companion, natural), natural);

    wear(&mut game, companion, "monofilament_whip");
    let armed = game.attack_range(companion, natural);
    assert_ne!(armed, natural, "the weapon must override, not be ignored");
    assert!(armed.max > natural.max);
}

/// Unarmed, the move's own range applies — the override is not "a weapon
/// slot exists", it is "a weapon with a range is worn".
#[test]
fn an_unarmed_companion_keeps_its_natural_range() {
    let mut game = Game::new(4802, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 30, 5);
    let natural = crate::battle::DamageRange { min: 6, max: 10 };
    assert_eq!(game.attack_range(companion, natural), natural);
}

/// A natural attack has a displayable band too, so a companion's unarmed
/// damage reads the same way a weapon's does — and a weapon replaces it on
/// the page exactly as it replaces it in the fight.
#[test]
fn a_programs_manifest_shows_the_band_it_actually_swings_for() {
    let mut game = Game::new(4803, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Owned, because only the player and their own programs may wear gear.
    let companion = spawn_tamed(&mut game, 30, 5);

    let natural = game
        .manifest(companion)
        .expect("a program has a manifest")
        .damage;
    assert!(natural.contains('–'), "no range in {natural:?}");

    wear(&mut game, companion, "monofilament_whip");
    let armed = game
        .manifest(companion)
        .expect("a program has a manifest")
        .damage;
    assert_ne!(
        armed, natural,
        "the worn weapon has to reach the page, not just the fight"
    );
}
