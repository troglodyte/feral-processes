//! The enemy side of the field: gathering a pack, partitioning it into
//! groups, initiative order, and what happens as groups fall.

use super::support::*;
use crate::tuning::{MAX_ENEMY_GROUPS, MAX_GROUP_SIZE};
use crate::*;

#[test]
fn gather_pack_pulls_in_nearby_hostiles_and_caps_the_pack_at_max_enemy_groups_worth() {
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
    // Both halves of the ceiling ride the zone curve, and zone 2 is where
    // they're small enough to bind on a fixture this size: one step in
    // doubles the group size to 2, and the group *count* is 2 of a possible
    // `MAX_ENEMY_GROUPS`. So the ceiling here is 4, and there are
    // deliberately more than 4 hostiles in range, or the pack size would
    // just be however many the radius happened to reach and the ceiling
    // would go untested.
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (ax, ay) = (spawn.x + 500, spawn.y);
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
                    mitigation: 0,
                },
            ))
            .id()
    };
    let anchor = spawn_hostile(&mut game, ax, ay);
    // Twelve in total, all inside the gather radius of 3. Zone 2 sizes
    // every one of them the same, wherever they stand.
    for i in 0..11 {
        spawn_hostile(&mut game, ax + i % 4, ay + i % 3);
    }

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack[0], anchor,
        "the creature actually bumped into should always be the pack's front"
    );
    assert_eq!(
        pack.len(),
        4,
        "twelve are in range, but a pack is capped at zone 2's per-group \
         size (2) times its group count (2)"
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
    // Deep enough for groups of two: at zone 1 every
    // group is capped at a single member, which would make the partition
    // order this pins unobservable.
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);
    let a = game.spawn_wild_creature("glitch", x, y).unwrap();
    let b = game.spawn_wild_creature("scrapper", x, y + 1).unwrap();
    let c = game.spawn_wild_creature("glitch", x, y + 2).unwrap();
    let d = game.spawn_wild_creature("scrapper", x + 1, y).unwrap();

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
    // Zone 4: deep enough that the per-group ceiling doesn't flatten these
    // to one member each, and that `max_enemy_groups` actually reaches
    // `MAX_ENEMY_GROUPS` — the overflow below is the point of the test.
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);
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
            spawned.push(game.spawn_wild_creature(species, x, y + i).unwrap());
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

/// The opening buffer, from the battle side: whatever is standing on the
/// ground around a zone-1 breach, bumping into it starts a one-on-one.
/// Four species share one tile here, which before the group count rode the
/// distance curve was a four-on-one against a player who has no companions
/// yet — `balance_sim::beatable_by_a_fresh_player` scores that as a loss
/// against every shipped species, boss or not.
#[test]
fn a_fight_at_the_danger_origin_is_a_single_program_however_many_are_standing_there() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let mut crowd = Vec::new();
    for (i, species) in ["glitch", "scrapper", "virus", "worm"].iter().enumerate() {
        crowd.push(
            game.spawn_wild_creature(species, spawn.x, spawn.y + i as i32)
                .unwrap(),
        );
    }

    game.start_battle(crowd.clone());

    let battle = game.world.resource::<BattleState>();
    assert_eq!(
        battle.groups.len(),
        1,
        "the origin allows one group, so three of the four wait their turn"
    );
    assert_eq!(
        battle.groups[0].members.len(),
        1,
        "and zone 1 caps that group at a single member"
    );
    assert!(
        crowd.iter().all(|&e| game.world.get_entity(e).is_ok()),
        "the three left out stay on the map, met on the next bump"
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
        let (x, y) = multi_group_ground(&mut game);
        let sprite = game.spawn_wild_creature("sprite", x, y).unwrap();
        let construct = game.spawn_wild_creature("construct", x, y + 1).unwrap();
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
    // Named rather than "whichever species sorts first", which is Cipher —
    // whose Encrypt carries a 35% stun, and a stunned player never lands
    // the one-shot this test is built around. Glitch's moveset is plain
    // damage, so the assertion below can't be eaten by an effect roll.
    let species_id = "glitch".to_string();
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.atk = 1000; // guarantees a one-shot kill on the front target below
    }
    // Deep and far enough for a group of two: a zone-1 spawn point caps a
    // group at one member, and there would be no second member to promote.
    // Both members carry explicit stats, so the distance scaling that comes
    // with moving them out here doesn't touch what this test asserts.
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);
    let front = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Hostile,
            Position { x, y },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 0,
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
            Position { x: x + 1, y },
            Stats {
                hp: 500,
                max_hp: 500,
                atk: 1,
                mitigation: 0,
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
    let (x, y) = multi_group_ground(&mut game);
    let glitch = game.spawn_wild_creature("glitch", x, y).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", x, y + 1).unwrap();
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

/// A planned target can die earlier in the same round than the member who
/// aimed at it. A plan names a *group*, not a slot in a vector that
/// re-letters under it, so the aim either follows that group to wherever it
/// now sits or it fizzles — it never slides onto whoever moved up.
#[test]
fn a_planned_target_follows_its_group_and_fizzles_when_that_group_dies() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (x, y) = multi_group_ground(&mut game);
    let glitch = game.spawn_wild_creature("glitch", x, y).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", x, y + 1).unwrap();
    game.start_battle(vec![glitch, scrapper]);

    assert_eq!(game.retarget(1), Some(1), "group 1 is standing");

    // The *front* group falls, so everything behind it shifts down one.
    game.world.get_mut::<Stats>(glitch).unwrap().hp = 0;
    game.finish_group_member(0, player);

    assert_eq!(
        game.retarget(0),
        None,
        "the group that was aimed at is gone — the turn is spent, not redirected"
    );
    assert_eq!(
        game.retarget(1),
        Some(0),
        "the survivor kept its identity and is found at its new index"
    );
    assert!(
        game.world.get::<Stats>(scrapper).is_some(),
        "the scrapper is the group that must not be hit by the stale aim"
    );
}

