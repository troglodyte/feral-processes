//! Placing, removing, upgrading, and describing structures, and the base platform they sit on.

use super::support::*;
use crate::tuning::MAX_BUILD_DISTANCE_FROM_HOME;
use crate::*;

#[test]
fn placing_a_home_stamps_a_walkable_platform_across_the_build_radius() {
    let mut game = Game::new(920, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 1);
    let (hx, hy) = (ppos.x, ppos.y + 1);

    let mut map = game.world.resource_mut::<WorldMap>();
    for (dx, dy) in [
        (0, 0),
        (MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
        (-MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
    ] {
        let tile = map.tile(hx + dx, hy + dy);
        assert_eq!(
            tile.biome,
            Biome::Platform,
            "({dx}, {dy}) from Home should be platform floor"
        );
        assert!(tile.walkable, "platform floor must always be walkable");
    }
    assert_ne!(
        map.tile(hx + MAX_BUILD_DISTANCE_FROM_HOME + 1, hy).biome,
        Biome::Platform,
        "one tile past the build radius should still be natural terrain"
    );
}

#[test]
fn placing_a_home_obliterates_hostiles_and_nests_inside_the_radius_only() {
    let mut game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let inside = game
        .world
        .spawn((
            Hostile,
            Position {
                x: ppos.x + 3,
                y: ppos.y + 3,
            },
        ))
        .id();
    let outside = game
        .world
        .spawn((
            Hostile,
            Position {
                x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 2,
                y: ppos.y,
            },
        ))
        .id();
    let nest_inside = game
        .world
        .spawn((
            Nest {
                species: "sprite".to_string(),
                pending_respawns: Vec::new(),
            },
            Position {
                x: ppos.x - 2,
                y: ppos.y + 1,
            },
        ))
        .id();

    place_home(&mut game, 0, 0);

    assert!(
        game.world.get_entity(inside).is_err(),
        "a hostile inside the radius is obliterated"
    );
    assert!(
        game.world.get_entity(nest_inside).is_err(),
        "a nest inside the radius is obliterated"
    );
    assert!(
        game.world.get_entity(outside).is_ok(),
        "a hostile outside the radius survives"
    );
}

#[test]
fn obliterating_a_nest_untethers_a_guardian_standing_outside_the_radius() {
    let mut game = Game::new(922, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let nest = game
        .world
        .spawn((
            Nest {
                species: "sprite".to_string(),
                pending_respawns: Vec::new(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ))
        .id();
    let guardian = game
        .world
        .spawn((
            NestGuardian { nest },
            Position {
                x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 3,
                y: ppos.y,
            },
        ))
        .id();

    place_home(&mut game, 0, 0);

    assert!(
        game.world.get::<NestGuardian>(guardian).is_none(),
        "a guardian outside the slab must lose its tether when its nest is obliterated, \
         not keep pointing at a despawned entity"
    );
}

#[test]
fn demolishing_the_home_clears_the_platform_back_to_natural_terrain() {
    let mut game = Game::new(923, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 1);

    let home = find_structure_by_kind(&mut game, "home").expect("the Home should be deployed");
    game.remove_structure(home).unwrap();

    assert_ne!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(ppos.x, ppos.y + 1)
            .biome,
        Biome::Platform,
        "demolishing the Home should leave no orphan sanctuary behind"
    );
    assert!(
        game.world.resource::<Platform>().center.is_none(),
        "the platform resource should forget its center once the Home is gone"
    );
}

#[test]
fn no_wild_creature_ever_spawns_on_platform_floor() {
    let mut game = Game::new(924, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 0);

    for _ in 0..400 {
        game.try_spawn_habitat_creature(ppos.x + 2, ppos.y + 2);
    }

    // Counted by position, not as a global Hostile tally: Game::new
    // seeds the zone with wild programs and only those inside the build
    // radius are obliterated, so survivors further out are expected and
    // have nothing to do with what this test is asserting.
    let on_platform = {
        let mut query = game.world.query_filtered::<&Position, With<Hostile>>();
        let positions: Vec<Position> = query.iter(&game.world).copied().collect();
        let mut map = game.world.resource_mut::<WorldMap>();
        positions
            .iter()
            .filter(|p| map.tile(p.x, p.y).biome == Biome::Platform)
            .count()
    };
    assert_eq!(
        on_platform, 0,
        "platform floor has no habitat species, so nothing can spawn on it"
    );
}

#[test]
fn place_structure_rejects_anything_but_home_until_a_home_exists() {
    let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);

    assert!(
        game.place_structure("armory", 1, 0).is_err(),
        "nothing should be buildable before a Home exists"
    );
    assert_eq!(
        game.view_entities(10, 10)
            .into_iter()
            .filter(|e| e.is_structure)
            .count(),
        0,
        "the rejected placement shouldn't have spawned anything"
    );

    game.place_structure("home", -1, 0).unwrap();
    game.place_structure("armory", 1, 0).unwrap();
    assert_eq!(
        game.view_entities(10, 10)
            .into_iter()
            .filter(|e| e.is_structure)
            .count(),
        2,
        "once a Home exists, other structures should be buildable"
    );
}

#[test]
fn place_structure_rejects_a_second_home() {
    let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);

    let err = game
        .place_structure("home", 1, 0)
        .expect_err("a second Home shouldn't be buildable while one already exists");
    assert!(err.contains("already deployed"), "unexpected error: {err}");
}

#[test]
fn place_structure_rejects_building_beyond_max_distance_from_home() {
    let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);
    place_home(&mut game, 0, 1);

    // Walk far enough away that the next placement lands outside the
    // build radius from Home.
    game.world.get_mut::<Position>(player).unwrap().x += 20;
    let err = game
        .place_structure("armory", 1, 0)
        .expect_err("structures beyond MAX_BUILD_DISTANCE_FROM_HOME shouldn't be buildable");
    assert!(err.contains("Too far from Home"), "unexpected error: {err}");

    // Walking back within range should make it buildable again.
    game.world.get_mut::<Position>(player).unwrap().x -= 20;
    game.place_structure("armory", 1, 0)
        .expect("building back within range of Home should succeed");
}

#[test]
fn remove_structure_refunds_a_percentage_of_its_build_cost() {
    let mut game = Game::new(303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);
    place_home(&mut game, -1, 0);
    game.place_structure("armory", 1, 0).unwrap();
    let armory = game
        .view_entities(10, 10)
        .into_iter()
        .find(|e| e.is_structure && !e.is_home)
        .unwrap()
        .entity;

    let before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));
    game.remove_structure(armory).unwrap();
    let after = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));

    assert!(
        after > before,
        "demolishing a structure should refund some of its build cost"
    );
    assert_eq!(
        game.view_entities(10, 10)
            .into_iter()
            .filter(|e| e.is_structure)
            .count(),
        1,
        "only the Home should remain after demolishing the armory"
    );
}

