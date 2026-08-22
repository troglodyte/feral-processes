//! Placing, removing, upgrading, and describing structures, and the base platform they sit on.

use super::support::*;
use crate::tuning::{MAX_BUILD_DISTANCE_FROM_HOME, STARTING_POCKET_RADIUS, haul_walk_radius};
use crate::*;

/// The pocket is a chamfered box, not the square it would be without
/// `PLATFORM_CORNER_CUT`: that many diagonal steps come off each of the four
/// corners, so the corner cell and the two beside it are unmined rock.
/// Checked at all four corners because the shape is written in absolute
/// values and a sign error would round three of them and leave one square.
#[test]
fn the_starting_pocket_has_its_corners_cut() {
    let mut game = Game::new(925, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let r = STARTING_POCKET_RADIUS;

    let grid = game.world.resource::<crate::base_grid::BaseGrid>();
    for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        for (dx, dy) in [(r, r), (r - 1, r), (r, r - 1)] {
            let (dx, dy) = (dx * sx, dy * sy);
            assert!(
                grid.is_solid(dx, dy),
                "({dx}, {dy}) is inside the cut corner and should still be rock"
            );
        }
        // The cells the cut stops at, so a deeper chamfer can't pass by
        // asserting only on what was removed.
        for (dx, dy) in [(r - 2, r), (r - 1, r - 1), (r, r - 2)] {
            let (dx, dy) = (dx * sx, dy * sy);
            assert!(
                grid.is_floor(dx, dy),
                "({dx}, {dy}) is the first cell past the cut and should be laid floor"
            );
        }
    }
}

/// The cut is footprint, not paint: `place_structure` measures against the
/// same `BaseGrid::is_floor` the pocket was laid into, so a cell with no
/// floor under it has nothing standing on it either. Without this the build
/// box stays square and a machine can hang off the rounded corner into rock.
#[test]
fn a_cut_corner_is_not_buildable() {
    let mut game = Game::new(926, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 40);
    place_home(&mut game);
    let r = STARTING_POCKET_RADIUS;

    let err = game
        .place_structure("armory", r, r)
        .expect_err("the corner cell is off the floor and shouldn't be buildable");
    assert!(err.contains("no floor there"), "unexpected error: {err}");

    // Diagonally in by one, which is the first cell the chamfer leaves —
    // and the assertion that stops the cut being fixed by shrinking the
    // whole build box.
    game.place_structure("armory", r - 1, r - 1)
        .expect("the cell just inside the cut is floor and should be buildable");
}

/// A guardian can be standing anywhere when its nest goes, so
/// `Game::despawn_nest` untethers the whole brood rather than leaving one
/// pointing at a despawned entity. Driven through `despawn_nest` itself —
/// deploying a Home used to be what obliterated a nest, and does not touch
/// the zone surface any more.
#[test]
fn obliterating_a_nest_untethers_a_guardian_standing_far_from_it() {
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
                x: ppos.x + 20,
                y: ppos.y,
            },
        ))
        .id();

    game.despawn_nest(nest);

    assert!(
        game.world.get::<NestGuardian>(guardian).is_none(),
        "a guardian away from its nest must lose its tether when the nest is obliterated, \
         not keep pointing at a despawned entity"
    );
}

#[test]
fn no_wild_creature_ever_spawns_on_platform_floor() {
    let mut game = Game::new(924, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game);

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
    stand_in_base(&mut game);
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

    place_home(&mut game);
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
    stand_in_base(&mut game);
    place_home(&mut game);

    let err = game
        .place_structure("home", 1, 0)
        .expect_err("a second Home shouldn't be buildable while one already exists");
    assert!(err.contains("already deployed"), "unexpected error: {err}");
}

/// The footprint is the laid floor and nothing else — walking out to the
/// pocket's edge and pointing further takes the build off it.
#[test]
fn place_structure_rejects_building_off_the_pockets_floor() {
    let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);
    place_home(&mut game);

    // The pocket's east edge, so the next placement lands one cell into rock.
    stand_in_base_at(&mut game, STARTING_POCKET_RADIUS, 0);
    let err = game
        .place_structure("armory", 1, 0)
        .expect_err("a cell with no floor under it shouldn't be buildable");
    assert!(err.contains("no floor there"), "unexpected error: {err}");

    // Pointing back into the pocket makes it buildable again.
    game.place_structure("armory", -1, 0)
        .expect("building back onto laid floor should succeed");
}

/// A refusal for want of materials is the one build refusal the player has to
/// go and *do* something about, so it goes in the base log rather than living
/// only in the status line, which ages out after `STATUS_LINE_SECONDS` while
/// they are looking at the map. It names the shortfall for the same reason:
/// "not enough" without a number sends them back to the build menu to work
/// out what they were short of.
#[test]
fn deploying_without_the_materials_logs_the_shortfall() {
    let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    place_home(&mut game);

    let held = count_item(&game, ids::CORE_FRAGMENT);
    let err = game
        .place_structure("armory", 1, 0)
        .expect_err("an Armory costs far more than the starting kit holds");
    assert!(err.contains("Not enough"), "unexpected error: {err}");

    let last = game.message_log(1);
    assert_eq!(
        last[0].source,
        MessageSource::Base,
        "a build refusal is base news, not field news"
    );
    assert!(
        last[0].text.contains("Armory")
            && last[0].text.contains("18 Core Fragment")
            && last[0].text.contains(&format!("have {held}")),
        "the log line should name what was being deployed and how short it fell: {}",
        last[0].text
    );
}

