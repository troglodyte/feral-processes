use super::*;

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

/// `settle_research` writes a part node's id into `Research`, and the part
/// is then craftable (`subroutine_known`).
#[test]
fn completing_a_part_node_marks_it_researched_and_the_part_known() {
    let mut game = Game::new(725, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let part = crate::routine_craft::Subroutine::Core(
        crate::routine_craft::Core::Damage,
        crate::routine_craft::Version::V1,
    );
    let node_id = crate::routine_tree::part_node_id(&part);
    assert!(!game.subroutine_known(&part));
    // Opens the routine tree — `select_research` refuses every routine
    // node until this is researched.
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
    assert!(game.world.resource::<Research>().0.contains(&node_id));
    assert!(game.subroutine_known(&part));
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

    game.discover_research("weapon_bench");
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
    let mut game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
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

/// Underground there is no answer to which machines are standing, so the
/// selection refuses rather than answering from the surface entrance tile —
/// `Game::queue_work_order`'s own reason for the same guard.
#[test]
fn selecting_research_in_the_stack_is_rejected() {
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

    descend(&mut game);
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
/// construction. So the node is its own block — and a block is a flag, not a
/// refusal: the project is taken on, its bill filed, and the row and the
/// corner both say what it waits on.
#[test]
fn a_base_with_no_research_node_takes_a_project_on_and_flags_it() {
    let mut game = Game::new(729, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .cloned()
        .unwrap();

    game.select_research("automation")
        .expect("a missing machine flags the project rather than refusing it");

    assert_eq!(active_research(&game), Some("automation".to_string()));
    assert_eq!(
        game.work_orders().len(),
        def.materials.len(),
        "the bill is filed at selection whatever stands"
    );
    let row = research_node(&game, "automation");
    assert_eq!(row.state, ResearchState::Active);
    let flag = row
        .blocked_by
        .expect("the running project's row carries the flag");
    assert!(
        flag.contains("Research Station"),
        "the flag must name the machine that is missing: {flag}"
    );
    assert_eq!(
        game.research_readout(),
        Some(ResearchReadout::Blocked {
            name: def.name.clone(),
            reason: flag,
        }),
        "the corner names the same thing as the row"
    );
}

/// Taking a project on flagged does not loosen completion: with all its data
/// and its bill unpayable it holds, and it lands the moment the bill can be
/// paid — `settle_research`'s gates are untouched by the selection change.
#[test]
fn a_flagged_project_holds_at_full_data_until_its_bill_can_be_paid() {
    let mut game = Game::new(731, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    spawn_machine_at(&mut game, "research_node", 2, 2);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .cloned()
        .unwrap();
    assert!(
        !def.materials.is_empty(),
        "the fixture is vacuous unless automation has a bill"
    );
    game.select_research("automation").unwrap();
    fill_research_progress(&mut game, "automation");

    game.settle_research();
    assert!(
        !game.is_researched("automation"),
        "full data with nothing to pay the bill from must hold"
    );
    assert_eq!(active_research(&game), Some("automation".to_string()));

    shelve_research_bill(&mut game, "automation", 6, 6);
    game.settle_research();
    assert!(
        game.is_researched("automation"),
        "and the bill arriving is all it was waiting on"
    );
}

/// A flag has to name something to do, and a bill naming a banked item
/// names nothing: no holding can ever pay it, and its producer — the
/// Research Node, for the currency — always `can_progress`, so the filed
/// order would hold a body forever. Reachable only through a mod's bill,
/// and refused, not flagged.
#[test]
fn a_bill_naming_a_banked_item_is_refused_rather_than_flagged() {
    let mut game = Game::new(732, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let currency = game.research_currency();
    let def: crate::research::ResearchDef = ron::from_str(&format!(
        r#"(id: "banked_bill", name: "Banked Bill", description: "d", cost: 1, materials: [("{}", 1)])"#,
        currency.as_str()
    ))
    .unwrap();
    game.world.resource_mut::<ResearchDb>().insert_for_test(def);
    game.discover_research("banked_bill");

    let err = game.select_research("banked_bill").unwrap_err();

    assert!(err.contains("banked"), "got: {err}");
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and nothing is filed");
}

/// A block is what stands between the project and completion, not what
/// stands in the base: a bill the shelves already hold in full is payable
/// whatever machine has since come down, so it flags nothing.
#[test]
fn a_bill_already_on_the_shelves_is_no_block_whatever_stands() {
    let mut game = Game::new(733, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let materials = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .unwrap()
        .materials
        .clone();
    assert!(!materials.is_empty(), "vacuous without a bill");
    shelve_research_bill(&mut game, "automation", 8, 8);
    for (item, _) in &materials {
        for machine in crate::game::base::work_orders::producers_of(&game, item) {
            game.world.despawn(machine);
        }
    }

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "a held bill is paid, not waited on: {:?}",
        game.research_readout()
    );
    assert_eq!(research_node(&game, "automation").blocked_by, None);
}

/// The same rule for the Research Node: once the data is all earned it
/// stands between the project and nothing, so a project at full data with
/// no node reads as what it actually waits on — its bill.
#[test]
fn a_missing_node_is_no_block_once_the_data_is_all_earned() {
    let mut game = Game::new(734, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    for node in crate::game::base::work_orders::producers_of(&game, &game.research_currency()) {
        game.world.despawn(node);
    }
    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { .. })
    ));

    fill_research_progress(&mut game, "automation");

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Stalled { .. })
        ),
        "full data, no node, bill unpaid: the bill is the wait: {:?}",
        game.research_readout()
    );
}

/// The reachability half. Without it the flag above could be permanent and
/// every test around it would still be green.
#[test]
fn building_the_missing_machine_clears_the_flag() {
    let mut game = Game::new(730, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // The bill's own producers stood up, and then the Research Node removed —
    // so the Research Node is the only thing in the way.
    let node = base_with_a_research_node(&mut game);
    game.world.despawn(node);
    game.select_research("automation").unwrap();
    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { .. })
    ));
    assert!(research_node(&game, "automation").blocked_by.is_some());

    spawn_machine_at(&mut game, "research_node", 2, 2);

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "a Research Node is all that was missing: {:?}",
        game.research_readout()
    );
    assert_eq!(research_node(&game, "automation").blocked_by, None);
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
    spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 4, 4);
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
