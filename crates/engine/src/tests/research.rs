//! The research tree and the recipes and structures it gates.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::items::DownedProgram;
use crate::views::{PinMark, ResearchReadout};
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
        game.research_nodes(ResearchTree::Base)
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

/// A finished project says so on the notification screen, not only in the
/// base log: research runs while the party is anywhere, and a log line is
/// what gets scrolled past. Named, described and listing what it opened, the
/// last through the same `Game::research_unlocks` sentence the research
/// screen draws.
#[test]
fn a_completed_project_raises_a_notification_naming_what_it_unlocks() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    shelve_research_bill(&mut game, "automation", 8, 8);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");
    let row = game
        .research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.id == "automation")
        .unwrap();
    while game.take_notification().is_some() {}

    game.tick();

    let note = std::iter::from_fn(|| game.take_notification())
        .find(|n| n.title == "Research Complete")
        .expect("completing a project raises its notification");
    assert!(note.body.contains(&row.name), "{:?}", note.body);
    assert!(note.body.contains(&row.description), "{:?}", note.body);
    assert!(
        !note.body.contains('{'),
        "an unfilled hole: {:?}",
        note.body
    );
    let unlocks = row.unlocks.expect("automation unlocks something");
    assert_eq!(note.detail.as_deref(), Some(unlocks.as_str()));
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