/// The other half of the same rule: when the group that dies is the *last*
/// one, a stale aim has nowhere to slide to and must still fizzle rather
/// than wrapping onto the front group.
#[test]
fn a_stale_target_group_index_does_not_wrap_onto_the_front_group() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (x, y) = multi_group_ground(&mut game);
    let glitch = game.spawn_wild_creature("glitch", x, y).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", x, y + 1).unwrap();
    game.start_battle(vec![glitch, scrapper]);

    game.world.get_mut::<Stats>(scrapper).unwrap().hp = 0;
    game.finish_group_member(1, player);

    assert_eq!(game.retarget(1), None);
    assert_eq!(game.retarget(0), Some(0), "the front group is untouched");
    assert!(
        game.world.get::<Stats>(glitch).is_some(),
        "the glitch must not inherit the aim meant for the scrapper"
    );
}

#[test]
fn ceil_sqrt_is_exact_at_perfect_squares() {
    for (n, expected) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (3, 2),
        (4, 2),
        (9, 3),
        (10, 4),
        (81, 9),
        (100, 10),
    ] {
        assert_eq!(
            crate::battle::ceil_sqrt(n),
            expected,
            "ceil_sqrt({n}) should be {expected} — a float sqrt().ceil() rounds \
             the wrong way at perfect squares"
        );
    }
}

/// A single-species cluster is one group, so without a per-group ceiling a
/// 30-strong cluster would fight as one 30-deep column regardless of what
/// the local danger curve allows.
#[test]
fn a_group_is_capped_at_the_local_group_size_and_the_rest_stay_on_the_map() {
    let mut game = Game::new(311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Clear of the starting population, which would otherwise join the
    // cluster. Which tile it is decides nothing about the ceiling.
    let (x, y) = (spawn.x + 500, spawn.y);

    let members: Vec<Entity> = (0..30)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i % 3).unwrap())
        .collect();

    let groups = game.group_pack(members.clone());

    assert_eq!(groups.len(), 1, "one species is one group");
    assert_eq!(
        groups[0].members.len(),
        4,
        "zone 3 is two escalation steps in, so a group caps at 2^2 — the
         doubling curve binds well below zone 3's own cap of 19, and it is
         the lower of the two that decides"
    );
    let still_alive = members
        .iter()
        .filter(|&&e| game.world.get_entity(e).is_ok())
        .count();
    assert_eq!(
        still_alive, 30,
        "members over the ceiling stay standing on the map, they are not despawned"
    );
}

/// The headline shape: four groups of a hundred, and nothing bigger, out of
/// a cluster that could supply five hundred.
#[test]
fn a_mixed_swarm_fights_as_four_groups_of_a_hundred() {
    let mut game = Game::new(313, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Zone 12 is where `zone_group_cap` saturates at `MAX_GROUP_SIZE` under
    // the linear curve (1 + 9 * 11 = 100). Anything shallower is bounded by
    // its own zone rather than by the hard ceiling this test is about.
    game.world.resource_mut::<ZoneLevel>().0 = 12;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);

    let mut cluster = Vec::new();
    for species in ["glitch", "scrapper", "drone", "sprite", "zero_day"] {
        for i in 0..105 {
            cluster.push(game.spawn_wild_creature(species, x, y + i % 5).unwrap());
        }
    }

    let groups = game.group_pack(cluster);

    assert_eq!(
        groups.len(),
        MAX_ENEMY_GROUPS,
        "five species can't all engage — the largest four do"
    );
    for group in &groups {
        assert_eq!(
            group.members.len(),
            MAX_GROUP_SIZE as usize,
            "no group may pass MAX_GROUP_SIZE, however deep the cluster is"
        );
    }
}

