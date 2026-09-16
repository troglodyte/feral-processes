//! Deriving the routine research tree — family, rung and prerequisite.

use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use crate::routine_tree::{family, gets_node, routine_prereq, scope_rank, version};
use crate::tests::support::test_assets_dir;
use crate::{DifficultyMode, Game};

fn shipped_abilities() -> AbilityDb {
    let (db, warnings) = AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    assert!(
        warnings.is_empty(),
        "the shipped set must load clean: {warnings:?}"
    );
    db
}

#[test]
fn version_orders_minor_and_major_tags_and_defaults_untagged_to_one_zero() {
    assert_eq!(version("Patch Single v1.0"), (1, 0));
    assert_eq!(version("Patch Party v1.1"), (1, 1));
    assert_eq!(version("Patch Single v2.0"), (2, 0));
    assert!(version("Patch Party v1.1") > version("Patch Single v1.0"));
    assert!(version("Patch Single v2.0") > version("Patch Party v1.1"));
    assert_eq!(version("Symlink Party"), (1, 0), "no tag means (1, 0)");
}

#[test]
fn gets_node_excludes_exclusive_permanent_passive_and_summon() {
    let db = shipped_abilities();
    for id in ["decompile", "bus_snoop", "fork_cluster", "fork_program"] {
        let def = db.get(id).unwrap_or_else(|| panic!("missing ability {id}"));
        assert!(!gets_node(&db, def), "{id} must not get a node");
    }
    for id in [
        "clock_skew",
        "core_dump",
        "hot_spare",
        "interrupt_request",
        "parity_guard",
        "quarantine",
    ] {
        let def = db.get(id).unwrap_or_else(|| panic!("missing ability {id}"));
        assert!(!gets_node(&db, def), "passive {id} must not get a node");
    }
    let hot_patch = db.get("hot_patch").unwrap();
    assert!(
        gets_node(&db, hot_patch),
        "hot_patch is an ordinary routine"
    );
}

#[test]
fn the_patch_family_chains_single_then_party_by_version() {
    let db = shipped_abilities();
    let hot_patch = db.get("hot_patch").unwrap(); // Patch Single v1.0
    let checksum_repair = db.get("checksum_repair").unwrap(); // Patch Single v2.0
    let mirror_restore = db.get("mirror_restore").unwrap(); // Patch Party v1.0
    let redundancy_sync = db.get("redundancy_sync").unwrap(); // Patch Party v1.1

    assert_eq!(
        routine_prereq(&db, hot_patch),
        None,
        "the Single root needs nothing"
    );
    assert_eq!(
        routine_prereq(&db, checksum_repair),
        Some("hot_patch".to_string()),
        "Single v2 follows Single v1"
    );
    assert_eq!(
        routine_prereq(&db, mirror_restore),
        Some("hot_patch".to_string()),
        "Party v1 roots off the Single root, not off Single v2"
    );
    assert_eq!(
        routine_prereq(&db, redundancy_sync),
        Some("mirror_restore".to_string()),
        "Party v1.1 follows Party v1.0"
    );
}

#[test]
fn a_family_rooted_at_group_has_a_parentless_group_root() {
    let mut db = AbilityDb::default();
    db.insert(fixture_ability(
        "only_group",
        "Only Group",
        AbilityTarget::WholeEnemyGroup,
    ));
    let def = db.get("only_group").unwrap();
    assert_eq!(scope_rank(def.target), 1);
    assert_eq!(family(def), "Only");
    assert_eq!(
        routine_prereq(&db, def),
        None,
        "with nothing at Single, the Group rung is the family's root"
    );
}

fn fixture_ability(id: &str, name: &str, target: AbilityTarget) -> AbilityDef {
    AbilityDef {
        id: id.into(),
        name: name.into(),
        description: "d".into(),
        target,
        effect: AbilityEffect::Buff {
            kind: crate::components::BuffKind::Atk,
            power: 1,
            duration: 1,
        },
        cooldown: 1,
        accuracy: 0,
        power_cost: 0.0,
        wild_weight: 0,
        research_zone: 0,
        exclusive: false,
        starter: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
        shape: None,
        range: None,
    }
}

