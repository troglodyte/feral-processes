//! Zone depth scaling and what survives a breach into the next zone.

use super::support::*;
use crate::tuning::{
    DISTANCE_STAT_STEP_TILES, GROUP_SIZE_STEP_TILES, MAX_BUILD_DISTANCE_FROM_HOME,
    MAX_DISTANCE_STAT_MULTIPLIER, MAX_ENEMY_GROUPS, MAX_GROUP_SIZE, NEST_AGGRO_LEASH_RADIUS,
    NEST_CACHE_FRAGMENT_ZONE_BONUS, NEST_CACHE_FRAGMENTS, NEST_CACHE_WORK_RESOURCE_MULT,
    NEST_DURABILITY, NEST_PURSUIT_STEPS_PER_TICK, NEST_RESPAWN_TICKS, WORK_RESOURCE_DROP,
};
use crate::*;

#[test]
fn entering_a_zone_portal_increments_zone_and_doubles_wild_stats() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.player_status().zone, 1);
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));

    game.move_player(1, 0);

    assert_eq!(
        game.player_status().zone,
        2,
        "walking onto a zone portal should advance the zone level"
    );

    let species_db = game.species_defs();
    let mut query = game
        .world
        .query_filtered::<(&Creature, &Stats, &Position), With<Hostile>>();
    let results: Vec<_> = query
        .iter(&game.world)
        .map(|(c, s, p)| (c.species.clone(), s.max_hp, *p))
        .collect();
    assert!(
        !results.is_empty(),
        "zone 2 should have spawned wild creatures"
    );
    for (species_id, max_hp, _pos) in results {
        let species = species_db.iter().find(|s| s.id == species_id).unwrap();
        // Zone 2 doubles base stats at minimum (`ZoneLevel::stat_multiplier`);
        // `distance_stat_multiplier` can scale it up further (capped at
        // `MAX_DISTANCE_STAT_MULTIPLIER`) depending how far from the
        // zone's entry point it spawned, and each spawn's individual
        // `Potential::hp_roll` can additionally scale it within
        // `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`. Checked as a range
        // rather than an exact figure since `WanderAi` may have already
        // moved this creature from its spawn position by the time this
        // runs.
        assert!(
            (max_hp as f32) >= (species.base_hp as f32) * 2.0 * MIN_INDIVIDUAL_ROLL,
            "zone 2 wild creatures should have at least doubled stats, times the roll floor"
        );
        assert!(
            (max_hp as f32)
                <= (species.base_hp as f32)
                    * 2.0
                    * MAX_DISTANCE_STAT_MULTIPLIER
                    * MAX_INDIVIDUAL_ROLL,
            "zone 2 wild creatures shouldn't exceed the zone doubling times the distance cap and roll ceiling"
        );
    }
}

#[test]
fn distance_stat_multiplier_measures_from_the_zone_spawn_point_when_no_home_exists() {
    let game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();

    assert_eq!(
        game.distance_stat_multiplier(spawn.x, spawn.y),
        1.0,
        "right at the spawn point, distance shouldn't add any scaling"
    );
    assert_eq!(
        game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES - 1, spawn.y),
        1.0,
        "just short of a full step away should still read as no scaling"
    );
    assert!(
        (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES, spawn.y) - 1.25).abs()
            < f32::EPSILON,
        "one full step away should add one step of bonus"
    );
    assert!(
        (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES * 2, spawn.y) - 1.5)
            .abs()
            < f32::EPSILON,
        "two full steps away should add two steps of bonus"
    );
    assert_eq!(
        game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
        MAX_DISTANCE_STAT_MULTIPLIER,
        "far enough away should cap rather than grow without bound"
    );
}

#[test]
fn distance_stat_multiplier_treats_the_whole_platform_as_distance_zero() {
    let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    place_home(&mut game, 0, 0);

    assert_eq!(
        game.distance_stat_multiplier(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y),
        1.0,
        "the platform edge is still perfectly safe territory"
    );
    assert_eq!(
        game.distance_stat_multiplier(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES - 1,
            spawn.y
        ),
        1.0,
        "one tile short of the first step past the edge is still unscaled"
    );
    assert!(
        (game.distance_stat_multiplier(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES,
            spawn.y
        ) - 1.25)
            .abs()
            < f32::EPSILON,
        "the first step up lands one full step past the platform edge — 22 tiles from Home"
    );
    assert_eq!(
        game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
        MAX_DISTANCE_STAT_MULTIPLIER,
        "the cap is unchanged"
    );
}

/// The cap is linear, not geometric, and that is the point of it.
///
/// Geometric growth from a base of 1 spends its whole early range in single
/// digits — zone 2 capped every group at 3, so a party of five met packs of
/// three however far out or however deep they pushed — and then runs away
/// past zone 4, where 27 and 81 are numbers no encounter design uses. A
/// straight line opens the early zones, which is where the game is actually
/// played, and keeps the tail somewhere a fight can still be read.
#[test]
fn zone_group_cap_is_linear_and_never_passes_max_group_size() {
    use crate::game::spawning::zone_group_cap;
    assert_eq!(
        zone_group_cap(1),
        1,
        "zone 1 is solo, whatever else is true"
    );
    assert_eq!(zone_group_cap(2), 10);
    assert_eq!(zone_group_cap(3), 19);
    assert_eq!(zone_group_cap(4), 28);
    assert_eq!(zone_group_cap(5), 37);
    assert_eq!(
        zone_group_cap(12),
        MAX_GROUP_SIZE,
        "1 + 9 * 11 is exactly 100, so zone 12 is where the clamp starts"
    );
    assert_eq!(
        zone_group_cap(99),
        MAX_GROUP_SIZE,
        "a deep zone must clamp rather than overflow the arithmetic"
    );
}

