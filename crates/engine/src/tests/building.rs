//! Placing, removing, upgrading, and describing structures, and the base platform they sit on.

use super::support::*;
use crate::components::Downed;
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
        .place_structure("armory", r, r, None)
        .expect_err("the corner cell is off the floor and shouldn't be buildable");
    assert!(err.contains("no floor there"), "unexpected error: {err}");

    // Diagonally in by one, which is the first cell the chamfer leaves —
    // and the assertion that stops the cut being fixed by shrinking the
    // whole build box.
    place_now(&mut game, "armory", r - 1, r - 1)
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
        place_now(&mut game, "armory", 1, 0).is_err(),
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
    place_now(&mut game, "armory", 1, 0).unwrap();
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
        .place_structure("home", 1, 0, None)
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
        .place_structure("armory", 1, 0, None)
        .expect_err("a cell with no floor under it shouldn't be buildable");
    assert!(err.contains("no floor there"), "unexpected error: {err}");

    // Pointing back into the pocket makes it buildable again.
    place_now(&mut game, "armory", -1, 0).expect("building back onto laid floor should succeed");
}

/// **A request the base cannot afford is filed, not refused**, and the crew
/// is what says so.
///
/// The old contract refused the deploy outright and logged the shortfall.
/// That refusal is gone with the verb it belonged to: a deploy is a request
/// now, and a request the base will be able to afford in ten minutes is a
/// perfectly good thing to file — production catches up and the crew starts
/// carrying. What survives is the *reporting* obligation, moved to the one
/// place that can still see a shortfall: a builder standing at a site with
/// nothing anywhere to fetch. It is base news for the same reason the
/// refusal was, and it names the numbers for the same reason too — "not
/// enough" without a figure sends the player back to the build menu to work
/// out what they were short of.
#[test]
fn a_request_the_base_cannot_afford_is_filed_and_the_crew_says_what_it_is_short_of() {
    let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    place_home(&mut game);
    // A body to post, or nobody ever walks to the site and nothing is ever
    // said — the announcement belongs to the crew, not to the keypress.
    spawn_tamed(&mut game, 10, 3);

    let held = count_item(&game, ids::CORE_FRAGMENT);
    file_build(&mut game, "armory", 1, 0)
        .expect("a request is filed whether or not the base can afford it yet");

    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        held,
        "filing a request charges nothing — the materials are fetched by hand later"
    );
    let site = game
        .build_site_at(1, 0)
        .expect("the request stands on the cell it was filed for");
    let outstanding = game
        .world
        .get::<BuildSite>(site)
        .expect("it is a build site")
        .outstanding();
    assert!(
        outstanding
            .iter()
            .any(|(item, qty)| item.as_str() == ids::CORE_FRAGMENT && *qty == 18),
        "the whole bill is outstanding, since nothing has been delivered: {outstanding:?}"
    );

    // Run the crew until it has walked to the site and found the base bare.
    for _ in 0..60 {
        game.tick();
    }
    let said = game
        .message_history(200)
        .into_iter()
        .find(|m| m.text.contains("nothing to raise"))
        .expect("the crew reports a base with nothing to fetch");
    assert_eq!(
        said.source,
        MessageSource::Base,
        "a shortfall is base news, not field news"
    );
    // **13, not 18** — and that figure is the whole feature working. The
    // crew walked to the party, took the five Core Fragments the starting
    // kit holds straight out of the pack, carried them to the cell and set
    // them down; what it reports short is what is left after everything the
    // base could actually reach has already been delivered.
    assert!(
        said.text.contains("Armory") && said.text.contains(&format!("{} Core Fragment", 18 - held)),
        "the line should name what is being raised and what is still outstanding: {}",
        said.text
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        0,
        "the pack was emptied into the site, one carry at a time"
    );
    assert_eq!(
        game.world
            .get::<BuildSite>(site)
            .expect("the site is still standing, waiting on the rest")
            .delivered_of(&ItemId::from(ids::CORE_FRAGMENT)),
        held,
        "and every unit that left the pack is standing on the cell"
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
    place_now(&mut game, "armory", 1, 0).unwrap();
    place_now(&mut game, "mining_node", 2, 0).unwrap();
    place_now(&mut game, "mining_node", -2, 0).unwrap();

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
    place_now(&mut game, "armory", 1, 0).unwrap();
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
    place_now(&mut game, "armory", 1, 0).unwrap();
    place_now(&mut game, "fabricator", 0, 1).unwrap();
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
    place_now(&mut game, "armory", 1, 0).unwrap();

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
    // `spawn_structure_at` is deliberately bare — see its own doc comment —
    // so a Recharger it stands carries no charge until given one. Fuelled
    // here because this test is about the trickle a *paying* supplier gives,
    // not about the dry gate `tests::power` covers.
    let recharger = find_structure_by_kind(&mut game, "recharger_node").unwrap();
    game.world
        .entity_mut(recharger)
        .insert(crate::components::PowerFuel {
            ticks_left: crate::tuning::POWER_UPKEEP_TICKS,
        });

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
    // See `a_recharger_node_in_range_nets_power_upward_on_a_real_tick`: a
    // bare `spawn_structure_at` carries no charge, and this test is about
    // regen ordering, not the dry gate.
    let recharger = find_structure_by_kind(&mut game, "recharger_node").unwrap();
    game.world
        .entity_mut(recharger)
        .insert(crate::components::PowerFuel {
            ticks_left: crate::tuning::POWER_UPKEEP_TICKS,
        });

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

    upgrade_now(&mut game, node).unwrap();

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
    upgrade_now(&mut game, node).unwrap();
    assert_eq!(
        game.world.get::<ResourceNode>(node).unwrap().level,
        Some(2),
        "tier feeds ResourceNode.level, which already drives mining_success_chance"
    );
}

#[test]
fn upgrading_refuses_past_max_tier() {
    let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    // Deep enough that the zone ceiling is out of the way: this test is about
    // the def's own `max_tier`, which is checked after the zone gate.
    set_zone(&mut game, 9);

    // A base that cannot afford the bill is deliberately *not* refused here:
    // upgrading files a request now, and a request the base cannot afford yet
    // is the whole point of a queue — the crew says so from the site instead.
    file_upgrade(&mut game, node).expect("filing costs nothing and checks no store");
    let site = game
        .build_site_at(
            game.world.get::<Position>(node).unwrap().x,
            game.world.get::<Position>(node).unwrap().y,
        )
        .expect("the request stands");
    game.cancel_build_request(site).unwrap();

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
        upgrade_now(&mut game, node).unwrap();
    }
    let err = game
        .upgrade_structure(node, None)
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
        .upgrade_structure(node, None)
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
    upgrade_now(&mut game, node).unwrap();
    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);

    let err = game
        .upgrade_structure(node, None)
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
        upgrade_now(&mut game, node).unwrap();
    }
    let err = game
        .upgrade_structure(node, None)
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
        .upgrade_structure(home, None)
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
    upgrade_now(&mut game, node).unwrap();
    stock_upgrade_materials(&mut game, 30);
    upgrade_now(&mut game, node).unwrap();

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
                // `level: None` above skips the reliability roll, so none of
                // the three roll modifiers has anything to act on here; the
                // classless worker is what makes this the ordinary payout curve.
                crate::systems::CycleModifiers {
                    keen_scavenger_level: 0,
                    base_int: crate::tuning::DEFAULT_BASE_INT,
                    class: None,
                    morale: 0.0,
                    need_strain: 0.0,
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
/// (a take) — so a job started from across the base earns into a
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

    take_everything_adjacent(&mut game);
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
    place_now(&mut game, "mining_node", r, 0).unwrap();
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
        place_now(&mut game, "line_driver", dx, dy)
            .unwrap_or_else(|e| panic!("driver {i} refused: {e}"));
    }

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let (dx, dy) = spots.next().unwrap();
    let err = game
        .place_structure("line_driver", dx, dy, None)
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
        .place_structure("cache_tap", 1, 0, None)
        .expect_err("the Tap waits on Cache Coherence");
    assert!(unresearched.contains("researched"), "{unresearched}");

    // Prereqs are checked ahead of the zone gate, so the Grid has to be in
    // hand for the refusal under test to be the zone rather than the tree.
    unlock_research_chain(&mut game, "power_grid");
    let too_early = game
        .select_research("cache_coherence")
        .expect_err("and Cache Coherence waits on the breach");
    assert!(too_early.contains("Zone 2"), "{too_early}");

    set_zone(&mut game, 2);
    unlock_research_chain(&mut game, "cache_coherence");
    place_now(&mut game, "cache_tap", 1, 0).expect("and then stand a Tap up");
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

    place_now(&mut game, "mining_node", 1, 0)
        .expect("a Mining Node is still a thing you can build in zone 3");
    place_now(&mut game, "cache_tap", -1, 0)
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

    // **The gate is the bill of materials, not a refusal.** A request may be
    // filed with fragments alone — it simply never gets raised, because
    // nothing in the base or the pack holds a unit of the material it is
    // denominated in, and `BuildSite::outstanding` keeps saying so. That is
    // what stops the grid growing until the run has a Tap running.
    file_build(&mut game, "line_driver", 1, 0)
        .expect("a request is filed regardless of what is in stock");
    let site = game.build_site_at(1, 0).expect("the request stands there");
    let outstanding = game
        .world
        .get::<BuildSite>(site)
        .expect("it is a build site")
        .outstanding();
    assert!(
        outstanding
            .iter()
            .any(|(item, _)| game.item_name(item).contains("Cache Grain")),
        "fragments alone leave the zone-2 material outstanding: {outstanding:?}"
    );
    game.cancel_build_request(site)
        .expect("and it can be called off again");

    let (_, before) = game.base_power();
    give(&mut game, &ItemId::from("cache_grain"), 12);
    place_now(&mut game, "line_driver", 1, 0).expect("with the grain in hand it stands up");
    let (_, after) = game.base_power();
    assert!(
        after > before,
        "and it feeds the grid it was bought to feed: {before} -> {after}"
    );
}

