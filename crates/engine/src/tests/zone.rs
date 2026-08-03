//! Zone depth scaling and what survives a breach into the next zone.

use super::support::*;
use crate::tuning::{
    DISTANCE_STAT_STEP_TILES, GROUP_SIZE_STEP_TILES, MAX_BUILD_DISTANCE_FROM_HOME,
    MAX_DISTANCE_STAT_MULTIPLIER, MAX_ENEMY_GROUPS, MAX_GROUP_SIZE,
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

#[test]
fn zone_group_cap_is_geometric_and_never_passes_max_group_size() {
    use crate::game::spawning::zone_group_cap;
    assert_eq!(
        zone_group_cap(1),
        1,
        "zone 1 is solo, whatever else is true"
    );
    assert_eq!(zone_group_cap(2), 3);
    assert_eq!(zone_group_cap(3), 9);
    assert_eq!(zone_group_cap(4), 27);
    assert_eq!(zone_group_cap(5), 81);
    assert_eq!(
        zone_group_cap(6),
        MAX_GROUP_SIZE,
        "3^5 is 243, so zone 6 clamps"
    );
    assert_eq!(
        zone_group_cap(99),
        MAX_GROUP_SIZE,
        "a deep zone must clamp rather than overflow the pow"
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

    // Zone 2, not zone 1: a cap of 3 leaves the first two doublings visible,
    // so the boundary case below is measuring the division and not the clamp.
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    assert_eq!(at(&game, 0), 1, "every zone starts solo at its entry point");
    assert_eq!(
        game.max_group_size(spawn.x + GROUP_SIZE_STEP_TILES - 1, spawn.y, None),
        1,
        "one tile short of a full step is still solo — the doubling is keyed \
         to whole steps, and an off-by-one here would double a step early"
    );
    assert_eq!(at(&game, 1), 2, "one step out doubles");
    assert_eq!(
        at(&game, 2),
        3,
        "two steps would be 4, but zone 2 caps at 3"
    );
    assert_eq!(at(&game, 10), 3, "and it stays capped however far out");

    game.world.resource_mut::<ZoneLevel>().0 = 5;
    assert_eq!(
        at(&game, 6),
        64,
        "six steps is 2^6, still under zone 5's cap of 81"
    );
    assert_eq!(
        at(&game, 7),
        81,
        "seven steps would be 128, so the zone cap binds"
    );

    game.world.resource_mut::<ZoneLevel>().0 = 6;
    assert_eq!(
        at(&game, 7),
        MAX_GROUP_SIZE,
        "zone 6 is where the hard ceiling is reachable"
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