#[test]
fn max_group_size_also_counts_from_the_platform_edge() {
    let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    place_home(&mut game, 0, 0);

    assert_eq!(
        game.max_group_size(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y, None),
        1,
        "groups shouldn't grow inside territory that's still stat-x1.0"
    );
    // The discriminating case: without the platform offset this is a full
    // GROUP_SIZE_STEP_TILES from spawn and would already have doubled.
    assert_eq!(
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES, spawn.y, None),
        1,
        "a full step from spawn is only half a step from the platform edge"
    );
    assert_eq!(
        game.max_group_size(
            spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + GROUP_SIZE_STEP_TILES,
            spawn.y,
            None
        ),
        2,
        "the first doubling lands one full step past the platform edge"
    );
}

#[test]
fn max_group_size_doubles_with_distance_and_caps_per_zone() {
    // No Home is placed, so distances count straight from the spawn point —
    // see the platform-edge test above for the case where one exists.
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let at = |game: &Game, steps: i32| {
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES * steps, spawn.y, None)
    };

    assert_eq!(at(&game, 0), 1, "right at spawn, groups are always solo");
    assert_eq!(
        at(&game, 10),
        1,
        "zone 1 is solo however far you walk — that is the whole point of zone 1"
    );

    // Zone 2, not zone 1: a cap of 10 leaves the first three doublings
    // visible, so the boundary case below is measuring the division and not
    // the clamp.
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    assert_eq!(at(&game, 0), 1, "every zone starts solo at its entry point");
    assert_eq!(
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES - 1, spawn.y, None),
        1,
        "one tile short of a full step is still solo — the doubling is keyed \
         to whole steps, and an off-by-one here would double a step early"
    );
    assert_eq!(at(&game, 1), 2, "one step out doubles");
    assert_eq!(at(&game, 3), 8, "and keeps doubling while under the cap");
    assert_eq!(
        at(&game, 4),
        10,
        "four steps would be 16, but zone 2 caps at 10"
    );
    assert_eq!(at(&game, 10), 10, "and it stays capped however far out");

    game.world.resource_mut::<ZoneLevel>().0 = 5;
    assert_eq!(
        at(&game, 5),
        32,
        "five steps is 2^5, still under zone 5's cap of 37"
    );
    assert_eq!(
        at(&game, 6),
        37,
        "six steps would be 64, so the zone cap binds"
    );

    game.world.resource_mut::<ZoneLevel>().0 = 12;
    assert_eq!(
        at(&game, 7),
        MAX_GROUP_SIZE,
        "zone 12 is where the linear cap first reaches the hard ceiling"
    );

    // The exponent is clamped, so an absurd distance must not shift past
    // the width of the type.
    game.world.resource_mut::<ZoneLevel>().0 = 99;
    assert_eq!(
        game.max_group_size(spawn.x + 10_000, spawn.y, None),
        MAX_GROUP_SIZE,
        "no zone or distance may push a group past MAX_GROUP_SIZE"
    );
}

/// The count of groups rides the same distance curve as their size, one
/// step per group rather than a doubling. The origin end is the part that
/// matters: a fight at your doorstep is one program, which is what makes a
/// zone-1 opening survivable for a player who has no companions yet.
#[test]
fn max_enemy_groups_gains_one_group_per_step_out_and_stops_at_the_ceiling() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let at = |game: &Game, steps: i32| {
        game.max_enemy_groups(spawn.x + GROUP_SIZE_STEP_TILES * steps, spawn.y, None)
    };

    assert_eq!(at(&game, 0), 1, "one group at the danger origin");
    assert_eq!(
        game.max_enemy_groups(spawn.x + GROUP_SIZE_STEP_TILES - 1, spawn.y, None),
        1,
        "one tile short of a full step is still a single group — an off-by-one \
         here would end the opening buffer a tile early"
    );
    assert_eq!(at(&game, 1), 2);
    assert_eq!(at(&game, 2), 3);
    assert_eq!(at(&game, 3), MAX_ENEMY_GROUPS);
    assert_eq!(
        at(&game, 10_000),
        MAX_ENEMY_GROUPS,
        "distance may not push a fight past the group ceiling"
    );

    // Zone depth doesn't enter into it — only distance does, so the ring
    // around a deep zone's entry point is quiet too.
    game.world.resource_mut::<ZoneLevel>().0 = 5;
    assert_eq!(at(&game, 0), 1, "every zone's entry point holds one group");

    // And it counts from the platform edge, like every other danger curve.
    place_home(&mut game, 0, 0);
    assert_eq!(
        game.max_enemy_groups(spawn.x + GROUP_SIZE_STEP_TILES, spawn.y, None),
        1,
        "a full step from spawn is only half a step from the platform edge"
    );
}

#[test]
fn stepping_through_a_portal_consumes_it_so_it_never_travels() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
    game.place_structure("portal", 1, 0).unwrap();

    game.move_player(1, 0);

    assert_eq!(
        game.world.resource::<ZoneLevel>().0,
        2,
        "stepping onto the portal breaches"
    );
    assert!(
        find_structure_by_kind(&mut game, "portal").is_none(),
        "a portal is one-use — carrying it forward would make every later breach free"
    );
    assert!(
        find_structure_by_kind(&mut game, "home").is_some(),
        "consuming the portal must not take the rest of the base with it"
    );
}