// ---------------------------------------------------------------------
// The Repair Bay: what brings a downed program back
// ---------------------------------------------------------------------

/// A base with a Repair Bay standing and one downed program beside it,
/// hurt by `wound` points. The program is placed by hand rather than walked
/// there, and these tests drive `Game::run_repair_bays` rather than a whole
/// `tick`: the walk is `drift_idle_staff`'s half of the feature and has its
/// own tests, and through a full tick the drift is what decides where the
/// body is standing when the Bay looks.
fn a_downed_program_at_a_bay(game: &mut Game, wound: i32) -> Entity {
    stand_in_base(game);
    place_home(&mut *game);
    let bay = spawn_machine_at(game, "repair_bay", 2, 0);
    let program = spawn_tamed(game, 10, 3);
    game.world.entity_mut(program).insert(Downed);
    let mut stats = game.world.get_mut::<Stats>(program).unwrap();
    stats.hp = (stats.max_hp - wound).max(1);
    let at = *game.world.get::<Position>(bay).unwrap();
    *game.world.get_mut::<Position>(program).unwrap() = Position {
        x: at.x + 1,
        y: at.y,
    };
    program
}

fn recovery_lines(game: &Game) -> Vec<String> {
    game.message_log(200)
        .into_iter()
        .filter(|e| e.text.contains("back on its feet"))
        .map(|e| e.text)
        .collect()
}

#[test]
fn a_bay_repairs_a_downed_program_and_stands_it_back_up() {
    let mut game = Game::new(3501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_downed_program_at_a_bay(&mut game, 3);

    game.run_repair_bays();
    assert!(
        game.world.get::<Stats>(program).unwrap().hp > 7,
        "one pass beside a Bay should have restored something"
    );
    assert!(
        game.world.get::<Downed>(program).is_some(),
        "and it is still down while it is still hurt"
    );

    for _ in 0..10 {
        game.run_repair_bays();
    }

    let stats = *game.world.get::<Stats>(program).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "it heals to full and no further");
    assert!(
        game.world.get::<Downed>(program).is_none(),
        "and stands off the bench at full Integrity"
    );
}

