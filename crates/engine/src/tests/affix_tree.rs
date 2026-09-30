//! The Affixes research tree: synthesised nodes, the gate, visibility and
//! discovery. Uses affixes of its own (`t_*`) so a test never depends on
//! what the shipped tree happens to contain.

use crate::affixes::AffixDb;
use crate::research::{ResearchDb, ResearchDef, ResearchTree};
use crate::resources::{DiscoveredResearch, Research};
use crate::tests::support::{modded_assets_dir, stand_in_base, test_assets_dir};
use crate::{DifficultyMode, Game};

const ALPHA: &str = r#"(id: "t_alpha", prefix: Some("Alpha"), stats: (deflection: 1),
    slots: Some([Armor]),
    research: Some((cost: 11, materials: [("logic_wafer", 2)], min_zone: 0,
                    apply_cost: [("core_fragment", 1)])))"#;
const BETA: &str = r#"(id: "t_beta", suffix: Some("of Beta"), stats: (decompiler: 1),
    research: Some((cost: 22, requires: ["t_alpha"],
                    apply_cost: [("core_fragment", 1)])))"#;
const GATE: &str = r#"(id: "t_gate", name: "Test Gate", description: "g", cost: 1,
    opens_affix_tree: true)"#;

/// The shipped Mod Bench node with its gate flag removed — the lenient rule's
/// own fixture, so a test of the tree sees it open from the start unless it
/// installs a gate of its own.
fn ungated_mod_bench() -> String {
    let body =
        std::fs::read_to_string(test_assets_dir().join("research").join("mod_bench.ron")).unwrap();
    assert!(
        body.contains("opens_affix_tree: true,"),
        "fixture assumption"
    );
    body.replace("opens_affix_tree: true,", "")
}

fn affix_game(tag: &str, seed: u32, affixes: &[(&str, &str)], research: &[(&str, &str)]) -> Game {
    let mut research = research.to_vec();
    let stripped = ungated_mod_bench();
    research.push(("mod_bench.ron", &stripped));
    let dir = modded_assets_dir(tag, &[], &[], &[], &research, &[]);
    for (name, body) in affixes {
        std::fs::write(dir.join("affixes").join(name), body).unwrap();
    }
    Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap()
}

fn node(game: &Game, id: &str) -> ResearchDef {
    game.world
        .resource::<ResearchDb>()
        .get(id)
        .unwrap_or_else(|| panic!("no node {id}"))
        .clone()
}

fn research(game: &mut Game, id: &str) {
    game.world
        .resource_mut::<Research>()
        .0
        .insert(id.to_string());
}

fn discover(game: &mut Game, id: &str) {
    game.world
        .resource_mut::<DiscoveredResearch>()
        .0
        .insert(id.to_string());
}

fn listed(game: &Game) -> Vec<String> {
    game.research_nodes(ResearchTree::Affixes)
        .into_iter()
        .map(|n| n.id)
        .collect()
}

#[test]
fn a_research_only_affix_becomes_a_hidden_node_carrying_its_block() {
    let game = affix_game("tree_fields", 9301, &[("t_alpha.ron", ALPHA)], &[]);
    let n = node(&game, "affix:t_alpha");
    assert_eq!(n.tree, ResearchTree::Affixes);
    assert_eq!(n.cost, 11);
    assert_eq!(n.materials, vec![("logic_wafer".into(), 2)]);
    assert_eq!(n.min_zone, 0);
    assert!(n.discoverable, "an affix node is hidden until found");
    assert!(!n.requires_subject);
    assert!(n.teaches.is_none(), "researched state is the Research set");
    assert_eq!(n.name, "Alpha");
    assert!(n.description.contains("Armor"), "{}", n.description);
}

#[test]
fn an_affix_with_no_research_block_gets_no_node() {
    let game = affix_game("tree_no_block", 9302, &[], &[]);
    let db = game.world.resource::<ResearchDb>();
    let mut nodes: Vec<String> = db
        .all()
        .filter(|d| d.tree == ResearchTree::Affixes)
        .map(|d| d.id.clone())
        .collect();
    let mut expected: Vec<String> = game
        .affix_defs()
        .iter()
        .filter(|d| d.research.is_some())
        .map(|d| format!("affix:{}", d.id.as_str()))
        .collect();
    nodes.sort();
    expected.sort();
    assert!(
        !expected.is_empty(),
        "the shipped set has research-only affixes"
    );
    assert_eq!(nodes, expected, "a drop-pool affix must not become a node");
}

#[test]
fn requires_maps_affix_ids_to_node_ids() {
    let game = affix_game(
        "tree_requires",
        9303,
        &[("t_alpha.ron", ALPHA), ("t_beta.ron", BETA)],
        &[],
    );
    assert_eq!(
        node(&game, "affix:t_beta").requires,
        vec!["affix:t_alpha".to_string()]
    );
}

