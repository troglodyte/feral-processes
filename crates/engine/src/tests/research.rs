//! The research tree and the recipes and structures it gates.

use super::support::*;
use crate::*;

#[test]
fn a_cronjob_worker_fills_the_unbounded_buffer_past_the_old_cap() {
    let mut game = Game::new(708, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
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

/// `PlayerStatus::inventory` is the one list every "what does the player
/// have" screen reads — the inventory screen, the base panel, the
/// have/need columns on the craft and build screens, and the trade
/// screen's sell rows. Filtering here is what makes a bank invisible in
/// all of them at once, so this asserts the filter *and* that it takes
/// nothing else with it.
#[test]
fn a_banked_item_is_not_an_inventory_row() {
    let mut game = Game::new(712, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 40);

    let listed = game.player_status().inventory;

    assert!(
        !listed
            .iter()
            .any(|r| r.copy.item == ItemId::from(ids::RESEARCH_DATA)),
        "a bank is not cargo and must not be listed: {listed:?}"
    );
    assert!(
        listed
            .iter()
            .any(|r| r.copy.item == ItemId::from(ids::CORE_FRAGMENT)),
        "ordinary cargo must be untouched by that filter: {listed:?}"
    );
}

/// Hiding the row everywhere would otherwise hide the number from the one
/// screen that spends it, so the research screen asks for it by name.
#[test]
fn the_bank_is_still_readable_by_name() {
    let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 40);

    assert_eq!(game.banked(&ItemId::from(ids::RESEARCH_DATA)), 40);
}

/// Where a Research Node's payout lands, and the whole of the research
/// economy: it feeds the one project the base is working, and reaches neither
/// the node's own buffer nor the player's bank.
#[test]
fn a_posted_program_feeds_the_active_project() {
    let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);

    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_progress(&game, "automation") > 0,
        "a research cronjob must feed the project it was selected for"
    );
    assert_eq!(
        research_data_held(&game),
        0,
        "and nothing may reach the player's bank — there is nothing to spend it on"
    );
    assert_eq!(
        node_output(&game, node, ids::RESEARCH_DATA),
        0,
        "a banked resource must never reach the node's own output buffer"
    );
}

/// Progress stops at the node's own `cost`. Unbounded, the screen draws
/// 700/540 and every unit past the goal is work the base threw away with no
/// sign of it.
#[test]
fn progress_saturates_at_the_nodes_cost() {
    let mut game = Game::new(721, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    let cost = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .unwrap()
        .cost;

    // Sampled every tick rather than read once at the end: the base can make
    // this bill, so the project completes and `settle_research` drops the entry
    // — a single read afterwards would be asserting on a zero and passing for
    // the wrong reason.
    for _ in 0..2_000 {
        game.tick();
        assert!(
            research_progress(&game, "automation") <= cost,
            "progress must saturate at the cost rather than run away past it"
        );
    }

    assert!(
        game.is_researched("automation"),
        "and the fixture has to get there, or the ceiling was never approached"
    );
}

/// With no project selected a cycle lands nowhere at all: not the bank, not
/// the buffer, not some other node's progress. Silent by design — the node
/// reads `Idle` in every case but this one, a hand-posted standing job, which
/// is the player's own instruction.
#[test]
fn with_no_project_a_research_cycle_lands_nowhere() {
    let mut game = Game::new(722, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = assign_worker_producing(&mut game, ItemId::from(ids::RESEARCH_DATA));

    for _ in 0..100 {
        game.tick();
    }

    assert_eq!(research_data_held(&game), 0, "no bank fills");
    assert_eq!(node_output(&game, node, ids::RESEARCH_DATA), 0, "no buffer");
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .progress
            .is_empty(),
        "and no node quietly accrues progress the player never asked for"
    );
}

/// The player working the node by hand delivers by the same rule. The two
/// paths share `deliver_payout` precisely so this cannot drift — a test
/// covering only the cronjob would not notice a second copy.
#[test]
fn the_player_working_a_research_node_feeds_the_project_too() {
    let mut game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
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
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();

    stand_player_at_post(&mut game, node);
    game.work_structure(node).expect("the node can be worked");
    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_progress(&game, "automation") > 0,
        "working a research node by hand must feed the project too"
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
fn research_progresses_while_the_party_is_underground() {
    let mut game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Posted from inside the base and then back out, since the whole point of
    // this test is where the party goes next.
    from_inside_the_base(&mut game, |g| {
        let node = base_with_a_research_node(g);
        g.select_research("automation").unwrap();
        let worker = spawn_tamed(g, 10, 3);
        park_at_post(g, worker, node);
    });
    dive_to_depth(&mut game, 2);

    for _ in 0..100 {
        game.tick();
    }

    assert!(
        research_progress(&game, "automation") > 0,
        "the base works its project while the party is in the Stack"
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
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "overclock");

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
        "the blueprint alone isn't enough — you still need the Fabricator"
    );

    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "fabricator", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(results.contains(&ItemId::from(ids::OVERCLOCK_CORE)));
}

#[test]
fn a_built_bench_alone_does_not_unlock_its_recipe() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "fabricator", 0, 1).unwrap();

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
    stand_in_base(&mut game);
    let arc_lance = ItemId::from("arc_lance");

    assert!(
        !game.craft_recipes().iter().any(|r| r.result == arc_lance),
        "a bench-gated item recipe must not be free from turn one"
    );

    // The Fabricator itself is research-gated; that gates the bench, not
    // the recipe, which has no research node of its own.
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "fabricator", 0, 1).unwrap();

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
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "overclock");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "fabricator", 0, 1).unwrap();

    assert_eq!(
        game.craft_cost(&ItemId::from(ids::OVERCLOCK_CORE), false),
        vec![
            (ItemId::from(ids::PORTAL_FRAGMENT), 6),
            (ItemId::from("cache_grain"), 2),
        ]
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
            "contract_broker".to_string(),
            "data_cache".to_string(),
            "defrag_bay".to_string(),
            "depot".to_string(),
            "home".to_string(),
            "mining_node".to_string(),
            "portal".to_string(),
            "quarantine_rack".to_string(),
            "recharger_node".to_string(),
            "refinery".to_string(),
            // Ungated on purpose: recovering from a wipe is not a blueprint
            // you have to earn, and a Bay you cannot build in the zone where
            // the first wipe happens is a dead run rather than pressure.
            "repair_bay".to_string(),
            "research_node".to_string(),
            "sandbox".to_string(),
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

    unlock_research_chain(&mut game, "weapon_bench");

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
    stand_in_base(&mut game);
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    let err = place_now(&mut game, "fabricator", 0, 1).unwrap_err();
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

/// Both gates, and the ordering between them: a project completes on the
/// progress a Research Node fed it **and** a bill the base can pay off its own
/// shelves, and `&&` short-circuits so nothing is spent while it is still half
/// researched.
#[test]
fn a_project_completes_on_progress_and_a_bill_the_base_pays() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let shelf = shelve_research_bill(&mut game, "automation", 8, 8);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");

    game.tick();

    assert!(game.is_researched("automation"));
    assert_eq!(
        node_output(&game, shelf, ids::CORE_FRAGMENT),
        0,
        "the bill came off the base's shelves"
    );
}