#[test]
fn a_downed_program_out_of_reach_of_the_bay_is_not_repaired() {
    let mut game = Game::new(3502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_downed_program_at_a_bay(&mut game, 3);
    // Well outside the shipped Bay's radius, and still inside the pocket.
    *game.world.get_mut::<Position>(program).unwrap() = Position { x: -4, y: -4 };
    let before = game.world.get::<Stats>(program).unwrap().hp;

    for _ in 0..10 {
        game.run_repair_bays();
    }

    assert_eq!(
        game.world.get::<Stats>(program).unwrap().hp,
        before,
        "a Bay reaches its own tile and its neighbours, not the whole base"
    );
    assert!(game.world.get::<Downed>(program).is_some());
}

/// The recovery is news once. `set_machine_status`' rule: entering a state
/// is news, staying in it is not — and a Bay ticking every beat is exactly
/// where a per-tick line would go unnoticed until a player read the log.
#[test]
fn the_recovery_is_announced_once_and_not_every_tick() {
    let mut game = Game::new(3503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    a_downed_program_at_a_bay(&mut game, 3);

    for _ in 0..30 {
        game.run_repair_bays();
    }

    assert_eq!(
        recovery_lines(&game).len(),
        1,
        "expected exactly one recovery line: {:?}",
        recovery_lines(&game)
    );
}

/// Everything the map would draw a recovery mark on, as `(is_structure,
/// tile)` — the structure flag included because the Bay carrying the mark
/// instead of the body in it is exactly the regression this asserts against.
fn recovery_marks(game: &mut Game) -> Vec<(bool, (i32, i32))> {
    game.view_entities(20, 20)
        .into_iter()
        .filter(|v| v.recovering)
        .map(|v| (v.is_structure, v.pos))
        .collect()
}

/// **The mark is the heal, asked a second way.** `EntityView::recovering` is
/// what the map draws its bouncing `+` from, and it has to answer for the
/// same three states `run_repair_bays` acts on: somebody in reach, somebody
/// out of reach, and nobody down at all. A flag that merely said "this is a
/// Repair Bay" would pass a draw test and light an empty Bay up forever.
///
/// **The mark rides the body, not the building**, which is what the
/// `is_structure` half of every assertion below holds. The two are different
/// tiles whenever a Bay reaches past the cell it stands on, and the patient
/// is the thing being mended — the Bay is only where it happens.
///
/// **Both ends have to be on the map, and neither is by default.**
/// `view_entities` selects on `Glyph`: `spawn_machine_at` writes none, so the
/// Bay is *placed* through the real deploy path rather than hand-spawned like
/// the fixture the other Bay tests share, and `spawn_tamed` writes none
/// either, so the body comes from `spawn_tamed_on_map`. An entity with no
/// `Glyph` has no view at all, which reads here as the flag having been lost.
#[test]
fn a_recovering_program_wears_the_mark_and_its_bay_never_does() {
    let mut game = Game::new(3505, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "repair_bay", 2, 0).expect("a Repair Bay is buildable from the start");
    let site = find_structure_by_kind(&mut game, "repair_bay").unwrap();
    let at = *game.world.get::<Position>(site).unwrap();
    let bay = (at.x, at.y);

    let program = spawn_tamed_on_map(&mut game, at.x + 1, at.y);
    game.world.entity_mut(program).insert(Downed);
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;

    assert_eq!(
        recovery_marks(&mut game),
        vec![(false, (bay.0 + 1, bay.1))],
        "the body being mended wears the mark, on its own tile and not the Bay's"
    );

    // The same rejection `a_downed_program_out_of_reach_of_the_bay_is_not_
    // repaired` makes: a body lying across the base is not in this Bay, and
    // a mark that ignored reach would say it was.
    *game.world.get_mut::<Position>(program).unwrap() = Position { x: -4, y: -4 };
    assert!(
        recovery_marks(&mut game).is_empty(),
        "a program out of reach of every Bay is not recovering"
    );

    // Back in reach, and healed off the bench. The `Downed` marker coming
    // off is what ends the recovery, so it is what has to end the mark.
    *game.world.get_mut::<Position>(program).unwrap() = Position {
        x: bay.0 + 1,
        y: bay.1,
    };
    assert_eq!(recovery_marks(&mut game), vec![(false, (bay.0 + 1, bay.1))]);
    for _ in 0..20 {
        game.run_repair_bays();
    }
    assert!(
        game.world.get::<Downed>(program).is_none(),
        "the fixture must actually reach full Integrity"
    );
    assert!(
        recovery_marks(&mut game).is_empty(),
        "a program back on its feet is not recovering, and nothing takes the mark over"
    );
}

/// A base with a Bay standing and one staff program beside it, at `hp` out
/// of its maximum.
///
/// The Bay is placed through the real deploy path for `a_bay_reports_itself_
/// occupied`'s reason, and the program is stood in reach by hand: the walk
/// is `drift_idle_staff`'s half of the feature and is tested there.
fn a_hurt_staff_program_at_a_bay(game: &mut Game, hp: i32) -> (Entity, (i32, i32)) {
    stand_in_base(game);
    place_home(&mut *game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(game, "repair_bay", 2, 0).expect("a Repair Bay is buildable from the start");
    let site = find_structure_by_kind(game, "repair_bay").unwrap();
    let at = *game.world.get::<Position>(site).unwrap();

    let program = spawn_tamed(game, 40, 3);
    game.world.get_mut::<Stats>(program).unwrap().hp = hp;
    *game.world.get_mut::<Position>(program).unwrap() = Position {
        x: at.x + 1,
        y: at.y,
    };
    (program, (at.x, at.y))
}

/// One admission pass, as `schedule_base_labour` makes it.
fn admit(game: &mut Game) {
    let staff = game.base_staff();
    let bays = game.repair_bays();
    game.admit_the_badly_hurt(&staff, &bays);
}

fn hp_of(game: &Game, who: Entity) -> i32 {
    game.world.get::<Stats>(who).unwrap().hp
}

/// **The threshold, at both sides of the line.** A staff program on its last
/// legs takes itself off the line and becomes a Bay's business; one that is
/// merely knocked about keeps working. Asserted either side of
/// `BAY_ADMISSION_HP_FRACTION` rather than at one hand-picked number, so
/// retuning the constant moves both cases together instead of turning half
/// the test vacuous.
#[test]
fn a_staff_program_is_admitted_to_a_bay_only_once_it_is_badly_hurt() {
    let mut game = Game::new(3506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let max = 40;
    let line = (max as f32 * crate::tuning::BAY_ADMISSION_HP_FRACTION).ceil() as i32;

    let (hurt, _) = a_hurt_staff_program_at_a_bay(&mut game, line - 1);
    admit(&mut game);
    assert!(
        game.world.get::<Downed>(hurt).is_some(),
        "a staff program under the line breaks off for repairs"
    );

    let mut game = Game::new(3506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (scratched, _) = a_hurt_staff_program_at_a_bay(&mut game, max);
    game.world.get_mut::<Stats>(scratched).unwrap().hp = line + 1;
    admit(&mut game);
    assert!(
        game.world.get::<Downed>(scratched).is_none(),
        "a program above the line stays on the line"
    );
}

/// **The exit is full Integrity and nothing else, so the gap the marker has
/// to cross to flicker is the whole bar.** A release line just above the
/// admission line is the shape this repo uses for needs and morale, and it
/// is the wrong one here: a Bay already had an exit — `run_repair_bays`
/// lifting `Downed` at full — and two ways out of one state is how they come
/// to disagree.
///
/// Written as a walk over every intermediate tick rather than a check at the
/// end: the failure this guards against is an *oscillation*, which a
/// before-and-after assertion cannot see at all. It counts the transitions
/// and demands exactly one in each direction.
#[test]
fn an_admitted_program_is_not_released_until_it_is_whole() {
    let mut game = Game::new(3507, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let max = 40;
    // Just under the line, which is the interesting side of it: started at 1
    // HP the early ticks are nowhere near the boundary and the oscillation
    // this test exists for could not happen on them anyway.
    let line = (max as f32 * crate::tuning::BAY_ADMISSION_HP_FRACTION).ceil() as i32;
    let (program, _bay) = a_hurt_staff_program_at_a_bay(&mut game, line - 1);

    let mut marked = false;
    let (mut admissions, mut releases) = (0, 0);
    let mut served_below_full = 0;
    for _ in 0..200 {
        admit(&mut game);
        let stats = *game.world.get::<Stats>(program).unwrap();
        let whole = stats.hp >= stats.max_hp;
        let now = game.world.get::<Downed>(program).is_some();
        if now != marked {
            if now {
                admissions += 1
            } else {
                releases += 1
            }
            marked = now;
        }
        // The continuity claim, asked on every tick of the climb rather than
        // at the two ends: from admission to full it is held, its Bay is lit,
        // and it is never handed back to the line in between.
        if !whole && marked {
            assert!(
                game.recovering_programs().contains(&program),
                "a program mid-climb at {}/{} must still be in its Bay",
                stats.hp,
                stats.max_hp
            );
            served_below_full += 1;
        }
        game.run_repair_bays();
    }

    let stats = *game.world.get::<Stats>(program).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "it should have mended");
    assert_eq!(admissions, 1, "it must be admitted once, not repeatedly");
    assert_eq!(
        releases, 1,
        "and released once — a second means it bounced back in off the line"
    );
    assert!(!marked, "and it ends the run back on the staff");
    assert!(
        served_below_full > 1,
        "the climb must actually take several ticks, or this proves nothing"
    );
}

/// **The wiring, through the real tick.** Every other test here calls
/// `admit_the_badly_hurt` the way the scheduler does; none of them would
/// notice if the call were deleted from `schedule_base_labour` altogether,
/// and the feature would be dead in play with a green suite behind it.
/// This one drives `Game::tick` and nothing else.
#[test]
fn the_scheduler_admits_a_badly_hurt_program_on_its_own() {
    let mut game = Game::new(3513, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (program, _) = a_hurt_staff_program_at_a_bay(&mut game, 2);
    assert!(game.world.get::<Downed>(program).is_none());

    for _ in 0..5 {
        game.tick();
    }

    assert!(
        game.world.get::<Downed>(program).is_some(),
        "the scheduler must make this call itself, not only the tests"
    );
}

/// **What a Bay's capacity turns out to be, pinned rather than assumed.**
/// `run_repair_bays` walks the downed and asks each one which Bay serves it;
/// there is no per-Bay slot, no queue and no rate to divide, so every body in
/// reach is mended at the full authored rate on the same tick. The shipped
/// Bay's `radius: 0` is `at_station`, its four orthogonal neighbours.
///
/// Worth a test now that a program occupies its Bay for the whole 20%-to-full
/// climb rather than the moment a corpse takes: if this ever became one slot,
/// the second body would starve in silence.
#[test]
fn a_bay_mends_everybody_in_reach_on_the_same_tick() {
    let mut game = Game::new(3512, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (first, bay) = a_hurt_staff_program_at_a_bay(&mut game, 2);
    let second = spawn_tamed(&mut game, 40, 3);
    game.world.get_mut::<Stats>(second).unwrap().hp = 2;
    *game.world.get_mut::<Position>(second).unwrap() = Position {
        x: bay.0 - 1,
        y: bay.1,
    };

    admit(&mut game);
    let before = (hp_of(&game, first), hp_of(&game, second));
    game.run_repair_bays();
    let after = (hp_of(&game, first), hp_of(&game, second));

    assert!(
        after.0 > before.0 && after.1 > before.1,
        "both bodies mend on the same tick: {before:?} -> {after:?}"
    );
    assert_eq!(
        after.0 - before.0,
        after.1 - before.1,
        "and at the same rate — a Bay divides nothing between them"
    );
}

/// **The mark and the heal are one predicate, and widening admission must
/// not have split them.** `Bays::serving` answers both, so a program a Bay
/// is putting Integrity into is a program wearing the map's `+` — in every
/// state a program can be in, not just the one the feature was built for.
///
/// An equivalence, so it is walked over states that answer *no* as well as
/// the one that answers yes: asserted only on the healing case it would pass
/// against a mark that was simply always lit.
#[test]
fn the_recovery_mark_is_lit_exactly_when_somebody_is_being_healed() {
    let mut game = Game::new(3508, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (program, bay) = a_hurt_staff_program_at_a_bay(&mut game, 40);
    let beside = Position {
        x: bay.0 + 1,
        y: bay.1,
    };
    let away = Position { x: -4, y: -4 };

    // (what the world looks like, whether the Bay should be working)
    let states: [(&str, i32, Position, bool); 4] = [
        ("whole and working", 40, beside, false),
        ("scratched but above the line", 30, beside, false),
        ("badly hurt and in reach", 2, beside, true),
        ("badly hurt but across the base", 2, away, false),
    ];

    for (what, hp, at, expected) in states {
        game.world.entity_mut(program).remove::<Downed>();
        game.world.get_mut::<Stats>(program).unwrap().hp = hp;
        *game.world.get_mut::<Position>(program).unwrap() = at;

        admit(&mut game);
        let lit = game.recovering_programs().contains(&program);
        let before = hp_of(&game, program);
        game.run_repair_bays();
        let healed = hp_of(&game, program) > before;

        assert_eq!(lit, expected, "the mark is wrong for a program {what}");
        assert_eq!(
            lit, healed,
            "the mark and the heal disagree for a program {what}: \
             lit {lit}, healed {healed}"
        );
    }
}

/// **Permadeath is where this changes the most.** That mode never benches —
/// `bench_or_dissolve` despawns instead — so `Downed` was unreachable in it
/// and a Repair Bay was a building that could not do anything at all. The
/// threshold is the mode-independent door, so a Bay works there now.
#[test]
fn a_bay_serves_a_hurt_program_under_permadeath_too() {
    let mut game = Game::new(3509, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let (program, _bay) = a_hurt_staff_program_at_a_bay(&mut game, 2);

    admit(&mut game);
    assert!(
        game.world.get::<Downed>(program).is_some(),
        "the threshold does not consult the difficulty mode"
    );
    assert!(game.recovering_programs().contains(&program));

    let before = hp_of(&game, program);
    game.run_repair_bays();
    assert!(
        hp_of(&game, program) > before,
        "and the Bay is no longer an inert building in this mode"
    );
}

/// **`Downed` is a one-way door without a Bay to walk to**, which is the
/// right price for a program that died and quite the wrong one for a program
/// that is merely hurt — it would delete a worker from the base for the rest
/// of the run and never say so. So nothing is admitted while no Bay stands.
#[test]
fn nothing_is_admitted_while_no_bay_stands() {
    let mut game = Game::new(3510, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let program = spawn_tamed(&mut game, 40, 3);
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;

    admit(&mut game);

    assert!(
        game.world.get::<Downed>(program).is_none(),
        "with nowhere to send it, a hurt program keeps working"
    );
}

/// The roles the threshold does *not* touch. A party member is the player's
/// to mend by resting, and benching one would take it out of a fight the
/// player is in the middle of.
///
/// **Driven through `Game::tick` and not through the `admit` helper**, and
/// that is the whole difference between this test and a vacuous one: the
/// role filter is `base_staff`, one level above `admit_the_badly_hurt`, so a
/// test that built the staff list itself would be asserting against its own
/// fixture. Deleting the filter has to be able to fail this.
#[test]
fn only_base_staff_are_admitted_to_a_bay() {
    let mut game = Game::new(3511, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (program, _) = a_hurt_staff_program_at_a_bay(&mut game, 2);
    game.add_companion(program).expect("it joins the party");
    assert_eq!(game.program_role(program), Some(ProgramRole::InParty));

    for _ in 0..5 {
        game.tick();
    }

    assert!(
        game.world.get::<Downed>(program).is_none(),
        "a party member at a sliver of Integrity is the player's problem, not a Bay's"
    );
}

/// The player's decision, pinned: without a Bay a downed program stays down
/// for as long as the run lasts. Nothing else heals it, and there is no
/// timer quietly doing the job.
#[test]
fn a_downed_program_with_no_bay_standing_stays_down() {
    let mut game = Game::new(3504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(program).insert(Downed);
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;

    for _ in 0..200 {
        game.tick();
    }

    assert_eq!(game.world.get::<Stats>(program).unwrap().hp, 1);
    assert!(
        game.world.get::<Downed>(program).is_some(),
        "a wipe with no Bay standing is a wipe until one is built"
    );
}

/// `per_tick` is mod-supplied and a field named for repair must never
/// damage. Asserted through the tick rather than on `RecoveryDef::rate`
/// alone, because the clamp only means anything where it is applied.
#[test]
fn a_negative_recovery_rate_never_damages_the_program_it_names() {
    let assets = assets_dir_with_extra_structure(
        "negative_bay",
        "harm_bay.ron",
        r#"(
            id: "harm_bay", name: "Harm Bay", glyph: 'h', color: Red,
            build_cost: [("core_fragment", 1)],
            work: None,
            recovery: Some((per_tick: -5, radius: 4)),
        )"#,
    );
    let mut game = Game::new(3505, DifficultyMode::Forgiving, &assets).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    spawn_machine_at(&mut game, "harm_bay", 2, 0);
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(program).insert(Downed);
    game.world.get_mut::<Stats>(program).unwrap().hp = 4;
    *game.world.get_mut::<Position>(program).unwrap() = Position { x: 3, y: 0 };

    for _ in 0..10 {
        game.run_repair_bays();
    }

    assert_eq!(
        game.world.get::<Stats>(program).unwrap().hp,
        4,
        "a negative rate floors at zero rather than draining the program"
    );
}

/// **The door lands where the party founded from.**
///
/// The Home still stands on `BASE_EXIT_CELL` — base space has one origin
/// and one way out of it — but the anchor it is reached through is a zone
/// fixture, and pinning that to the sector's arrival point made every run's
/// base open at the same tile whatever ground the player had walked to.
#[test]
fn founding_stands_the_anchor_where_the_party_is() {
    let mut game = Game::new(3601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_player_at(&mut game, 7, -3);

    game.place_structure("home", 0, 0, None)
        .expect("founding is the one build made from the open grid");

    assert_eq!(
        game.anchor_position(),
        Some((7, -3)),
        "the anchor follows the deploy rather than staying at the arrival point"
    );
    // The claim that matters: the party can step through it from where they
    // are standing, which is what `enter_base` asks.
    game.enter_base()
        .expect("the door is underfoot the moment the Home is up");
}

/// A link is walked *onto* to descend, so an anchor sharing its tile could
/// never be stood on — the step that would reach it drops the party into the
/// Stack instead.
#[test]
fn founding_on_a_stack_link_is_refused() {
    let mut game = Game::new(3602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let link = {
        let mut query = game.world.query_filtered::<&Position, With<SurfaceLink>>();
        *query
            .iter(&game.world)
            .next()
            .expect("a fresh sector is seeded with links")
    };
    stand_player_at(&mut game, link.x, link.y);

    let err = game
        .place_structure("home", 0, 0, None)
        .expect_err("the anchor cannot share a tile with a link");
    assert!(err.contains("link"), "unexpected refusal: {err}");
    assert!(
        !game.has_home(),
        "a refused founding leaves the run without a base"
    );
}

/// **The Home is free**, and that is a floor under the run rather than a
/// discount: the wizard's Kit step can spend the whole allowance on gear, so
/// a priced Home is a run that can never open a base at all.
#[test]
fn founding_costs_nothing() {
    let mut game = Game::new(3603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.entity_mut(player).insert(Inventory::default());

    game.place_structure("home", 0, 0, None)
        .expect("an empty pack is enough to found a base");
    assert!(game.has_home(), "the Home is standing");
}

// ------------------------------------------------------- the program rule

/// `program_tier_required` is the whole of the rule: a fresh deploy always
/// wants a Mk1 program, and an upgrade wants whatever tier it raises the
/// structure to.
#[test]
fn the_tier_a_goal_demands_is_its_own_tier() {
    use crate::game::catalog::program_tier_required;

    assert_eq!(program_tier_required(BuildGoal::New), 1);
    assert_eq!(program_tier_required(BuildGoal::Upgrade { to_tier: 3 }), 3);
}

/// `programs_for_build`'s floor is `>=`, never `==`. A run whose roster has
/// outgrown zone 1 could not build at all under a strict match.
#[test]
fn a_deeper_program_still_qualifies_for_a_shallow_build() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let deep = tame_at_zone(&mut game, 5);
    // The spare is the roster floor's, not this test's: `programs_for_build`
    // offers nothing at all to a base holding one program, whatever its
    // depth, so a one-program fixture would pass this assertion's inverse
    // for a reason that has nothing to do with `>=`. Every fixture in this
    // run of tests carries one for that reason.
    tame_at_zone(&mut game, 5);

    let eligible = game.programs_for_build(1);

    assert!(
        eligible.iter().any(|p| p.entity == deep),
        "zone >= tier is a floor, not a match — a zone 5 program may raise a Mk1"
    );
}

/// The other side of the same floor: a program caught shallower than the
/// tier being raised does not qualify.
#[test]
fn a_shallow_program_does_not_qualify_for_a_deep_upgrade() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let shallow = tame_at_zone(&mut game, 2);
    // Deep enough, and there so the list is non-empty for a reason that is
    // about depth: without it the roster floor would empty this list on its
    // own and the assertion below would hold whatever `>=` did.
    let deep = tame_at_zone(&mut game, 3);

    let eligible = game.programs_for_build(3);

    assert!(!eligible.iter().any(|p| p.entity == shallow));
    assert!(
        eligible.iter().any(|p| p.entity == deep),
        "the deep one is still offered, so the list is not simply empty"
    );
}

/// The floor's edge, not just its interior: a program caught at *exactly*
/// the tier being raised must qualify. This is the modal case in play — a
/// roster at its own depth raising a structure at that same depth, a zone 1
/// program building a Mk1 — and neither test above pins it: the shallow
/// test above (zone 5 against tier 1) still passes if the floor were `>`
/// instead of `>=`, and `a_shallow_program_does_not_qualify_for_a_deep_upgrade`
/// (zone 2 against tier 3) only gets more excluded under `>`. Only equality
/// tells `>=` and `>` apart.
#[test]
fn a_program_at_exactly_the_required_zone_qualifies() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let at_depth = tame_at_zone(&mut game, 3);
    tame_at_zone(&mut game, 3); // the roster floor's spare — see above

    let eligible = game.programs_for_build(3);

    assert!(
        eligible.iter().any(|p| p.entity == at_depth),
        "a program exactly at the required zone must qualify — the floor is >=, not >"
    );
}

/// The Home is exempt twice over, and the second exemption is the one that
/// matters: it runs no job, so the rule below already frees it, but it is
/// also exempt by `category()`. A fresh run owns zero programs and one is
/// granted only as an achievements reward, so a `home.ron` edited to declare
/// `work:` must not be able to leave a new game unable to found a base.
#[test]
fn the_home_a_shelf_and_a_portal_need_no_program() {
    let game = Game::new(20260908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert!(!game.structure_needs_program(&HOME_STRUCTURE_ID.into()));
    assert!(!game.structure_needs_program(&"depot".into()));
    assert!(
        !game.structure_needs_program(&"depot_mk6".into()),
        "the whole ladder, not just the first shelf"
    );
    assert!(
        !game.structure_needs_program(&"portal".into()),
        "a doorway that is spent on the way through takes no body with it"
    );
    assert!(game.structure_needs_program(&"fabricator".into()));
    assert!(
        game.structure_needs_program(&"nothing_ships_this".into()),
        "an id with no def behind it still costs one — place_structure \
         refuses it as unknown long before the cost is asked about"
    );
}

/// **A body is spent on a machine that will take a body.** The program cost
/// follows `StructureDef::runs_a_job` — an extractor, an assembler or a rig
/// — so everything a program never stands at is free of it: a Shield, a
/// Patch Node, a Repair Bay, a Relay, a shelf, the doorway out of the
/// sector. Asserted over the shipped ids rather than by re-deriving the
/// predicate, because the point of the rule is what the *content* costs.
#[test]
fn only_a_machine_that_runs_a_job_costs_a_program() {
    let game = Game::new(20260911, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for id in [
        "mining_node",
        "research_node",
        "lathe",
        "compiler",
        "teardown_rig",
    ] {
        assert!(
            game.structure_needs_program(&id.into()),
            "{id} runs a job, so a program is what it is staffed with"
        );
    }
    for id in [
        "shield",
        "patch_node",
        "repair_bay",
        "relay",
        "data_cache",
        "sandbox",
        "recharger_node",
        "defrag_bay",
        "line_driver",
        "market",
        "contract_broker",
    ] {
        assert!(
            !game.structure_needs_program(&id.into()),
            "{id} runs no job, so there is nothing for a committed program to do"
        );
    }
}

/// The weapon in the player's hand is not a spare part — it cannot be
/// offered to a build even though it is still owned and still at depth.
///
/// **Two programs, and the assertion names both.** `programs_for_build`
/// offers nothing whatsoever to a base holding one program — the roster
/// floor, folded in so no frontend can light a menu the engine will refuse
/// — so a one-program fixture would satisfy `is_empty()` without the wield
/// having anything to do with it. The spare is what makes the list
/// non-empty for the right reason, and asserting it is *present* is what
/// keeps this test from passing on a `programs_for_build` that returned
/// nothing at all. Every exclusion test below carries the same pair.
#[test]
fn the_wielded_program_is_not_offered_to_a_build() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = spawn_tamed(&mut game, 10, 3);
    let spare = spawn_tamed(&mut game, 10, 3);
    game.wield_program(p)
        .expect("a fresh program is free to wield");

    let eligible = game.programs_for_build(1);
    assert!(
        !eligible.iter().any(|e| e.entity == p),
        "you cannot build with the thing in your hand"
    );
    assert!(
        eligible.iter().any(|e| e.entity == spare),
        "and the spare beside it is still offered, so the list is not simply empty"
    );
}

/// A program away on a sortie cannot be reached to spend on a build, so it
/// is not offered even though nothing else disqualifies it.
#[test]
fn a_sortied_program_is_not_offered_to_a_build() {
    use crate::resources::{Sortie, Sorties};

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = spawn_tamed(&mut game, 10, 3);
    let spare = spawn_tamed(&mut game, 10, 3);
    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![p]));

    let eligible = game.programs_for_build(1);
    assert!(
        !eligible.iter().any(|e| e.entity == p),
        "a program away on a sortie is not reachable to spend"
    );
    assert!(
        eligible.iter().any(|e| e.entity == spare),
        "and the one at home still is, so the list is not simply empty"
    );
}

/// `Downed` is the roster slot a wipe is meant to cost — offering it to a
/// build would let a build request quietly refund that cost.
#[test]
fn a_downed_program_is_not_offered_to_a_build() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = spawn_tamed(&mut game, 10, 3);
    let spare = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(p).insert(Downed);

    let eligible = game.programs_for_build(1);
    assert!(
        !eligible.iter().any(|e| e.entity == p),
        "a downed program is the roster slot a wipe is supposed to cost"
    );
    assert!(
        eligible.iter().any(|e| e.entity == spare),
        "and the one still standing is offered, so the list is not simply empty"
    );
}

/// Freeing — let alone despawning — a program holding a load destroys the
/// load, so a carrier is withheld from the build list entirely.
#[test]
fn a_program_carrying_goods_is_not_offered_to_a_build() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = spawn_tamed(&mut game, 10, 3);
    let spare = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(p).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 1,
    });

    let eligible = game.programs_for_build(1);
    assert!(
        !eligible.iter().any(|e| e.entity == p),
        "despawning a carrier destroys its load"
    );
    assert!(
        eligible.iter().any(|e| e.entity == spare),
        "and the empty-handed one is offered, so the list is not simply empty"
    );
}

/// The program rule may never demand a depth the tier ceiling would not
/// have let the player reach. If `upgrade_ceiling` ever loosens, this is
/// what says so out loud instead of leaving an unsatisfiable upgrade.
///
/// This calls the real `upgrade_ceiling`, not a restated copy of its
/// formula — a hardcoded `ceiling = zone` would still pass after
/// `upgrade_ceiling` itself changed, which is exactly the silent drift this
/// test exists to catch. `mining_node`'s `upgrade.max_tier` is 5
/// (`assets/structures/mining_node.ron`), matching every other shipped
/// upgradeable structure, so `upgrade_ceiling` reduces to `zone` for every
/// zone in this loop — but it is `upgrade_ceiling` computing that, not the
/// test asserting it.
#[test]
fn the_upgrade_ceiling_keeps_the_program_rule_satisfiable() {
    use crate::game::catalog::program_tier_required;

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let upgrade = game
        .world
        .resource::<StructureDb>()
        .get("mining_node")
        .unwrap()
        .upgrade
        .clone()
        .unwrap();

    for zone in 1..=5u32 {
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        let ceiling = game.upgrade_ceiling(&upgrade);
        assert!(
            program_tier_required(BuildGoal::Upgrade { to_tier: ceiling }) <= zone,
            "a Mk{ceiling} upgrade offered in zone {zone} must not need a deeper program"
        );
    }
}

// --------------------------------------------- committing and refunding

/// The commit is a retirement, not a loan: the entity is gone from the world
/// and off the roster the moment the order is filed. What comes back is the
/// snapshot, and only if the order is called off.
#[test]
fn committing_a_program_takes_it_off_the_roster() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    let before = game.owned_pets().len();

    let snapshot = game.commit_program(p).expect("committed");

    assert_eq!(game.owned_pets().len(), before - 1);
    assert!(
        game.world.get_entity(p).is_err(),
        "the entity is consumed, not parked"
    );
    assert!(snapshot.tamed, "the snapshot records what it was");
}

/// `Party` is a resource holding raw `Entity` values and it outlives the
/// entity, so a slot left pointing at a despawned program is a dangling
/// reference every battle, every roster draw and every save then reads.
#[test]
fn committing_a_party_member_clears_its_slot() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    enlist(&mut game, p);
    assert!(game.world.resource::<Party>().0.contains(&p));

    game.commit_program(p).expect("committed");

    assert!(
        !game.world.resource::<Party>().0.contains(&p),
        "no slot points at a dead entity"
    );
}

/// The wield clears itself, and this is the lock on that.
///
/// `commit_program` deliberately contains **no** explicit clear:
/// `wielded_program` filters `resources::WieldedProgram` through an
/// existence check so every despawning path inherits the immunity, and its
/// doc asks that no caller add one. That makes this a test of the pair
/// rather than of a line — it fails if that filter is ever dropped, which is
/// the moment a commit would start leaving the run swinging a despawned
/// entity. Deleting anything inside `commit_program` will not fail it, and
/// that is the correct answer, not a vacuous one.
#[test]
fn committing_the_wielded_program_unwields_it() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.wield_program(p).expect("wielded");

    game.commit_program(p).expect("committed");

    assert!(
        game.wielded_program().is_none(),
        "the run does not wield a despawned entity"
    );
}

/// The refund is a resurrection rather than a return, so "whole" is the
/// whole of the test: the name it answered to, and — load-bearing in both
/// directions — the `ProgramId` its own memories and every other program's
/// memories of it are keyed to.
#[test]
fn a_refunded_program_comes_back_whole() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.rename_companion(p, Some("Bellwether".to_string()))
        .expect("named");
    let label = game.creature_label(p);
    let id = game.world.get::<ProgramId>(p).unwrap().0;

    let snapshot = game.commit_program(p).expect("committed");
    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(game.creature_label(back), label);
    assert_eq!(
        game.world.get::<ProgramId>(back).unwrap().0,
        id,
        "a fresh id would orphan its memories and everyone else's memories of it"
    );
    assert_eq!(game.owned_pets().len(), 1, "back on the roster");
}

/// Spec test 8's other half. The snapshot is taken before the commit clears
/// the wield, so it records the roles as they were — and a cancelled order
/// that quietly disarmed the player would be a second cost the cancel never
/// advertised.
#[test]
fn a_refunded_program_is_taken_back_in_hand() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.wield_program(p).expect("wielded");

    let snapshot = game.commit_program(p).expect("committed");
    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.wielded_program(),
        Some(back),
        "the weapon a cancelled order took comes back to the hand"
    );
}

