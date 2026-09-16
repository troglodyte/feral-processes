//! Deriving the routine research tree — family, rung and prerequisite.

use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use crate::routine_tree::{family, gets_node, routine_prereq, scope_rank, version};
use crate::species::SpeciesDb;
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

#[test]
fn a_discovered_save_writes_and_a_pre_feature_save_loads_with_none() {
    let mut game = Game::new(9110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::DiscoveredRoutines>()
        .0
        .insert("hyperthread".to_string());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_discovered_routines_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert!(
        loaded
            .world
            .resource::<crate::resources::DiscoveredRoutines>()
            .0
            .contains("hyperthread"),
        "a save round trip must carry a discovery over"
    );

    // A pre-feature save has no `discovered_routines:` line at all — strip
    // it out of the real file rather than trusting a RON round trip, which
    // a `#[serde(skip)]` would pass green even if the save on disk carried
    // nothing (CLAUDE.md's own trap on this).
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("discovered_routines"),
        "the fixture must actually have the key to strip, or this proves nothing"
    );
    // The RON pretty-printer wraps a non-empty `Vec` across several lines
    // (`discovered_routines: [` ... `"hyperthread",` ... `],`), so a
    // single-line filter would leave the array's body dangling with no
    // opening bracket. `in_block` skips every line from the opener through
    // its matching `],`.
    let mut in_block = false;
    let stripped: String = text
        .lines()
        .filter(|l| {
            let trimmed = l.trim_start();
            if in_block {
                if trimmed == "]," || trimmed == "]" {
                    in_block = false;
                }
                return false;
            }
            if trimmed.starts_with("discovered_routines:") {
                // Ends with `[` only for the multi-line, non-empty form;
                // the empty form is `discovered_routines: [],` on one line.
                in_block = trimmed.ends_with('[');
                return false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();
    let reloaded = Game::load(&path, &test_assets_dir()).expect("a pre-feature save still loads");
    let _ = std::fs::remove_file(&path);
    assert!(
        reloaded
            .world
            .resource::<crate::resources::DiscoveredRoutines>()
            .0
            .is_empty(),
        "a save written before this feature existed has discovered nothing"
    );
}

#[test]
fn knowing_a_rung_or_discovering_one_makes_its_family_discovered() {
    let mut game = Game::new(9111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.family_discovered("Hyperthread"));

    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert("priority_boost".to_string());
    assert!(
        game.family_discovered("Hyperthread"),
        "a known rung counts as discovered even with nothing in DiscoveredRoutines"
    );
}

#[test]
fn hyperthread_is_discoverable_and_a_field_routine_family_is_not() {
    let game = Game::new(9112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.family_is_discoverable("Hyperthread"),
        "hyperthread (v2.0) carries a positive wild_weight"
    );
    assert!(
        !game.family_is_discoverable("Symlink"),
        "a field routine's family has no wild carrier and no species kit slot"
    );
}

/// Independent derivation, so a bug in `family_is_discoverable`'s own OR of
/// wild pool and species kit cannot mark a family discoverable that nothing
/// in the shipped assets actually carries.
#[test]
fn every_discoverable_family_has_at_least_one_carried_rung() {
    let game = Game::new(9113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abilities = game.world.resource::<AbilityDb>();
    let species = game.world.resource::<SpeciesDb>();
    let wild_ids: std::collections::HashSet<&str> = abilities
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.as_str())
        .collect();
    let kit_ids: std::collections::HashSet<&str> = species
        .all()
        .flat_map(|s| s.abilities.iter())
        .map(|a| a.id.as_str())
        .collect();

    let mut families: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for def in abilities.all() {
        families.insert(crate::routine_tree::family(def));
    }
    for family in families {
        if !game.family_is_discoverable(&family) {
            continue;
        }
        let carried = abilities.all().any(|def| {
            crate::routine_tree::family(def) == family
                && (wild_ids.contains(def.id.as_str()) || kit_ids.contains(def.id.as_str()))
        });
        assert!(
            carried,
            "{family:?} reads as discoverable but nothing carries any of its rungs"
        );
    }
}
