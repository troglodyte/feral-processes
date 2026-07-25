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
fn battle_with_a_pack_of(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position {
                        x: 5 + i as i32,
                        y: 5,
                    },
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
