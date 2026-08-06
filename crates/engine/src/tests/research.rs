//! The research tree and the recipes and structures it gates.

use super::support::*;
use crate::*;

#[test]
fn a_cronjob_worker_fills_the_unbounded_buffer_past_the_old_cap() {
    let mut game = Game::new(708, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = assign_worker_producing(&mut game, ItemId::from(ids::CORE_FRAGMENT));
    let before = node_output(&game, node, ids::CORE_FRAGMENT);
    let carried = held(&game, &ItemId::from(ids::CORE_FRAGMENT));

    for _ in 0..100 {
        game.tick();
    }

    assert!(
        node_output(&game, node, ids::CORE_FRAGMENT) > before,
        "a working cronjob keeps producing — its buffer is what bounds it, not the old cap"
    );
    // The contrast that makes the banked tests below mean something: ordinary
    // salvage still has to be walked over to and collected.
    assert_eq!(
        held(&game, &ItemId::from(ids::CORE_FRAGMENT)),
        carried,
        "unbanked salvage must stay in the buffer, not reach the player's cargo"
    );
}

/// Where a banked payout lands, and the whole of what `ItemDef::banked`
/// buys. A unit that reached the node's buffer would be back on the collect
/// key and inside a neighbouring machine's pull range.
#[test]
fn a_research_cronjob_banks_straight_to_the_player() {
    let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = assign_worker_producing(&mut game, ItemId::from(ids::RESEARCH_DATA));
    let before = research_data_held(&game);

    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_data_held(&game) > before,
        "a research cronjob must bank over time (was {before}, now {})",
        research_data_held(&game)
    );
    assert_eq!(
        node_output(&game, node, ids::RESEARCH_DATA),
        0,
        "a banked resource must never reach the node's own output buffer"
    );
}

/// The player working the node by hand delivers by the same rule. The two
/// paths share `deliver_payout` precisely so this cannot drift — a test
/// covering only the cronjob would not notice a second copy.
#[test]
fn the_player_working_a_research_node_banks_it_too() {
    let mut game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Spawned bare rather than through `assign_worker_producing`: a posted
    // program would run the cronjob path on the same node and leave the test
    // unable to say which path did the banking.
    let node = game
        .world
        .spawn((
            Structure {
                kind: "test_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::RESEARCH_DATA),
                level: None,
            },
            work_node_parts(),
        ))
        .id();
    let before = research_data_held(&game);

    game.work_structure(node).expect("the node can be worked");
    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_data_held(&game) > before,
        "working a research node by hand must bank it (was {before}, now {})",
        research_data_held(&game)
    );
    assert_eq!(
        node_output(&game, node, ids::RESEARCH_DATA),
        0,
        "the player-gather path must not fill the buffer either"
    );
}

/// The base keeps running while the party is four frames down, and banking
/// touches `Inventory` rather than `Position`, so research accrues the whole
/// time. Pinned because a later refactor reaching for the player's tile
/// would break it silently — that tile is the surface entrance, not where
/// the party is standing.
#[test]
fn research_banks_while_the_party_is_underground() {
    let mut game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assign_worker_producing(&mut game, ItemId::from(ids::RESEARCH_DATA));
    dive_to_depth(&mut game, 2);
    let before = research_data_held(&game);

    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_data_held(&game) > before,
        "the base banks research while the party is in the Stack (was {before}, now {})",
        research_data_held(&game)
    );
}

#[test]
fn a_save_round_trip_preserves_unlocked_research() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "weapon_bench");

    let path = std::env::temp_dir().join(format!("feral_research_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(loaded.is_researched("automation"));
    assert!(loaded.is_researched("weapon_bench"));
    assert!(
        !loaded.is_researched("commerce"),
        "loading must not invent research the player never took"
    );
}

/// Everything compilable from turn one: the three consumable starters plus
/// the Scavenged gear tier, which declares a `craftable` with no
/// `requires_structure`. Anything else must be gated behind research, a
/// bench, or both — so this set is pinned rather than counted.
#[test]
fn only_the_starters_and_scavenged_gear_need_no_research_or_bench() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut results: Vec<String> = game
        .craft_recipes()
        .into_iter()
        .map(|r| r.result.as_str().to_string())
        .collect();
    results.sort();
    assert_eq!(
        results,
        [
            "handshake_forge",
            "ice_breaker",
            "kinetic_edge",
            "outlet",
            "packet_buffer",
            "power_cell",
            "probe_service",
            "scrap_ward",
            "shiv_routine",
        ],
        "nothing else is free"
    );
}