/// A weapon taken up while the order stood is not displaced by the refund:
/// the program comes back as staff rather than knocking the live wield out
/// of the player's hand.
#[test]
fn a_refunded_program_does_not_snatch_back_an_occupied_hand() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.wield_program(p).expect("wielded");
    let snapshot = game.commit_program(p).expect("committed");
    let other = tame_at_zone(&mut game, 1);
    game.wield_program(other).expect("a second weapon in hand");

    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.wielded_program(),
        Some(other),
        "the hand that is already full keeps what is in it"
    );
    assert_eq!(game.program_role(back), Some(ProgramRole::Staff));
}

/// The same rule, but the hand was filled by a weapon rather than a second
/// program: `wield_program` is the only place that unequips a weapon to make
/// room, and `refund_program` must defer to it too, or the run ends up
/// holding both a weapon and a wielded program at once — the pairing
/// `views.rs` documents as mutually exclusive.
#[test]
fn a_refunded_program_does_not_snatch_a_weapon_out_of_the_hand() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.wield_program(p).expect("wielded");
    let snapshot = game.commit_program(p).expect("committed");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(player, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .expect("a weapon takes the empty hand");

    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.wielded_program(),
        None,
        "the weapon in hand is not knocked out to make room"
    );
    assert!(
        game.player_status().weapon.is_some(),
        "the equipped weapon is still worn"
    );
    assert_eq!(game.program_role(back), Some(ProgramRole::Staff));
}

