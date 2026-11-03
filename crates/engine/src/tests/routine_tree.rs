//! The routine research tree: its gate, its listing and its place beside the
//! base tree. The part nodes themselves are held by `tests/routine_craft.rs`.

use crate::research::ResearchTree;
use crate::routine_craft::{Core, Subroutine, Version};
use crate::routine_tree::part_node_id;
use crate::tests::support::{
    base_with_a_research_node, discover_all_research, stand_in_base, test_assets_dir,
    unlock_research_chain,
};
use crate::views::ResearchState;
use crate::{DifficultyMode, DiscoverySource, Game};

fn open_routine_tree(game: &mut Game) {
    unlock_research_chain(game, "routine_fabrication");
}

fn listed_routine_ids(game: &Game) -> std::collections::HashSet<String> {
    game.research_nodes(ResearchTree::Routines)
        .into_iter()
        .map(|n| n.id)
        .collect()
}

/// A zone-1 part that a new game has not researched.
fn damage_node() -> String {
    part_node_id(&Subroutine::Core(Core::Damage, Version::V1))
}

#[test]
fn the_closed_tree_lists_nothing_and_select_research_refuses() {
    let mut game = Game::new(9136, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    assert!(!game.routine_tree_open());
    assert!(listed_routine_ids(&game).is_empty());
    let err = game.select_research(&damage_node()).unwrap_err();
    assert!(err.contains("Routine Fabrication"), "got: {err}");
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .is_none(),
        "the closed-tree refusal must file nothing"
    );
}

/// No research node grants a routine: the whole routine tree is part nodes
/// and capacity steps (ADR 0029).
#[test]
fn the_routine_tree_holds_only_part_and_capacity_nodes() {
    let game = Game::new(9141, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::research::ResearchDb>();
    let routine_nodes: Vec<&str> = db
        .all()
        .filter(|d| d.tree == ResearchTree::Routines)
        .map(|d| d.id.as_str())
        .collect();
    assert!(!routine_nodes.is_empty());
    for id in routine_nodes {
        assert!(
            id.starts_with("part/") || id.starts_with("capacity/"),
            "{id} is a routine-tree node that is neither a part nor a capacity step"
        );
    }
}

#[test]
fn the_open_tree_lists_the_parts_of_the_zone_and_none_above_it() {
    let mut game = Game::new(9137, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    open_routine_tree(&mut game);
    discover_all_research(&mut game);
    let listed = listed_routine_ids(&game);
    assert!(listed.contains(&damage_node()), "{listed:?}");
    assert!(
        !listed.contains(&crate::routine_tree::capacity_node_id(
            crate::tuning::CRAFT_PART_CAP_MAX
        )),
        "a capacity step above the party's zone is absent"
    );
}

#[test]
fn with_no_flagged_node_loaded_the_tree_is_open() {
    let body = std::fs::read_to_string(
        crate::tests::support::test_assets_dir()
            .join("research")
            .join("routine_fabrication.ron"),
    )
    .unwrap();
    assert!(
        body.contains("opens_routine_tree"),
        "fixture assumption: the shipped file carries the flag"
    );
    let stripped = body.replace("opens_routine_tree: true,", "");
    let dir = crate::tests::support::modded_assets_dir(
        "no_opener",
        &[],
        &[],
        &[],
        &[("routine_fabrication.ron", &stripped)],
        &[],
    );
    let game = Game::new(9138, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(
        game.routine_tree_open(),
        "no loaded node carries opens_routine_tree, so the tree is open from the start"
    );
}

/// `research_nodes(tree)` walks `listed_research(tree)`, so a project
/// belonging to the *other* tree never shows as `ResearchState::Active`
/// among either tree's own rows. `Game::active_research_progress` is the
/// unfiltered door both screens' headers use instead.
#[test]
fn active_research_progress_is_unfiltered_by_tree_and_shows_on_the_other_screen() {
    let mut game = Game::new(9150, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    open_routine_tree(&mut game);
    let node = damage_node();
    game.discover_research(&node, DiscoverySource::Study);
    game.select_research(&node).unwrap();

    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .all(|n| n.state != ResearchState::Active),
        "a routine project must not read Active among the base tree's own rows"
    );

    let (name, earned, cost) = game
        .active_research_progress()
        .expect("a routine project is running");
    assert_eq!(earned, 0);
    assert!(cost > 0);

    let base_row = game
        .research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.state == ResearchState::Available)
        .expect("some base node must still read Available with a routine project running");
    let err = game.select_research(&base_row.id).unwrap_err();
    assert_eq!(
        err,
        format!("The base is already working on {name} — abandon it first.")
    );
}

#[test]
fn the_base_tree_lists_exactly_the_authored_nodes() {
    let mut game = Game::new(9139, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
    let rows = game.research_nodes(ResearchTree::Base);
    assert!(!rows.is_empty());
    let research_dir = test_assets_dir().join("research");
    let ron_file_count = std::fs::read_dir(&research_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("ron"))
        .count();
    assert_eq!(rows.len(), ron_file_count);
}

/// `field_ops` is one of the nine nodes task 3 deleted — a save made before
/// that deletion (or before a mod's own tree change) could carry it as the
/// active project forever, with no door that ever re-selects on its own.
#[test]
fn a_stale_active_research_clears_on_load() {
    let mut game = Game::new(9140, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .id = Some("field_ops".to_string());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_stale_active_research_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).expect("a stale save must still load");
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded
            .world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .is_none(),
        "a project naming a node the loaded tree no longer has must clear"
    );
}

fn boost_attack_node() -> String {
    part_node_id(&Subroutine::Core(Core::BoostAttack, Version::V1))
}

#[test]
fn a_new_game_has_both_starting_parts_researched() {
    let game = Game::new(9142, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for part in crate::tuning::STARTING_PARTS {
        assert!(game.is_researched(&part_node_id(&part)), "{part:?}");
    }
}

#[test]
fn every_part_node_but_the_starting_parts_is_discoverable_and_no_capacity_node_is() {
    let game = Game::new(9143, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let starting: Vec<String> = crate::tuning::STARTING_PARTS
        .iter()
        .map(part_node_id)
        .collect();
    let db = game.world.resource::<crate::research::ResearchDb>();
    for d in db.all().filter(|d| d.tree == ResearchTree::Routines) {
        let expected = d.id.starts_with("part/") && !starting.contains(&d.id);
        assert_eq!(d.discoverable, expected, "{}", d.id);
    }
}

#[test]
fn an_undiscovered_part_is_unlisted_and_refused_until_found() {
    let mut game = Game::new(9144, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    open_routine_tree(&mut game);
    assert!(!listed_routine_ids(&game).contains(&damage_node()));
    assert!(listed_routine_ids(&game).contains(&boost_attack_node()));
    assert_eq!(
        game.select_research(&damage_node()).unwrap_err(),
        "Unknown research."
    );
    assert!(game.discover_research(&damage_node(), DiscoverySource::Study));
    assert!(listed_routine_ids(&game).contains(&damage_node()));
}

/// An older save researched parts before they had to be found.
#[test]
fn a_researched_but_undiscovered_part_stays_listed() {
    let mut game = Game::new(9145, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    open_routine_tree(&mut game);
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert(damage_node());
    assert!(listed_routine_ids(&game).contains(&damage_node()));
}