/// Every menu that lists what is standing nearby indexes this scan, so its
/// order *is* their order. Bevy's query iteration is not stable, which left
/// those menus reshuffling between openings — a list you cannot learn the
/// shape of. Name first, then position, so two Mining Nodes still resolve to
/// a fixed order rather than swapping rows.
#[test]
fn the_nearby_scan_lists_entities_by_name_then_position() {
    let mut game = Game::new(305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("armory", 1, 0).unwrap();
    game.place_structure("mining_node", 2, 0).unwrap();
    game.place_structure("mining_node", -2, 0).unwrap();

    let listed: Vec<(String, (i32, i32))> = game
        .view_entities(10, 10)
        .into_iter()
        .filter(|e| e.is_structure)
        .map(|e| (e.label, e.pos))
        .collect();
    let mut expected = listed.clone();
    expected.sort();
    assert_eq!(
        listed, expected,
        "the scan must arrive sorted, not merely be sortable"
    );
    assert!(
        listed.iter().filter(|(l, _)| l == "Mining Node").count() == 2,
        "the fixture needs two of one kind for the position tiebreak to mean anything: {listed:?}"
    );
}

#[test]
fn remove_structure_refunds_a_percentage_of_its_build_cost() {
    let mut game = Game::new(303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);
    place_home(&mut game);
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
    stand_in_base(&mut game);
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
    place_home(&mut game);
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
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "firewall");
    assert!(
        game.craft_recipes()
            .iter()
            .all(|r| r.result != ItemId::from(ids::FIREWALL_PLATING)),
        "Firewall Plating shouldn't be craftable before an Armory is built"
    );

    place_home(&mut game);
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
    assert_eq!(
        recipe.cost,
        vec![
            (ItemId::from(ids::PORTAL_FRAGMENT), 6),
            (ItemId::from("cache_grain"), 2),
        ]
    );

    // Exactly the recipe's cost (6), not a padded amount: any excess
    // pushes cargo over the inventory cap and the compile is refused.
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 6);
    give(&mut game, &ItemId::from("cache_grain"), 2);
    game.craft(&ItemId::from(ids::FIREWALL_PLATING), 1, false)
        .unwrap();
    assert_eq!(
        held_any(&game, &ItemId::from(ids::FIREWALL_PLATING)),
        1,
        "both stores: a compiled piece of gear carries the quality it rolled, \
         so it only stacks in `Inventory` when that came out exactly at spec"
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
            mitigation: 1,
        },
        Tamed { owner: player },
        Experience::default(),
        PowerReserve::default(),
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
                level: None,
            },
            work_node_parts(),
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
    stand_ample_grid_supply(&mut unboosted);
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
                level: None,
            },
            work_node_parts(),
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
    stand_ample_grid_supply(&mut boosted);
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
                level: None,
            },
            work_node_parts(),
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
            interval: 1,
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
    stand_ample_grid_supply(&mut game);
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
                level: None,
            },
            work_node_parts(),
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
    assert!(
        regen.radius >= MAX_BUILD_DISTANCE_FROM_HOME,
        "the Recharger Node should at least cover the base it is first built on"
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
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    // `stand_in_base` + `spawn_structure_at`, not `spawn_recharger_node`:
    // `power_regen_system` reads `Locale::Base`'s own cell, never the
    // player's `Position`, so a fixture placing the Recharger at an offset
    // from `Position` would be testing the wrong coordinate space.
    stand_in_base(&mut game);
    spawn_structure_at(&mut game, "recharger_node", 0, 0);

    game.wait();

    let hunger = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        (hunger - 50.85).abs() < 1e-4,
        "expected +1.0 regen less 0.15 decay, got {hunger}"
    );
}

#[test]
fn a_recharger_node_past_the_base_footprint_does_not_reach_the_player() {
    let mut game = Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    let reach = recharger_reach(&game);
    stand_in_base(&mut game);
    spawn_structure_at(&mut game, "recharger_node", reach + 1, 0);

    game.wait();

    let hunger = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        (hunger - 49.85).abs() < 1e-4,
        "expected decay only, got {hunger}"
    );
}

#[test]
fn reaching_a_recharger_node_while_drained_costs_no_integrity() {
    let mut game = Game::new(405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(0.1);
    let before = *game.world.get::<Stats>(player).unwrap();
    stand_in_base(&mut game);
    spawn_structure_at(&mut game, "recharger_node", 0, 0);

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
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
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

    stock_upgrade_materials(&mut game, 20);

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
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);

    assert_eq!(game.world.get::<ResourceNode>(node).unwrap().level, Some(1));
    stock_upgrade_materials(&mut game, 20);
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
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    // Deep enough that the zone ceiling is out of the way: this test is about
    // the def's own `max_tier` and about materials, both of which are checked
    // after the zone gate.
    set_zone(&mut game, 9);

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
        stock_upgrade_materials(&mut game, 20);
        game.upgrade_structure(node).unwrap();
    }
    let err = game
        .upgrade_structure(node)
        .expect_err("a maxed node can't be upgraded further");
    assert!(err.contains("fully upgraded"), "unexpected error: {err}");
}