/// `warp_to_zone` exists for the savetool, and its whole value is that it
/// is not a shortcut: writing `ZoneLevel` directly would relabel the party
/// as being in zone 4 while leaving zone 1's map and zone 1's wild programs
/// around them.
///
/// The wild population is what discriminates. The spawn point does *not* —
/// `find_walkable_start` spirals out from the origin, so for most seeds it
/// answers (0, 0) in every sector and is identical either way. An earlier
/// version of this test asserted on it and would have passed against a
/// `ZoneLevel` write.
#[test]
fn warping_forward_lands_exactly_where_stepping_through_the_breaches_lands() {
    let wild = |game: &mut Game| {
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Stats), With<Hostile>>();
        let mut found: Vec<_> = query
            .iter(&game.world)
            .map(|(c, s)| (c.species.clone(), s.max_hp))
            .collect();
        found.sort();
        found
    };

    let mut warped = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = wild(&mut warped);
    warped.warp_to_zone(4).unwrap();

    let mut stepped = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for _ in 0..3 {
        stepped.enter_next_zone();
    }

    assert_eq!(warped.player_status().zone, 4);
    let after = wild(&mut warped);
    assert_ne!(
        after, before,
        "warping must generate new sectors, not just relabel the current one"
    );
    assert_eq!(
        after,
        wild(&mut stepped),
        "and must leave exactly what walking through three portals leaves"
    );
}

/// There is no portal back, so a breach only runs forward.
#[test]
fn warping_to_a_zone_that_is_not_ahead_is_refused() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.warp_to_zone(3).unwrap();

    assert!(
        game.warp_to_zone(3).is_err(),
        "the current zone is not ahead"
    );
    assert!(game.warp_to_zone(1).is_err(), "and zone 1 is behind");
    assert_eq!(
        game.player_status().zone,
        3,
        "a refused warp must not have moved the party part of the way"
    );
}

#[test]
fn breaching_carries_every_structure_and_its_offset_from_home() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, node) = build_a_base(&mut game);
    let before = {
        let h = *game.world.get::<Position>(home).unwrap();
        let n = *game.world.get::<Position>(node).unwrap();
        (n.x - h.x, n.y - h.y)
    };

    game.enter_next_zone();

    assert!(
        game.world.get_entity(home).is_ok(),
        "the Home travels through the breach"
    );
    assert!(
        game.world.get_entity(node).is_ok(),
        "so does everything built around it"
    );
    let h = *game.world.get::<Position>(home).unwrap();
    let n = *game.world.get::<Position>(node).unwrap();
    assert_eq!(
        (n.x - h.x, n.y - h.y),
        before,
        "the base's layout must be preserved exactly, not reshuffled"
    );
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_eq!(
        (h.x, h.y),
        (spawn.x, spawn.y),
        "the Home lands at the new spawn point"
    );
}

#[test]
fn breaching_with_a_base_still_populates_the_new_zone() {
    for seed in 0u32..12 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        build_a_base(&mut game);

        game.enter_next_zone();

        let hostiles = {
            let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
            query.iter(&game.world).count()
        };
        assert!(
            hostiles > 0,
            "seed {seed}: a zone breached into with a base must still have wild programs \
             in it. The platform has no habitat species and is exactly as wide as the \
             initial spawn scatter, so a scatter that never reaches past its edge leaves \
             the whole zone empty."
        );
    }
}

#[test]
fn breaching_preserves_structure_durability_and_node_stock() {
    let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = build_a_base(&mut game);
    game.world.get_mut::<Durability>(node).unwrap().hp = 7;
    game.world.get_mut::<ResourceNode>(node).unwrap().amount = 2;

    game.enter_next_zone();

    assert_eq!(
        game.world.get::<Durability>(node).unwrap().hp,
        7,
        "damage travels with the structure"
    );
    assert_eq!(
        game.world.get::<ResourceNode>(node).unwrap().amount,
        2,
        "so does mined-down stock"
    );
}

#[test]
fn breaching_restamps_the_platform_around_the_new_spawn_point() {
    let mut game = Game::new(942, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    build_a_base(&mut game);

    game.enter_next_zone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_eq!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(spawn.x, spawn.y)
            .biome,
        Biome::Platform,
        "the slab is re-stamped on the new map"
    );
    assert_eq!(
        game.world.resource::<Platform>().center,
        Some((spawn.x, spawn.y)),
        "and the resource follows it"
    );
}

#[test]
fn breaching_leaves_a_cronjob_assignment_pointing_at_a_live_structure() {
    let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = build_a_base(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: node,
        progress: 0,
        required: 10,
    });

    game.enter_next_zone();

    let task = game
        .world
        .get::<Task>(worker)
        .expect("the cronjob survives the breach");
    assert_eq!(
        task.target, node,
        "and still points at the structure that travelled with it"
    );
    assert!(
        game.world.get_entity(task.target).is_ok(),
        "which is still alive"
    );
}

#[test]
fn zone_transition_carries_tamed_companions_and_the_base_but_leaves_wild_creatures_behind() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Clear anything the world's own initial habitat spawn happened to
    // place on the tiles this test is about to use for its own fixtures
    // (portal, home, wild) — the exact initial layout isn't this test's
    // concern, and asserting it stays untouched would make the test
    // fragile to unrelated changes in spawn odds/roll counts.
    let stray: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Position)>();
        query
            .iter(&game.world)
            .filter(|(e, p)| {
                *e != player
                    && ((p.x, p.y) == (ppos.x + 1, ppos.y)
                        || (p.x, p.y) == (ppos.x + 3, ppos.y)
                        || (p.x, p.y) == (ppos.x + 5, ppos.y))
            })
            .map(|(e, _)| e)
            .collect()
    };
    for e in stray {
        game.world.despawn(e);
    }

    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    let species = game.species_defs().into_iter().next().unwrap();
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position {
                x: ppos.x + 3,
                y: ppos.y,
            },
            Stats {
                hp: 5,
                max_hp: 5,
                atk: 1,
                def: 1,
            },
        ))
        .id();

    let home = game
        .world
        .spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position {
                x: ppos.x + 5,
                y: ppos.y,
            },
        ))
        .id();

    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));

    game.move_player(1, 0);

    assert_eq!(game.player_status().zone, 2);
    assert!(
        game.world.get::<Tamed>(companion).is_some(),
        "the companion should still be tamed after breaching"
    );
    assert!(
        game.world.get::<Creature>(wild).is_none(),
        "wild creatures should be left behind, not carried through the portal"
    );
    assert!(
        game.world.get::<Structure>(home).is_some(),
        "the base travels through the breach with the player"
    );
    let companion_pos = *game.world.get::<Position>(companion).unwrap();
    let player_pos = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        companion_pos, player_pos,
        "the companion should travel with the player into the new zone"
    );
}