#[test]
fn removing_home_cascades_to_destroy_every_other_structure_and_refunds_each() {
    let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    unlock_research_chain(&mut game, "weapon_bench");
    let player = game.player_entity();
    // Just enough Core Fragments to afford Home + armory + fabricator
    // and no more: a big surplus (as a naive "plenty of buffer" amount
    // would be) leaves cargo sitting at or above capacity once combined
    // with starting gear, which would clamp the refund this test exists
    // to check — see `removing_home_cascade_refund_is_capped_to_available_room`
    // for that clamping behavior instead.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 31);
    place_home(&mut game, -1, 0);
    game.place_structure("armory", 1, 0).unwrap();
    game.place_structure("fabricator", 0, 1).unwrap();
    let home = game
        .view_entities(10, 10)
        .into_iter()
        .find(|e| e.is_home)
        .unwrap()
        .entity;

    let before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));
    game.remove_structure(home).unwrap();
    let after = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));

    assert_eq!(
        game.view_entities(10, 10)
            .into_iter()
            .filter(|e| e.is_structure)
            .count(),
        0,
        "removing Home should cascade to remove every other structure too"
    );
    assert!(
        after > before,
        "the cascade should refund a share of every demolished structure's cost, including Home's own"
    );
}

#[test]
fn armory_and_fabricator_are_not_cronjob_workable() {
    let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for id in ["armory", "fabricator"] {
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap_or_else(|| panic!("{id}.ron should load as a structure"));
        assert!(
            def.work.is_none(),
            "{id} should unlock crafting instead of being cronjob-workable"
        );
    }
}

#[test]
fn researching_and_building_an_armory_unlocks_firewall_plating() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "firewall");
    assert!(
        game.craft_recipes()
            .iter()
            .all(|r| r.result != ItemId::from(ids::FIREWALL_PLATING)),
        "Firewall Plating shouldn't be craftable before an Armory is built"
    );

    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 18);
    game.place_structure("armory", 1, 0).unwrap();

    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == ItemId::from(ids::FIREWALL_PLATING))
        .expect("researching it and building an Armory should unlock the recipe");
    assert_eq!(recipe.cost, vec![(ItemId::from(ids::PORTAL_FRAGMENT), 6)]);

    // Exactly the recipe's cost (6), not a padded amount: any excess
    // pushes cargo over the inventory cap and the compile is refused.
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 6);
    game.craft(&ItemId::from(ids::FIREWALL_PLATING), 1).unwrap();
    assert_eq!(
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&ItemId::from(ids::FIREWALL_PLATING)),
        1
    );
}