#[test]
fn upgrading_is_refused_until_you_have_breached_to_the_matching_zone() {
    let mut game = Game::new(978, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);

    let err = game
        .upgrade_structure(node)
        .expect_err("zone 1 caps every structure at Mk1");
    assert!(
        err.contains("zone 2"),
        "the refusal should name the zone that unlocks the next tier: {err}"
    );
    assert_eq!(
        game.world.get::<StructureTier>(node).unwrap().0,
        1,
        "a refused upgrade must not charge or advance the tier"
    );
}

#[test]
fn breaching_raises_the_upgrade_ceiling_one_tier() {
    let mut game = Game::new(979, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);

    set_zone(&mut game, 2);
    stock_upgrade_materials(&mut game, 20);
    game.upgrade_structure(node).unwrap();
    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);

    let err = game
        .upgrade_structure(node)
        .expect_err("zone 2 stops at Mk2");
    assert!(err.contains("zone 3"), "unexpected error: {err}");
}

#[test]
fn the_defs_max_tier_still_wins_in_a_deep_zone() {
    let mut game = Game::new(981, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);

    set_zone(&mut game, 9);
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
        stock_upgrade_materials(&mut game, 20);
        game.upgrade_structure(node).unwrap();
    }
    let err = game
        .upgrade_structure(node)
        .expect_err("a zone past the def's ceiling doesn't raise it");
    assert!(
        err.contains("fully upgraded"),
        "a permanent ceiling reads differently from a zone one: {err}"
    );
}

