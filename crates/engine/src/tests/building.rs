//! Placing, removing, upgrading, and describing structures, and the base platform they sit on.

use super::support::*;
use crate::tuning::{MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_RADIUS_TILES, haul_walk_radius};
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
        (MAX_BUILD_DISTANCE_FROM_HOME, 0),
        (0, -MAX_BUILD_DISTANCE_FROM_HOME),
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

/// The slab is a chamfered box, not the square it was until the corners were
/// cut: `PLATFORM_CORNER_CUT` diagonal steps come off each of the four, so
/// the corner tile and the two beside it are natural terrain. Checked at all
/// four corners because `Platform::covers` works in absolute values and a
/// sign error would round three of them and leave one square.
#[test]
fn the_base_slab_has_its_corners_cut() {
    let mut game = Game::new(925, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_home(&mut game, 0, 1);
    let (hx, hy) = (ppos.x, ppos.y + 1);
    let r = MAX_BUILD_DISTANCE_FROM_HOME;

    let mut map = game.world.resource_mut::<WorldMap>();
    for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        for (dx, dy) in [(r, r), (r - 1, r), (r, r - 1)] {
            let (dx, dy) = (dx * sx, dy * sy);
            assert_ne!(
                map.tile(hx + dx, hy + dy).biome,
                Biome::Platform,
                "({dx}, {dy}) is inside the cut corner and should be natural terrain"
            );
        }
        // The tiles the cut stops at, so a deeper chamfer can't pass by
        // asserting only on what was removed.
        for (dx, dy) in [(r - 2, r), (r - 1, r - 1), (r, r - 2)] {
            let (dx, dy) = (dx * sx, dy * sy);
            assert_eq!(
                map.tile(hx + dx, hy + dy).biome,
                Biome::Platform,
                "({dx}, {dy}) is the first tile past the cut and should be platform floor"
            );
        }
    }
}

/// A save written before the corners were cut keeps its square slab, because
/// the cut happens when the floor is *stamped* and `Game::load` restores a
/// zone map verbatim. Breaching is what repairs it: `enter_next_zone` stamps
/// a fresh slab at the new spawn point through the same `Platform::covers`,
/// onto a newly generated map whose override overlay is empty.
///
/// This is why a legacy square base needs no migration, and the claim is
/// worth pinning: the alternative on offer was a `savetool` pass rewriting
/// every existing save.
#[test]
fn breaching_recuts_the_corners_of_a_legacy_square_slab() {
    let mut game = Game::new(927, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    let home = game.home_position().expect("the Home was just placed");
    let r = MAX_BUILD_DISTANCE_FROM_HOME;
    let corners = |cx: i32, cy: i32| {
        let mut out = Vec::new();
        for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            for (dx, dy) in [(r, r), (r - 1, r), (r, r - 1)] {
                out.push((cx + dx * sx, cy + dy * sy));
            }
        }
        out
    };

    // The legacy state: floor painted over the twelve tiles the cut removes.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for (x, y) in corners(home.x, home.y) {
            map.set_override(
                x,
                y,
                Tile {
                    biome: Biome::Platform,
                    walkable: true,
                },
            );
        }
    }
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for (x, y) in corners(home.x, home.y) {
            assert_eq!(
                map.tile(x, y).biome,
                Biome::Platform,
                "({x}, {y}) should be square legacy floor before the breach"
            );
        }
    }

    game.enter_next_zone();

    let moved = game
        .home_position()
        .expect("the base travels rather than being despawned");
    let mut map = game.world.resource_mut::<WorldMap>();
    // The positive half, and not belt-and-braces: a fresh zone map carries no
    // overrides at all, so the corner assertions below would pass just as
    // happily if the breach had stamped no slab whatsoever.
    for (dx, dy) in [(0, 0), (r, 0), (0, -r), (r - 2, r), (r - 1, r - 1)] {
        assert_eq!(
            map.tile(moved.x + dx, moved.y + dy).biome,
            Biome::Platform,
            "({dx}, {dy}) from the travelled Home should be freshly stamped floor"
        );
    }
    for (x, y) in corners(moved.x, moved.y) {
        assert_ne!(
            map.tile(x, y).biome,
            Biome::Platform,
            "({x}, {y}) is a cut corner and the breach should have left it natural"
        );
    }
}