/// The shared-helper regression. `settle_research` writes a routine node's
/// ability straight into `KnownRoutines` (spec §1 "Researched means
/// known"), and `grant_research_knowledge` — the tool half — is what the
/// project path calls rather than keeping a copy of either.
#[test]
fn completing_a_routine_node_teaches_its_ability() {
    let mut game = Game::new(725, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // `symlink` is Symlink Party's whole family, with no wild carrier and
    // no species kit slot (see
    // `routine_tree::hyperthread_is_discoverable_and_a_field_routine_
    // family_is_not`), so it is always-visible rather than needing a
    // discovery first — the one thing this test is not about.
    let node_id = "routine/symlink".to_string();
    let ability = "symlink".to_string();
    // Opens the routine tree — `select_research` refuses every routine
    // node until this is researched (spec §2 "Opening the tree").
    unlock_research_chain(&mut game, "routine_fabrication");
    research_prereqs_of(&mut game, &node_id);
    let zone = game
        .world
        .resource::<ResearchDb>()
        .get(&node_id)
        .map(|d| d.min_zone)
        .unwrap_or(0);
    set_zone(&mut game, zone);
    base_with_a_research_node(&mut game);
    shelve_research_bill(&mut game, &node_id, 8, 8);
    game.select_research(&node_id).unwrap();
    fill_research_progress(&mut game, &node_id);

    game.tick();

    assert!(game.is_researched(&node_id));
    assert!(
        game.world
            .resource::<crate::resources::KnownRoutines>()
            .0
            .contains(&ability),
        "completing must teach {ability}, not just mark the node"
    );
    assert!(
        !game.world.resource::<Research>().0.contains(&node_id),
        "a routine node's completion must never touch `Research` — spec §1 \
         'researched means known' is one record, not two"
    );
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
        .research_nodes(ResearchTree::Base)
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
        .research_nodes(ResearchTree::Base)
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
        !err.contains("Research Station"),
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
        .research_nodes(ResearchTree::Base)
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
        err.contains("Research Station"),
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

/// Selecting files one order per material line, marked as the project's
/// own — the provenance that lets them be withdrawn again — and **on top of
/// the queue**, ahead of anything the player had already filed, in the
/// bill's own order.
#[test]
fn selecting_files_one_order_per_material_line_at_the_top() {
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
    let (first_item, _) = &bill[0];
    game.queue_work_order(WorkOrder::batch(first_item.clone(), 1))
        .unwrap();

    game.select_research("automation").unwrap();

    let filed: Vec<(ItemId, u32, bool)> = game
        .work_orders()
        .iter()
        .map(|o| (o.item.clone(), o.qty, o.for_research))
        .collect();
    let mut expected: Vec<(ItemId, u32, bool)> = bill
        .iter()
        .map(|(item, need)| (item.clone(), *need, true))
        .collect();
    expected.push((first_item.clone(), 1, false));
    assert_eq!(
        filed, expected,
        "the project's lines lead, in bill order, above the player's own"
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
        .research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.state == ResearchState::Available && n.blocked_by.is_none())
        .expect("something is open once Automation is in");
    game.select_research(&open.id).unwrap();
    let ranks: Vec<u8> = game
        .research_nodes(ResearchTree::Base)
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
    for node in game.research_nodes(ResearchTree::Base) {
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&node.id)
            .expect("a listed node should exist in the db");
        assert!(
            !def.unlocks_structures.is_empty()
                || !def.unlocks_recipes.is_empty()
                || !def.unlocks_tools.is_empty()
                || def.teaches.is_some(),
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
        // Base tree only: these tests are about the base tree's own zone
        // gating, and `ResearchDb::all()` now also carries one synthesised
        // routine node per eligible ability, many of them roots of their
        // own family with no prerequisite — exactly what
        // `a_node_can_report_both_a_missing_prereq_and_its_zone` needs to
        // *not* pick.
        .find(|d| d.min_zone == zone && d.tree == crate::research::ResearchTree::Base)
        .unwrap_or_else(|| panic!("the shipped base tree should band something at zone {zone}"))
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
    game.research_nodes(ResearchTree::Base)
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
        game.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == gated.id),
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
        !err.contains("Research Station"),
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
        game.research_nodes(ResearchTree::Base)
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

    let rows = game.research_nodes(ResearchTree::Base);

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
        .research_nodes(ResearchTree::Base)
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
    game.research_nodes(ResearchTree::Base)
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
/// so. The stock block folds in every `ItemDef::banked` pool **by the flag**, so
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

/// What a node hands over is derived from the four `unlocks_*` lists, not read
/// off the prose — the description is authored and a mod's is whatever the
/// modder wrote, so the one line the screen can rely on has to come from the
/// lists themselves.
#[test]
fn a_research_node_reports_everything_it_hands_over() {
    let game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        research_node(&game, "weapon_bench").unlocks.as_deref(),
        Some("Unlocks: Fabricator"),
        "a structure is named by its own display name"
    );
    assert_eq!(
        research_node(&game, "ablative").unlocks.as_deref(),
        Some("Unlocks: Ablative Plating"),
        "a recipe is named by what it produces"
    );

    let deep = research_node(&game, "deep_analysis");
    let line = deep.unlocks.expect("a node that teaches tools says so");
    assert_eq!(
        line, "Unlocks: Core Tap, Gear Puller",
        "deep_analysis grants only tools now — its routines moved to the routine tree (todo #101)"
    );
}

/// The corner over the map names the running project. No Research Node and
/// no project is nothing to say; a node standing idle says so, because an
/// idle lab is the state the readout exists to catch.
#[test]
fn the_research_readout_follows_the_project() {
    let mut game = Game::new(741, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.research_readout(), None, "no node, no project");

    base_with_a_research_node(&mut game);
    assert_eq!(game.research_readout(), Some(ResearchReadout::Idle));

    game.select_research("automation").unwrap();
    let row = game
        .research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.id == "automation")
        .unwrap();
    assert_eq!(
        game.research_readout(),
        Some(ResearchReadout::Earning {
            name: row.name.clone(),
            earned: 0,
            cost: row.cost,
        })
    );
}

/// Full progress and an unpaid bill is the stall `research_material_shortfall`
/// already names, and the readout names the same item rather than a figure
/// that has stopped moving.
#[test]
fn a_project_waiting_on_its_bill_reads_as_stalled_on_the_material() {
    let mut game = Game::new(742, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");

    let short_of = game
        .research_material_shortfall()
        .expect("precondition: nothing on the shelves pays the bill");
    let Some(ResearchReadout::Stalled {
        name,
        short_of: named,
    }) = game.research_readout()
    else {
        panic!("expected a stall, got {:?}", game.research_readout());
    };
    assert_eq!(named, short_of);
    assert!(!name.is_empty());
}

/// A project whose node was demolished is still the project: the readout
/// follows `ActiveResearch`, and only the idle line needs a node standing.
#[test]
fn the_running_project_is_read_out_with_no_node_standing() {
    let mut game = Game::new(743, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let nodes = crate::game::base::work_orders::producers_of(&game, &game.research_currency());
    for node in nodes {
        game.world.despawn(node);
    }

    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Earning { .. })
    ));
}