#[test]
fn a_structure_without_an_upgrade_def_cannot_be_upgraded() {
    let mut game = Game::new(973, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
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

    let gained =
        run_one_full_gather_cycle_at_tier(&mut game, "mining_node", ids::CORE_FRAGMENT, Some(3));

    assert_eq!(
        gained, 5,
        "tier 3 plus two zones' worth of bonus — not the 12 the old \
         tier x zone-multiplier form paid"
    );
}

#[test]
fn a_structures_tier_survives_a_save_and_load_round_trip() {
    let mut game = Game::new(975, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    set_zone(&mut game, 3);
    stock_upgrade_materials(&mut game, 20);
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
        "the research tree is a fixed ladder — cortex, its deepest node, costs 45 — so a \
         payout that doubled per zone would collapse the whole tree in a handful of \
         cycles rather than accelerate it"
    );
}

/// `flat_payout` keeps a node producing something consumed one at a time —
/// a taming catalyst, a key — off the payout curve written for bulk
/// salvage. Scaling one put a Mk5 in zone 5 at nine units a cycle,
/// guaranteed, against a demand of one per use.
///
/// Asserted against `resolve_gather_cycle` directly rather than through a
/// deployed node, because no shipped structure sets the flag any more: the
/// Compiler, which used to, now assembles its catalysts out of Core
/// Fragments pulled from a neighbour instead of printing them from nothing.
/// The field is mod-facing now, and a test routed through a real structure
/// would have to invent shipped content to have a subject —
/// `run_one_full_gather_cycle_at_tier` reads the flag off the `StructureDb`
/// by kind, so a made-up id would silently take the scaling branch and pass
/// for the wrong reason.
#[test]
fn flat_payout_takes_a_node_off_the_tier_and_depth_curve() {
    let mut game = Game::new(963, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // `level: None` always yields, which keeps this off the reliability roll
    // entirely — what is under test is the payout, not `mining_success_chance`.
    let node = ResourceNode {
        resource: ItemId::from(ids::CORE_FRAGMENT),
        level: None,
    };
    let tier = StructureTier(5);
    let cycle = |game: &mut Game, flat: bool| {
        game.world.resource_scope(|world, mut rng: Mut<GameRng>| {
            crate::systems::resolve_gather_cycle(
                &node,
                Some(&tier),
                ZoneLevel(5),
                flat,
                // `level: None` above skips the reliability roll, so neither
                // roll modifier has anything to act on here; the classless
                // worker is what makes this the ordinary payout curve.
                crate::systems::CycleModifiers {
                    keen_scavenger_level: 0,
                    base_int: crate::tuning::DEFAULT_BASE_INT,
                    class: None,
                },
                world.resource::<ItemDb>(),
                &mut rng,
            )
            .map(|(_, qty)| qty)
            .expect("a node that always yields never fizzles")
        })
    };

    assert_eq!(
        cycle(&mut game, true),
        1,
        "a flat-payout Mk5 in zone 5 pays one a cycle, not tier plus four zones' bonus"
    );
    assert!(
        cycle(&mut game, false) > 1,
        "and the ordinary curve it is opting out of really does scale, \
         or this test would pass with the flag ignored"
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
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    game.world.get_mut::<ResourceNode>(node).unwrap().level = None;
    let resource = game
        .world
        .get::<ResourceNode>(node)
        .unwrap()
        .resource
        .clone();
    // Measured in the node's own buffer: a cycle the player ran deposits
    // there too, so the deposit pool is not left as the only thing pacing
    // the one path that bypasses it.
    let held = |game: &Game| node_output(game, node, resource.as_str());
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
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();

    game.work_structure(node)
        .expect("a deployed node is workable");
    assert!(
        game.world.get::<Task>(player).is_some(),
        "starting the job should put the same Task on you a worker would carry"
    );

    // Somewhere to step *to*: base space is solid until something lays
    // floor, and a step into rock is refused rather than being a step at
    // all — which would make the assertion below pass for the wrong reason.
    // The starting pocket lands in slice-1 Task 6; until then a test that
    // walks in here lays its own ground.
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .lay_floor(1, 0);

    game.move_player(1, 0);

    assert!(
        game.world.get::<Task>(player).is_none(),
        "walking away should end the job, not leave it running"
    );
}

/// A cycle you run yourself pays into the *node's* buffer
/// (`player_gather_system`), and `c` reaches only the four orthogonal tiles
/// (`collect_adjacent`) — so a job started from across the base earns into a
/// buffer that is nowhere near you, and the extraction lines read as though
/// you were pocketing it. The work menu lists everything within
/// `MENU_SCAN_RADIUS`, so this is a refusal at the action rather than a
/// hidden row, the same shape as `assign_cronjob`'s walk check.
#[test]
fn working_a_node_you_are_not_standing_beside_is_refused() {
    let mut game = Game::new(952, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();
    let node_pos = *game.world.get::<Position>(node).unwrap();
    stand_in_base_at(&mut game, node_pos.x + 6, node_pos.y);

    let err = game
        .work_structure(node)
        .expect_err("a node you could not collect from must not be workable");

    assert!(err.contains("next to"), "unexpected refusal: {err}");
    assert!(
        game.world.get::<Task>(player).is_none(),
        "a refused job must leave no Task behind"
    );
}

/// The rule is `hauling::at_station`, not "roughly beside it": a diagonal
/// neighbour is the one tile that looks adjacent and is out of `c`'s reach,
/// which is exactly the buffer a player would never find.
#[test]
fn a_diagonal_neighbour_is_not_close_enough_to_work() {
    let mut game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();
    let node_pos = *game.world.get::<Position>(node).unwrap();

    stand_in_base_at(&mut game, node_pos.x + 1, node_pos.y + 1);
    assert!(
        game.work_structure(node).is_err(),
        "a diagonal is not a tile the node can be collected from"
    );

    stand_in_base_at(&mut game, node_pos.x + 1, node_pos.y);
    game.work_structure(node)
        .expect("standing on one of the node's four station tiles works it");
    assert!(
        game.world.get::<Task>(player).is_some(),
        "or the refusal is blanket rather than about reach"
    );
}

/// Spawns a workable structure with a full node into the base's starting
/// pocket, away from anything else.
///
/// The pocket is laid here rather than by each caller because it is what
/// makes the cell the machine stands on a real one: posting to it and
/// working it both walk `BaseGrid`, and a machine standing in unmined rock
/// is refused as walled in whatever else the test set up.
fn workable_structure(game: &mut Game, x: i32, y: i32) -> Entity {
    game.lay_starting_pocket();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.work.is_some() && d.raidable)
        .expect("a workable, raidable structure should exist");
    game.world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            ResourceNode {
                resource: def.work.as_ref().unwrap().produces.clone(),
                level: None,
            },
        ))
        .id()
}

/// Every entity currently carrying a `Task` of `kind` aimed at `structure`.
fn holders(game: &mut Game, structure: Entity, kind: TaskKind) -> Vec<Entity> {
    game.world
        .query::<(Entity, &Task)>()
        .iter(&game.world)
        .filter(|(_, t)| t.target == structure && t.kind == kind)
        .map(|(e, _)| e)
        .collect()
}

#[test]
fn a_second_cronjob_on_one_structure_displaces_the_first() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 10, 3);
    stand_player_at_post(&mut game, structure);

    game.assign_cronjob(first, structure).unwrap();
    game.assign_cronjob(second, structure).unwrap();

    assert!(
        game.world.get::<Task>(first).is_none(),
        "the first worker should have been stood down by the second"
    );
    assert!(game.world.get::<Task>(second).is_some());
    assert_eq!(
        holders(&mut game, structure, TaskKind::GatherResource).len(),
        1,
        "a structure must never have two programs drawing from one node"
    );
}

#[test]
fn a_guard_and_a_cronjob_can_share_a_structure() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    let guard = spawn_tamed(&mut game, 10, 3);

    game.assign_cronjob(worker, structure).unwrap();
    game.assign_guard(guard, structure).unwrap();

    assert!(
        game.world.get::<Task>(worker).is_some(),
        "posting a guard must not displace the cronjob worker — the two jobs \
         are counted separately"
    );
    assert!(game.world.get::<Task>(guard).is_some());
}

#[test]
fn a_second_guard_on_one_structure_displaces_the_first() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 10, 3);

    game.assign_guard(first, structure).unwrap();
    game.assign_guard(second, structure).unwrap();

    assert!(game.world.get::<Task>(first).is_none());
    assert_eq!(holders(&mut game, structure, TaskKind::Guard).len(), 1);
}