#[test]
fn cronjob_assignment_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(6, DifficultyMode::Forgiving, &assets).unwrap();

    let structure_def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.work.is_some())
        .expect("at least one workable structure should exist");
    let structure = game
        .world
        .spawn((
            Structure {
                kind: structure_def.id.clone(),
            },
            Position { x: 3, y: 3 },
            ResourceNode {
                resource: structure_def.work.as_ref().unwrap().produces.clone(),
                amount: 20,
                capacity: 20,
                level: None,
            },
        ))
        .id();

    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let player = game.player_entity();
    game.world.spawn((
        Creature {
            species: species.id.clone(),
        },
        Position { x: 3, y: 4 },
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 1,
            def: 1,
        },
        Tamed { owner: player },
        Experience::default(),
        Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 3,
            required: 6,
        },
    ));

    let path = std::env::temp_dir().join(format!(
        "feral_processes_cronjob_test_{}_{}.bin",
        std::process::id(),
        6
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<&Task>();
    let task = query
        .iter(&loaded.world)
        .next()
        .expect("restored creature should still have its cronjob task");
    assert_eq!(task.progress, 3);
    assert_eq!(task.required, 6);
    let target_pos = loaded
        .world
        .get::<Position>(task.target)
        .expect("task target should resolve to a structure entity");
    assert_eq!((target_pos.x, target_pos.y), (3, 3));
}

#[test]
fn a_mined_out_node_refills_instead_of_stalling_the_cronjob() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 1,
                capacity: 2,
                level: None,
            },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    // First tick mines the last unit down to 0.
    game.tick();
    assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 0);

    // The node refills to capacity on the next tick rather than
    // leaving the assigned creature permanently idle.
    game.tick();
    assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 1);
    assert!(
        game.world.get::<Task>(worker).is_some(),
        "the cronjob should keep running once the node refills"
    );
}

#[test]
fn cronjob_work_grants_no_more_xp_once_the_worker_hits_the_work_level_cap() {
    let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Experience>(worker).unwrap().level = crate::tuning::WORK_XP_LEVEL_CAP;
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    for _ in 0..3 {
        game.tick();
    }

    let exp = game.world.get::<Experience>(worker).unwrap();
    assert_eq!(
        exp.level,
        crate::tuning::WORK_XP_LEVEL_CAP,
        "a capped worker shouldn't level further from cronjob work"
    );
    assert_eq!(
        exp.xp, 0,
        "a capped worker shouldn't earn any work XP at all"
    );
}

/// `WORK_XP_PER_CYCLE` is 5; 40% is chosen so the boosted result (7, from
/// `round(5 * 1.4)`) can't coincide with the unboosted one (5) through
/// rounding — a smaller boost percentage risks the two values landing on
/// the same integer and hiding a broken hookup behind a passing assertion.
#[test]
fn cronjob_work_xp_is_boosted_by_a_running_xp_boost_field_buff() {
    let mut unboosted = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut unboosted, 10, 3);
    let structure = unboosted
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ))
        .id();
    unboosted.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });
    unboosted.tick();
    let unboosted_xp = unboosted.world.get::<Experience>(worker).unwrap().xp;

    let mut boosted = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut boosted, 10, 3);
    let structure = boosted
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ))
        .id();
    boosted.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });
    let player = boosted.player_entity();
    boosted.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::XpBoost,
            name: "Test XP Boost".to_string(),
            power: 40,
            remaining: 10,
            source: BuffSource::Routine,
        }],
    });
    boosted.tick();
    let boosted_xp = boosted.world.get::<Experience>(worker).unwrap().xp;

    assert_eq!(
        unboosted_xp, 5,
        "an unboosted cycle earns WORK_XP_PER_CYCLE"
    );
    assert_eq!(
        boosted_xp, 7,
        "a 40% XpBoost should turn WORK_XP_PER_CYCLE (5) into 7 for a companion's own cronjob income"
    );
}

#[test]
fn cronjob_work_still_grants_xp_below_the_work_level_cap() {
    let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    assert!(
        game.world.get::<Experience>(worker).unwrap().level < crate::tuning::WORK_XP_LEVEL_CAP,
        "a freshly tamed program should start well under the work level cap"
    );
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    game.tick();

    let xp = game.world.get::<Experience>(worker).unwrap().xp;
    assert!(xp > 0, "a worker under the cap should still earn work XP");
}