// ---------------------------------------------------------------------
// Studying a subject (Task 7): every node from sector 2 up refuses
// selection until a tamed program is standing in a `studies` structure's
// pen — `Game::research_block`'s gate, not a second arm on
// `select_research`, so the row the screen marks blocked and the sentence
// the player is refused with cannot disagree.
// ---------------------------------------------------------------------

/// The gate lives in `Game::research_block`, so the refusal is asserted
/// against a **live call** to it rather than hardcoded prose — the same
/// drift this repo keeps recording for `chain_break`.
#[test]
fn a_subject_gated_node_is_refused_without_a_pinned_subject() {
    let mut game = Game::new(4400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .cloned()
        .expect("paging ships and is gated at zone 2 with no other prereqs");
    assert!(
        def.requires_subject,
        "the fixture is vacuous unless paging is actually gated"
    );
    let want = game
        .research_block(&def)
        .expect("nobody is pinned yet, so the gate must be live");

    let err = game.select_research("paging").unwrap_err();

    assert_eq!(
        err, want,
        "select_research's refusal must be research_block's own sentence"
    );
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and nothing is filed");
}

/// **A reachability test, not only a refusal test.** A refusal can ship
/// permanent with every refusal test still green — pinning a program in the
/// pen must make the identical selection succeed.
#[test]
fn pinning_a_subject_makes_a_subject_gated_selection_succeed() {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);

    game.select_research("paging")
        .expect("a subject standing in the pen must clear the gate");

    assert_eq!(active_research(&game), Some("paging".to_string()));
}

/// The strain mark's three terms, asserted one at a time: a subject-gated
/// project that is **earning** is the only state that stresses a body.
///
/// Read off the pin alone this would have been a second answer to "which body
/// is under study" — the fault `view_pinned_at`'s settle gate already closed
/// once, with the brackets latching at selection and riding the whole walk to
/// the pen. `Strained` is the same walk's answer, narrowed.
#[test]
fn an_earning_subject_gated_project_strains_its_subject() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;
    let at_pen = |game: &Game| game.view_pinned_at(pen, half, half)[half as usize][half as usize];

    // Pinned, nothing selected: held, not worked.
    assert_eq!(at_pen(&game), PinMark::Settled);

    game.select_research("paging")
        .expect("a subject standing in the pen clears the gate");

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "the fixture is vacuous unless the project is actually earning"
    );
    assert_eq!(
        at_pen(&game),
        PinMark::Strained,
        "the body a subject-gated project is spending strains while it earns"
    );
}

/// The stall is `research_material_shortfall`'s, and the mark reads it through
/// `research_readout` rather than testing for it a second time — so a project
/// parked on an unpaid bill lets its subject go still.
#[test]
fn a_project_stalled_on_its_bill_lets_its_subject_settle() {
    let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;
    let at_pen = |game: &Game| game.view_pinned_at(pen, half, half)[half as usize][half as usize];

    game.select_research("paging").unwrap();
    assert_eq!(at_pen(&game), PinMark::Strained);

    fill_research_progress(&mut game, "paging");

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Stalled { .. })
        ),
        "precondition: nothing on the shelves pays paging's bill"
    );
    assert_eq!(
        at_pen(&game),
        PinMark::Settled,
        "a project waiting on a material line is not working its subject"
    );
}