/// A posted program sets off from the player's tile, not from the tile it
/// was beaten on.
///
/// A tamed program's `Position` is written once, at capture, and never
/// again — `views.rs` says so and `render/base.rs` refuses to draw a
/// companion because of it. So the stale tile can be anywhere the player has
/// ever fought, and a walk measured from it strands a worker outside
/// `haul_walk_radius` of its own machine: `haul_step_system` finds it absent
/// from the field, steps nowhere this tick and every tick after, and the
/// cronjob produces nothing for the rest of the run while looking scheduled.
#[test]
fn a_posted_program_starts_from_the_player() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    // Where the player is standing when they post it: at the machine's east
    // station cell, so the assignment turns on nothing but the two positions.
    let player_pos = Position { x: 3, y: 0 };
    stand_in_base_at(&mut game, player_pos.x, player_pos.y);
    // Tamed far enough out that the old rule would have refused it outright,
    // and far enough that it could never have walked in.
    let reach = haul_walk_radius(STARTING_POCKET_RADIUS);
    let worker = spawn_tamed_on_map(&mut game, 2, reach + 5);

    game.assign_cronjob(worker, structure)
        .expect("a program you are carrying can be posted wherever you are standing");

    assert_eq!(
        game.world.get::<Position>(worker).copied(),
        Some(player_pos),
        "the program should be standing where the player posted it from"
    );
}

/// The refusal survives, but it is now about where *the player* is: the
/// program starts from their tile, so a structure they cannot reach is one
/// the program cannot reach either.
#[test]
fn posting_to_a_structure_the_player_cannot_reach_is_refused() {
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    let worker = spawn_tamed_on_map(&mut game, 2, 1);
    let reach = haul_walk_radius(STARTING_POCKET_RADIUS);
    stand_in_base_at(&mut game, 2, reach + 5);

    let err = game
        .assign_cronjob(worker, structure)
        .expect_err("a post the program could never walk to must not be accepted");

    // Not the walled-in wording: the structure has a free tile beside it,
    // the player is simply further off than a walk can cover.
    assert!(err.contains("No route"), "unexpected refusal: {err}");
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "a refused cronjob must leave no Task behind"
    );
}

#[test]
fn the_players_own_work_holds_the_cronjob_slot() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let structure = workable_structure(&mut game, 2, 0);
    let worker = spawn_tamed(&mut game, 10, 3);

    stand_player_at_post(&mut game, structure);
    game.work_structure(structure).unwrap();
    let player = game.player_entity();
    assert!(game.world.get::<Task>(player).is_some());

    game.assign_cronjob(worker, structure).unwrap();

    assert!(
        game.world.get::<Task>(player).is_none(),
        "a cronjob on the node you are working yourself must break your own \
         work, or both draw from it at once"
    );
    assert_eq!(
        holders(&mut game, structure, TaskKind::GatherResource).len(),
        1
    );
}

/// A tamed worker forced onto a named species, so a test can post a
/// specific `base_speed`. `spawn_tamed` builds from `generic_species`,
/// whose own `base_speed` is whatever the first ability-less species on the
/// roster happens to declare — not something a test should reason about.
fn tamed_of(game: &mut Game, species: &str) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    game.world.get_mut::<Creature>(worker).unwrap().species = SpeciesId::from(species);
    worker
}

/// `base_speed` paces a machine as well as combat initiative: the cycle a
/// posted program runs is the structure's own rate scaled by how far its
/// species sits from `DEFAULT_BASE_SPEED`, baked into `Task::required` at
/// the moment it is posted.
#[test]
fn a_quicker_program_is_posted_on_a_shorter_cycle() {
    let mut game = Game::new(954, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = workable_structure(&mut game, 2, 0);
    stand_player_at_post(&mut game, node);

    let sprite = tamed_of(&mut game, "sprite");
    game.assign_cronjob(sprite, node).unwrap();
    let quick = game.world.get::<Task>(sprite).unwrap().required;

    let construct = tamed_of(&mut game, "construct");
    game.assign_cronjob(construct, node).unwrap();
    let slow = game.world.get::<Task>(construct).unwrap().required;

    assert!(
        quick < slow,
        "posting a faster species must buy a shorter cycle — got {quick} against {slow}"
    );
}

/// The player has no species, so their deviation is zero. This is the other
/// half of the pressure `base_int` set up: it has to stay true that a dull
/// program is worse than doing the job yourself.
///
/// Checked on two rates, and the second one is what gives the test teeth.
/// The player's initiative baseline (`PLAYER_BASE_SPEED`, 11) is a shade
/// above the roster's (`DEFAULT_BASE_SPEED`, 10), and pacing work off the
/// wrong one of those is the live mistake here — but a Mining Node cannot
/// see it: `10 * 0.95` rounds straight back to 10, so that node alone would
/// pass this test either way. A Research Node's 14 does discriminate, at
/// `14 * 0.95 -> 13`.
#[test]
fn working_a_node_by_hand_still_costs_exactly_the_machines_own_rate() {
    let mut game = Game::new(955, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);

    for (i, kind) in ["mining_node", "research_node"].iter().enumerate() {
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| &d.id == kind)
            .unwrap_or_else(|| panic!("{kind} ships with the game"));
        let work = def.work.expect("both of these are worked structures");
        let node = game
            .world
            .spawn((
                Structure {
                    kind: kind.to_string(),
                },
                Position {
                    x: 3 + i as i32 * 6,
                    y: 4,
                },
                ResourceNode {
                    resource: work.produces.clone(),
                    level: None,
                },
            ))
            .id();
        stand_player_at_post(&mut game, node);

        game.work_structure(node).unwrap();

        let required = game
            .world
            .get::<Task>(game.player_entity())
            .expect("working a node puts the same Task on the player a worker carries")
            .required;
        assert_eq!(
            required, work.ticks_per_unit,
            "the player works a {kind} at the def's own rate, not at PLAYER_BASE_SPEED"
        );
    }
}