/// The party half of the same rule: a slot the order emptied is given back.
#[test]
fn a_refunded_party_member_returns_to_its_slot() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let lead = tame_at_zone(&mut game, 1);
    let p = tame_at_zone(&mut game, 1);
    enlist(&mut game, lead);
    enlist(&mut game, p);

    let snapshot = game.commit_program(p).expect("committed");
    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.world.resource::<Party>().0,
        vec![lead, back],
        "it falls back in behind the member that was ahead of it"
    );
}

/// Spec test 9. `Party` is capped at `MAX_PARTY_SIZE` and `add_companion`
/// refuses past it; a refund that pushed straight into the vec would be the
/// one door that overfills the party, and `BattleState::planned` indexes it
/// positionally.
#[test]
fn a_refunded_party_member_does_not_overfill_a_full_party() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    enlist(&mut game, p);
    let snapshot = game.commit_program(p).expect("committed");
    for _ in 0..crate::tuning::MAX_PARTY_SIZE {
        let filler = tame_at_zone(&mut game, 1);
        enlist(&mut game, filler);
    }

    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.world.resource::<Party>().0.len(),
        crate::tuning::MAX_PARTY_SIZE,
        "the cap holds"
    );
    assert_eq!(
        game.program_role(back),
        Some(ProgramRole::Staff),
        "it comes back as staff rather than as a sixth slot"
    );
}

