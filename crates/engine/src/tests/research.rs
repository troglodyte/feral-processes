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

/// Where a banked payout lands, and the whole of what `ItemDef::banked`
/// buys. A unit that reached the node's buffer would be back on the collect
/// key and inside a neighbouring machine's pull range.
#[test]
fn a_research_cronjob_banks_straight_to_the_player() {
    let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
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
    let before = research_data_held(&game);

    stand_player_at_post(&mut game, node);
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
    // Posted from inside the base and then back out, since the whole point of
    // this test is where the party goes next.
    from_inside_the_base(&mut game, |g| {
        assign_worker_producing(g, ItemId::from(ids::RESEARCH_DATA))
    });
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
            "depot".to_string(),
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
            missing: vec!["Automation".to_string()],
            // Weapon Fabrication is a bootstrap node, so the prereq is the
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
/// Not `support::unlock_research_chain`, which differs in both halves that
/// matter here: it takes the node too, and it funds the chain with a flat
/// 1000 rather than exactly what it spends. Both would make these tests
/// vacuous — the first has nothing left to refuse, and the second leaves the
/// player rich enough that `the_zone_gate_is_refused_before_the_cost` could
/// not tell a zone refusal from a cost one. It also raises `ZoneLevel` to
/// clear the chain's own bands, which is the very thing being tested.
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
        let cost = game
            .world
            .resource::<ResearchDb>()
            .get(&prereq)
            .expect("a resolved prereq")
            .cost;
        grant_research_data(game, cost);
        game.unlock_research(&prereq)
            .unwrap_or_else(|e| panic!("prereq {prereq} should be buyable: {e}"));
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
fn unlock_research_refuses_a_node_above_the_players_zone() {
    let mut game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    research_prereqs_of(&mut game, &gated.id);
    grant_research_data(&mut game, gated.cost);

    let err = game.unlock_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(!game.is_researched(&gated.id));
    // The half that fails if the refusal is ever moved below the payment —
    // without it this passes against a build that charges and then refuses.
    assert_eq!(
        research_data_held(&game),
        gated.cost,
        "a refused unlock must not charge the player"
    );
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
fn the_zone_gate_is_refused_before_the_cost() {
    let mut game = Game::new(720, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    research_prereqs_of(&mut game, &gated.id);
    assert_eq!(
        research_data_held(&game),
        0,
        "the fixture spends exactly what it grants, or this asserts nothing"
    );

    let err = game.unlock_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(
        !err.contains("Research Data"),
        "the zone is the reason, not the balance: {err}"
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