/// Every deployed structure carries a buffer, not just the ones that
/// produce: a collect must be able to reach any of them, and a storage
/// building declares neither `work` nor `assembles` and still needs an
/// output size.
#[test]
fn deploying_a_structure_gives_it_an_empty_stock_sized_by_its_def() {
    let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);

    let stock = game
        .world
        .get::<Stock>(node)
        .expect("a deployed structure gets a Stock");
    assert!(
        stock.input.is_empty() && stock.output.is_empty(),
        "a freshly deployed machine has nothing buffered"
    );
    assert_eq!(
        stock.capacity,
        crate::tuning::DEFAULT_OUTPUT_CAPACITY,
        "mining_node.ron sets no capacity, so it takes the default"
    );

    let home = find_home(&mut game).unwrap();
    assert!(
        game.world.get::<Stock>(home).is_some(),
        "a Home produces nothing but is still collectable from"
    );
}

/// `MachineStatus` marks the things that can stall. Absent means "not a
/// machine" — a Home has no job to be starved of, and giving it a status
/// would put a permanently-Running row in the structure report.
#[test]
fn only_structures_that_run_a_job_get_a_machine_status() {
    let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    let home = find_home(&mut game).unwrap();

    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Idle),
        "a work node with nobody posted to it is idle — it starts optimistic \
         and `idle_machine_system` is what actually corrects it, which for a \
         long time nothing did for an extractor"
    );
    assert!(game.world.get::<MachineStatus>(home).is_none());
}

/// Stock is per-structure player state, so it has to persist. Both halves:
/// a machine that came home mid-batch must not have its staged ingredients
/// silently refunded, nor its finished goods silently voided.
#[test]
fn partially_filled_buffers_survive_a_save_and_load_round_trip() {
    let mut game = Game::new(976, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    {
        let mut stock = game.world.get_mut::<Stock>(node).unwrap();
        stock.input.insert(ItemId::from(ids::CORE_FRAGMENT), 3);
        stock.output.insert(ItemId::from(ids::POWER_CELL), 7);
    }

    let path = std::env::temp_dir().join(format!("feral_stock_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = find_structure_by_kind(&mut loaded, "mining_node").unwrap();
    let stock = loaded.world.get::<Stock>(restored).unwrap();
    assert_eq!(
        stock.input.get(&ItemId::from(ids::CORE_FRAGMENT)).copied(),
        Some(3),
        "staged ingredients are not refunded by saving"
    );
    assert_eq!(
        stock.output.get(&ItemId::from(ids::POWER_CELL)).copied(),
        Some(7),
        "finished goods waiting to be collected are not voided by saving"
    );
    assert_eq!(
        stock.capacity,
        crate::tuning::DEFAULT_OUTPUT_CAPACITY,
        "capacity comes back off the def, not the save"
    );
}

/// Puts a tamed worker on a node of `kind` standing at an absolute tile,
/// with `capacity` units of output room, and returns the node. Absolute
/// rather than relative so a test can park it next to the player and
/// collect from it.
fn worked_node_at(
    game: &mut Game,
    kind: &str,
    resource: &str,
    x: i32,
    y: i32,
    capacity: u32,
) -> Entity {
    stand_ample_grid_supply(game);
    let worker = spawn_tamed(game, 10, 3);
    let node = game
        .world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            ResourceNode {
                resource: ItemId::from(resource),
                level: None,
            },
            Stock::new(capacity),
            MachineStatus::default(),
        ))
        .id();
    stand_player_at_post(game, node);
    game.assign_cronjob(worker, node).unwrap();
    node
}

fn base_log_hits(game: &Game, needle: &str) -> usize {
    game.message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text.contains(needle))
        .count()
}

/// The largest felt change in the whole design: fragments stop appearing in
/// your pocket while you are away. You come home and harvest. It is also the
/// only thing that makes clogging real — a node that pays straight into the
/// player is an infinite source and nothing upstream of it can ever back up.
#[test]
fn a_worked_node_fills_its_own_buffer_and_not_the_players_pocket() {
    let mut game = Game::new(980, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = worked_node_at(&mut game, "mining_node", ids::CORE_FRAGMENT, 3, 4, 20);
    let before = count_item(&game, ids::CORE_FRAGMENT);

    for _ in 0..40 {
        game.tick();
    }

    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        before,
        "a cronjob no longer reaches into the player's cargo"
    );
    assert!(
        game.world
            .get::<Stock>(node)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0)
            > 0,
        "it deposits into its own buffer instead"
    );
}

/// A stalled base must not flood the log pane — the whole point of tracking
/// status is that a machine says so on the way *into* a state, not every
/// tick it spends there.
#[test]
fn a_node_at_output_capacity_clogs_and_says_so_exactly_once() {
    let mut game = Game::new(981, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = worked_node_at(&mut game, "mining_node", ids::CORE_FRAGMENT, 3, 4, 1);

    for _ in 0..60 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Clogged)
    );
    assert_eq!(
        game.world.get::<Stock>(node).unwrap().output_used(),
        1,
        "production stops at the buffer's capacity"
    );
    assert_eq!(
        base_log_hits(&game, "clogged"),
        1,
        "entering the state is news; staying in it is not"
    );
}