/// The cut is footprint, not paint: `place_structure` measures against the
/// same `Platform::covers`, so a tile with no floor under it has nothing
/// standing on it either. Without this the build box stays square and a
/// machine can hang off the rounded corner onto wild ground.
#[test]
fn a_cut_corner_is_not_buildable() {
    let mut game = Game::new(926, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 40);
    place_home(&mut game, 0, 0);
    let r = MAX_BUILD_DISTANCE_FROM_HOME;

    let err = game
        .place_structure("armory", r, r)
        .expect_err("the corner tile is off the slab and shouldn't be buildable");
    assert!(err.contains("Too far from Home"), "unexpected error: {err}");

    // Diagonally in by one, which is the first tile the chamfer leaves —
    // and the assertion that stops the cut being fixed by shrinking the
    // whole build box.
    game.place_structure("armory", r - 1, r - 1)
        .expect("the tile just inside the cut is slab and should be buildable");
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

/// A refusal for want of materials is the one build refusal the player has to
/// go and *do* something about, so it goes in the base log rather than living
/// only in the status line, which ages out after `STATUS_LINE_SECONDS` while
/// they are looking at the map. It names the shortfall for the same reason:
/// "not enough" without a number sends them back to the build menu to work
/// out what they were short of.
#[test]
fn deploying_without_the_materials_logs_the_shortfall() {
    let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    place_home(&mut game, -1, 0);

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
    unlock_research_chain(&mut game, "armor_bench");
    place_home(&mut game, 0, 1);
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
    let reach = recharger_reach(&game);
    spawn_recharger_node(&mut game, reach + 1, 0);

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
    set_zone(&mut game, 2);
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
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1000);

    set_zone(&mut game, 2);
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
    let node = deploy_upgradeable_node(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    set_zone(&mut game, 3);
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
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let mut pos = game.world.get_mut::<Position>(player).unwrap();
    pos.x = node_pos.x + 6;
    pos.y = node_pos.y;

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
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();
    let node_pos = *game.world.get::<Position>(node).unwrap();

    {
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x = node_pos.x + 1;
        pos.y = node_pos.y + 1;
    }
    assert!(
        game.work_structure(node).is_err(),
        "a diagonal is not a tile the node can be collected from"
    );

    {
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x = node_pos.x + 1;
        pos.y = node_pos.y;
    }
    game.work_structure(node)
        .expect("standing on one of the node's four station tiles works it");
    assert!(
        game.world.get::<Task>(player).is_some(),
        "or the refusal is blanket rather than about reach"
    );
}

/// Spawns a workable structure with a full node, away from anything else.
fn workable_structure(game: &mut Game, x: i32, y: i32) -> Entity {
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
    let structure = workable_structure(&mut game, 3, 4);
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
    let structure = workable_structure(&mut game, 3, 4);
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
    let structure = workable_structure(&mut game, 3, 4);
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
    let structure = workable_structure(&mut game, 3, 4);
    // Where the player is standing when they post it: at the machine's east
    // station tile, so the assignment turns on nothing but the two positions.
    let player_pos = Position { x: 4, y: 4 };
    *game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap() = player_pos;
    // Tamed far enough out that the old rule would have refused it outright,
    // and far enough that it could never have walked in.
    let reach = haul_walk_radius(game.build_radius());
    let worker = spawn_tamed_on_map(&mut game, 3, 4 + reach + 5);

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
    let structure = workable_structure(&mut game, 3, 4);
    let worker = spawn_tamed_on_map(&mut game, 3, 3);
    let reach = haul_walk_radius(game.build_radius());
    *game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap() = Position {
        x: 3,
        y: 4 + reach + 5,
    };

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
    let structure = workable_structure(&mut game, 3, 4);
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
    let node = workable_structure(&mut game, 3, 4);
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

/// The base slab is the one safe ground, and `wander_ai_system` was the one
/// mover that did not honour it: it checked `walkable` alone, so an ordinary
/// wild program could stroll onto a base a *pursuing* nest guardian was
/// forbidden to enter (`pursuit_field`). Both now read
/// `Tile::open_to_hostiles`.
///
/// Placed one tile off the slab edge and ticked, rather than relying on the
/// spawn-side test to catch it: that one only noticed because a seeded
/// program happened to be adjacent, which is luck rather than coverage.
#[test]
fn a_wild_program_will_not_wander_onto_the_base_slab() {
    let mut game = Game::new(925, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);

    // Just outside the slab, so every tick offers it a step onto platform
    // floor and nothing else is needed to tempt it in.
    let edge = MAX_BUILD_DISTANCE_FROM_HOME + 1;
    let species = game.species_defs().into_iter().next().unwrap().id;
    let wanderer = game.spawn_wild_creature(&species, 0, edge).unwrap();

    for _ in 0..200 {
        game.tick();
        // A tick can legitimately end the creature (a cull, a fight), and a
        // despawned program proves nothing either way.
        let Some(pos) = game.world.get::<Position>(wanderer).copied() else {
            return;
        };
        let biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        assert_ne!(
            biome,
            Biome::Platform,
            "a wild program wandered onto the base slab at ({}, {})",
            pos.x,
            pos.y
        );
    }
}

/// A structure whose only job is to widen the slab — the shape the Heap
/// Pillar ships in, written here as a mod so the derivation is under test
/// before any shipped asset sets the field.
const WIDENING_PILLAR: &str = r#"(
    id: "test_pillar",
    name: "Test Pillar",
    description: "Widens the base by one tile.",
    glyph: 'I',
    color: Cyan,
    build_cost: [("core_fragment", 1)],
    work: None,
    raidable: false,
    build_radius_bonus: 1,
)"#;

/// A bonus that lands a base on `GROWN_RADIUS` in one structure, so a
/// fixture wanting a grown base does not have to count out six Pillars.
const GROWN_PILLAR: &str = r#"(
    id: "test_grown_pillar",
    name: "Test Grown Pillar",
    description: "Widens the base to a realistic grown size.",
    glyph: 'I',
    color: Cyan,
    build_cost: [("core_fragment", 1)],
    work: None,
    raidable: false,
    build_radius_bonus: 6,
)"#;

