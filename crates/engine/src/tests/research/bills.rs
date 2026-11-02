use super::*;

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
/// test can assert the shelves are byte-identical.
pub(super) fn all_shelf_stock(game: &Game) -> Vec<(ItemId, u32)> {
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
fn a_blocked_node_carries_the_sentence_it_is_flagged_with() {
    let mut game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("automation")
        .cloned()
        .unwrap();
    let want = game.research_block(&def).expect("nothing stands");

    let row = research_node(&game, "automation");
    assert_eq!(row.state, ResearchState::Available);
    assert_eq!(row.blocked_by.as_deref(), Some(want.as_str()));

    // And the sentence rides the row into `Active`: the flag is what says
    // the running project is waiting, so it cannot drop off at selection.
    game.select_research("automation").unwrap();
    let row = research_node(&game, "automation");
    assert_eq!(row.state, ResearchState::Active);
    assert_eq!(row.blocked_by.as_deref(), Some(want.as_str()));
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
    let mut game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    discover_all_research(&mut game);

    let node = research_node(&game, "ablative");

    assert_eq!(
        node.conversions,
        vec!["Portal Fragment x12, Cache Grain x3 into Ablative Plating.".to_string()],
        "a recipe's own cost is what the line quotes, quantities included"
    );
}

#[test]
fn a_research_node_that_unlocks_no_conversion_reports_none() {
    let mut game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    discover_all_research(&mut game);

    let node = research_node(&game, "paging");

    assert!(
        node.conversions.is_empty(),
        "a Depot converts nothing and must add no line: {:?}",
        node.conversions
    );
}

pub(super) fn research_node(game: &Game, id: &str) -> ResearchStatus {
    game.research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("{id:?} should be a shipped research node"))
}