#[test]
fn breaching_wipes_the_currency_and_craft_currency_stacks() {
    let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from(ids::PORTAL_FRAGMENT), 25);
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 40);
        inv.add(ItemId::from(ids::CREDITS), 30);
    }

    game.enter_next_zone();

    assert_eq!(
        count_item(&game, ids::PORTAL_FRAGMENT),
        0,
        "the next zone's portal has to be funded in the zone you leave from"
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        0,
        "and so does everything the base is bought with"
    );
    assert_eq!(
        count_item(&game, ids::CREDITS),
        30,
        "Credits are the one liquid thing that crosses a breach — selling \
         a doomed stockpile before you go is the point of a trader"
    );
}

#[test]
fn breaching_keeps_everything_that_is_not_spendable_currency() {
    let mut game = Game::new(946, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from(ids::RESEARCH_DATA), 60);
        inv.add(ItemId::from(ids::POWER_CELL), 4);
    }
    game.world
        .get_mut::<ItemFusions>(player)
        .unwrap()
        .increment(ItemId::from(ids::ICE_BREAKER));

    game.enter_next_zone();

    assert_eq!(
        count_item(&game, ids::RESEARCH_DATA),
        60,
        "banked research is progress, not pocket money"
    );
    assert_eq!(
        count_item(&game, ids::POWER_CELL),
        7,
        "3 from the starting kit plus the 4 added; supplies are carried, not confiscated"
    );
    assert_eq!(
        count_item(&game, ids::ICE_BREAKER),
        3,
        "the starting kit's catalysts make the trip too"
    );
    assert_eq!(
        game.world
            .get::<ItemFusions>(player)
            .unwrap()
            .tier(&ItemId::from(ids::ICE_BREAKER)),
        1,
        "fusion progress is not currency"
    );
}

/// A `FieldBuff` is player state, not zone-local — the inverse of the
/// `BuybackLedger` trap, where anything zone-local has to be wiped by name
/// in `enter_next_zone`. Nothing should be wiping this one, so a breach
/// must leave it running untouched.
#[test]
fn breaching_keeps_a_running_field_buff() {
    let mut game = Game::new(948, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::XpBoost,
            name: "Overclock Protocol".to_string(),
            power: 20,
            remaining: 9,
            source: BuffSource::Consumable,
        },
    );

    game.enter_next_zone();

    let buff = game
        .world
        .get::<FieldBuff>(player)
        .unwrap()
        .active
        .first()
        .cloned()
        .expect("a field buff is player state, not zone-local — it must survive a breach");
    assert_eq!(buff.kind, FieldBuffKind::XpBoost);
    assert_eq!(
        buff.remaining, 9,
        "a breach must not itself age or reset a buff"
    );
}

#[test]
fn the_decohere_message_only_fires_when_there_was_something_to_lose() {
    let mut game = Game::new(947, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), u32::MAX);

    game.enter_next_zone();

    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|(_, m)| m.contains("decohere")),
        "an empty wallet shouldn't be announced as a loss"
    );

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 3);
    game.enter_next_zone();

    // "{qty} {name}", the same unpluralized shape `describe_structure`
    // uses for a teleport cost — item names are modder-supplied data, not
    // English to inflect.
    assert!(
        game.message_log(20)
            .iter()
            .any(|(_, m)| m.contains("3 Portal Fragment")),
        "a real loss is named and counted: {:?}",
        game.message_log(20)
    );
}

#[test]
fn portal_cost_grows_by_half_the_base_rate_per_zone() {
    let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let portal = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "portal")
        .expect("portal.ron should load");
    let fragments = |game: &Game, def: &StructureDef| {
        game.structure_build_cost(def)
            .into_iter()
            .find(|(item, _)| item.as_str() == ids::PORTAL_FRAGMENT)
            .map(|(_, qty)| qty)
            .expect("a portal is bought with portal fragments")
    };

    assert_eq!(fragments(&game, &portal), 10, "zone 1 pays the base rate");

    game.world.insert_resource(ZoneLevel(2));
    assert_eq!(
        fragments(&game, &portal),
        15,
        "each zone adds half the base rate, not another whole one"
    );

    game.world.insert_resource(ZoneLevel(5));
    assert_eq!(
        fragments(&game, &portal),
        30,
        "the ramp stays linear in the base rate all the way down"
    );

    let node = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "mining_node")
        .expect("mining_node.ron should load");
    assert_eq!(
        game.structure_build_cost(&node),
        node.build_cost,
        "only a zone-portal structure scales; everything else is flat at any depth"
    );
}

#[test]
fn portal_build_cost_ramps_with_current_zone_level() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    place_home(&mut game, -1, 0);

    // Zone 1: base rate from portal.ron, 10 PortalFragment, unramped.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
    game.place_structure("portal", 1, 0).unwrap();
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "zone 1 portal should cost the base rate"
    );

    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);
    // The Home travelled through the breach with the rest of the base
    // (see `breaching_carries_every_structure_and_its_offset_from_home`),
    // so the new zone needs no fresh Home before building.

    // Zone 2: base rate plus half of it again (10 + 5 = 15), not double.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 14);
    assert!(
        game.place_structure("portal", 1, 0).is_err(),
        "14 fragments shouldn't be enough for a zone-2 portal"
    );
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 1);
    game.place_structure("portal", 1, 0).unwrap();
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "zone 2 portal should cost the base rate plus half again"
    );
}

#[test]
fn zone_level_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));
    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_zone_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.player_status().zone,
        2,
        "zone level should survive a save/load round trip"
    );
}