/// Makes sure `Game::new` still loads the shipped ability set clean once
/// this module exists — a cheap sanity check that nothing above panics
/// against real assets.
#[test]
fn the_shipped_ability_set_still_loads_clean_through_game_new() {
    let game = Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.world.resource::<AbilityDb>().all().next().is_some());
}

fn shipped_research() -> (AbilityDb, crate::research::ResearchDb) {
    let assets = test_assets_dir();
    let db = shipped_abilities();
    let (structures, sw) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
    assert!(sw.is_empty());
    let (tools, tw) = crate::tools::ToolDb::load_dir(&assets.join("tools")).unwrap();
    assert!(tw.is_empty());
    let (research, rw) =
        crate::research::ResearchDb::load_dir(&assets.join("research"), &structures, &db, &tools)
            .unwrap();
    assert!(rw.is_empty(), "the shipped tree must load clean: {rw:?}");
    (db, research)
}

#[test]
fn every_eligible_ability_gets_exactly_one_synthesised_node_that_teaches_it() {
    let (abilities, research) = shipped_research();
    let mut teachers: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for def in research.all() {
        if let Some(taught) = &def.teaches {
            teachers
                .entry(taught.clone())
                .or_default()
                .push(def.id.clone());
        }
    }
    for ability in abilities.all().filter(|d| gets_node(&abilities, d)) {
        let nodes = teachers.get(&ability.id).cloned().unwrap_or_default();
        assert_eq!(
            nodes,
            vec![crate::routine_tree::node_id(&ability.id)],
            "{} must have exactly one node teaching it",
            ability.id
        );
    }
    // No node outside the eligible set teaches anything.
    for (ability, nodes) in &teachers {
        assert_eq!(
            nodes.len(),
            1,
            "{ability} is taught by more than one node: {nodes:?}"
        );
    }
}

#[test]
fn a_synthesised_nodes_tree_is_routines_and_a_ron_nodes_tree_is_base() {
    let (_, research) = shipped_research();
    let synthesised = research
        .get("routine/hot_patch")
        .expect("hot_patch gets a node");
    assert_eq!(synthesised.tree, crate::research::ResearchTree::Routines);
    let base = research.get("cortex").expect("cortex is a real .ron node");
    assert_eq!(base.tree, crate::research::ResearchTree::Base);
}

#[test]
fn min_zone_follows_research_zone_and_defaults_to_one() {
    let mut db = AbilityDb::default();
    let mut gated = fixture_ability("gated_single", "Gated Single", AbilityTarget::OneAlly);
    gated.research_zone = 3;
    db.insert(gated);
    db.insert(fixture_ability(
        "ungated_single",
        "Ungated Single",
        AbilityTarget::OneAlly,
    ));
    let nodes = crate::routine_tree::synthesise_nodes(&db);
    let gated_node = nodes
        .iter()
        .find(|n| n.id == crate::routine_tree::node_id("gated_single"))
        .unwrap();
    assert_eq!(gated_node.min_zone, 3);
    let ungated_node = nodes
        .iter()
        .find(|n| n.id == crate::routine_tree::node_id("ungated_single"))
        .unwrap();
    assert_eq!(
        ungated_node.min_zone, 1,
        "an ability authoring no research_zone opens at zone 1"
    );
}

#[test]
fn routine_research_cost_is_monotonic_in_scope_and_version() {
    use crate::tuning::routine_research_cost;
    assert!(routine_research_cost(1, (1, 0)) > routine_research_cost(0, (1, 0)));
    assert!(routine_research_cost(2, (1, 0)) > routine_research_cost(1, (1, 0)));
    assert!(routine_research_cost(0, (2, 0)) > routine_research_cost(0, (1, 0)));
    assert!(routine_research_cost(0, (3, 0)) > routine_research_cost(0, (2, 0)));
}