/// The gather radius has to widen with the swarm, or a group scattered
/// across a 21-tile span pulls into the fight in fragments.
#[test]
fn gather_radius_widens_with_the_local_group_size() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs().into_iter().next().unwrap().id.clone()
    };
    let mut game = Game::new(312, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 8;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Zone 8, fully unlocked: groups of 64, so a radius of ceil_sqrt(64) = 8.
    let (ax, ay) = (spawn.x + 500, spawn.y);
    let hostile = |game: &mut Game, x: i32, y: i32| {
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
                    mitigation: 0,
                },
            ))
            .id()
    };
    let anchor = hostile(&mut game, ax, ay);
    hostile(&mut game, ax + 8, ay);

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack.len(),
        2,
        "eight tiles out is inside a zone-8 swarm's radius, though it is well \
         outside the PACK_GATHER_RADIUS a small pack uses"
    );
}

/// A swarm is an attrition wall, not a linear damage multiplier: only the
/// front `ceil(sqrt(n))` members of a group get an initiative slot.
#[test]
fn only_the_front_ceil_sqrt_of_a_group_acts_each_round() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 6;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);

    let members: Vec<Entity> = (0..9)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i).unwrap())
        .collect();
    game.start_battle(members);

    let acting = game
        .roll_initiative()
        .into_iter()
        .filter(|a| matches!(a, crate::battle::Actor::Enemy { .. }))
        .count();

    assert_eq!(
        acting, 3,
        "a group of nine should swing three at a time, not nine"
    );
}

/// `begin_battle` is the seam the arena fights through: it takes groups
/// already built and opens the battle around them verbatim. `start_battle`
/// is the only path that caps a pack, so a group handed straight to
/// `begin_battle` keeps every member the caller put in it — here six, at a
/// zone whose `group_size_ceiling` is one.
#[test]
fn begin_battle_opens_a_battle_around_pre_built_groups_without_capping_them() {
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = spawn_wild_on_player_tile(&mut game);
    let species_id = game.world.get::<Creature>(species).unwrap().species.clone();

    let mut members = vec![species];
    members.extend((0..5).map(|_| spawn_wild_on_player_tile(&mut game)));
    let groups = vec![
        EnemyGroup {
            species: species_id.clone(),
            members: members[..3].to_vec(),
        },
        EnemyGroup {
            species: species_id,
            members: members[3..].to_vec(),
        },
    ];

    game.begin_battle(groups);

    let state = game.world.resource::<BattleState>();
    assert_eq!(state.groups.len(), 2, "both groups should be kept");
    assert_eq!(state.groups[0].members.len(), 3);
    assert_eq!(state.groups[1].members.len(), 3);
    assert!(game.has_active_battle());
    assert!(
        !game.battle_log().is_empty(),
        "the intercept line should still open the pane"
    );
}

#[test]
fn gather_pack_does_not_sweep_a_bystanding_boss_into_an_ordinary_fight() {
    // A boss is `is_boss` because it *spawns as its own group*, and past
    // zone 1 it brings its own escort — manufactured in `spawn_pack`, not
    // gathered here. So a boss standing near an ordinary cluster is a
    // separate fight, and bumping the cluster must not drag it in.
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (ax, ay) = (spawn.x + 500, spawn.y);
    let spawn_hostile = |game: &mut Game, species: &str, x: i32, y: i32| {
        game.world
            .spawn((
                Creature {
                    species: species.to_string(),
                },
                Hostile,
                Position { x, y },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
            ))
            .id()
    };
    let anchor = spawn_hostile(&mut game, "drone", ax, ay);
    let neighbour = spawn_hostile(&mut game, "drone", ax + 1, ay);
    let boss = spawn_hostile(&mut game, "overseer", ax, ay + 1);

    let pack = game.gather_pack(anchor);

    assert!(pack.contains(&anchor));
    assert!(
        pack.contains(&neighbour),
        "an ordinary neighbour in range still joins the fight"
    );
    assert!(
        !pack.contains(&boss),
        "the boss standing beside the cluster is its own fight, not part of this one"
    );
}

#[test]
fn bumping_the_boss_itself_still_starts_the_boss_fight() {
    // The filter is about what gets *swept in*, never about the anchor:
    // walking into a boss has to fight the boss.
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (ax, ay) = (spawn.x + 500, spawn.y);
    let boss = game
        .world
        .spawn((
            Creature {
                species: "overseer".to_string(),
            },
            Hostile,
            Position { x: ax, y: ay },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 0,
            },
        ))
        .id();

    assert_eq!(game.gather_pack(boss), vec![boss]);
}