/// The same, with a bonus far past the ceiling — the clamp is what is under
/// test, not how many structures fit on a slab.
const HUGE_PILLAR: &str = r#"(
    id: "test_huge_pillar",
    name: "Test Huge Pillar",
    description: "Widens the base absurdly.",
    glyph: 'I',
    color: Cyan,
    build_cost: [("core_fragment", 1)],
    work: None,
    raidable: false,
    build_radius_bonus: 99,
)"#;

fn game_with_pillar(tag: &str, body: &str) -> Game {
    let dir = assets_dir_with_extra_structure(tag, "test_pillar.ron", body);
    let game = Game::new(700, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    game
}

/// Spawns `kind` as a deployed structure at `(x, y)` without going through
/// `place_structure`, which would refuse anything outside the very radius
/// under test.
fn spawn_structure_of(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: StructureId::from(kind),
            },
            Position { x, y },
        ))
        .id()
}

#[test]
fn a_base_with_nothing_deployed_is_the_starting_radius() {
    let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.build_radius(),
        MAX_BUILD_DISTANCE_FROM_HOME,
        "a fresh run builds at the starting radius"
    );
}

#[test]
fn each_deployed_widening_structure_adds_its_bonus() {
    let mut game = game_with_pillar("build_radius_sum", WIDENING_PILLAR);
    spawn_structure_of(&mut game, "test_pillar", 1, 0);
    spawn_structure_of(&mut game, "test_pillar", 2, 0);

    assert_eq!(
        game.build_radius(),
        MAX_BUILD_DISTANCE_FROM_HOME + 2,
        "the bonus stacks additively across deployed structures"
    );
}