/// The first half of the pair, on its own: full progress and empty shelves
/// completes nothing and consumes nothing.
#[test]
fn progress_alone_does_not_complete_a_project() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");
    let before = all_shelf_stock(&game);

    game.tick();

    assert!(!game.is_researched("automation"));
    assert_eq!(all_shelf_stock(&game), before, "nothing left a shelf");
}

/// And the second half: a paid bill with no progress behind it completes
/// nothing and — the assertion that fails if the two gates are ever swapped —
/// spends none of the materials either.
#[test]
fn a_full_bill_alone_does_not_complete_a_project() {
    let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let shelf = shelve_research_bill(&mut game, "automation", 8, 8);
    let stocked = node_output(&game, shelf, ids::CORE_FRAGMENT);
    game.select_research("automation").unwrap();

    game.tick();

    assert!(!game.is_researched("automation"));
    assert_eq!(
        node_output(&game, shelf, ids::CORE_FRAGMENT),
        stocked,
        "a project still short of progress must not spend its materials"
    );
}

/// The whole point of moving the bill off the player's pack: the base pays it
/// out of a shelf the player is nowhere near.
#[test]
fn a_completed_project_consumes_its_bill_from_a_depot_the_player_is_nowhere_near() {
    let mut game = Game::new(723, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let shelf = shelve_research_bill(&mut game, "automation", 400, -400);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");

    game.tick();

    assert!(game.is_researched("automation"));
    assert_eq!(node_output(&game, shelf, ids::CORE_FRAGMENT), 0);
}

/// Completion clears the project out, drops its progress row — the one thing
/// that ever removes one — and takes its work orders with it.
#[test]
fn completing_clears_the_project_drops_its_progress_row_and_withdraws_its_orders() {
    let mut game = Game::new(724, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    shelve_research_bill(&mut game, "automation", 8, 8);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");
    assert!(
        !game.work_orders().is_empty(),
        "the fixture is vacuous unless selecting filed something"
    );

    game.tick();

    assert_eq!(active_research(&game), None);
    assert_eq!(research_progress(&game, "automation"), 0);
    assert!(
        game.work_orders().iter().all(|o| !o.for_research),
        "a completed project takes its own orders back out"
    );
}

/// The shared-helper regression. The routines and tools a node hands over are
/// granted by `grant_research_knowledge`, which the project path calls rather
/// than keeping a copy of — a copy is what drifts.
#[test]
fn completing_grants_the_nodes_abilities_and_tools() {
    let mut game = Game::new(725, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let taught = game
        .world
        .resource::<ResearchDb>()
        .all()
        .find(|d| !d.unlocks_abilities.is_empty())
        .map(|d| (d.id.clone(), d.unlocks_abilities.clone()))
        .expect("the shipped tree teaches routines somewhere");
    research_prereqs_of(&mut game, &taught.0);
    base_with_a_research_node(&mut game);
    shelve_research_bill(&mut game, &taught.0, 8, 8);
    game.select_research(&taught.0).unwrap();
    fill_research_progress(&mut game, &taught.0);

    game.tick();

    assert!(game.is_researched(&taught.0));
    for ability in &taught.1 {
        assert!(
            game.world
                .resource::<crate::resources::KnownRoutines>()
                .0
                .contains(ability),
            "completing must teach {ability}, not just mark the node"
        );
    }
}

/// Nothing in this feature may shift the seeded stream — a retune of what a
/// project costs must not move which programs a run spawns.
#[test]
fn settling_research_draws_no_rng() {
    assert!(
        rng_unadvanced_by(726, |game| {
            base_with_a_research_node(game);
            shelve_research_bill(game, "automation", 8, 8);
            game.select_research("automation").unwrap();
            fill_research_progress(game, "automation");
            game.settle_research();
            assert!(game.is_researched("automation"), "it did complete");
        }),
        "selecting and settling a project must draw nothing from the seeded stream"
    );
}

/// Selection refuses a node whose prerequisites are outstanding, and files
/// nothing when it does.
#[test]
fn selecting_research_fails_while_a_prerequisite_is_missing() {
    let mut game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);

    let err = game.select_research("weapon_bench").unwrap_err();

    assert!(
        err.contains("Routine Fabrication"),
        "the error should name the missing prereq: {err}"
    );
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and files nothing");
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
            missing: vec!["Routine Fabrication".to_string()],
            // Weapon Fabrication is a zone-1 node, so the prereq is the
            // only thing in its way — the contrast that makes
            // `a_node_can_report_both_a_missing_prereq_and_its_zone` mean
            // something.
            min_zone: None,
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
}

#[test]
fn selecting_an_already_researched_node_is_rejected() {
    let mut game = Game::new(67, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "automation");
    base_with_a_research_node(&mut game);

    let err = game.select_research("automation").unwrap_err();

    assert!(err.contains("already"), "got: {err}");
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty());
}

#[test]
fn unknown_research_is_rejected() {
    let mut game = Game::new(68, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);

    assert!(game.select_research("not_a_node").is_err());
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty());
}