/// Regression test for a nearly-empty zone: `find_walkable_start`
/// always re-centers a freshly generated zone's spawn box near world
/// origin, and the terrain noise there has roughly the same period as
/// that box — so a blind, one-attempt-per-slot spawn (the previous
/// behavior of `spawn_initial_creatures`) could land almost all 14
/// rolls on an unwalkable or habitat-mismatched tile for an unlucky
/// seed, leaving the new zone feeling all but abandoned. Sweeps a
/// range of seeds (rather than trusting one lucky one) to confirm the
/// retry-until-`count` fix reliably delivers the full population.
#[test]
fn zone_transition_reliably_populates_the_new_zone_regardless_of_seed() {
    for seed in 0u32..20 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        // The zone-1 starting spawn can, for some seeds, happen to
        // place a wild creature right on the tile the portal is about
        // to go on — clear it so the walk onto the portal deterministically
        // enters the portal rather than picking a fight instead.
        let blockers: Vec<Entity> = {
            let mut query = game
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&game.world)
                .filter(|(_, p)| p.x == ppos.x + 1 && p.y == ppos.y)
                .map(|(e, _)| e)
                .collect()
        };
        for e in blockers {
            game.world.despawn(e);
        }
        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));
        game.move_player(1, 0);
        assert_eq!(
            game.player_status().zone,
            2,
            "seed {seed}: portal should advance the zone"
        );

        let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
        let count = query.iter(&game.world).count();
        assert!(
            count >= 14,
            "seed {seed}: zone 2 should have spawned at least the 14 requested wild \
             creatures, found {count}"
        );
    }
}

/// Nest provocation: `Game::attack_nest` marking guardians `Pursuing`, and
/// every path that removes a guardian from the world (a destroyed nest, a
/// tamed capture) removing the marker with it. Nothing moves a `Pursuing`
/// guardian yet — that's Task 4's `nest_aggro_tick`.
fn guardians_of(game: &mut Game, nest: Entity) -> Vec<Entity> {
    let mut query = game.world.query::<(Entity, &NestGuardian)>();
    query
        .iter(&game.world)
        .filter(|(_, g)| g.nest == nest)
        .map(|(e, _)| e)
        .collect()
}

#[test]
fn attacking_a_nest_provokes_only_its_own_guardians() {
    let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Far enough apart (tether radius 5) that the two guardian clusters
    // cannot possibly overlap — otherwise "no guardian of the other nest"
    // could pass by accident rather than by the provocation actually being
    // scoped to one nest.
    let nest_a = game.spawn_nest("scrapper", 100, 100);
    let nest_b = game.spawn_nest("scrapper", 300, 300);
    let guardians_a = guardians_of(&mut game, nest_a);
    let guardians_b = guardians_of(&mut game, nest_b);
    assert!(!guardians_a.is_empty(), "nest_a should have guardians");
    assert!(!guardians_b.is_empty(), "nest_b should have guardians");

    game.attack_nest(nest_a);

    for &guardian in &guardians_a {
        assert!(
            game.world.get::<Pursuing>(guardian).is_some(),
            "every guardian of the attacked nest should be provoked"
        );
    }
    for &guardian in &guardians_b {
        assert!(
            game.world.get::<Pursuing>(guardian).is_none(),
            "a guardian of an untouched nest must not be provoked"
        );
    }
}

#[test]
fn destroying_a_nest_clears_pursuing() {
    let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game.spawn_nest("scrapper", 110, 110);
    let guardians = guardians_of(&mut game, nest);
    assert!(!guardians.is_empty());

    game.provoke_nest(nest);
    for &guardian in &guardians {
        assert!(game.world.get::<Pursuing>(guardian).is_some());
    }

    game.world.get_mut::<Durability>(nest).unwrap().hp = 0;
    game.despawn_nest(nest);

    assert!(
        game.world.get::<Nest>(nest).is_none(),
        "the nest itself should be gone"
    );
    for &guardian in &guardians {
        assert!(
            game.world.get::<Pursuing>(guardian).is_none(),
            "a guardian of a destroyed nest must not still be marked pursuing"
        );
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "and must no longer be tethered to a dead nest"
        );
    }
}

#[test]
fn a_guardian_respawned_at_a_besieged_nest_is_already_pursuing() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Open ground the whole way, and — unlike this test before Task 4's
    // review — the nest sits inside the player's pursuit field rather
    // than off at an arbitrary far corner of the map. A guardian
    // `nest_aggro_tick` can never reach gives up on the spot (see the
    // "absent from the field" rule), which would strip the survivor's
    // `Pursuing` within the first tick or two and defeat this test's
    // premise long before the respawn timer ever fires.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=16 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }

    let nest_pos = Position {
        x: ppos.x + 15,
        y: ppos.y,
    };
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: vec![NEST_RESPAWN_TICKS],
            },
            nest_pos,
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();
    // A guardian still standing and already marked Pursuing, as an
    // attack_nest hit would leave it — this is what makes the nest
    // "besieged" for nest_has_pursuers, independent of the respawn queue.
    // Fifteen tiles out: comfortably inside the field's reach, but far
    // enough that ten ticks of closing (one tile each) never brings it to
    // adjacency and starts a fight, which would only complicate what this
    // test is actually checking.
    let survivor = spawn_pursuing_guardian(&mut game, nest, "scrapper", nest_pos.x, nest_pos.y);

    for _ in 0..NEST_RESPAWN_TICKS {
        game.tick();
    }

    let new_guardian = guardians_of(&mut game, nest)
        .into_iter()
        .find(|&e| e != survivor)
        .expect("a replacement guardian should have spawned");
    assert!(
        game.world.get::<Pursuing>(new_guardian).is_some(),
        "a guardian respawned at a besieged nest should already be pursuing"
    );
}