/// A project that declares no `requires_subject` spends no body, however many
/// are pinned — the first of the three terms, and the one a mark derived from
/// "is research running" alone would miss.
#[test]
fn an_ungated_project_leaves_a_pinned_body_settled() {
    let mut game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;

    game.select_research("automation")
        .expect("automation is ungated");
    assert!(
        !game
            .world
            .resource::<ResearchDb>()
            .get("automation")
            .unwrap()
            .requires_subject,
        "the fixture is vacuous unless automation really spends no subject"
    );
    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "and unless it is actually earning"
    );

    assert_eq!(
        game.view_pinned_at(pen, half, half)[half as usize][half as usize],
        PinMark::Settled,
    );
}

/// **Exactly one body strains, and it is the gate's own.** A second station
/// with its own settled subject keeps plain brackets: `Game::pinned_subject`
/// names the `(x, y)`-sorted first station's occupant, and that is the body
/// `Game::settle_research` will spend — so the mark cannot point at a
/// different program than the one being consumed.
#[test]
fn a_second_stations_subject_stays_settled_while_the_first_strains() {
    let mut game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    // Sorted **after** the first station's `(2, 2)`, so `study_station` keeps
    // naming the first one and the gate keeps naming its body — which is the
    // asymmetry under test. Its own floor is laid explicitly: the starting
    // pocket does not reach a second 2x2 machine, and `pin_subject` refuses a
    // pen with no floor under it.
    for x in 2..=5 {
        for y in -1..=1 {
            game.world.resource_mut::<BaseGrid>().lay_floor(x, y);
        }
    }
    let second = spawn_machine_at(&mut game, "research_node", 4, 0);
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 5);
    let first_pen = game.study_pen(first).unwrap();
    let second_pen = game.study_pen(second).unwrap();
    // The run's starting program wanders the base and may be standing on a
    // pen, which `pin_subject` refuses outright. Nothing about that refusal
    // is under test here.
    for (body, pos) in game.base_bodies() {
        if (pos.x, pos.y) == first_pen || (pos.x, pos.y) == second_pen {
            let mut p = game.world.get_mut::<Position>(body).unwrap();
            p.x = 0;
            p.y = 0;
        }
    }
    pin_subject_at_pen(&mut game, a, first);
    pin_subject_at_pen(&mut game, b, second);

    game.select_research("paging").unwrap();

    assert_eq!(
        game.pinned_subject(),
        Some(a),
        "the fixture is vacuous unless the gate names the first station's body"
    );
    // One window wide enough to hold both pens, so the two answers come out
    // of a single walk rather than two.
    let half = 24;
    let mark = |pen: (i32, i32)| {
        game.view_pinned_at(first_pen, half, half)[(pen.1 - first_pen.1 + half) as usize]
            [(pen.0 - first_pen.0 + half) as usize]
    };
    assert_eq!(mark(first_pen), PinMark::Strained);
    assert_eq!(mark(second_pen), PinMark::Settled);
}

/// The eight nodes with no `min_zone` gate — what gets a base running —
/// stay selectable with nobody pinned. Looped rather than named one at a
/// time, so a mod or a retune that grows the ungated set is covered for
/// free.
#[test]
fn every_ungated_node_is_selectable_with_nobody_pinned() {
    let ungated: Vec<String> = {
        let game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.tree == ResearchTree::Base && !d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !ungated.is_empty(),
        "the fixture is vacuous with nothing ungated"
    );
    for id in ungated {
        let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let zone = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .expect("walked out of the same db")
            .min_zone;
        set_zone(&mut game, zone);

        game.select_research(&id)
            .unwrap_or_else(|e| panic!("{id} should be selectable with nobody pinned: {e}"));
    }
}