#[test]
fn a_leveled_node_doesnt_always_yield_on_a_completed_cycle() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 20,
                capacity: 20,
                level: Some(1),
            },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    let player = game.player_entity();
    let starting_fragments = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));

    for _ in 0..40 {
        game.tick();
    }

    let gained = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT))
        - starting_fragments;
    assert!(
        gained < 40,
        "a level-1 node succeeding on every single one of 40 cycles is implausible at ~50% odds, got {gained}"
    );
}

#[test]
fn recharger_node_loads_as_a_permanent_base_wide_power_source() {
    let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "recharger_node")
        .expect("recharger_node.ron should load");
    assert_eq!(def.build_cost, vec![(ItemId::from(ids::CORE_FRAGMENT), 10)]);
    let regen = def
        .power_regen
        .as_ref()
        .expect("the Recharger Node should regenerate Power");
    assert_eq!(regen.per_tick, 1.0);
    assert_eq!(
        regen.radius, MAX_BUILD_DISTANCE_FROM_HOME,
        "the Recharger Node should cover the whole base"
    );
    assert!(
        def.enables_rest.is_none(),
        "resting moved to Home; the Recharger Node is no longer a rest gate"
    );
    assert!(
        def.temporary.is_none(),
        "the Recharger Node should be a permanent structure"
    );
}

#[test]
fn a_recharger_node_in_range_nets_power_upward_on_a_real_tick() {
    let mut game = Game::new(403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    spawn_recharger_node(&mut game, 0, 0);

    game.wait();

    let hunger = game.world.get::<Needs>(player).unwrap().hunger;
    assert!(
        (hunger - 50.85).abs() < 1e-4,
        "expected +1.0 regen less 0.15 decay, got {hunger}"
    );
}

#[test]
fn a_recharger_node_past_the_base_footprint_does_not_reach_the_player() {
    let mut game = Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    spawn_recharger_node(&mut game, MAX_BUILD_DISTANCE_FROM_HOME + 1, 0);

    game.wait();

    let hunger = game.world.get::<Needs>(player).unwrap().hunger;
    assert!(
        (hunger - 49.85).abs() < 1e-4,
        "expected decay only, got {hunger}"
    );
}

#[test]
fn reaching_a_recharger_node_while_drained_costs_no_integrity() {
    let mut game = Game::new(405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 0.1;
    let before = *game.world.get::<Stats>(player).unwrap();
    spawn_recharger_node(&mut game, 0, 0);

    game.wait();

    let after = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        after.hp, before.hp,
        "regen runs before decay, so arriving drained must not cost Integrity"
    );
}

#[test]
fn upgrading_a_node_costs_materials_and_raises_its_tier() {
    let mut game = Game::new(970, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);

    assert_eq!(
        game.world.get::<StructureTier>(node).unwrap().0,
        1,
        "structures deploy at Mk1"
    );
    let before = count_item(&game, ids::CORE_FRAGMENT);

    game.upgrade_structure(node).unwrap();

    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);
    assert_eq!(
        before - count_item(&game, ids::CORE_FRAGMENT),
        20,
        "reaching tier 2 costs the def's 10 per tier x 2"
    );
}

#[test]
fn upgrading_a_node_makes_its_extraction_more_reliable() {
    let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);

    assert_eq!(game.world.get::<ResourceNode>(node).unwrap().level, Some(1));
    game.upgrade_structure(node).unwrap();
    assert_eq!(
        game.world.get::<ResourceNode>(node).unwrap().level,
        Some(2),
        "tier feeds ResourceNode.level, which already drives mining_success_chance"
    );
}

#[test]
fn upgrading_refuses_past_max_tier_and_without_materials() {
    let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);

    let err = game
        .upgrade_structure(node)
        .expect_err("no materials left after building it");
    assert!(err.contains("Not enough"), "unexpected error: {err}");

    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);
    let max = game
        .world
        .resource::<StructureDb>()
        .get("mining_node")
        .unwrap()
        .upgrade
        .as_ref()
        .unwrap()
        .max_tier;
    for _ in 1..max {
        game.upgrade_structure(node).unwrap();
    }
    let err = game
        .upgrade_structure(node)
        .expect_err("a maxed node can't be upgraded further");
    assert!(err.contains("fully upgraded"), "unexpected error: {err}");
}