#[test]
fn a_guardian_respawned_at_a_calm_nest_is_not_pursuing() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: vec![NEST_RESPAWN_TICKS],
            },
            Position { x: 130, y: 130 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();

    for _ in 0..NEST_RESPAWN_TICKS {
        game.tick();
    }

    let new_guardian = guardians_of(&mut game, nest)
        .into_iter()
        .next()
        .expect("a replacement guardian should have spawned");
    assert!(
        game.world.get::<Pursuing>(new_guardian).is_none(),
        "a guardian respawned at a calm nest should not be pursuing"
    );
}

#[test]
fn a_pursuing_guardian_does_not_also_wander() {
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = spawn_bare_nest(&mut game, 140, 140);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", 141, 140);

    // Freeze `nest_aggro_tick` itself with an unrelated active battle, so
    // `Pursuing` survives genuinely across every tick below rather than by
    // a distance trick — `nest_aggro_tick` no longer leaves a far-off
    // guardian frozen-but-still-`Pursuing` indefinitely (see the "absent
    // from the field" rule this task's review added: such a guardian now
    // gives up immediately instead), so a real battle is the only thing
    // left that can hold this state open long enough to prove
    // `wander_ai_system`'s own `Without<Pursuing>` filter is what's
    // keeping the guardian still, not a side effect of `nest_aggro_tick`'s
    // own guard.
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    let before = *game.world.get::<Position>(guardian).unwrap();
    for _ in 0..10 {
        game.tick();
    }
    let after = *game.world.get::<Position>(guardian).unwrap();

    assert_eq!(
        before, after,
        "wander_ai_system must exclude a Pursuing guardian even while nest_aggro_tick is \
         separately frozen by the battle — otherwise the two systems could double-move it \
         once nest_aggro_tick resumes"
    );
    assert!(
        game.world.get::<Pursuing>(guardian).is_some(),
        "the guardian should still be genuinely Pursuing throughout — that's what this test \
         is checking wander's exclusion against"
    );
}

#[test]
fn decompiling_a_pursuing_guardian_strips_the_marker() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 150, y: 150 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();
    let guardian = game
        .world
        .spawn((
            Creature {
                species: "scrapper".to_string(),
            },
            Hostile,
            WanderAi::default(),
            NestGuardian { nest },
            Pursuing,
            Position { x: 151, y: 150 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![guardian]);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ICE_BREAKER), 50);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..50 {
        if game.world.get::<Tamed>(guardian).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }

    assert!(
        game.world.get::<Tamed>(guardian).is_some(),
        "the capture roll should land within 50 attempts at this skill/potency"
    );
    assert!(
        game.world.get::<Pursuing>(guardian).is_none(),
        "taming a pursuing guardian should strip the Pursuing marker along with NestGuardian"
    );
}

/// The per-tick pursuit step (`Game::nest_aggro_tick`) that wires
/// `Pursuing` (provocation) to `pursuit_field` (routing) together — this
/// is the half of nest aggression that actually moves a guardian and
/// starts a fight.
fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[test]
fn a_pursuer_closes_on_the_player_each_tick() {
    let mut game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // A generous hand-carved lane rather than hoping procedurally
    // generated terrain happens to be open between the two points.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=12 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }
    assert!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(ppos.x + 10, ppos.y)
            .walkable,
        "the lane precondition this test depends on"
    );

    let nest = spawn_bare_nest(&mut game, ppos.x + 10, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 10, ppos.y);

    let before = chebyshev(ppos, *game.world.get::<Position>(guardian).unwrap());
    game.tick();
    let after = chebyshev(ppos, *game.world.get::<Position>(guardian).unwrap());

    assert_eq!(
        before - after,
        NEST_PURSUIT_STEPS_PER_TICK as i32,
        "a pursuer should close on the player by exactly NEST_PURSUIT_STEPS_PER_TICK each tick"
    );
}

#[test]
fn a_pursuer_that_reaches_the_player_starts_a_battle() {
    let mut game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=4 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }

    let nest = spawn_bare_nest(&mut game, ppos.x + 2, ppos.y);
    spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 2, ppos.y);

    assert!(!game.has_active_battle());
    game.tick();
    assert!(
        game.has_active_battle(),
        "a pursuer that reaches the player should start a battle within the tick it arrives"
    );
}