/// Off the base there is no answer to which machines are standing, so the
/// selection refuses rather than answering from the surface entrance tile —
/// `Game::queue_work_order`'s own reason for the same guard.
#[test]
fn selecting_research_off_the_base_is_rejected() {
    let mut game = Game::new(727, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // The **whole** base stood up, so the only thing refusing is the locale.
    // Built from inside and then walked out: with a bare Research Node the
    // material line is refused anyway and the test passes with `require_base`
    // deleted, which is what it used to do.
    from_inside_the_base(&mut game, |g| {
        base_with_a_research_node(g);
        g.select_research("automation")
            .expect("the fixture is vacuous unless this base can take a project");
        g.abandon_research().unwrap();
    });

    let err = game.select_research("automation").unwrap_err();

    assert!(
        !err.contains("Research Node"),
        "the locale is the reason, not the plant: {err}"
    );
    assert_eq!(active_research(&game), None);
    assert!(
        game.work_orders().is_empty(),
        "and nothing is filed — every `queue_work_order` would refuse its own \
         `require_base` through the `let _`, leaving a project with no bill"
    );
}

/// One project at a time, and abandoning is how you change your mind — which
/// the refusal says, because a player who cannot see why the key did nothing
/// reads it as the screen being broken.
#[test]
fn a_second_project_is_refused_while_one_is_running() {
    let mut game = Game::new(728, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let filed = game.work_orders().len();

    let second = game
        .research_nodes()
        .into_iter()
        .find(|n| n.state == ResearchState::Available && n.blocked_by.is_none())
        .expect("a fresh base has a second node open to it");
    let err = game.select_research(&second.id).unwrap_err();

    assert!(err.contains("abandon"), "got: {err}");
    assert_eq!(
        active_research(&game).as_deref(),
        Some("automation"),
        "the running project is untouched"
    );
    assert_eq!(game.work_orders().len(), filed, "and nothing else is filed");
}

/// Nothing makes the research currency until a Research Node is standing, and
/// `work_orders::chain_break` cannot say so — it refuses every banked item by
/// construction. So the node is its own refusal.
#[test]
fn a_base_with_no_research_node_cannot_take_a_project() {
    let mut game = Game::new(729, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);

    let err = game.select_research("automation").unwrap_err();

    assert!(
        err.contains("Research Node"),
        "the refusal must name the machine that is missing: {err}"
    );
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty());
}

/// The reachability half. Without it the refusal above could be permanent and
/// every test around it would still be green.
#[test]
fn building_the_missing_machine_makes_the_same_selection_succeed() {
    let mut game = Game::new(730, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // The bill's own producers stood up, and then the Research Node removed —
    // so the Research Node is the only thing in the way.
    let node = base_with_a_research_node(&mut game);
    game.world.despawn(node);
    assert!(game.select_research("automation").is_err());

    spawn_machine_at(&mut game, "research_node", 2, 2);

    game.select_research("automation")
        .expect("a Research Node is all that was missing");
}

/// Selecting files one High-band order per material line, marked as the
/// project's own — the provenance that lets them be withdrawn again.
#[test]
fn selecting_files_one_high_order_per_material_line() {
    let mut game = Game::new(731, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let bill = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .unwrap()
        .materials
        .clone();
    assert!(!bill.is_empty(), "the fixture node must author a bill");

    game.select_research("automation").unwrap();

    let filed: Vec<(ItemId, u32, OrderPriority, bool)> = game
        .work_orders()
        .iter()
        .map(|o| (o.item.clone(), o.qty, o.priority, o.for_research))
        .collect();
    assert_eq!(
        filed,
        bill.iter()
            .map(|(item, need)| (item.clone(), *need, OrderPriority::High, true))
            .collect::<Vec<_>>(),
        "one High order per line, every one of them the project's"
    );
}

/// Abandoning takes the project's own orders back out, leaves the player's
/// alone, and **keeps** the progress — coming back to a long project is not
/// destructive.
#[test]
fn abandoning_withdraws_the_projects_orders_and_keeps_its_progress() {
    let mut game = Game::new(732, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    spawn_machine_at(&mut game, "mining_node", 4, 4);
    game.select_research("automation").unwrap();
    let mine = game.work_orders()[0].item.clone();
    game.queue_work_order(WorkOrder::batch(mine.clone(), 3))
        .expect("the player's own order for the same material");
    fill_research_progress(&mut game, "automation");
    let earned = research_progress(&game, "automation");

    game.abandon_research().unwrap();

    assert_eq!(active_research(&game), None);
    assert_eq!(
        research_progress(&game, "automation"),
        earned,
        "the work already done is kept"
    );
    let left: Vec<(ItemId, u32)> = game
        .work_orders()
        .iter()
        .map(|o| (o.item.clone(), o.qty))
        .collect();
    assert_eq!(
        left,
        vec![(mine, 3)],
        "exactly the player's own order survives"
    );
}

#[test]
fn abandoning_nothing_is_refused() {
    let mut game = Game::new(733, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);

    assert!(game.abandon_research().is_err());
}

#[test]
fn research_nodes_lists_active_before_available_before_locked_before_unlocked() {
    let mut game = Game::new(69, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "automation");
    base_with_a_research_node(&mut game);
    let open = game
        .research_nodes()
        .into_iter()
        .find(|n| n.state == ResearchState::Available && n.blocked_by.is_none())
        .expect("something is open once Automation is in");
    game.select_research(&open.id).unwrap();
    let ranks: Vec<u8> = game
        .research_nodes()
        .iter()
        .map(|n| match n.state {
            ResearchState::Active => 0,
            ResearchState::Available => 1,
            ResearchState::Locked { .. } => 2,
            ResearchState::Unlocked => 3,
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

/// The cheapest node in a band, found by asking the db rather than named,
/// so retuning which node sits in which band leaves the tests below still
/// measuring what they mean to. `ResearchDb::all` is already cheapest-first.
fn cheapest_gated_node(game: &Game, zone: u32) -> ResearchDef {
    game.world
        .resource::<ResearchDb>()
        .all()
        .find(|d| d.min_zone == zone)
        .unwrap_or_else(|| panic!("the shipped tree should band something at zone {zone}"))
        .clone()
}

/// Unlocks everything `id` transitively requires — and deliberately *not*
/// `id` itself, which is the node under test.
///
/// Not `support::unlock_research_chain` on `id` itself, which would leave
/// nothing to refuse; it is what lands each *prerequisite*. That helper also
/// raises `ZoneLevel` to clear the chain's own bands, which is the very thing
/// being tested, so it is called per prereq — every prereq of a gated node
/// sits at or below its band — rather than on the node under test.
///
/// Every prereq of a gated node sits in a band at or below its own — that is
/// `no_research_node_is_gated_below_its_own_prerequisite` — so at the zone
/// the caller is testing, all of them are buyable.
fn research_prereqs_of(game: &mut Game, id: &str) {
    let requires = game
        .world
        .resource::<ResearchDb>()
        .get(id)
        .expect("a shipped node")
        .requires
        .clone();
    for prereq in requires {
        if game.is_researched(&prereq) {
            continue;
        }
        research_prereqs_of(game, &prereq);
        unlock_research_chain(game, &prereq);
    }
}

fn research_state(game: &Game, id: &str) -> ResearchState {
    game.research_nodes()
        .into_iter()
        .find(|n| n.id == id)
        .map(|n| n.state)
        .expect("a shipped node should be listed")
}

#[test]
fn a_node_above_the_players_zone_reports_its_zone() {
    let game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    match research_state(&game, &gated.id) {
        ResearchState::Locked { min_zone, .. } => assert_eq!(
            min_zone,
            Some(2),
            "{} is a zone-2 node and the party is in zone 1",
            gated.id
        ),
        other => panic!("expected {} to be Locked, got {other:?}", gated.id),
    }
}

/// A gated node is not filtered out of the menu, for the reason
/// `Game::upgrade_ceiling` records about a structure stalled at its zone
/// ceiling: hiding the stalled rows means a player who never breached never
/// learns the tier exists. The visible zone-3 band *is* the reason to breach.
#[test]
fn a_zone_gated_node_is_still_listed() {
    let game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 3);
    assert!(
        game.research_nodes().iter().any(|n| n.id == gated.id),
        "{} must stay on the menu at zone 1 — it is what tells the player \
         there is a reason to breach",
        gated.id
    );
}

#[test]
fn select_research_refuses_a_node_above_the_players_zone() {
    let mut game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    research_prereqs_of(&mut game, &gated.id);
    base_with_a_research_node(&mut game);

    let err = game.select_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(!game.is_researched(&gated.id));
    // The half that fails if the refusal is ever moved below the filing —
    // without it this passes against a build that files and then refuses.
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and files nothing");
}

#[test]
fn breaching_makes_a_zone_gated_node_available() {
    let mut game = Game::new(718, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    research_prereqs_of(&mut game, &gated.id);
    assert!(
        matches!(
            research_state(&game, &gated.id),
            ResearchState::Locked {
                min_zone: Some(2),
                ..
            }
        ),
        "the fixture is vacuous unless the node starts gated"
    );

    game.enter_next_zone();

    assert_eq!(
        research_state(&game, &gated.id),
        ResearchState::Available,
        "with its prereqs met, reaching zone 2 is the whole of what {} was waiting on",
        gated.id
    );
}

#[test]
fn a_node_can_report_both_a_missing_prereq_and_its_zone() {
    let game = Game::new(719, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let deep = cheapest_gated_node(&game, 3);
    match research_state(&game, &deep.id) {
        ResearchState::Locked { missing, min_zone } => {
            assert!(
                !missing.is_empty(),
                "{} sits on an unresearched chain, so it owes a prereq too",
                deep.id
            );
            assert_eq!(
                min_zone,
                Some(3),
                "and the zone is a second, separate reason"
            );
        }
        other => panic!("expected {} to be Locked, got {other:?}", deep.id),
    }
}

/// `upgrade_structure` checks its ceilings before the materials check "so the
/// player is never sent to find fragments they couldn't have spent". Same
/// argument: a broke player at zone 1 must hear about the zone.
#[test]
fn the_zone_gate_is_refused_before_the_machines() {
    let mut game = Game::new(720, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    research_prereqs_of(&mut game, &gated.id);
    // Deliberately no Research Node: the zone must still be the reason, or a
    // player at zone 1 is sent to build machinery for a node they could not
    // have taken.
    stand_in_base(&mut game);

    let err = game.select_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(
        !err.contains("Research Node"),
        "the zone is the reason, not the base's plant: {err}"
    );
}

/// A node gated below its own prerequisite is a gate that can never fire:
/// the prereq lock outlives it, so the zone is never the reason the node is
/// unbuyable, and the menu shows a reason that disappears without the node
/// becoming available. Catches a band edit that makes a gate unreachable.
#[test]
fn no_research_node_is_gated_below_its_own_prerequisite() {
    let game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<ResearchDb>();
    for def in db.all() {
        for prereq_id in &def.requires {
            let prereq = db
                .get(prereq_id)
                .expect("a dangling prereq is dropped at load, so this must resolve");
            assert!(
                prereq.min_zone <= def.min_zone,
                "{} is zone {} but requires {}, which is zone {} — the gate could never fire",
                def.id,
                def.min_zone,
                prereq.id,
                prereq.min_zone
            );
        }
    }
}

/// The one way this feature could softlock a run: gate a node that unlocks
/// the structure the player needs in order to *reach* the zone that ungates
/// it. Researching the Zone Portal is fine and a mod is free to do it —
/// gating it is what breaks.
///
/// Vacuously true today, since no shipped node names the portal at all, and
/// that is exactly the point: the property is currently safe by accident,
/// and one content edit could remove it silently.
///
/// Asserted against the loaded `ResearchDb` rather than by reading the
/// files, so a node dropped at load time cannot make it pass for the wrong
/// reason.
#[test]
fn nothing_needed_to_breach_is_locked_behind_research() {
    let game = Game::new(714, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<ResearchDb>().all() {
        if def.unlocks_structures.iter().any(|s| s == "portal") {
            assert_eq!(
                def.min_zone, 0,
                "{} gates the Zone Portal behind zone {} — the portal is how you \
                 reach that zone, so the run cannot get there",
                def.id, def.min_zone
            );
        }
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

// --- Material bills ------------------------------------------------------
//
// `ResearchDef::materials` — the goods a node consumes beside its Research
// Data. These use a scratch node rather than a shipped one so what is being
// asserted is the *mechanism*, and a retune of the shipped bills cannot make
// them fail for a reason that isn't a bug.

/// A node priced in goods the player carries from turn one, so a fixture
/// never has to build the chain that makes them.
const BILLED_NODE: &str = r#"(
    id: "billed",
    name: "Billed Node",
    description: "A node with a material bill.",
    cost: 5,
    materials: [("core_fragment", 6), ("power_cell", 2)],
)"#;

fn billed_assets(tag: &str) -> ScratchAssets {
    assets_dir_with_extra_research(tag, "billed.ron", BILLED_NODE)
}

/// What every material line of every `Stock` on the base is holding, so a
/// refusal test can assert the shelves are byte-identical rather than merely
/// that the pack is.
/// Stands a Depot beside the party's base cell with `qty` of `item` on its
/// shelf, through `Game::spawn_structure` — the one place a structure's
/// component list is written, so this cannot ship a Depot without the
/// `Stock` that makes it one. A real `place_now` would need laid floor and a
/// Home, neither of which is what these tests are about.
fn shelf_beside_the_party(game: &mut Game, item: &str, qty: u32) -> Entity {
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "depot")
        .expect("the shipped catalogue has a Depot");
    let depot = game.spawn_structure(&def, 1, 0, None);
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(item), qty);
    depot
}

fn all_shelf_stock(game: &Game) -> Vec<(ItemId, u32)> {
    let mut held: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
    for e in game.world.iter_entities() {
        if let Some(stock) = e.get::<Stock>() {
            for (item, qty) in stock.output.iter() {
                *held.entry(item.clone()).or_default() += qty;
            }
        }
    }
    held.into_iter().collect()
}

#[test]
fn a_project_spends_its_material_bill_off_the_base() {
    let assets = billed_assets("research_bill_base");
    let mut game = Game::new(901, DifficultyMode::Forgiving, &assets).unwrap();
    base_with_a_research_node(&mut game);
    let shelf = shelve_research_bill(&mut game, "billed", 8, 8);
    let extra = 4;
    {
        let mut stock = game.world.get_mut::<Stock>(shelf).unwrap();
        *stock
            .output
            .entry(ItemId::from(ids::CORE_FRAGMENT))
            .or_default() += extra;
    }
    set_inventory(&mut game, &[("core_fragment", 10), ("power_cell", 3)]);
    game.select_research("billed").unwrap();
    fill_research_progress(&mut game, "billed");

    game.tick();

    assert!(game.is_researched("billed"));
    assert_eq!(
        node_output(&game, shelf, ids::CORE_FRAGMENT),
        extra,
        "exactly the bill came off the shelf and no more"
    );
    assert_eq!(
        held(&game, &ItemId::from(ids::CORE_FRAGMENT)),
        10,
        "and the player's own pack is not a research bill's source"
    );
    assert_eq!(held(&game, &ItemId::from("power_cell")), 3);
}

/// The bill is drawn across the *whole* base, not one shelf: a line split
/// between two Depots is still payable, which is what
/// `work_orders::base_holding` counts and what the screen therefore shows.
#[test]
fn a_bill_split_across_two_shelves_is_still_paid() {
    let assets = billed_assets("research_bill_split");
    let mut game = Game::new(902, DifficultyMode::Forgiving, &assets).unwrap();
    base_with_a_research_node(&mut game);
    let near = shelf_beside_the_party(&mut game, ids::CORE_FRAGMENT, 3);
    let far = shelf_beside_the_party(&mut game, ids::CORE_FRAGMENT, 3);
    let cells = shelve_research_bill(&mut game, "billed", 12, 12);
    {
        // The fragment lines are the split under test; take the shelved
        // fragments back out so the two Depots are the only source.
        let mut stock = game.world.get_mut::<Stock>(cells).unwrap();
        stock.output.remove(&ItemId::from(ids::CORE_FRAGMENT));
    }
    game.select_research("billed").unwrap();
    fill_research_progress(&mut game, "billed");

    game.tick();

    assert!(game.is_researched("billed"));
    assert_eq!(
        node_output(&game, near, ids::CORE_FRAGMENT) + node_output(&game, far, ids::CORE_FRAGMENT),
        0,
        "both halves of the six were taken"
    );
}

/// The whole bill or nothing — `commit_caravan_basket`'s rule inside
/// `stock::spend_bill_from_base`, and the reason it is two passes rather than
/// a take-as-you-go loop that would strand the line it could pay.
#[test]
fn a_project_one_line_short_spends_none_of_the_others() {
    let assets = billed_assets("research_bill_refuse");
    let mut game = Game::new(903, DifficultyMode::Forgiving, &assets).unwrap();
    base_with_a_research_node(&mut game);
    let shelf = shelve_research_bill(&mut game, "billed", 8, 8);
    {
        // One cell short of the bill, with the fragment line whole.
        let mut stock = game.world.get_mut::<Stock>(shelf).unwrap();
        let held = stock.output.get_mut(&ItemId::from("power_cell")).unwrap();
        *held -= 1;
    }
    game.select_research("billed").unwrap();
    fill_research_progress(&mut game, "billed");
    let before = all_shelf_stock(&game);

    game.tick();

    assert!(!game.is_researched("billed"));
    assert_eq!(
        all_shelf_stock(&game),
        before,
        "the line the base *could* pay must not be spent against a bill that fails"
    );
}

/// A node that authored no bill is the pre-materials game, which is what
/// `#[serde(default)]` on the field buys and what a mod's untouched tree
/// still gets: progress alone completes it.
#[test]
fn a_node_with_no_material_bill_completes_on_progress_alone() {
    let assets = assets_dir_with_extra_research(
        "research_no_bill",
        "unbilled.ron",
        r#"(
    id: "unbilled",
    name: "Unbilled Node",
    description: "A node priced in Research Data alone.",
    cost: 5,
)"#,
    );
    let mut game = Game::new(905, DifficultyMode::Forgiving, &assets).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("unbilled").unwrap();
    fill_research_progress(&mut game, "unbilled");

    game.tick();

    assert!(game.is_researched("unbilled"));
    assert!(
        game.work_orders().is_empty(),
        "and a node with no bill files no orders"
    );
}

/// The screen's `have` column and the gate that spends the bill are one call,
/// `work_orders::base_holding` — so a row drawn as paid for cannot then fail to
/// complete. And it counts the base, not the pack: a Depot across the base is
/// where a project's materials actually are.
#[test]
fn a_materials_have_column_counts_a_depot_across_the_base() {
    let assets = billed_assets("research_bill_view");
    let mut game = Game::new(906, DifficultyMode::Forgiving, &assets).unwrap();
    stand_in_base(&mut game);
    // In the pack, which is deliberately not a source: the column must read 0.
    set_inventory(&mut game, &[("core_fragment", 6), ("power_cell", 2)]);

    let bill = |game: &Game| {
        game.research_nodes()
            .into_iter()
            .find(|n| n.id == "billed")
            .unwrap()
            .materials
            .iter()
            .map(|m| (m.name.clone(), m.need, m.have))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        bill(&game),
        vec![
            ("Core Fragment".to_string(), 6, 0),
            ("Power Cell".to_string(), 2, 0)
        ],
        "what the player is carrying is theirs, not the project's"
    );

    // Nowhere near the party, which is the point.
    let shelf = shelve_research_bill(&mut game, "billed", 300, -300);
    assert!(shelf != Entity::PLACEHOLDER);

    assert_eq!(
        bill(&game),
        vec![
            ("Core Fragment".to_string(), 6, 6),
            ("Power Cell".to_string(), 2, 2)
        ],
        "a shelf across the base is counted, wherever the player is standing"
    );
}

/// The active project heads the list and carries its progress — the two things
/// the screen's top row is for.
#[test]
fn the_active_project_is_the_first_row() {
    let mut game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .progress
        .insert("automation".to_string(), 3);

    let rows = game.research_nodes();

    assert_eq!(rows[0].id, "automation");
    assert_eq!(rows[0].state, ResearchState::Active);
    assert_eq!(rows[0].progress, 3);
}

/// A blocked row carries the sentence that would refuse it, because it *is*
/// that sentence — `Game::research_block` is one call, so the screen cannot
/// offer a row the selection turns down for a reason it never showed.
#[test]
fn a_blocked_node_carries_the_sentence_that_would_refuse_it() {
    let mut game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let refusal = game.select_research("automation").unwrap_err();

    let row = game
        .research_nodes()
        .into_iter()
        .find(|n| n.id == "automation")
        .unwrap();

    assert_eq!(row.blocked_by.as_deref(), Some(refusal.as_str()));
}

/// A node's conversion lines are derived from what it unlocks, not authored
/// beside the description — so a recipe retuned in `assets/items/` or a
/// machine repointed in `assets/structures/` cannot leave the research menu
/// quoting the old one. Both halves are folded here because a node may
/// unlock a bench and a recipe at once and the screen draws one list.
#[test]
fn a_research_node_reports_the_conversions_its_structures_perform() {
    let game = Game::new(714, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let node = research_node(&game, "routine_fabrication");

    assert!(
        node.conversions
            .iter()
            .any(|c| c.contains("Core Fragment") && c.contains("into Blank Substrate")),
        "the Lathe's conversion must be named: {:?}",
        node.conversions
    );
    assert!(
        node.conversions
            .iter()
            .any(|c| c.contains("Blank Substrate") && c.contains("into Routine Disk")),
        "the Disk Press's conversion must be named: {:?}",
        node.conversions
    );
    assert_eq!(
        node.conversions.len(),
        3,
        "one line per assembling structure and no line for the Log Scraper, \
         which is a work node and converts nothing: {:?}",
        node.conversions
    );
}

#[test]
fn a_research_node_reports_the_recipes_it_unlocks_with_their_quantities() {
    let game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let node = research_node(&game, "ablative");

    assert_eq!(
        node.conversions,
        vec!["Portal Fragment x12, Cache Grain x3 into Ablative Plating.".to_string()],
        "a recipe's own cost is what the line quotes, quantities included"
    );
}

#[test]
fn a_research_node_that_unlocks_no_conversion_reports_none() {
    let game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let node = research_node(&game, "paging");

    assert!(
        node.conversions.is_empty(),
        "a Depot converts nothing and must add no line: {:?}",
        node.conversions
    );
}

fn research_node(game: &Game, id: &str) -> ResearchStatus {
    game.research_nodes()
        .into_iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("{id:?} should be a shipped research node"))
}

/// The project and the progress behind it both survive a reload — a run picked
/// up a week later is working the same node with the same work behind it.
///
/// A save→load test and not a RON round trip: the round trip cannot see a
/// skipped field, so it would be vacuous against exactly the mistake that
/// matters here.
#[test]
fn an_active_project_and_its_progress_survive_a_save_round_trip() {
    let mut game = Game::new(734, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .progress
        .insert("automation".to_string(), 5);

    let path =
        std::env::temp_dir().join(format!("feral_research_project_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(active_research(&loaded).as_deref(), Some("automation"));
    assert_eq!(research_progress(&loaded, "automation"), 5);
}

/// The additive half: a save written before projects existed loads with none,
/// and nothing else about research moves.
#[test]
fn a_save_written_before_this_change_loads_with_no_project() {
    let mut game = Game::new(735, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "automation");

    let path =
        std::env::temp_dir().join(format!("feral_research_legacy_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    // The two fields as an older save leaves them: absent, which
    // `#[serde(default)]` reads as this.
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(active_research(&loaded), None);
    assert_eq!(research_progress(&loaded, "automation"), 0);
    assert!(
        loaded.is_researched("automation"),
        "and what the run already researched is untouched"
    );
}

/// An old save's banked Research Data is dropped on load, with a line saying
/// so. The stock strip folds in every `ItemDef::banked` pool **by the flag**, so
/// a leftover pool would sit across the top of every base screen for the rest of
/// the run with nothing to spend it on. The fold itself stays — it names no
/// item, and a mod may ship another banked one.
#[test]
fn a_legacy_saves_banked_research_data_is_dropped_on_load() {
    let mut game = Game::new(736, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    grant_research_data(&mut game, 40);
    let cargo = held(&game, &ItemId::from(ids::CORE_FRAGMENT));

    let path = std::env::temp_dir().join(format!("feral_research_bank_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.banked(&ItemId::from(ids::RESEARCH_DATA)),
        0,
        "a bank with nothing to spend it on is dropped rather than left on the strip"
    );
    assert_eq!(
        held(&loaded, &ItemId::from(ids::CORE_FRAGMENT)),
        cargo,
        "and ordinary cargo in the same store is untouched"
    );
}

/// A Research Node standing with no project selected is something the player
/// has to decide about, so it asks — and stops the moment one is picked.
#[test]
fn a_research_node_with_no_project_asks_for_attention() {
    let mut game = Game::new(737, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);

    fn asked(game: &mut Game) -> bool {
        game.attention()
            .iter()
            .any(|row| row.kind == AttentionKind::NoResearchProject)
    }
    assert!(
        asked(&mut game),
        "an idle Research Node asks for the player"
    );

    game.select_research("automation").unwrap();

    assert!(!asked(&mut game), "and stops once the base has a project");
}

/// The whole loop, end to end on the base a player actually has: one Research
/// Node, one program, no shortcuts. The project earns its progress, the base
/// makes its bill, and it completes.
///
/// The bug this exists to catch: `research_wants` raised a want for every
/// deployed Research Node for as long as a project was active, **including
/// after its progress had saturated**. Sitting above `settle_orders`, that want
/// held the base's only body on a node producing nothing — `deliver_payout`
/// returns 0 once progress reaches `cost` — while the material orders the
/// project had just filed were cut by `truncate(staff.len())` and never worked.
/// The project could not complete, and the only ways out were abandoning it,
/// demolishing a node, or finding a second program.
///
/// Every other completion test shortcuts both halves (`fill_research_progress`
/// plus `shelve_research_bill`), which is exactly why none of them saw it.
#[test]
fn a_one_program_base_can_finish_a_project_on_its_own() {
    let mut game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    spawn_tamed(&mut game, 10, 3);
    game.select_research("automation").unwrap();

    for _ in 0..4000 {
        game.tick();
    }

    assert!(
        game.is_researched("automation"),
        "a base with one body has to be able to finish what it started \
         (progress {}, orders still standing {:?})",
        research_progress(&game, "automation"),
        game.work_orders()
            .iter()
            .map(|o| (o.item.clone(), o.qty))
            .collect::<Vec<_>>()
    );
}

/// And the half that says why it works: a project with its progress already in
/// stops holding a body at the node, because there is nothing left to feed it.
#[test]
fn a_saturated_project_frees_its_research_node() {
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.select_research("automation").unwrap();
    game.tick();
    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(node),
        "the fixture is vacuous unless the body starts on the node"
    );

    fill_research_progress(&mut game, "automation");
    game.tick();

    assert!(
        game.world
            .get::<Task>(worker)
            .is_none_or(|t| t.target != node),
        "a node with nothing left to feed must not hold the base's only body"
    );
}

/// Refusal 6: a material line nothing in the base could ever make is refused
/// with `work_orders::chain_break`'s own sentence, **verbatim**.
///
/// Its own test because no other one can reach it: `base_with_a_research_node`
/// stands a producer for every item any shipped bill names and everything
/// behind it, and `a_base_with_no_research_node_cannot_take_a_project` is
/// refusal 5. So this stands the Research Node and deliberately nothing else.
///
/// The sentence is fetched by calling `chain_break` rather than hardcoded: the
/// same words appear on the work-order screen, and two spellings of one
/// refusal is the drift this repo keeps recording.
#[test]
fn a_material_with_no_producer_names_the_missing_machine() {
    let mut game = Game::new(738, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    spawn_machine_at(&mut game, "research_node", 2, 2);
    let material = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .unwrap()
        .materials[0]
        .0
        .clone();
    let want = crate::game::base::work_orders::chain_break(&game, &material)
        .expect("nothing is standing that makes it, so the line is broken");

    let err = game.select_research("automation").unwrap_err();

    assert_eq!(
        err, want,
        "the refusal is chain_break's own sentence, not a second wording of it"
    );
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and nothing is filed");
}

/// The two save keys are additive behind `#[serde(default)]`, and this is the
/// half that can actually fail: the real save's own keys, **stripped out of the
/// RON**, rather than a save written by this binary — which always carries them
/// and leaves the test green with both attributes deleted.
///
/// `routes::a_pre_routes_save_loads_with_no_routes`' shape.
#[test]
fn a_save_written_before_projects_existed_loads_with_none() {
    let mut game = Game::new(739, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    unlock_research_chain(&mut game, "automation");
    game.select_research("routine_fabrication").unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .progress
        .insert("routine_fabrication".to_string(), 4);

    let dir = std::env::temp_dir().join(format!("feral_research_pre_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();

    // Emptied first, `routes::a_pre_routes_save_loads_with_no_routes`' shape:
    // both keys then serialise as one line each and can be stripped whole. A
    // populated `research_progress` is a multi-line list, and cutting its
    // header alone leaves its rows orphaned and the RON unparseable.
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.active_research = None;
    data.research_progress.clear();
    let text = crate::save::to_ron(&data).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| {
            let l = l.trim_start();
            !l.starts_with("active_research:") && !l.starts_with("research_progress:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() + 1 < text.lines().count(),
        "both keys must have been there to strip, or this proves nothing"
    );
    let old_path = dir.join("old.bin");
    let old = crate::save::from_ron(&stripped).expect("a pre-project save still parses");
    crate::save::save_to_file(&old_path, &old).unwrap();

    let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(active_research(&loaded), None);
    assert_eq!(research_progress(&loaded, "routine_fabrication"), 0);
    assert!(
        loaded.is_researched("automation"),
        "and what the run already researched is untouched"
    );
}

/// Neither key costs a `SAVE_FORMAT_VERSION` bump: the save is field-named RON,
/// so an absent key is a `#[serde(default)]` away from loading.
#[test]
fn save_format_version_is_unchanged_by_research_projects() {
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        32,
        "two additive fields under field-named RON must not cost a version \
         bump — see the doc comment on SAVE_FORMAT_VERSION"
    );
}

/// A cycle that landed nothing because there was nowhere to put it says
/// nothing, where a *clogged* ordinary node still says so — the clog figure is
/// the only signal a buffer is full.
///
/// The case: a hand-posted standing job on a Research Node with no project
/// selected. "Your subroutine extracted 0 Research Data", every fourteen ticks
/// for the rest of the run, reads as the node being broken; the attention row
/// already says what to do about it, and the player's own instruction is not
/// news.
#[test]
fn a_research_cycle_with_nowhere_to_land_is_not_announced() {
    let mut game = Game::new(740, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    game.set_standing_job(node, true, false).unwrap();

    for _ in 0..600 {
        game.tick();
    }

    let said = game
        .message_history(500)
        .iter()
        .filter(|l| l.text.contains("Research Data"))
        .map(|l| l.text.clone())
        .collect::<Vec<_>>();
    assert!(
        said.is_empty(),
        "a cycle that landed nothing must not announce a payout: {said:?}"
    );
}

/// The contrast that makes the rule above mean something: with a project
/// running, the same node's cycles *are* announced.
#[test]
fn a_research_cycle_that_lands_is_still_announced() {
    let mut game = Game::new(741, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.select_research("automation").unwrap();
    park_at_post(&mut game, worker, node);

    for _ in 0..200 {
        game.tick();
    }

    assert!(
        game.message_history(500)
            .iter()
            .any(|l| l.text.contains("Research Data")),
        "a cycle that fed the project has to say so"
    );
}

/// A project with its progress in and its bill short has one surface and this
/// is it. The bill's work order is *removed* when it completes, so a line
/// consumed before the project settled leaves nothing in the queue for the
/// player to find — and without this row, nothing on any screen says why the
/// base has stopped.
#[test]
fn a_project_waiting_on_materials_asks_for_the_player() {
    let mut game = Game::new(742, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");

    let stalled = |game: &mut Game| {
        game.attention()
            .iter()
            .any(|r| r.kind == AttentionKind::ResearchStalled)
    };
    assert!(
        stalled(&mut game),
        "progress in and an empty base is exactly the stall this row is for"
    );

    let shelf = shelve_research_bill(&mut game, "automation", 300, -300);
    assert!(shelf != Entity::PLACEHOLDER);

    assert!(
        !stalled(&mut game),
        "and it stops the moment the base is holding the bill"
    );
}

/// It names the item, because "research is stalled" without saying on what is a
/// row that cannot be acted on.
#[test]
fn the_stalled_row_names_what_the_project_is_short_of() {
    let mut game = Game::new(743, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");
    let want = game
        .item_name(&ItemId::from(ids::CORE_FRAGMENT))
        .to_string();

    let row = game
        .attention()
        .into_iter()
        .find(|r| r.kind == AttentionKind::ResearchStalled)
        .expect("the project is stalled");

    assert!(row.text.contains(&want), "got: {}", row.text);
}

/// A project still *earning* is not stalled — the row is about the material
/// gate, and firing it while the Research Nodes are still working would read as
/// the base being broken on every project from the moment it was picked.
#[test]
fn a_project_still_earning_is_not_stalled() {
    let mut game = Game::new(744, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();

    assert!(
        !game
            .attention()
            .iter()
            .any(|r| r.kind == AttentionKind::ResearchStalled),
        "a project that has earned nothing yet is working, not stuck"
    );
}
