use super::*;

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