#[test]
fn the_battle_a_pursuer_starts_includes_its_packmates() {
    let mut game = Game::new(712, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Both `max_group_size` and `max_enemy_groups` are 1 right at the
    // danger origin (by design — see `Game::max_group_size`'s doc), which
    // would truncate this fight back down to a single member regardless of
    // how the pack was gathered. `multi_group_ground` is ground far enough
    // out that a full `MAX_ENEMY_GROUPS` fight is allowed there.
    let (gx, gy) = multi_group_ground(&game);
    let player = game.player_entity();
    *game.world.get_mut::<Position>(player).unwrap() = Position { x: gx, y: gy };
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=2 {
            for dy in -2..=2 {
                map.set_override(
                    gx + dx,
                    gy + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }

    let nest = spawn_bare_nest(&mut game, gx, gy);
    // Two different species so a single-species group cap can't be the
    // reason both end up in the fight — each gets its own group, and
    // `multi_group_ground` guarantees more than one group is allowed here.
    spawn_pursuing_guardian(&mut game, nest, "scrapper", gx + 1, gy);
    spawn_pursuing_guardian(&mut game, nest, "wraith", gx + 1, gy + 1);

    game.tick();

    assert!(game.has_active_battle());
    let total_members: usize = game
        .battle_view()
        .unwrap()
        .groups
        .iter()
        .map(|g| g.count)
        .sum();
    assert!(
        total_members > 1,
        "the battle a pursuer starts should pull in the packmate standing beside it \
         (gather_pack), not just the one that reached the player; found {total_members}"
    );
}

#[test]
fn a_pursuer_beyond_the_leash_gives_up() {
    let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = spawn_bare_nest(&mut game, 200, 200);
    let start = Position {
        x: 200 + NEST_AGGRO_LEASH_RADIUS + 1,
        y: 200,
    };
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", start.x, start.y);

    game.tick();

    assert!(
        game.world.get::<Pursuing>(guardian).is_none(),
        "a pursuer past NEST_AGGRO_LEASH_RADIUS from its own nest should give up"
    );
    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        start,
        "giving up on the leash must not also have taken a step toward the player — \
         the leash check runs before the field is even built"
    );
}

#[test]
fn pursuers_never_step_onto_the_base_platform() {
    let mut game = Game::new(714, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    let half = MAX_BUILD_DISTANCE_FROM_HOME;

    // Open ground generous enough that a detour around the platform's edge
    // has somewhere to go, sized off `half` rather than a guessed constant
    // so it stays correct if the build radius ever changes.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -(half + 4)..=(half * 2 + 10) {
            for dy in -(half + 10)..=(half + 10) {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }
    // Stamped after the walkable override (so Platform wins inside the
    // slab) but before the nest and pursuer are placed — stamp_platform
    // despawns every Hostile and Nest standing inside it. Sits east of the
    // player, not on top of it: `half - 3` would have put the player's own
    // tile inside the slab.
    let platform_center = (ppos.x + half + 1, ppos.y);
    game.stamp_platform(platform_center.0, platform_center.1);

    // Wall off the southern detour entirely, so the only way around the
    // slab is north — where the nest sits — instead of leaving dijkstra a
    // coin-flip between two equally short routes. The first version of
    // this test left both open: the field routed the pursuer south half
    // the time, which pulled it more than `NEST_AGGRO_LEASH_RADIUS` from a
    // nest placed north of the slab, stripped `Pursuing`, and handed it to
    // `wander_ai_system` — which has no Platform exclusion — long before it
    // ever reached the player.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -(half + 4)..=(half * 2 + 10) {
            for dy in -(half + 10)..=-(half + 1) {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::DataVoid,
                        walkable: false,
                    },
                );
            }
        }
    }

    // The pursuer sits on the far side of the slab, directly in line with
    // the player, so the straight route is blocked and reaching adjacency
    // at all means detouring around the platform's edge — north, per the
    // wall above. The nest sits on that forced corridor rather than at the
    // pursuer's own tile: putting it there keeps both the start and the
    // final adjacent-to-player position within 8 of it (against a
    // 15-tile leash), so the leash can't be what stops the chase partway
    // through the one detour this test leaves available.
    let guardian_pos = (platform_center.0 + half + 1, ppos.y);
    let nest_pos = (platform_center.0, ppos.y + half + 1);
    let nest = spawn_bare_nest(&mut game, nest_pos.0, nest_pos.1);
    let guardian =
        spawn_pursuing_guardian(&mut game, nest, "scrapper", guardian_pos.0, guardian_pos.1);

    for _ in 0..80 {
        if game.has_active_battle() {
            break;
        }
        game.tick();
        let pos = *game.world.get::<Position>(guardian).unwrap();
        let biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        assert_ne!(
            biome,
            Biome::Platform,
            "a pursuer must never step onto the base platform, but stands at {pos:?}"
        );
    }

    assert!(
        game.has_active_battle(),
        "the pursuer should have detoured around the platform and reached the player within \
         80 ticks — if it never does, this test can't tell 'never stepped on Platform' apart \
         from 'never stepped at all'"
    );
}

#[test]
fn nest_aggro_tick_is_a_no_op_during_a_battle() {
    let mut game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // A walkable lane, the same as tests 1/2/5 carve — otherwise a
    // guardian standing on unwalkable natural terrain would already be
    // absent from the field and wouldn't have moved even without the
    // battle guard, and this test would pass for the wrong reason.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=4 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }

    // Close enough that, absent the battle guard, it would engage or at
    // least step this very tick — so this test would actually catch a
    // missing `has_active_battle` check rather than passing vacuously.
    let nest = spawn_bare_nest(&mut game, ppos.x + 3, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 2, ppos.y);
    let before = *game.world.get::<Position>(guardian).unwrap();

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    game.tick();

    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        before,
        "nest_aggro_tick must not move a pursuer while a battle is already running"
    );
}

#[test]
fn nest_aggro_tick_is_a_no_op_while_underground() {
    let mut game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Adjacent to the surface entrance tile. `Position` stays pinned there
    // for as long as the party is underground (see CLAUDE.md's
    // load-bearing-seams note on `Locale::Stack`), so this pursuer would
    // engage this very tick if `nest_aggro_tick` didn't know to leave that
    // surface `Position` alone while the party is four frames down.
    let nest = spawn_bare_nest(&mut game, ppos.x + 1, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 1, ppos.y);

    game.enter_stack(ppos.x, ppos.y);
    assert!(
        game.is_underground(),
        "test premise: the party must actually be underground"
    );

    game.tick();

    assert!(
        !game.has_active_battle(),
        "nest_aggro_tick must not fight the player's surface Position while the party is \
         underground"
    );
    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        Position {
            x: ppos.x + 1,
            y: ppos.y
        },
        "a pursuer must not move while the party is underground either"
    );
}

/// The nest cache: `Game::grant_nest_cache`, called from `attack_nest`'s
/// `destroyed` branch. Content comes entirely from the nest's `SpeciesDef` —
/// these tests lean on shipped species rather than hardcoded item ids
/// wherever the range rolls involved make that reliable, and mod in a single
/// throwaway species only where a shipped one's equipment chance is too low
/// to force deterministically.
fn one_shot_nest(game: &mut Game, nest: Entity) {
    let player = game.player_entity();
    // Comfortably above NEST_DURABILITY (60), so a single attack_nest call
    // is always lethal regardless of the player's own starting atk.
    game.world.get_mut::<Stats>(player).unwrap().atk = 1000;
    game.attack_nest(nest);
}