/// The clog is not a dead end — it is a prompt. Emptying the buffer starts
/// the node again, and that resumption is worth a line of its own.
#[test]
fn collecting_from_a_clogged_node_lets_it_resume() {
    let mut game = Game::new(982, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = *game.world.get::<Position>(game.player_entity()).unwrap();
    let node = worked_node_at(
        &mut game,
        "mining_node",
        ids::CORE_FRAGMENT,
        p.x + 1,
        p.y,
        1,
    );
    let carried = count_item(&game, ids::CORE_FRAGMENT);

    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Clogged),
        "the fixture has to actually clog, or the rest of this proves nothing"
    );

    game.collect_adjacent();
    for _ in 0..40 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<Stock>(node).unwrap().output_used(),
        1,
        "it went back to work and filled the buffer again"
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT) - carried,
        1,
        "and the collect landed the first unit in the player's cargo"
    );
}

/// Only the item destination moved. A worker still earns from a completed
/// cycle, which is the other half of what a cronjob is for.
#[test]
fn a_worker_still_earns_xp_from_a_completed_cycle() {
    let mut game = Game::new(983, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = worked_node_at(&mut game, "mining_node", ids::CORE_FRAGMENT, 3, 4, 20);
    let worker = holders(&mut game, node, TaskKind::GatherResource)[0];
    let before = {
        let e = game.world.get::<Experience>(worker).unwrap();
        (e.level, e.xp)
    };

    for _ in 0..40 {
        game.tick();
    }

    // Compared as (level, xp), not xp alone: four cycles is enough to level
    // this worker, and a level-up resets `xp` to the remainder — so a bare
    // `xp > before` can fail on a worker that earned *more*, not less.
    let after = game.world.get::<Experience>(worker).unwrap();
    assert!(
        (after.level, after.xp) > before,
        "a completed cycle still pays the worker (level {} xp {})",
        after.level,
        after.xp
    );
}

/// The reach the shipped Recharger Node def declares.
fn recharger_reach(game: &Game) -> i32 {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == "recharger_node")
        .and_then(|d| d.power_regen.map(|r| r.radius))
        .expect("the Recharger Node regenerates Power")
}

/// How wide a base these fixtures call "grown". Deliberately not
/// `MAX_BUILD_RADIUS_TILES`, which is a backstop rather than a target: at the
/// ceiling a stamp lays 40,401 tiles and a hauling walk searches four times
/// that, which measures the pathological case and costs the suite minutes.
const GROWN_RADIUS: i32 = 10;

/// A base mined out to `GROWN_RADIUS`: the floor laid by hand, since slice
/// 2 is what sells the player a way to lay it. `GROWN_PILLAR` used to widen
/// a slab here; the footprint is `BaseGrid` now, so the fixture writes to it.
fn fully_grown_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for y in -GROWN_RADIUS..=GROWN_RADIUS {
            for x in -GROWN_RADIUS..=GROWN_RADIUS {
                grid.lay_floor(x, y);
            }
        }
    }
    assert_eq!(
        game.world.resource::<crate::base_grid::BaseGrid>().radius(),
        GROWN_RADIUS,
        "precondition: the base reaches well past the pocket it started with"
    );
    stand_in_base(&mut game);
    game
}

/// A posting the cronjob menu accepts has to be a posting that arrives, and
/// the walk that delivers it is bounded by a radius. Bounded by the size the
/// pocket *started* at, a base mined out past it refuses postings across its
/// own width — a machine you can see from your Home and cannot staff.
#[test]
fn a_program_walks_across_a_fully_grown_base_to_its_post() {
    let mut game = fully_grown_base(702);
    let r = GROWN_RADIUS;
    game.place_structure("mining_node", r, 0).unwrap();
    let node = game
        .find_blocking_structure_at(r, 0)
        .expect("the node was just deployed");
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let worker = spawn_tamed(&mut game, 500, 3);

    // Posted from the opposite edge of the base: its whole width separates
    // the program from its machine.
    stand_in_base_at(&mut game, -r, 0);
    game.assign_cronjob(worker, node).unwrap();

    for _ in 0..200 {
        if game::base::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos) {
            break;
        }
        game.tick();
    }
    assert!(
        game::base::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos),
        "a worker posted from one edge of a full-size base must reach the other"
    );
}

/// A base with a Home down and materials to spare, so nothing below is
/// measuring the build cost.
fn base_with_room_to_build(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 500);
    // Standing in the base rather than out on the zone surface: deploying,
    // upgrading, demolishing and working a machine by hand are all
    // `Game::require_base` now that the base is out of phase.
    stand_in_base(&mut game);
    game
}