#[test]
fn a_cycle_among_research_only_affixes_is_dropped_by_the_research_load() {
    let cyc_a = r#"(id: "t_ca", prefix: Some("Ca"), stats: (deflection: 1),
        research: Some((cost: 5, requires: ["t_cb"], apply_cost: [("core_fragment", 1)])))"#;
    let cyc_b = r#"(id: "t_cb", prefix: Some("Cb"), stats: (deflection: 1),
        research: Some((cost: 5, requires: ["t_ca"], apply_cost: [("core_fragment", 1)])))"#;
    let dir = crate::tests::support::scratch_assets_dir("tree_cycle");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t_ca.ron"), cyc_a).unwrap();
    std::fs::write(dir.join("t_cb.ron"), cyc_b).unwrap();
    let assets = test_assets_dir();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    let (affixes, aw) = AffixDb::load_dir(&dir, &items).unwrap();
    assert!(aw.is_empty(), "AffixDb does not see a cycle: {aw:?}");
    let (structures, _) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (tools, _) = crate::tools::ToolDb::load_dir(&assets.join("tools")).unwrap();
    let (db, warnings) = ResearchDb::load_dir(
        &assets.join("research"),
        &structures,
        &abilities,
        &tools,
        &affixes,
    )
    .unwrap();
    assert!(db.get("affix:t_ca").is_none() && db.get("affix:t_cb").is_none());
    assert_eq!(
        warnings.iter().filter(|w| w.contains("cycle")).count(),
        2,
        "{warnings:?}"
    );
}

#[test]
fn a_flagged_node_gates_the_tree_and_no_flagged_node_leaves_it_open() {
    let mut game = affix_game("tree_gate", 9304, &[("t_alpha.ron", ALPHA)], &[]);
    assert!(
        game.affix_tree_open(),
        "no loaded node carries opens_affix_tree, so the tree is open"
    );
    let gate: ResearchDef = ron::from_str(GATE).unwrap();
    game.world
        .resource_mut::<ResearchDb>()
        .insert_for_test(gate);
    assert!(
        !game.affix_tree_open(),
        "a flagged, unresearched node shuts it"
    );
    research(&mut game, "t_gate");
    assert!(game.affix_tree_open());
}

#[test]
fn affix_researched_reads_the_node() {
    let mut game = affix_game("tree_researched", 9305, &[("t_alpha.ron", ALPHA)], &[]);
    let alpha = "t_alpha".into();
    assert!(!game.affix_researched(&alpha));
    research(&mut game, "affix:t_alpha");
    assert!(game.affix_researched(&alpha));
    assert!(!game.affix_researched(&"no_such".into()));
}

#[test]
fn the_tree_lists_nothing_while_shut_and_only_found_nodes_once_open() {
    let mut game = affix_game(
        "tree_listing",
        9306,
        &[("t_alpha.ron", ALPHA), ("t_beta.ron", BETA)],
        &[],
    );
    let gate: ResearchDef = ron::from_str(GATE).unwrap();
    game.world
        .resource_mut::<ResearchDb>()
        .insert_for_test(gate);
    discover(&mut game, "affix:t_alpha");
    research(&mut game, "affix:t_beta");
    assert!(
        listed(&game).is_empty(),
        "shut: nothing, known nodes included"
    );
    research(&mut game, "t_gate");
    let ids = listed(&game);
    assert!(ids.contains(&"affix:t_alpha".to_string()), "discovered");
    assert!(ids.contains(&"affix:t_beta".to_string()), "researched");
}

#[test]
fn an_open_tree_still_hides_undiscovered_nodes() {
    let game = affix_game("tree_listing2", 9307, &[("t_alpha.ron", ALPHA)], &[]);
    assert!(game.affix_tree_open());
    assert!(listed(&game).is_empty());
}

#[test]
fn select_research_refuses_a_hidden_or_shut_affix_node() {
    let mut game = affix_game("tree_select", 9308, &[("t_alpha.ron", ALPHA)], &[]);
    stand_in_base(&mut game);
    assert_eq!(
        game.select_research("affix:t_alpha").unwrap_err(),
        "Unknown research.",
        "open but not discovered"
    );
    discover(&mut game, "affix:t_alpha");
    let gate: ResearchDef = ron::from_str(GATE).unwrap();
    game.world
        .resource_mut::<ResearchDb>()
        .insert_for_test(gate);
    assert_eq!(
        game.select_research("affix:t_alpha").unwrap_err(),
        "Unknown research.",
        "discovered but the tree is shut"
    );
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .is_none()
    );
    research(&mut game, "t_gate");
    assert_ne!(
        game.select_research("affix:t_alpha").unwrap_err(),
        "Unknown research.",
        "open and discovered: past visibility, on to the ordinary bill checks"
    );
}

#[test]
fn the_discovery_pool_admits_affix_nodes_only_once_the_gate_is_researched() {
    let mut game = affix_game("tree_pool", 9309, &[("t_alpha.ron", ALPHA)], &[]);
    let gate: ResearchDef = ron::from_str(GATE).unwrap();
    game.world
        .resource_mut::<ResearchDb>()
        .insert_for_test(gate);
    let before = game.eligible_discoveries();
    assert!(
        !before.iter().any(|id| id.starts_with("affix:")),
        "{before:?}"
    );
    research(&mut game, "t_gate");
    let after = game.eligible_discoveries();
    assert!(after.contains(&"affix:t_alpha".to_string()), "{after:?}");
    // Nothing else moved: the gate's own opening removes no base node.
    let mut stripped = after.clone();
    stripped.retain(|id| !id.starts_with("affix:"));
    assert_eq!(stripped, before);
}

#[test]
fn the_shipped_tree_opens_when_the_mod_bench_is_researched() {
    let mut game = Game::new(9310, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.affix_tree_open());
    research(&mut game, "mod_bench");
    assert!(game.affix_tree_open());
}