#[test]
fn the_build_radius_clamps_at_its_ceiling() {
    let mut game = game_with_pillar("build_radius_clamp", HUGE_PILLAR);
    spawn_structure_of(&mut game, "test_huge_pillar", 1, 0);

    assert_eq!(
        game.build_radius(),
        MAX_BUILD_RADIUS_TILES,
        "no amount of bonus takes a base past the ceiling"
    );
}

/// The claim that buys the whole design: because the radius is derived from
/// deployed structures rather than stored, it comes back on load with no
/// save-format change at all — this must pass at the current
/// `SAVE_FORMAT_VERSION`.
#[test]
fn a_widened_footprint_survives_a_save_and_load() {
    let dir =
        assets_dir_with_extra_structure("build_radius_save", "test_pillar.ron", WIDENING_PILLAR);
    let mut game = Game::new(701, DifficultyMode::Forgiving, &dir).unwrap();
    place_home(&mut game, 0, 0);
    spawn_structure_of(&mut game, "test_pillar", 1, 0);
    let home = game.home_position().expect("the fixture just placed one");
    game.stamp_platform(home.x, home.y);
    let before = game.world.resource::<Platform>().radius;
    assert_eq!(
        before,
        MAX_BUILD_DISTANCE_FROM_HOME + 1,
        "precondition: the Pillar widened the live footprint"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_build_radius_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        loaded.world.resource::<Platform>().radius,
        before,
        "the radius is rediscovered from the structures the save already carries"
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

/// A base grown to `GROWN_RADIUS`, with the Pillar's bonus taken in one
/// structure rather than counted out one at a time.
fn fully_grown_base(tag: &str, seed: u32) -> (Game, ScratchAssets) {
    let dir = assets_dir_with_extra_structure(tag, "test_pillar.ron", GROWN_PILLAR);
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap();
    place_home(&mut game, 0, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    spawn_structure_of(&mut game, "test_grown_pillar", 1, 0);
    let home = game.home_position().expect("the fixture just placed one");
    game.stamp_platform(home.x, home.y);
    assert_eq!(
        game.world.resource::<Platform>().radius,
        GROWN_RADIUS,
        "precondition: the base is grown well past its start"
    );
    (game, dir)
}

/// A posting the cronjob menu accepts has to be a posting that arrives, and
/// the walk that delivers it is bounded by a radius. Left on the old
/// constant, a fully grown base refuses postings across its own width — a
/// machine you can see from your Home and cannot staff.
#[test]
fn a_program_walks_across_a_fully_grown_base_to_its_post() {
    let (mut game, dir) = fully_grown_base("grown_base_walk", 702);
    let r = GROWN_RADIUS;
    game.place_structure("mining_node", r, 0).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let node = game
        .find_blocking_structure_at(ppos.x + r, ppos.y)
        .expect("the node was just deployed");
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let worker = spawn_tamed(&mut game, 500, 3);

    // Posted from the opposite edge of the slab: the whole width of the base
    // separates the program from its machine.
    stand_player_at(&mut game, ppos.x - r, ppos.y);
    game.assign_cronjob(worker, node).unwrap();

    for _ in 0..200 {
        if game::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos) {
            break;
        }
        game.tick();
    }
    assert!(
        game::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos),
        "a worker posted from one edge of a full-size base must reach the other"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Halving the starting radius put every existing save in the position a
/// pre-corner-cut save was already in: a slab in `tile_overrides` wider than
/// anything the current shape claims. Demolishing the Home has to take all
/// of it, or the run keeps a ring of orphan floor around nothing forever.
#[test]
fn demolishing_a_home_clears_floor_wider_than_the_current_radius() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    let home = game.home_position().expect("the fixture just placed one");
    // The slab a save written before the halving carries, laid straight
    // into the map because `stamp_platform` derives its own radius and can
    // no longer produce one this wide.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dy in -MAX_BUILD_RADIUS_TILES..=MAX_BUILD_RADIUS_TILES {
            for dx in -MAX_BUILD_RADIUS_TILES..=MAX_BUILD_RADIUS_TILES {
                map.set_override(
                    home.x + dx,
                    home.y + dy,
                    Tile {
                        biome: Biome::Platform,
                        walkable: true,
                    },
                );
            }
        }
    }

    game.clear_platform();

    let mut map = game.world.resource_mut::<WorldMap>();
    for dy in -MAX_BUILD_RADIUS_TILES..=MAX_BUILD_RADIUS_TILES {
        for dx in -MAX_BUILD_RADIUS_TILES..=MAX_BUILD_RADIUS_TILES {
            assert_ne!(
                map.tile(home.x + dx, home.y + dy).biome,
                Biome::Platform,
                "orphan floor left at ({dx}, {dy})"
            );
        }
    }
}