/// A structure can declare how many of it may stand at once, and the Heap
/// Pillar is the first to use it: growth is bounded by a number you can
/// tune in the asset rather than by the backstop radius.
#[test]
fn a_capped_structure_refuses_the_one_past_its_limit_and_costs_nothing() {
    let mut game = base_with_room_to_build(721);
    unlock_research_chain(&mut game, "cache_coherence");
    give(&mut game, &ItemId::from("cache_grain"), 500);
    let cap = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "line_driver")
        .expect("line_driver.ron ships")
        .max_deployed;
    assert!(cap > 0, "the Line Driver declares a limit");

    // Placed in a ring around the player so each one has its own free cell.
    let mut spots = (1..=4)
        .flat_map(|d| [(d, 0), (-d, 0), (0, d), (0, -d)])
        .filter(|&(dx, dy)| (dx, dy) != (0, 0));
    for i in 0..cap {
        let (dx, dy) = spots.next().expect("enough free floor for the cap");
        game.place_structure("line_driver", dx, dy)
            .unwrap_or_else(|e| panic!("driver {i} refused: {e}"));
    }

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let (dx, dy) = spots.next().unwrap();
    let err = game
        .place_structure("line_driver", dx, dy)
        .expect_err("one past the limit must be refused");

    assert!(
        err.to_lowercase().contains("line driver"),
        "the refusal should name what is capped: {err}"
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        before,
        "a refused build must not have spent anything"
    );
}

/// Every other structure is uncapped, and stays that way by defaulting —
/// an existing file and any mod that never heard of the field is unlimited.
#[test]
fn a_structure_that_declares_no_limit_is_unlimited() {
    let game = Game::new(722, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let capped: Vec<String> = game
        .structure_defs()
        .into_iter()
        .filter(|d| d.max_deployed > 0)
        .map(|d| d.id.to_string())
        .collect();
    assert_eq!(
        capped,
        vec!["line_driver".to_string()],
        "only the grid supplier is capped; the field defaults to no limit"
    );
}

/// The zone-2 material is gated twice, and both gates already existed: the
/// research entry's `min_zone` and the research itself. Asserted through the
/// shipped assets rather than a fixture, because the whole feature is
/// `.ron` — an entry that lost its `min_zone` would still place a Cache Tap
/// in zone 1 and nothing else in the suite would notice.
#[test]
fn a_cache_tap_waits_for_the_second_zone_and_its_research() {
    let mut game = Game::new(801, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 60);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 8);
    grant_research_data(&mut game, 1000);

    let unresearched = game
        .place_structure("cache_tap", 1, 0)
        .expect_err("the Tap waits on Cache Coherence");
    assert!(unresearched.contains("researched"), "{unresearched}");

    // Prereqs are checked ahead of the zone gate, so the Grid has to be in
    // hand for the refusal under test to be the zone rather than the tree.
    game.unlock_research("power_grid")
        .expect("the Grid is a zone-1 node");
    let too_early = game
        .unlock_research("cache_coherence")
        .expect_err("and Cache Coherence waits on the breach");
    assert!(too_early.contains("Zone 2"), "{too_early}");

    set_zone(&mut game, 2);
    game.unlock_research("cache_coherence")
        .expect("a breached run may learn it");
    game.place_structure("cache_tap", 1, 0)
        .expect("and then stand a Tap up");
}

/// The layering property, which is the whole of "a new material does not
/// retire the old one": breaching past the zone that introduced Cache Grain
/// leaves Core Fragments extractable exactly as before. A tier that replaced
/// its predecessor would strand every recipe still denominated in fragments.
#[test]
fn core_fragments_keep_flowing_once_the_second_zone_material_arrives() {
    let mut game = Game::new(802, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 120);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 8);
    unlock_research_chain(&mut game, "cache_coherence");
    set_zone(&mut game, 3);

    game.place_structure("mining_node", 1, 0)
        .expect("a Mining Node is still a thing you can build in zone 3");
    game.place_structure("cache_tap", -1, 0)
        .expect("and the Tap does not expire when the next zone arrives");

    let defs = game.structure_defs();
    let produced = |id: &str| {
        defs.iter()
            .find(|d| d.id.as_str() == id)
            .and_then(|d| d.work.as_ref())
            .map(|w| w.produces.clone())
            .expect("both are producing structures")
    };
    assert_eq!(
        produced("mining_node"),
        ItemId::from(ids::CORE_FRAGMENT),
        "the Mining Node's output is untouched by the tier above it"
    );
    assert_eq!(
        produced("cache_tap"),
        ItemId::from("cache_grain"),
        "and the Tap is what the new material comes out of"
    );
}

/// The advanced building half of the payoff: a Line Driver is denominated in
/// the zone-2 material, so the grid stops growing at five Pillars until the
/// run has a Tap running. Refused for the material alone, with the research
/// already in hand and fragments to spare.
#[test]
fn a_line_driver_is_refused_without_the_zone_two_material() {
    let mut game = Game::new(803, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 8);
    unlock_research_chain(&mut game, "cache_coherence");

    let broke = game
        .place_structure("line_driver", 1, 0)
        .expect_err("fragments alone do not buy one");
    assert!(broke.contains("Cache Grain"), "{broke}");

    let (_, before) = game.base_power();
    give(&mut game, &ItemId::from("cache_grain"), 12);
    game.place_structure("line_driver", 1, 0)
        .expect("with the grain in hand it stands up");
    let (_, after) = game.base_power();
    assert!(
        after > before,
        "and it feeds the grid it was bought to feed: {before} -> {after}"
    );
}