/// Freeing a carrier drops its load and despawning it destroys it outright.
/// `programs_for_build` already withholds one, but a rule that lives only in
/// a picker is a rule a second frontend skips — so the engine door refuses
/// too, and refuses **before** anything moves.
#[test]
fn committing_a_carrying_program_is_refused() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.world.entity_mut(p).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 3,
    });

    assert!(
        game.commit_program(p).is_none(),
        "a carrier is not spendable"
    );
    assert!(
        game.world.get_entity(p).is_ok(),
        "and a refusal costs nothing"
    );
}

/// A sortied program is away and cannot be reached, and `Sorties` is the
/// third resource holding a raw `Entity` — committing one would leave a
/// squad counting down around a corpse.
#[test]
fn committing_a_sortied_program_is_refused() {
    use crate::resources::{Sortie, Sorties};

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![p]));

    assert!(game.commit_program(p).is_none());
    assert!(game.world.get_entity(p).is_ok());
}

/// `Downed` is the roster slot a wipe is meant to cost. Spending one on a
/// build and cancelling would hand it back whole, which is a repair with no
/// Repair Bay.
#[test]
fn committing_a_downed_program_is_refused() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    game.world.entity_mut(p).insert(Downed);

    assert!(game.commit_program(p).is_none());
    assert!(game.world.get_entity(p).is_ok());
}

/// Ownership is the outermost guard: a wild creature standing in the base is
/// an `Entity` like any other, and nothing about the argument type says it
/// is yours.
#[test]
fn committing_something_you_do_not_own_is_refused() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    assert!(game.commit_program(wild).is_none());
    assert!(game.world.get_entity(wild).is_ok());
}

/// The `NextProgramId` write-back. `spawn_creature_from_save` mints into its
/// context and never into the resource — `Game::load` writes it back itself
/// — so a refund that dropped its scratch context would hand out the same id
/// twice the next time one was minted.
///
/// The two halves are the whole point: a real snapshot must *not* advance the
/// counter, and a sentinel one must.
#[test]
fn a_refund_neither_reissues_nor_loses_a_program_id() {
    use crate::resources::NextProgramId;

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    let snapshot = game.commit_program(p).expect("committed");
    let counter = game.world.resource::<NextProgramId>().0;

    let back = game.refund_program(&snapshot).expect("refunded");

    assert_eq!(
        game.world.resource::<NextProgramId>().0,
        counter,
        "a snapshot carrying its own name mints nothing"
    );
    assert_eq!(
        game.world.get::<ProgramId>(back).unwrap().0,
        snapshot.program_id
    );

    // The sentinel arm — what a save written before ids existed carries.
    let mut sentinel = snapshot.clone();
    sentinel.program_id = 0;
    let minted = game.refund_program(&sentinel).expect("refunded");

    assert_eq!(game.world.get::<ProgramId>(minted).unwrap().0, counter);
    assert_eq!(
        game.world.resource::<NextProgramId>().0,
        counter + 1,
        "the id it minted is written back, not dropped with the scratch context"
    );
}

/// A committed program's post goes with the entity, and the base notices on
/// its own: occupancy is read off the live `Task` components
/// (`displace_task_holder`), never cached on the structure, so nothing is
/// left pointing at the despawned worker.
#[test]
fn committing_a_posted_program_leaves_no_task_behind() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    let target = game.player_entity();
    game.world.entity_mut(p).insert(Task {
        kind: TaskKind::Guard,
        target,
        progress: 1,
        required: 4,
    });

    game.commit_program(p).expect("committed");

    let mut tasks = game.world.query::<(Entity, &Task)>();
    assert_eq!(
        tasks.iter(&game.world).count(),
        0,
        "the posting died with the body"
    );
}

/// A refund does not re-post the program it gives back. `schedule_base_labour`
/// hands out work on the next tick, and re-inserting the snapshot's cronjob
/// into a live base could put two bodies on one machine — the exact thing
/// `displace_task_holder` exists to prevent.
#[test]
fn a_refunded_program_comes_back_unposted() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = tame_at_zone(&mut game, 1);
    let target = game.player_entity();
    game.world.entity_mut(p).insert(Task {
        kind: TaskKind::Guard,
        target,
        progress: 1,
        required: 4,
    });
    let snapshot = game.commit_program(p).expect("committed");
    assert!(
        snapshot.cronjob.is_some(),
        "the fixture really did record a posting"
    );

    let back = game.refund_program(&snapshot).expect("refunded");

    assert!(
        game.world.get::<Task>(back).is_none(),
        "the scheduler posts it again; the refund does not"
    );
}

// ------------------------------------- the two filing doors take a program

/// A base with a Home, the party inside it, and `programs` tamed programs on
/// the roster — the whole staging a build order now needs.
///
/// Seeded at zone 1 by default; a test about depth tames its own at the
/// depth it wants.
fn a_base_with_programs(seed: u32, programs: usize) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    for _ in 0..programs {
        tame_at_zone(&mut game, 1);
    }
    game
}

/// The `BuildSite` standing at the party's cell plus `(dx, dy)`, if one is.
fn filed_at(game: &mut Game, dx: i32, dy: i32) -> Option<Entity> {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    game.build_site_at(px + dx, py + dy)
}

/// The headline of the whole feature: a deploy with no program named is
/// refused, and the sentence says what is missing rather than leaving the
/// player to guess which of the build screen's rules bit them.
#[test]
fn deploying_without_a_program_is_refused_and_says_why() {
    let mut game = a_base_with_programs(20260907, 2);

    let err = game
        .place_structure("mining_node", 1, 0, None)
        .expect_err("a deploy costs a tamed program");

    assert!(
        err.contains("tamed program"),
        "the refusal names what is missing: {err}"
    );
    assert!(
        filed_at(&mut game, 1, 0).is_none(),
        "and a refused deploy files nothing"
    );
}