/// A base with the Heap Pillar researched and the fragments to buy a few.
fn base_ready_for_pillars(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    game.world
        .resource_mut::<Research>()
        .0
        .insert("heap_allocation".to_string());
    game
}

#[test]
fn a_heap_pillar_lays_floor_one_tile_past_the_old_edge() {
    let mut game = base_ready_for_pillars(710);
    let home = game.home_position().expect("the fixture just placed one");
    let edge = MAX_BUILD_DISTANCE_FROM_HOME;
    assert_ne!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(home.x + edge + 1, home.y)
            .biome,
        Biome::Platform,
        "precondition: the ring beyond the edge is still wild ground"
    );

    game.place_structure("heap_pillar", 1, 0).unwrap();

    assert_eq!(
        game.world.resource::<Platform>().radius,
        edge + 1,
        "a Pillar widens the live footprint"
    );
    assert_eq!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(home.x + edge + 1, home.y)
            .biome,
        Biome::Platform,
        "and the new ring gets floor, so it can be built on"
    );
}

#[test]
fn the_new_ring_is_buildable_and_the_one_past_it_is_not() {
    let mut game = base_ready_for_pillars(711);
    game.place_structure("heap_pillar", 1, 0).unwrap();
    let edge = MAX_BUILD_DISTANCE_FROM_HOME + 1;

    game.place_structure("mining_node", edge, 0)
        .expect("the ring a Pillar just laid is base ground like any other");
    let err = game
        .place_structure("mining_node", edge + 1, 0)
        .expect_err("one tile past the new edge is still outside the base");
    assert!(
        err.contains("Too far from Home"),
        "unexpected refusal: {err}"
    );
}

/// The refusal has to come before anything is spent — the same ordering
/// `install_routine` keeps between checking knowledge and taking the disk.
/// Asserting the refusal alone would pass against a build that charged for
/// it, which is the half that matters.
#[test]
fn a_pillar_whose_new_ring_holds_a_link_is_refused_and_costs_nothing() {
    let mut game = base_ready_for_pillars(712);
    let home = game.home_position().expect("the fixture just placed one");
    game.world.spawn((
        SurfaceLink,
        Position {
            x: home.x + MAX_BUILD_DISTANCE_FROM_HOME + 1,
            y: home.y,
        },
    ));
    let before = count_item(&game, ids::CORE_FRAGMENT);

    let err = game
        .place_structure("heap_pillar", 1, 0)
        .expect_err("growing the base over a link would swallow it");

    assert!(err.contains("link"), "unexpected refusal: {err}");
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        before,
        "a refused build must not have spent anything"
    );
    assert_eq!(
        game.world.resource::<Platform>().radius,
        MAX_BUILD_DISTANCE_FROM_HOME,
        "and must not have widened the base either"
    );
}