#[test]
fn a_researched_recipe_stays_hidden_until_its_bench_is_built() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "overclock");

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
        "the blueprint alone isn't enough — you still need the Fabricator"
    );

    place_home(&mut game, 1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("fabricator", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(results.contains(&ItemId::from(ids::OVERCLOCK_CORE)));
}

#[test]
fn a_built_bench_alone_does_not_unlock_its_recipe() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game, 1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("fabricator", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
        "the Fabricator is a bench now, not an unlock"
    );
}

/// The Standard/Premium gear tiers declare their own recipe with a
/// `requires_structure` bench and no research node of their own. Building
/// the bench is the whole unlock — but it is still a real gate.
#[test]
fn an_item_declared_recipe_stays_hidden_until_its_bench_is_built() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let arc_lance = ItemId::from("arc_lance");

    assert!(
        !game.craft_recipes().iter().any(|r| r.result == arc_lance),
        "a bench-gated item recipe must not be free from turn one"
    );

    // The Fabricator itself is research-gated; that gates the bench, not
    // the recipe, which has no research node of its own.
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game, 1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("fabricator", 0, 1).unwrap();

    assert!(
        game.craft_recipes().iter().any(|r| r.result == arc_lance),
        "standing the bench should be enough — no research node names this recipe"
    );
}

/// Gear sources can be declared from either side. Both are honoured, an
/// item named twice is rolled once at the better chance, and the list is
/// ordered so a seeded run always spends its rolls the same way.
#[test]
fn equipment_drops_merge_both_declaration_sides_taking_the_better_chance() {
    let game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut scrapper = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "scrapper")
        .expect("scrapper ships");
    let arc_lance = ItemId::from("arc_lance");
    let chance_of = |drops: &[(ItemId, f32)], id: &ItemId| {
        drops
            .iter()
            .find(|(i, _)| i == id)
            .unwrap_or_else(|| panic!("{} should be droppable here", id.as_str()))
            .1
    };

    // Item side alone: arc_lance.ron names scrapper.
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(chance_of(&drops, &arc_lance), 0.1);
    let mut sorted = drops.clone();
    sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    assert_eq!(drops, sorted, "roll order must be deterministic");

    // Declared on both sides: one entry, the better chance.
    scrapper.equipment_drop = Some((arc_lance.clone(), 0.5));
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(
        drops.iter().filter(|(i, _)| *i == arc_lance).count(),
        1,
        "declared twice, rolled once"
    );
    assert_eq!(chance_of(&drops, &arc_lance), 0.5);

    // The weaker of the two loses, whichever side it came from.
    scrapper.equipment_drop = Some((arc_lance.clone(), 0.02));
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(chance_of(&drops, &arc_lance), 0.1);
}

/// A species-side `equipment_drop` is legacy but still supported, so a
/// third-party species mod that predates item-side `droppable` keeps
/// dropping what it always did.
#[test]
fn a_species_side_equipment_drop_still_works_on_its_own() {
    let game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut sprite = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "sprite")
        .expect("sprite ships");
    // Nothing names power_cell in a `droppable`, so this can only arrive
    // from the species side.
    let power_cell = ItemId::from(ids::POWER_CELL);
    sprite.equipment_drop = Some((power_cell.clone(), 0.25));

    let drops = game.equipment_drops_for(&sprite);
    assert!(drops.contains(&(power_cell, 0.25)));
}

#[test]
fn a_researched_recipe_carries_the_cost_from_its_ron_file() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "overclock");
    place_home(&mut game, 1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("fabricator", 0, 1).unwrap();

    assert_eq!(
        game.craft_cost(&ItemId::from(ids::OVERCLOCK_CORE)),
        vec![(ItemId::from(ids::PORTAL_FRAGMENT), 6)]
    );
}

#[test]
fn a_structure_named_by_no_research_file_is_buildable_from_the_start() {
    let game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            "assembly_bay".to_string(),
            "data_cache".to_string(),
            "home".to_string(),
            "mining_node".to_string(),
            "portal".to_string(),
            "recharger_node".to_string(),
            "refinery".to_string(),
            "research_node".to_string(),
            "winding_node".to_string(),
        ],
        "exactly the structures named by no research file start available"
    );
}