/// The other half: a program named is a program *spent*. It leaves the
/// roster and the order is holding it — asserted on both sides, because a
/// site that took the program without recording it would look identical
/// from the roster alone and would refund nothing on a cancel.
#[test]
fn deploying_with_a_program_files_the_order_and_spends_it() {
    let mut game = a_base_with_programs(20260907, 1);
    let spend = tame_at_zone(&mut game, 1);

    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("a program deep enough, with a body left behind");

    assert_eq!(
        game.owned_pets().len(),
        1,
        "the committed program left the roster and the spare stayed"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != spend),
        "and it is the one that was named that went"
    );
    let filed = game.build_site_programs();
    assert_eq!(filed.len(), 1, "one order stands");
    assert!(
        filed[0].is_some(),
        "and the order is holding the program it was paid with"
    );
}

/// The Home is exempt, and `structure_needs_program` is what exempts it — a
/// fresh run owns zero programs, so a Home that cost one could never be
/// founded and the run could never open a base at all.
#[test]
fn founding_a_home_needs_no_program() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.place_structure(HOME_STRUCTURE_ID, 0, 0, None)
        .expect("the Home is exempt at every tier");

    assert!(game.has_home(), "and it is standing");
}

/// A Depot is filed like any other request — the crew still raise it out of
/// its own materials — but it commits nobody. The order stands holding no
/// program at all, which is what makes `BuildSite::program` an `Option`
/// reachable for the first time: before this, only the Home was exempt and
/// the Home files no request.
#[test]
fn a_depot_is_filed_without_spending_a_program() {
    let mut game = a_base_with_programs(20260907, 2);
    let held = game.owned_pets().len();

    game.place_structure("depot", 1, 0, None)
        .expect("a shelf costs no body");

    assert_eq!(
        game.owned_pets().len(),
        held,
        "and nothing left the roster to pay for it"
    );
    assert!(filed_at(&mut game, 1, 0).is_some(), "the order stands");
    let filed = game.build_site_programs();
    assert_eq!(filed.len(), 1, "one order stands");
    assert!(
        filed[0].is_none(),
        "and it is holding no program — a Depot commits nobody"
    );
}

/// A Zone Portal commits nobody either. It is the doorway a run leaves
/// through — walked onto once and despawned by `enter_next_zone` — so a body
/// committed to it is destroyed with it, and the `BuildQuality` the
/// commitment buys is worth nothing on a structure that runs no job.
#[test]
fn a_zone_portal_is_filed_without_spending_a_program() {
    let mut game = a_base_with_programs(20260909, 2);
    let held = game.owned_pets().len();

    game.place_structure("portal", 1, 0, None)
        .expect("a doorway costs no body");

    assert_eq!(
        game.owned_pets().len(),
        held,
        "and nothing left the roster to pay for it"
    );
    let filed = game.build_site_programs();
    assert_eq!(filed.len(), 1, "one order stands");
    assert!(
        filed[0].is_none(),
        "and it is holding no program — a Portal commits nobody"
    );
}

/// The roster floor is about a body being spent, so an exempt structure is
/// past it before it is asked: a base holding its last program can still
/// stand up somewhere to put things. `a_one_program_base_may_not_spend_its_only_body`
/// is the same fixture on the other side of the exemption.
#[test]
fn a_one_program_base_may_still_build_a_depot() {
    let mut game = a_base_with_programs(20260907, 0);
    tame_at_zone(&mut game, 1);

    game.place_structure("depot", 1, 0, None)
        .expect("the roster floor never fires for a build that spends nobody");

    assert_eq!(
        game.owned_pets().len(),
        1,
        "and the one body is still on the roster"
    );
}

/// A program handed to an exempt build is *not* spent — `commit_for_build`
/// returns `Ok(None)` before it reads the offer at all. The frontend never
/// offers one, but the engine is the thing that has to be right about it: a
/// depot that quietly ate a body when a caller passed one would read as the
/// exemption not working.
#[test]
fn a_depot_offered_a_program_still_spends_nothing() {
    let mut game = a_base_with_programs(20260907, 2);
    let offered = tame_at_zone(&mut game, 1);
    let held = game.owned_pets().len();

    game.place_structure("depot", 1, 0, Some(offered))
        .expect("an exempt build accepts an offer and declines it");

    assert_eq!(
        game.owned_pets().len(),
        held,
        "the offered program is still owned"
    );
    assert!(
        game.owned_pets().iter().any(|p| p.entity == offered),
        "and it is that one, not a body of the same count"
    );
}

/// **A build order may never take the base to zero programs.** Nothing would
/// be left to fetch the materials or raise the site, so the order could
/// never finish — `build_is_workable`'s deadlock reached from the other
/// side. The refusal is about the roster, not about the program named: the
/// one offered here is perfectly eligible.
#[test]
fn a_one_program_base_may_not_spend_its_only_body() {
    let mut game = a_base_with_programs(20260907, 0);
    let only = tame_at_zone(&mut game, 1);

    let err = game
        .place_structure("mining_node", 1, 0, Some(only))
        .expect_err("the last program may not be spent");

    assert!(err.contains("last program"), "{err}");
    assert_eq!(
        game.owned_pets().len(),
        1,
        "and it is still on the roster — a refusal moves nothing"
    );
    assert!(
        filed_at(&mut game, 1, 0).is_none(),
        "and no order was filed"
    );
}

/// An upgrade demands the tier it *reaches*, not the tier the machine is at,
/// and the refusal names the depth so the player knows how deep to go.
#[test]
fn a_mk3_upgrade_refuses_a_zone_2_program() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 3);
    stock_upgrade_materials(&mut game, 60);
    upgrade_now(&mut game, node).expect("a Mk2 to upgrade from");
    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);

    let shallow = tame_at_zone(&mut game, 2);
    let held = game.owned_pets().len();

    let err = game
        .upgrade_structure(node, Some(shallow))
        .expect_err("a Mk3 wants a zone 3 program");

    assert!(
        err.contains("zone 3"),
        "the refusal names the depth needed: {err}"
    );
    assert_eq!(
        game.owned_pets().len(),
        held,
        "a refused upgrade spends nothing"
    );
}

/// The floor is `>=`, not `==`, on this door too: a program caught deeper
/// than the tier being raised is spent without complaint.
#[test]
fn a_deeper_program_may_pay_for_a_shallow_upgrade() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 1);
    let deep = tame_at_zone(&mut game, 7);

    game.upgrade_structure(node, Some(deep))
        .expect("zone 7 clears a Mk2's zone 2 floor");

    assert!(
        game.owned_pets().iter().all(|p| p.entity != deep),
        "and the deep one is what was spent"
    );
}

/// An upgrade with no program named is refused the same way a deploy is, and
/// the sentence names the mark being bought so the two doors do not read as
/// the same failure.
#[test]
fn upgrading_without_a_program_is_refused_and_says_why() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 1);
    tame_at_zone(&mut game, 1);

    let err = game
        .upgrade_structure(node, None)
        .expect_err("an upgrade costs a tamed program");

    assert!(err.contains("tamed program"), "{err}");
    assert!(
        err.contains("Mk2"),
        "and it names the mark being bought: {err}"
    );
    let pos = *game.world.get::<Position>(node).unwrap();
    assert!(
        game.build_site_at(pos.x, pos.y).is_none(),
        "and nothing was filed"
    );
}

/// The upgrade site carries the program it was paid with, exactly as a
/// deploy's does — the two goals are one rule, and a cancel has to be able
/// to hand either one back.
#[test]
fn an_upgrade_order_holds_the_program_it_was_paid_with() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 1);
    let spend = tame_at_zone(&mut game, 2);

    game.upgrade_structure(node, Some(spend))
        .expect("a zone 2 program pays for a Mk2");

    assert!(
        game.build_site_programs().iter().any(|p| p.is_some()),
        "the upgrade order is holding it"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != spend),
        "and it left the roster"
    );
}

/// **`commit_program` returning `None` is a refusal, not an absence.** A
/// downed program is deep enough, owned, and not the last body — every
/// question above the commit says yes — and the commit still refuses it,
/// because a downed slot is what a wipe is supposed to cost. Filing the
/// order anyway would raise a structure for nothing.
#[test]
fn a_program_the_commit_refuses_files_no_order() {
    let mut game = a_base_with_programs(20260907, 2);
    let downed = tame_at_zone(&mut game, 1);
    game.world.entity_mut(downed).insert(Downed);
    let held = game.owned_pets().len();

    let err = game
        .place_structure("mining_node", 1, 0, Some(downed))
        .expect_err("a downed program cannot be committed");

    assert!(
        err.contains("downed"),
        "the refusal says which state it is in: {err}"
    );
    assert!(
        filed_at(&mut game, 1, 0).is_none(),
        "and no free structure was filed on a refused commit"
    );
    assert_eq!(game.owned_pets().len(), held, "the roster is untouched");
}

