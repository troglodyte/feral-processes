//! The enemy side of the field: gathering a pack, partitioning it into
//! groups, initiative order, and what happens as groups fall.

use super::support::*;
use crate::*;

#[test]
fn gather_pack_pulls_in_nearby_hostiles_and_caps_at_max_pack_size() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Far enough out that zone 1's pack cap (2) is fully unlocked.
    let (ax, ay) = (spawn.x + PACK_SIZE_STEP_TILES * 5, spawn.y);
    let spawn_hostile = |game: &mut Game, x: i32, y: i32| {
        game.world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x, y },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 0,
                },
            ))
            .id()
    };
    let anchor = spawn_hostile(&mut game, ax, ay);
    for i in 1..=3 {
        spawn_hostile(&mut game, ax + i, ay);
    }

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack[0], anchor,
        "the creature actually bumped into should always be the pack's front"
    );
    assert_eq!(
        pack.len(),
        PACK_SIZE_PER_ZONE as usize,
        "zone 1's pack cap should bind with 3 other Hostiles in range"
    );
}

/// A pack partitions into one group per species, in first-appearance
/// order. `gather_pack` walks an ECS query, so the deterministic order
/// has to come from the partition step itself — an incidental query
/// order is exactly the kind of thing that produced this repo's
/// unsorted-habitat-lookup flake.
#[test]
fn a_mixed_pack_partitions_into_one_group_per_species_in_first_appearance_order() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let b = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
    let d = game.spawn_wild_creature("scrapper", 6, 5).unwrap();

    game.start_battle(vec![a, b, c, d]);

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 2, "two species means two groups");
    assert_eq!(battle.groups[0].species, "glitch", "glitch appeared first");
    assert_eq!(battle.groups[0].members, vec![a, c]);
    assert_eq!(battle.groups[1].species, "scrapper");
    assert_eq!(battle.groups[1].members, vec![b, d]);
}

/// Only `MAX_ENEMY_GROUPS` species can engage at once. The overflow stays
/// on the map as ordinary hostiles rather than being despawned — the
/// player meets them on the next bump.
#[test]
fn a_pack_of_more_than_four_species_engages_the_four_largest_and_leaves_the_rest() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // glitch x3, scrapper x2, virus x2, worm x2, sprite x1 -> sprite is
    // the smallest group and the one left out.
    let mut spawned = Vec::new();
    for (species, count) in [
        ("glitch", 3),
        ("scrapper", 2),
        ("virus", 2),
        ("worm", 2),
        ("sprite", 1),
    ] {
        for i in 0..count {
            spawned.push(game.spawn_wild_creature(species, 5, 5 + i).unwrap());
        }
    }
    let sprite = *spawned.last().unwrap();

    game.start_battle(spawned.clone());

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), MAX_ENEMY_GROUPS);
    assert!(
        battle.groups.iter().all(|g| g.species != "sprite"),
        "the smallest group should be the one left out"
    );
    assert!(
        game.world.get_entity(sprite).is_ok(),
        "an un-engaged hostile must stay on the map, never be despawned"
    );
}

/// Initiative order must be reproducible under a fixed seed. Every roll
/// goes through the existing `GameRng`, so a seeded test can assert an
/// exact order without touching the wall clock.
#[test]
fn initiative_order_is_reproducible_under_a_fixed_seed() {
    let order_for = |seed: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let b = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![a, b]);
        game.roll_initiative()
    };
    assert_eq!(order_for(1234), order_for(1234), "same seed, same order");
}

/// Speed has to actually bias the order, or the stat is decoration.
/// Sampled rather than asserted per-round: a d10 on top of an 8-point
/// gap still lets the Construct win occasionally, and a test that
/// forbade that would be asserting the die doesn't exist.
#[test]
fn a_faster_species_wins_initiative_far_more_often_than_a_slower_one() {
    let mut sprite_first = 0;
    for seed in 0..200u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let sprite = game.spawn_wild_creature("sprite", 5, 5).unwrap();
        let construct = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![sprite, construct]);
        let order = game.roll_initiative();
        let pos = |e: Entity| {
            order
                .iter()
                .position(|a| game.actor_entity(*a) == Some(e))
                .unwrap()
        };
        if pos(sprite) < pos(construct) {
            sprite_first += 1;
        }
    }
    assert!(
        sprite_first > 150,
        "a Sprite (14) should beat a Construct (6) far more often than not, got {sprite_first}/200"
    );
}

#[test]
fn defeating_the_front_pack_member_continues_the_battle_against_the_next_one() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.atk = 1000; // guarantees a one-shot kill on the front target below
    }
    let front = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 0,
            },
        ))
        .id();
    let second = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Hostile,
            Position { x: 6, y: 5 },
            Stats {
                hp: 500,
                max_hp: 500,
                atk: 1,
                def: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![front, second]);

    player_attacks(&mut game);

    assert!(
        game.has_active_battle(),
        "a pack member is still alive, so the fight should continue rather than end"
    );
    let view = game
        .battle_view()
        .expect("battle should still be active with the second member up front");
    assert_eq!(
        view.groups.len(),
        1,
        "both members are the same species, so they share one group"
    );
    assert_eq!(
        view.groups[0].count, 1,
        "only the second (surviving) member should remain, now as the front"
    );
    assert_eq!(
        view.groups[0].front_hp, 500,
        "the new front should be the untouched second pack member"
    );
}

/// Wiping the front group promotes whatever sat behind it — the central
/// tension of the reach rule: clearing front-to-back is not
/// automatically correct, because it walks the back rank into melee.
#[test]
fn wiping_the_front_group_promotes_the_group_behind_it() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    game.start_battle(vec![glitch, scrapper]);
    let player = game.player_entity();

    assert_eq!(
        game.world.resource::<BattleState>().groups[0].species,
        "glitch"
    );

    game.world.get_mut::<Stats>(glitch).unwrap().hp = 0;
    let battle_over = game.finish_group_member(0, player);

    assert!(!battle_over, "the scrapper group is still standing");
    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 1);
    assert_eq!(
        battle.groups[0].species, "scrapper",
        "the surviving group should have shifted into index 0"
    );
}

/// A planned target can die earlier in the same round than the member
/// who aimed at it, leaving a stale group index behind. Falling back to
/// the front group is the difference between a wasted turn and an
/// out-of-bounds panic.
#[test]
fn a_stale_target_group_index_falls_back_to_the_front_group() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    game.start_battle(vec![glitch, scrapper]);

    assert_eq!(game.retarget(1), Some(1), "group 1 is standing");

    game.world.get_mut::<Stats>(scrapper).unwrap().hp = 0;
    game.finish_group_member(1, player);

    assert_eq!(
        game.retarget(1),
        Some(0),
        "a stale index must fall back to the lowest surviving group"
    );
}