/// Growth is irreversible, which is what removes the whole shrink question:
/// no orphaned outer structures, no partial `clear_platform`, no state the
/// build rules say is impossible. The Home cascade is the one exception,
/// because there the base is coming down entirely.
#[test]
fn a_pillar_cannot_be_demolished_except_with_its_home() {
    let mut game = base_ready_for_pillars(713);
    game.place_structure("heap_pillar", 1, 0).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let pillar = game
        .find_blocking_structure_at(ppos.x + 1, ppos.y)
        .expect("the Pillar was just deployed");

    game.remove_structure(pillar)
        .expect_err("a Pillar cannot come down on its own");
    assert!(
        game.world.get::<Structure>(pillar).is_some(),
        "the refused demolition left it standing"
    );

    let home = game
        .find_blocking_structure_at(ppos.x, ppos.y)
        .expect("the Home is on the player's tile");
    game.remove_structure(home)
        .expect("demolishing the Home cascades over everything, Pillars included");
    assert!(
        game.world.get::<Structure>(pillar).is_none(),
        "the cascade must take the Pillar with it"
    );
    assert_eq!(
        game.world.resource::<Platform>().radius,
        MAX_BUILD_DISTANCE_FROM_HOME,
        "no Home means no slab, so the radius resets"
    );
}

/// Breaching despawns no structures — the base travels, repositioned around
/// the new spawn point — so the Pillars travel with it and `enter_next_zone`
/// re-stamps at the right size with no code of its own. This asserts the
/// consequence rather than the mechanism, which is what would catch a later
/// change that starts rebuilding the slab from the constant.
#[test]
fn a_widened_base_breaches_at_the_size_it_had() {
    let mut game = base_ready_for_pillars(714);
    game.place_structure("heap_pillar", 1, 0).unwrap();
    game.place_structure("heap_pillar", 2, 0).unwrap();
    let grown = MAX_BUILD_DISTANCE_FROM_HOME + 2;
    assert_eq!(game.world.resource::<Platform>().radius, grown);

    game.enter_next_zone();

    assert_eq!(
        game.world.resource::<Platform>().radius,
        grown,
        "the base arrived in the next sector smaller than it left"
    );
    let home = game
        .home_position()
        .expect("the base travels rather than being rebuilt");
    assert_eq!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(home.x + grown, home.y)
            .biome,
        Biome::Platform,
        "and the new zone's slab was stamped at the grown size"
    );
}

/// A shipped Pillar's radius through the real save path, where the modded
/// fixture above proves the derivation and this proves the asset.
#[test]
fn a_shipped_pillars_width_survives_a_save_and_load() {
    let mut game = base_ready_for_pillars(715);
    game.place_structure("heap_pillar", 1, 0).unwrap();
    let before = game.world.resource::<Platform>().radius;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_heap_pillar_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.world.resource::<Platform>().radius, before);
}

/// The slab always covers every structure standing on it.
///
/// Inert for any base built under the current rules — `place_structure`
/// refuses anything outside the footprint, so the outermost structure is
/// never further out than the radius already is. What it is for is a base
/// built under *older* rules: halving the starting radius left every
/// existing save with a 15x15 slab of stamped floor against a buildable 9x9,
/// which is 156 tiles that look exactly like base and refuse to be built on,
/// with the player standing on one of them.
///
/// It has to live in `build_radius` rather than in the load path, because
/// `enter_next_zone` re-stamps from that derivation — a load-path fix would
/// hand the base back and take it away again at the next breach.
#[test]
fn the_slab_covers_the_structures_standing_on_it() {
    let mut game = base_ready_for_pillars(716);
    let home = game.home_position().expect("the fixture just placed one");
    let reach = MAX_BUILD_DISTANCE_FROM_HOME + 3;
    // Spawned rather than deployed: `place_structure` would refuse the very
    // position a legacy save is full of.
    spawn_structure_of(&mut game, "data_cache", home.x + reach, home.y);

    assert_eq!(
        game.build_radius(),
        reach,
        "a base is at least as wide as the structures standing on it"
    );

    game.enter_next_zone();
    assert_eq!(
        game.world.resource::<Platform>().radius,
        reach,
        "and it stays that wide through a breach, where the re-stamp happens"
    );
}