/// **The refusal order is load-bearing.** A player standing where nothing
/// can be built is told about the place, not about a program they were never
/// going to spend — so the world's refusals all resolve before the program
/// block is reached, and passing no program at all does not change which
/// sentence comes back.
#[test]
fn a_bad_cell_is_reported_before_the_program_cost() {
    let mut game = a_base_with_programs(20260908, 2);
    // One step past the pocket's east edge, which is unmined rock.
    stand_in_base_at(&mut game, STARTING_POCKET_RADIUS, 0);

    let err = game
        .place_structure("mining_node", 1, 0, None)
        .expect_err("there is no floor out there");

    assert!(
        err.contains("no floor there"),
        "the place comes first, ahead of the program: {err}"
    );
}

/// The same ordering on the upgrade door: a machine already on order reads
/// as the standing request, not as a missing program, because that is the
/// errand the player can actually act on.
#[test]
fn a_standing_upgrade_request_is_reported_before_the_program_cost() {
    let mut game = Game::new(20260908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 1);
    let spend = tame_at_zone(&mut game, 2);
    game.upgrade_structure(node, Some(spend))
        .expect("the first request is filed");

    let err = game
        .upgrade_structure(node, None)
        .expect_err("one request at a time");

    assert!(
        err.contains("already on order"),
        "the standing request comes first, ahead of the program: {err}"
    );
}

/// A committed program is **gone**, not merely marked. The proof that a
/// pending order's program cannot be spent a second time is that the entity
/// it names no longer exists — so offering it to a second order is refused
/// at `commit_program`'s ownership guard rather than by any count of
/// outstanding requests.
///
/// This is the answer to the question `count_build_requests` raises. That
/// figure counts `BuildGoal::New` only, so a pending upgrade does not eat a
/// `max_deployed` slot; anything counting *committed programs* would have to
/// count both goals instead. Nothing does, and nothing needs to: the commit
/// despawns the body, so the roster is the count and it is right for both
/// goals by construction.
#[test]
fn a_program_already_committed_to_an_order_cannot_pay_for_a_second_one() {
    let mut game = Game::new(20260909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 2);
    tame_at_zone(&mut game, 2);
    let spend = tame_at_zone(&mut game, 2);
    game.upgrade_structure(node, Some(spend))
        .expect("the upgrade order takes it");

    let err = game
        .place_structure("mining_node", 2, 0, Some(spend))
        .expect_err("the same program may not pay twice");

    assert!(
        err.contains("can't be committed"),
        "it is refused for being already spent, not for being absent: {err}"
    );
    assert!(
        filed_at(&mut game, 2, 0).is_none(),
        "and no second order was filed"
    );
}

/// The deploy line names the program it took. A commit that said nothing
/// would leave the player working out which of their roster went by counting
/// the manifest afterwards.
#[test]
fn the_deploy_line_names_the_program_it_committed() {
    let mut game = a_base_with_programs(20260909, 1);
    let spend = tame_at_zone(&mut game, 1);
    let who = game.creature_label(spend);

    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");

    assert!(
        game.message_history(50)
            .iter()
            .any(|e| e.text.contains(&who) && e.text.contains("Mining Node")),
        "the log names both the structure and the program: {:?}",
        game.message_history(50)
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>()
    );
}

// ------------------------------ the two destruction doors give it back

/// The refund half of the mechanic, and the door the player reaches for.
/// The commit is a *retirement*, so an order the player changes their mind
/// about would destroy a program outright without this — a rob rather than
/// a refund, and one that reads as the picker having eaten the roster.
#[test]
fn calling_off_an_order_gives_the_program_back() {
    let mut game = a_base_with_programs(20260907, 1);
    let spend = tame_at_zone(&mut game, 1);
    game.rename_companion(spend, Some("Bellwether".to_string()))
        .expect("named");

    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");
    assert_eq!(game.owned_pets().len(), 1, "the order is holding it");

    let site = filed_at(&mut game, 1, 0).expect("a site");
    game.cancel_build_request(site).expect("cancelled");

    let back = game.owned_pets();
    assert_eq!(back.len(), 2, "the committed program is back on the roster");
    assert!(
        back.iter().any(|p| p.name.contains("Bellwether")),
        "and it is the one that was named that came back: {:?}",
        back.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
    );
}

/// The cancel says so, in the base log. A program that reappeared on the
/// roster with nothing said would leave the player counting the manifest to
/// find out whether the cancel had cost them anything — the same reason the
/// deploy line names what it took.
#[test]
fn the_cancel_line_names_the_program_it_gave_back() {
    let mut game = a_base_with_programs(20260907, 1);
    let spend = tame_at_zone(&mut game, 1);
    let who = game.creature_label(spend);
    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");

    let site = filed_at(&mut game, 1, 0).expect("a site");
    game.cancel_build_request(site).expect("cancelled");

    assert!(
        game.message_history(50)
            .iter()
            .any(|e| e.text.contains(&who) && e.text.contains("comes back")),
        "the log names the program the cancel handed back: {:?}",
        game.message_history(50)
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>()
    );
}

/// The second door, called directly. `clear_pending_build_at` warns in its
/// own doc that nothing fails to compile when only one of the two is wired,
/// and this is the assertion that makes that true of the program as well as
/// of the materials.
#[test]
fn a_site_wiped_with_its_cell_gives_the_program_back_too() {
    let mut game = a_base_with_programs(20260907, 1);
    let spend = tame_at_zone(&mut game, 1);
    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");
    let (px, py) = game.base_pos().expect("the fixture stands in the base");

    game.clear_pending_build_at(px + 1, py);

    assert_eq!(
        game.owned_pets().len(),
        2,
        "the second door refunds like the first"
    );
}

/// And the door as a player actually reaches it: demolishing the machine an
/// upgrade was filed against takes the request with it, so the program the
/// request was paid with has to come back the same way a cancel's does.
/// Reached through `remove_structure` rather than by calling the helper,
/// because a consequence gated behind a path nothing walks is green and
/// unreachable at once.
#[test]
fn demolishing_a_machine_under_upgrade_gives_its_program_back() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    set_zone(&mut game, 2);
    tame_at_zone(&mut game, 1);
    let spend = tame_at_zone(&mut game, 2);
    game.upgrade_structure(node, Some(spend))
        .expect("a zone 2 program pays for a Mk2");
    let held = game.owned_pets().len();

    game.remove_structure(node).expect("demolished");

    assert_eq!(
        game.owned_pets().len(),
        held + 1,
        "the cell going out from under the order hands the program back"
    );
}

/// The other end of the lifecycle, and the reason `consume_site` needs no
/// change: a build that *finishes* has spent the program. The site despawns
/// and the snapshot goes with it, and demolishing the machine afterwards
/// refunds materials — never a program, which would make a deploy-and-scrap
/// loop free.
#[test]
fn a_finished_structure_never_gives_the_program_back() {
    let mut game = a_base_with_programs(20260907, 0);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    // Tough enough to outlast the ambient GC Entropy Sweeps these ticks run
    // through — `tests::construction::builder`'s reason.
    spawn_tamed(&mut game, 500, 3);
    let spend = tame_at_zone(&mut game, 1);

    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");
    assert_eq!(game.owned_pets().len(), 1, "the order is holding it");

    for _ in 0..400 {
        if filed_at(&mut game, 1, 0).is_none() {
            break;
        }
        game.tick();
    }
    let built = find_structure_by_kind(&mut game, "mining_node").expect("the crew raises it");
    assert_eq!(game.owned_pets().len(), 1, "spent at completion");

    game.remove_structure(built).expect("demolished");

    assert_eq!(
        game.owned_pets().len(),
        1,
        "deconstruction returns materials, not programs"
    );
}

/// The one way a refund can fail, and it must not fail quietly.
///
/// A snapshot naming a species whose `.ron` has been deleted between
/// sessions cannot be respawned — `refund_program` answers `None` rather
/// than panicking, because deleting a species file is a supported thing to
/// do to an install. What is not supported is a program going off the roster
/// with no line anywhere accounting for it, which would read as the cancel
/// having eaten it.
#[test]
fn a_program_that_cannot_be_restored_is_said_rather_than_lost_quietly() {
    let mut game = a_base_with_programs(20260907, 1);
    let spend = tame_at_zone(&mut game, 1);
    game.rename_companion(spend, Some("Bellwether".to_string()))
        .expect("named");
    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");
    let site = filed_at(&mut game, 1, 0).expect("a site");
    // The install loses the species out from under the standing order.
    game.world
        .get_mut::<BuildSite>(site)
        .unwrap()
        .program
        .as_mut()
        .unwrap()
        .species = "a_species_this_install_never_shipped".to_string();

    game.cancel_build_request(site)
        .expect("a cancel does not take the run down over it");

    assert_eq!(
        game.owned_pets().len(),
        1,
        "there is nothing left to rebuild it from"
    );
    assert!(
        game.message_history(50)
            .iter()
            .any(|e| e.text.contains("Bellwether") && e.text.contains("does not come back")),
        "and the log says so, by name: {:?}",
        game.message_history(50)
            .iter()
            .map(|e| e.text.clone())
            .collect::<Vec<_>>()
    );
}