/// Every node with `min_zone >= 2` is refused without a subject, and the
/// same pin makes it reachable — the census that `decision 13`'s 19/8 split
/// actually behaves as the refusal test above shows for one node.
#[test]
fn every_subject_gated_node_refuses_selection_without_a_pinned_subject() {
    let gated: Vec<String> = {
        let game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !gated.is_empty(),
        "the fixture is vacuous with nothing gated"
    );
    for id in gated {
        let mut game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .cloned()
            .expect("walked out of the same db");
        set_zone(&mut game, def.min_zone);
        let want = game
            .research_block(&def)
            .unwrap_or_else(|| panic!("{id} should be blocked with nobody pinned"));

        let err = game
            .select_research(&id)
            .expect_err(&format!("{id} should be refused with nobody pinned"));

        assert_eq!(
            err, want,
            "{id}'s refusal must be research_block's own sentence"
        );
    }
}

/// The third refusal on `Game::unpin_subject`: pulling the subject out from
/// under a project that needs it is refused, and abandoning the project is
/// how you change your mind — `select_research`'s "already active" refusal,
/// in shape.
#[test]
fn unpin_subject_is_refused_while_a_subject_gated_project_is_active() {
    let mut game = Game::new(4406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging").unwrap();

    let err = game
        .unpin_subject(program)
        .expect_err("the active project still needs this subject");
    assert!(err.contains("abandon"), "unexpected error: {err}");
    assert!(
        game.world
            .get::<crate::components::UnderStudy>(program)
            .is_some(),
        "the refusal must not have unpinned it"
    );

    game.abandon_research().unwrap();
    game.unpin_subject(program)
        .expect("abandoning the project frees the subject to be unpinned");
}

// ---------------------------------------------------------------------
// C1 (final whole-branch review): `research_nodes` must apply the same
// subject term `select_research` refuses on. Task 7's own tests compared
// `select_research`'s refusal against a live `research_block` call — the
// same door twice — which is why a subject gate could reach `select_research`
// while `research_nodes` (the screen) still read `research_block_memo`
// directly and never asked the subject question at all. These tests drive
// `research_nodes` itself.
// ---------------------------------------------------------------------

/// The screen's own row must carry the same block reason the selection
/// door refuses with, and pinning a subject must clear it there too — not
/// only at `select_research`.
#[test]
fn research_nodes_blocks_a_subject_gated_node_the_same_way_select_research_does() {
    let mut game = Game::new(4407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);

    let def = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .cloned()
        .expect("paging ships and is gated at zone 2 with no other prereqs");
    assert!(
        def.requires_subject,
        "the fixture is vacuous unless paging is actually gated"
    );

    let want = game
        .research_block(&def)
        .expect("nobody is pinned yet, so the gate must be live");

    let row = research_node(&game, "paging");
    assert_eq!(
        row.blocked_by,
        Some(want),
        "research_nodes's row must carry the same block reason select_research refuses with"
    );

    // Reachability: pinning a subject must clear the SCREEN's block line,
    // not only make `select_research` succeed.
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let row = research_node(&game, "paging");
    assert_eq!(
        row.blocked_by, None,
        "pinning a subject must clear research_nodes's block line too"
    );
}

/// The eight ungated nodes must still show no block line from the screen
/// with nobody pinned — companion census to the test above, so a subject
/// term applied unconditionally would be caught here.
#[test]
fn research_nodes_reports_no_block_for_every_ungated_node_with_nobody_pinned() {
    let ungated: Vec<String> = {
        let game = Game::new(4410, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.tree == ResearchTree::Base && !d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !ungated.is_empty(),
        "the fixture is vacuous with nothing ungated"
    );
    for id in ungated {
        let mut game = Game::new(4411, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let zone = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .expect("walked out of the same db")
            .min_zone;
        set_zone(&mut game, zone);
        let row = research_node(&game, &id);
        assert_eq!(
            row.blocked_by, None,
            "{id} should show no block on the screen with nobody pinned"
        );
    }
}

// ---------------------------------------------------------------------
// M1 (final whole-branch review): `Game::release_study_station` must not
// over-abandon, and must not silently swallow `abandon_research`'s refusal.
// ---------------------------------------------------------------------

/// Destroying a Station that happens to hold a subject must not deselect an
/// *unrelated, ungated* project — `release_study_station` used to call
/// `abandon_research` whenever the destroyed structure held any subject at
/// all, regardless of whether the active project needed one.
#[test]
fn releasing_a_stations_subject_does_not_abandon_an_ungated_project() {
    let mut game = Game::new(4700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    game.select_research("automation")
        .expect("automation is ungated and should be selectable immediately");
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    assert_eq!(
        active_research(&game),
        Some("automation".to_string()),
        "precondition"
    );

    game.release_study_station(node);

    assert_eq!(
        active_research(&game),
        Some("automation".to_string()),
        "an ungated project must survive the destruction of a station that merely \
         happened to be holding an unrelated pinned subject"
    );
}

/// `abandon_research` refuses during an active battle — its own first
/// rung — and `release_study_station` used to discard that refusal with a
/// bare `let _`, leaving a subject-gated project active with no subject and
/// no word to the player anywhere in the log.
#[test]
fn releasing_a_subjects_station_during_a_battle_does_not_swallow_the_refusal() {
    let mut game = Game::new(4701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging")
        .expect("the pinned subject clears the gate");

    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    assert!(game.has_active_battle(), "precondition");

    let before = game.message_log(50).len();
    game.release_study_station(node);

    assert_eq!(
        active_research(&game),
        Some("paging".to_string()),
        "abandon_research must have refused during the battle, leaving the project active"
    );
    let after = game.message_log(50);
    // The pen-release line alone always fires — `release_study_station`'s
    // first, unconditional log — so the refusal must add a *second* line on
    // top of it, not merely leave that one line standing.
    assert_eq!(
        after.len(),
        before + 2,
        "the refusal must not be silently discarded — a second line must say so, \
         beside the pen-release line: {after:?}"
    );
}

// ---------------------------------------------------------------------
// Completion spends the subject (Task 8)
// ---------------------------------------------------------------------

/// A minimal filler row for `components::DownedPrograms`, used only to
/// saturate the store — nothing about its content matters to the tests
/// that spawn it.
fn filler_downed_program() -> DownedProgram {
    DownedProgram {
        species: "test_generic".to_string(),
        level: 1,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 50,
        carried: None,
    }
}

/// Decision 5's short-circuit, the subject half: full progress and a full
/// bill do not complete a subject-gated project once its subject is gone —
/// `a_full_bill_alone_does_not_complete_a_project`'s failure, with a worse
/// loss, since a program is not refundable the way a shelf material is.
///
/// The subject leaves by a direct removal rather than through
/// `unpin_subject`, which now refuses this exact case — the door under test
/// here is `settle_research`'s own gate.
#[test]
fn a_subject_gated_project_does_not_complete_without_a_pinned_subject() {
    let mut game = Game::new(4500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging").unwrap();
    game.world
        .entity_mut(program)
        .remove::<crate::components::UnderStudy>();
    let shelf = shelve_research_bill(&mut game, "paging", 8, 8);
    let stocked = node_output(&game, shelf, ids::BYTECODE_BLOCK);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(!game.is_researched("paging"));
    assert_eq!(
        node_output(&game, shelf, ids::BYTECODE_BLOCK),
        stocked,
        "a project missing its subject must not spend its materials either"
    );
}

/// M2 (final whole-branch review): once progress is full and the bill is
/// paid, a subject-gated project with no pinned subject is exactly as
/// stalled as one short a material or a full `DownedPrograms` store —
/// `settle_research`'s own ordering already treats it that way — but
/// `research_material_shortfall` never said so, so the HUD read "Earning
/// n/cost" forever with no stall line once the subject was gone. The
/// subject leaves by direct removal, matching
/// `a_subject_gated_project_does_not_complete_without_a_pinned_subject`
/// above — reachable in play via M1's battle case, a second Station
/// resolving first, or the subject simply being walked off the pen.
#[test]
fn a_subject_gated_project_with_no_pinned_subject_reads_as_stalled() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging").unwrap();
    game.world
        .entity_mut(program)
        .remove::<crate::components::UnderStudy>();
    // The bill is fully paid, so a material line cannot be the reason this
    // reads as a stall — isolating the subject as the only thing missing.
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");
    assert_eq!(
        game.pinned_subject(),
        None,
        "precondition: nobody is in the pen"
    );

    let shortfall = game.research_material_shortfall().expect(
        "a full bill and full progress with no pinned subject is exactly as stalled \
         as one short a material",
    );
    assert!(!shortfall.is_empty(), "the stall must name something");

    match game.research_readout() {
        Some(ResearchReadout::Stalled { short_of, .. }) => {
            assert_eq!(
                short_of, shortfall,
                "the readout must report the same stall research_material_shortfall does"
            );
        }
        other => panic!("expected a Stalled readout, got {other:?}"),
    }
}

/// The conversion: `downed_program_for_with_overkill(subject, 0.0)` →
/// `push_downed_program` → despawn. The level comes off the subject's real
/// `Experience`, not `ZoneLevel` — `ability_user_level`'s distinction from a
/// wild kill — and a routine installed on the subject comes back as
/// `carried`, since nothing about the test species' (declared-nothing) kit
/// claims it.
#[test]
fn completing_a_subject_gated_project_spends_the_pinned_subject() {
    let mut game = Game::new(4501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Experience>(program).unwrap().level = 4;
    let species = game.world.get::<Creature>(program).unwrap().species.clone();
    let prize = game
        .world
        .resource::<AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(def, _)| def.id.clone())
        .next()
        .expect("some shipped ability is wild-poolable");
    game.world.get_mut::<Routines>(program).unwrap().0 = vec![prize.clone()];
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(game.is_researched("paging"));
    assert!(
        game.world.get_entity(program).is_err(),
        "the subject leaves the world when it is spent"
    );
    let held = game
        .world
        .get::<DownedPrograms>(game.player_entity())
        .unwrap()
        .0
        .clone();
    assert_eq!(held.len(), 1, "exactly one DownedProgram must appear");
    assert_eq!(held[0].species, species);
    assert_eq!(
        held[0].level, 4,
        "level must come from the subject's real Experience, not ZoneLevel"
    );
    assert_eq!(
        held[0].carried,
        Some(prize),
        "a routine installed on the subject comes back as carried"
    );
}

/// A full `DownedPrograms` store refuses the conversion, and that refusal
/// blocks the whole completion — reported the way a material shortfall is,
/// through `Game::research_material_shortfall` — rather than eating the
/// body and discovering the push failed afterward.
#[test]
fn a_full_downed_programs_store_blocks_a_subject_gated_completion() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.select_research("paging").unwrap();
    let shelf = shelve_research_bill(&mut game, "paging", 8, 8);
    let stocked = node_output(&game, shelf, ids::BYTECODE_BLOCK);
    fill_research_progress(&mut game, "paging");
    let cost = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .unwrap()
        .cost;
    {
        let player = game.player_entity();
        let mut held = game.world.get_mut::<DownedPrograms>(player).unwrap();
        held.0 = (0..crate::tuning::MAX_DOWNED_PROGRAMS)
            .map(|_| filler_downed_program())
            .collect();
    }
    assert_eq!(
        game.research_material_shortfall(),
        Some("room for another downed program".to_string()),
        "a full store must be reported before the bill is even considered"
    );

    game.tick();

    assert!(
        !game.is_researched("paging"),
        "a full store must block completion"
    );
    assert!(
        game.world
            .get::<crate::components::UnderStudy>(program)
            .is_some(),
        "the subject must still be pinned — nothing was spent"
    );
    assert_eq!(
        research_progress(&game, "paging"),
        cost,
        "the progress must be intact"
    );
    assert_eq!(
        node_output(&game, shelf, ids::BYTECODE_BLOCK),
        stocked,
        "the materials must be intact too"
    );
}