/// The floor term never lifts a base past its ceiling, and it is a floor
/// rather than a second budget: `max(start + bonuses, outermost)` cannot
/// grow a base by building an ordinary structure at its own edge, because
/// there `outermost` is only ever equal to the radius already in force.
///
/// The cost, which is real and worth knowing: on a base already wider than
/// the starting radius, a Pillar buys nothing until the bonuses have caught
/// up with the width it came in at. Only a save written before the starting
/// radius was halved can be in that position, and it corrects itself.
#[test]
fn the_structure_floor_is_a_floor_and_not_a_second_budget() {
    let mut game = base_ready_for_pillars(717);
    let home = game.home_position().expect("the fixture just placed one");
    spawn_structure_of(
        &mut game,
        "data_cache",
        home.x + MAX_BUILD_RADIUS_TILES + 50,
        home.y,
    );
    assert_eq!(
        game.build_radius(),
        MAX_BUILD_RADIUS_TILES,
        "nothing takes a base past its ceiling, however far out a structure sits"
    );

    let mut game = base_ready_for_pillars(718);
    let home = game.home_position().expect("the fixture just placed one");
    let legacy = MAX_BUILD_DISTANCE_FROM_HOME + 2;
    spawn_structure_of(&mut game, "data_cache", home.x + legacy, home.y);

    game.place_structure("heap_pillar", 1, 0).unwrap();
    assert_eq!(
        game.build_radius(),
        legacy,
        "a Pillar is absorbed while the bonuses are still under the width \
         the base came in at"
    );
    game.place_structure("heap_pillar", 2, 0).unwrap();
    game.place_structure("heap_pillar", 3, 0).unwrap();
    assert_eq!(
        game.build_radius(),
        legacy + 1,
        "and starts paying again the moment they pass it"
    );
}

/// Building an ordinary structure at the very edge must not widen the base.
#[test]
fn a_structure_at_the_edge_does_not_grow_the_base() {
    let mut game = base_ready_for_pillars(719);
    let before = game.build_radius();
    game.place_structure("data_cache", MAX_BUILD_DISTANCE_FROM_HOME, 0)
        .expect("the slab edge is buildable ground");
    assert_eq!(
        game.build_radius(),
        before,
        "the floor term is a floor, not a ratchet you can walk outward"
    );
}

/// The link refusal has to ask what the radius would actually *become*, not
/// assume the bonus lands whole.
///
/// On a base already wider than the starting radius the bonus is absorbed,
/// so the ring the refusal was scanning is ground the base was never going
/// to claim — and a link out there refused a Pillar that would not have
/// touched it. Measured on the `chains` template: radius 7, a link at
/// (6, -8) eight tiles out, and every placement refused.
#[test]
fn a_link_past_a_pillars_actual_reach_does_not_refuse_it() {
    let mut game = base_ready_for_pillars(720);
    let home = game.home_position().expect("the fixture just placed one");
    let legacy = MAX_BUILD_DISTANCE_FROM_HOME + 2;
    spawn_structure_of(&mut game, "data_cache", home.x + legacy, home.y);
    // Re-stamped because that is what the load path does: the cached radius
    // the refusal reads is otherwise still the one from Home placement, and
    // the test would pass against the bug.
    game.stamp_platform(home.x, home.y);
    assert_eq!(
        game.world.resource::<Platform>().radius,
        legacy,
        "precondition: a legacy-wide base"
    );
    // One tile beyond the widest this base could be after a single Pillar,
    // whose bonus this base absorbs entirely.
    game.world.spawn((
        SurfaceLink,
        Position {
            x: home.x + legacy + 1,
            y: home.y,
        },
    ));

    game.place_structure("heap_pillar", 1, 0)
        .expect("a link outside the ground the base would claim is not in the way");
}