#[test]
fn destroying_a_nest_grants_its_species_work_resource() {
    let mut game = Game::new(720, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // scrapper is the can_nest species with a work_resource
    // (assets/species/scrapper.ron: power_cell) — wraith and trojan have
    // none, and using them would make this test vacuous.
    let resource = ItemId::from(ids::POWER_CELL);
    let nest = game.spawn_nest("scrapper", 400, 400);
    let before = held(&game, &resource);

    one_shot_nest(&mut game, nest);

    let after = held(&game, &resource);
    let minimum = NEST_CACHE_WORK_RESOURCE_MULT * WORK_RESOURCE_DROP.start();
    assert!(
        after >= before + minimum,
        "destroying the nest should have granted at least {minimum} power_cell, went from \
         {before} to {after}"
    );
}

#[test]
fn destroying_a_nest_grants_craft_currency() {
    let mut game = Game::new(721, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let currency = game.craft_currency();
    let nest = game.spawn_nest("scrapper", 410, 410);
    let before = held(&game, &currency);

    one_shot_nest(&mut game, nest);

    let after = held(&game, &currency);
    // Zone 1 (the default here), so NEST_CACHE_FRAGMENT_ZONE_BONUS
    // contributes nothing — the floor is the bare range roll.
    let minimum = *NEST_CACHE_FRAGMENTS.start();
    assert!(
        after >= before + minimum,
        "destroying the nest should have granted at least {minimum} craft currency, went from \
         {before} to {after}"
    );
}

#[test]
fn a_non_lethal_hit_on_a_nest_grants_nothing() {
    let mut game = Game::new(722, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game.spawn_nest("scrapper", 420, 420);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    game.attack_nest(nest);

    assert!(
        game.world.get::<Durability>(nest).unwrap().hp > 0,
        "test premise: a fresh player's ordinary hit must not one-shot NEST_DURABILITY"
    );
    let after = game.world.get::<Inventory>(player).unwrap().items.clone();
    assert_eq!(
        before, after,
        "a non-lethal hit on a nest must not grant any part of the cache"
    );
}

#[test]
fn a_deeper_zone_pays_a_larger_nest_cache() {
    let seed = 723;
    // Same seed, same species, same tile for both runs, so the two games
    // consume GameRng identically right up to the currency roll itself —
    // ZoneLevel doesn't change how many draws spawn_nest/spawn_wild_creature
    // make (see spawn_wild_creature_scaled), only the resulting stats. That
    // makes this a comparison of the same underlying roll plus a different
    // zone bonus, not two independent unseeded rolls.
    let currency_gained_at = |zone: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        let currency = game.craft_currency();
        let nest = game.spawn_nest("scrapper", 430, 430);
        let before = held(&game, &currency);
        one_shot_nest(&mut game, nest);
        held(&game, &currency) - before
    };

    let zone_1 = currency_gained_at(1);
    let zone_4 = currency_gained_at(4);
    let minimum_gap = 3 * NEST_CACHE_FRAGMENT_ZONE_BONUS;
    assert!(
        zone_4 >= zone_1 + minimum_gap,
        "a nest cleared at zone 4 should pay at least {minimum_gap} more craft currency than \
         the same nest at zone 1 (zone_1={zone_1}, zone_4={zone_4})"
    );
}

#[test]
fn destroying_a_nest_rolls_its_species_gear_table_repeatedly() {
    // No shipped can_nest species has an equipment chance high enough to
    // force a repeated-roll outcome without new RNG-forcing plumbing (see
    // the task brief's fallback clause), so this mods in one whose
    // equipment_drop chance is 1.0 — every one of NEST_CACHE_EQUIPMENT_ROLLS
    // passes then lands for certain, which is the only way to observe
    // ROLLS=3 differ from ROLLS=1 without touching the engine.
    let dir = modded_assets_dir(
        "nest_cache_gear",
        &[],
        &[],
        &[(
            "nest_cache_test.ron",
            r#"(
                id: "nest_cache_test",
                name: "Cache Test",
                glyph: 'X',
                color: Yellow,
                base_hp: 50,
                base_atk: 5,
                base_def: 2,
                taming_difficulty: 0.5,
                habitats: [],
                moves: [],
                work_resource: None,
                equipment_drop: Some(("shiv_routine", 1.0)),
                can_nest: true,
            )"#,
        )],
        &[],
        &[],
    );
    let mut game = Game::new(724, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let gear = ItemId::from("shiv_routine");
    let nest = game.spawn_nest("nest_cache_test", 440, 440);
    let before = held(&game, &gear);

    one_shot_nest(&mut game, nest);

    let after = held(&game, &gear);
    assert!(
        after >= before + 2,
        "a guaranteed gear roll repeated NEST_CACHE_EQUIPMENT_ROLLS times should yield more \
         than one copy of it — the only observable difference from a single pass — went from \
         {before} to {after}"
    );
}

#[test]
fn the_cache_lines_are_loot_kind() {
    let mut game = Game::new(725, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // scrapper's work_resource guarantees the resource-drop cache line lands
    // even on an unseeded roll (WORK_RESOURCE_DROP's minimum is > 0 and the
    // Buffer is uncapped), so this doesn't need to force any range roll —
    // unlike the gear-table test above, which does.
    let nest = game.spawn_nest("scrapper", 450, 450);

    one_shot_nest(&mut game, nest);

    let log = game.message_log(20);
    let cache_lines: Vec<_> = log
        .iter()
        .filter(|(_, text)| text.contains("wreckage") || text.contains("cache"))
        .collect();
    assert!(
        !cache_lines.is_empty(),
        "test premise: a nest cache line should have been logged, got: {log:?}"
    );
    for (kind, text) in &cache_lines {
        assert_eq!(
            *kind,
            MessageKind::Loot,
            "cache line {text:?} must be MessageKind::Loot so it survives \
             retain_outcomes_since_battle and follows the player onto the map, not \
             MessageKind::Info which would be pruned when the swarm fight ends"
        );
    }
}