#[test]
fn a_structure_without_an_upgrade_def_cannot_be_upgraded() {
    let mut game = Game::new(973, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let home = find_structure_by_kind(&mut game, "home").unwrap();
    let err = game
        .upgrade_structure(home)
        .expect_err("Home declares no upgrade path");
    assert!(err.contains("can't be upgraded"), "unexpected error: {err}");
}

#[test]
fn tier_adds_to_payout_on_top_of_the_zone_bonus() {
    let mut game = Game::new(974, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3;

    let gained = run_one_full_gather_cycle_at_tier(&mut game, ids::CORE_FRAGMENT, Some(3));

    assert_eq!(
        gained, 5,
        "tier 3 plus two zones' worth of bonus — not the 12 the old \
         tier x zone-multiplier form paid"
    );
}

#[test]
fn a_structures_tier_survives_a_save_and_load_round_trip() {
    let mut game = Game::new(975, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.upgrade_structure(node).unwrap();
    game.upgrade_structure(node).unwrap();

    let path = std::env::temp_dir().join(format!("feral_tier_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = find_structure_by_kind(&mut loaded, "mining_node").unwrap();
    assert_eq!(
        loaded.world.get::<StructureTier>(restored).unwrap().0,
        3,
        "a Mk3 node must not come back as Mk1"
    );
    assert_eq!(
        loaded.world.get::<ResourceNode>(restored).unwrap().level,
        Some(3),
        "and its extraction reliability with it — WorkDef::level only carries the \
         tier-1 baseline"
    );
}

/// Depth pays, but on the economy's own linear curve — deliberately not the
/// `stat_multiplier` doubling that scales wild programs. Payout used to
/// borrow that curve, which is what let income outrun every sink in the game.
#[test]
fn a_worked_node_pays_out_more_the_deeper_the_zone() {
    let mut game = Game::new(960, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;

    let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

    assert_eq!(
        gained, 4,
        "a Mk1 node in zone 4 pays its tier plus three zones' bonus, not the \
         8x that zone's stat multiplier would give"
    );
}

#[test]
fn a_zone_one_node_still_pays_exactly_one() {
    let mut game = Game::new(962, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.world.resource::<ZoneLevel>().0,
        1,
        "runs start at zone 1"
    );

    let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

    assert_eq!(
        gained, 1,
        "zone 1's multiplier is 1 << 0 == 1, so the opening game is unchanged"
    );
}

#[test]
fn a_banked_resource_never_scales_with_zone_depth() {
    let mut game = Game::new(961, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 5;

    let gained = run_one_full_gather_cycle(&mut game, ids::RESEARCH_DATA);

    assert_eq!(
        gained, 1,
        "research_data has a bank_limit of 200 — scaling it would fill the bank in ~13 \
         cycles and turn the research economy into 'no room to store it' spam"
    );
}

/// Working a node yourself is the same job a cronjob runs, so it has to pay
/// the same `systems::node_payout` — a second, parallel payout formula for
/// the player is exactly the drift this repo has been bitten by before.
///
/// The node's reliability roll is switched off for the measurement so the
/// assertion is about the payout, not about `mining_success_chance`.
#[test]
fn working_a_node_yourself_pays_what_a_cronjob_pays() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    game.world.get_mut::<ResourceNode>(node).unwrap().level = None;
    let resource = game
        .world
        .get::<ResourceNode>(node)
        .unwrap()
        .resource
        .clone();
    let held = |game: &Game| {
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&resource)
    };
    let before = held(&game);

    game.work_structure(node)
        .expect("a deployed node is workable");

    let mut ticks = 0;
    while held(&game) == before && ticks < 40 {
        game.wait();
        ticks += 1;
    }

    let zone = *game.world.resource::<ZoneLevel>();
    let tier = game
        .world
        .get::<StructureTier>(node)
        .map(|t| t.0)
        .unwrap_or(1);
    assert_eq!(
        held(&game) - before,
        crate::systems::node_payout(tier, zone),
        "a cycle the player ran must pay exactly what the same cycle pays a worker"
    );
}

/// The job is something you are standing there doing, so stepping away ends
/// it rather than leaving it running unattended.
#[test]
fn walking_away_stops_the_job() {
    let mut game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();

    game.work_structure(node)
        .expect("a deployed node is workable");
    assert!(
        game.world.get::<Task>(player).is_some(),
        "starting the job should put the same Task on you a worker would carry"
    );

    game.move_player(1, 0);

    assert!(
        game.world.get::<Task>(player).is_none(),
        "walking away should end the job, not leave it running"
    );
}
