use super::*;

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

/// A material line nothing in the base makes yet flags the project with
/// `work_orders::chain_break`'s own sentence, **verbatim** — and takes it on.
///
/// Its own test because no other one can reach it: `base_with_a_research_node`
/// stands a producer for every item any shipped bill names and everything
/// behind it, and `a_base_with_no_research_node_takes_a_project_on_and_flags_it`
/// covers the node. So this stands the Research Node and deliberately nothing
/// else.
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
    let bill = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .unwrap()
        .materials
        .len();

    game.select_research("automation")
        .expect("a missing machine flags the project rather than refusing it");

    assert_eq!(
        research_node(&game, "automation").blocked_by.as_deref(),
        Some(want.as_str()),
        "the flag is chain_break's own sentence, not a second wording of it"
    );
    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { reason, .. }) if reason == want
    ));
    assert_eq!(
        game.work_orders().len(),
        bill,
        "and the bill is filed, to stall until the machine goes up"
    );
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
    // A bootstrap node, so this test stays about the save keys rather than
    // needing a pinned subject the moment a node it named gained the gate.
    game.select_research("power_grid").unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .progress
        .insert("power_grid".to_string(), 4);

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
    assert_eq!(research_progress(&loaded, "power_grid"), 0);
    assert!(
        loaded.is_researched("automation"),
        "and what the run already researched is untouched"
    );
}

/// Neither key costs a `SAVE_FORMAT_VERSION` bump of its own: the save is
/// field-named RON, so an absent key is a `#[serde(default)]` away from
/// loading. This pins the current version, 37, which ruins raised.
#[test]
fn save_format_version_is_unchanged_by_research_projects() {
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        40,
        "these fields add no bump of their own; a changed version is a \
         deliberate bump — see the doc comment on SAVE_FORMAT_VERSION"
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
    let mut game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);

    let weapon_bench = research_node(&game, "weapon_bench").unlocks.unwrap();
    assert!(
        weapon_bench.starts_with("Unlocks: Assembler: Trace Sniffer, "),
        "a structure is named by its own display name, then the machine recipes it gates: {weapon_bench}"
    );
    assert!(
        weapon_bench.contains(", Logic Probe"),
        "and the hand-compiled items it gates: {weapon_bench}"
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
/// It reads flagged, naming the Research Node — a demolished node is
/// exactly when the player needs reminding the project is still selected.
#[test]
fn the_running_project_is_read_out_with_no_node_standing() {
    let mut game = Game::new(743, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    game.select_research("automation").unwrap();
    let nodes = crate::game::base::work_orders::producers_of(&game, &game.research_currency());
    for node in nodes {
        game.world.despawn(node);
    }

    match game.research_readout() {
        Some(ResearchReadout::Blocked { name, reason }) => {
            assert_eq!(name, "Automation");
            assert!(reason.contains("Research Station"), "got: {reason}");
        }
        other => panic!("expected the project flagged on its missing node, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Studying a subject (Task 7): every node from sector 2 up refuses
// selection until a tamed program is standing in a `studies` structure's
// pen — `Game::research_block`'s gate, not a second arm on
// `select_research`, so the row the screen marks blocked and the sentence
// the player is refused with cannot disagree.
// ---------------------------------------------------------------------