#[test]
fn a_research_gated_structure_is_hidden_from_the_build_menu_until_researched() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let hidden: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(!hidden.contains(&"fabricator".to_string()));

    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();

    let shown: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(shown.contains(&"fabricator".to_string()));
}

#[test]
fn placing_an_unresearched_structure_is_rejected_even_when_called_directly() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    let err = game.place_structure("fabricator", 0, 1).unwrap_err();
    assert!(
        err.contains("researched"),
        "filtering the menu is not a gate: {err}"
    );
}

#[test]
fn nothing_is_researched_at_the_start_of_a_game() {
    let game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.is_researched("automation"));
    assert!(
        game.research_nodes()
            .iter()
            .all(|n| n.state != ResearchState::Unlocked),
        "a fresh game starts with an entirely locked tree"
    );
}

#[test]
fn unlocking_research_consumes_exactly_its_cost() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 20);
    game.unlock_research("automation").unwrap();
    assert!(game.is_researched("automation"));
    assert_eq!(
        research_data_held(&game),
        12,
        "automation costs 8 of the 20 granted"
    );
}

#[test]
fn unlocking_research_fails_without_enough_research_data() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 7);
    let err = game.unlock_research("automation").unwrap_err();
    assert!(err.contains("Research Data"), "got: {err}");
    assert!(!game.is_researched("automation"));
}

#[test]
fn unlocking_research_fails_while_a_prerequisite_is_missing() {
    let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 500);
    let err = game.unlock_research("weapon_bench").unwrap_err();
    assert!(
        err.contains("Automation"),
        "the error should name the missing prereq: {err}"
    );
    assert!(!game.is_researched("weapon_bench"));
    assert_eq!(
        research_data_held(&game),
        500,
        "a rejected unlock must not charge the player"
    );
}

#[test]
fn a_locked_node_reports_which_prerequisites_are_missing() {
    let game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| n.id == "weapon_bench")
        .unwrap();
    assert_eq!(
        node.state,
        ResearchState::Locked {
            missing: vec!["Automation".to_string()]
        }
    );
}

#[test]
fn a_prerequisite_free_node_is_available_immediately() {
    let game = Game::new(66, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| n.id == "automation")
        .unwrap();
    assert_eq!(node.state, ResearchState::Available);
    assert!(
        !node.affordable,
        "available is about prereqs; affordability is separate"
    );
}

#[test]
fn researching_the_same_node_twice_is_rejected() {
    let mut game = Game::new(67, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    let err = game.unlock_research("automation").unwrap_err();
    assert!(err.contains("already"), "got: {err}");
}

#[test]
fn unknown_research_is_rejected() {
    let mut game = Game::new(68, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.unlock_research("not_a_node").is_err());
}

#[test]
fn research_nodes_lists_available_before_locked_before_unlocked() {
    let mut game = Game::new(69, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    let ranks: Vec<u8> = game
        .research_nodes()
        .iter()
        .map(|n| match n.state {
            ResearchState::Available => 0,
            ResearchState::Locked { .. } => 1,
            ResearchState::Unlocked => 2,
        })
        .collect();
    let mut sorted = ranks.clone();
    sorted.sort();
    assert_eq!(ranks, sorted, "menu order must group by state");
}

#[test]
fn the_data_cache_is_buildable_without_any_research() {
    let game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.buildable_structure_defs()
            .iter()
            .any(|d| d.id == "data_cache"),
        "buffer expansion must not be gated behind research the player \
         can't afford while the cap is at its tightest"
    );
}

#[test]
fn no_research_node_is_left_unlocking_nothing() {
    let game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for node in game.research_nodes() {
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&node.id)
            .expect("a listed node should exist in the db");
        assert!(
            !def.unlocks_structures.is_empty()
                || !def.unlocks_recipes.is_empty()
                || !def.unlocks_abilities.is_empty(),
            "{} unlocks nothing and is dead weight in the tree",
            node.id
        );
    }
}

#[test]
fn the_research_node_is_a_cronjob_worked_research_data_source() {
    let game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "research_node")
        .expect("research_node.ron should load");
    let work = def.work.expect("the Research Node must be workable");
    assert_eq!(work.produces, ItemId::from(ids::RESEARCH_DATA));
}
