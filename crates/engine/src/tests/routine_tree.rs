//! Deriving the routine research tree — family, rung and prerequisite.

use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use crate::research::ResearchTree;
use crate::routine_tree::{family, gets_node, routine_prereq, scope_rank, version};
use crate::species::SpeciesDb;
use crate::tests::support::{set_zone, stand_in_base, test_assets_dir, unlock_research_chain};
use crate::views::ResearchState;
use crate::{DifficultyMode, Game};

fn open_routine_tree(game: &mut Game) {
    unlock_research_chain(game, "routine_fabrication");
}

fn listed_routine_ids(game: &Game) -> std::collections::HashSet<String> {
    game.research_nodes(ResearchTree::Routines)
        .into_iter()
        .map(|n| n.id)
        .collect()
}

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

// --- Visibility (task 5) ---
//
// The Patch family: hot_patch (Single v1.0, zone 1, root) -> checksum_repair
// (Single v2.0) and mirror_restore (Party v1.0) -> redundancy_sync (Party
// v1.1). checksum_repair/cold_boot/mirror_restore carry a wild_weight, so
// the family is discoverable.

#[test]
fn an_undiscovered_family_lists_nothing() {
    let mut game = Game::new(9130, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    let listed = listed_routine_ids(&game);
    for id in [
        "routine/hot_patch",
        "routine/checksum_repair",
        "routine/mirror_restore",
        "routine/redundancy_sync",
    ] {
        assert!(!listed.contains(id), "{id} must be hidden until discovered");
    }
}

#[test]
fn discovering_one_rung_lists_only_the_root() {
    let mut game = Game::new(9131, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    game.world
        .resource_mut::<crate::resources::DiscoveredRoutines>()
        .0
        .insert("checksum_repair".to_string());
    let listed = listed_routine_ids(&game);
    assert!(listed.contains("routine/hot_patch"), "the root is revealed");
    for id in [
        "routine/checksum_repair",
        "routine/mirror_restore",
        "routine/redundancy_sync",
    ] {
        assert!(
            !listed.contains(id),
            "{id}'s own prerequisite is not researched yet, so it stays hidden"
        );
    }
}

#[test]
fn researching_the_root_lists_exactly_its_two_children() {
    let mut game = Game::new(9132, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    game.world
        .resource_mut::<crate::resources::DiscoveredRoutines>()
        .0
        .insert("checksum_repair".to_string());
    unlock_research_chain(&mut game, "routine/hot_patch");
    let listed = listed_routine_ids(&game);
    assert!(listed.contains("routine/checksum_repair"));
    assert!(listed.contains("routine/mirror_restore"));
    assert!(
        !listed.contains("routine/redundancy_sync"),
        "redundancy_sync's own prerequisite (mirror_restore) is still unresearched"
    );
}

#[test]
fn an_always_visible_family_lists_its_nodes_at_the_right_zone_and_none_below_it() {
    let mut game = Game::new(9133, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    assert!(
        !game.family_is_discoverable("Hardened Shell"),
        "fixture assumption: nothing carries Hardened Shell"
    );
    assert!(
        !listed_routine_ids(&game).contains("routine/hardened_shell"),
        "zone 1: hardened_shell's own gate is zone 2"
    );
    set_zone(&mut game, 2);
    let listed = listed_routine_ids(&game);
    assert!(listed.contains("routine/hardened_shell"));
    assert!(
        !listed.contains("routine/hardened_shell_party"),
        "zone 2: hardened_shell_party's own gate is zone 3"
    );
    set_zone(&mut game, 3);
    assert!(listed_routine_ids(&game).contains("routine/hardened_shell_party"));
}

#[test]
fn a_discovered_patch_in_zone_one_lists_hot_patch_as_locked() {
    let mut game = Game::new(9134, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    game.world
        .resource_mut::<crate::resources::DiscoveredRoutines>()
        .0
        .insert("mirror_restore".to_string());
    let rows = game.research_nodes(ResearchTree::Routines);
    let hot_patch = rows
        .iter()
        .find(|n| n.id == "routine/hot_patch")
        .expect("discovered and its own (empty) prereq is trivially met");
    assert!(
        matches!(
            hot_patch.state,
            ResearchState::Locked {
                min_zone: Some(2),
                ..
            }
        ),
        "got {:?}",
        hot_patch.state
    );
}

#[test]
fn a_known_child_with_a_hidden_parent_still_gets_a_graph_cell() {
    let mut game = Game::new(9135, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // The tree is deliberately left closed — an old save can know a rung
    // without ever having researched `routine_fabrication` under the old
    // tree, and `routine/hot_patch` (mirror_restore's own prerequisite)
    // must not be listed while it is closed.
    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert("mirror_restore".to_string());
    assert!(!game.routine_tree_open());
    let listed = listed_routine_ids(&game);
    assert!(
        listed.contains("routine/mirror_restore"),
        "already known, listed unconditionally"
    );
    assert!(
        !listed.contains("routine/hot_patch"),
        "the tree is closed for everything else"
    );

    let graph = game.research_graph(ResearchTree::Routines);
    assert!(
        graph.cells.iter().any(|c| c.id == "routine/mirror_restore"),
        "a listed node with an unlisted parent must still get a cell"
    );
    assert!(
        !graph
            .edges
            .iter()
            .any(|(_, to)| to == "routine/mirror_restore"),
        "the hidden parent contributes no edge"
    );
}

#[test]
fn the_closed_tree_lists_nothing_and_select_research_refuses() {
    let mut game = Game::new(9136, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    assert!(listed_routine_ids(&game).is_empty());
    let err = game.select_research("routine/symlink").unwrap_err();
    assert!(err.contains("Routine Fabrication"), "got: {err}");
}

#[test]
fn select_research_refuses_an_unlisted_open_tree_node_and_files_nothing() {
    let mut game = Game::new(9137, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    open_routine_tree(&mut game);
    // Patch is discoverable and undiscovered, so hot_patch is unlisted even
    // though the tree itself is open.
    let err = game.select_research("routine/hot_patch").unwrap_err();
    assert_eq!(err, "Unknown research.");
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .is_none(),
        "nothing may be filed for a node the player was never shown"
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

#[test]
fn the_base_trees_output_is_unchanged_apart_from_the_deleted_nodes() {
    let game = Game::new(9139, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let rows = game.research_nodes(ResearchTree::Base);
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|n| n.teaches.is_none()),
        "no base-tree row ever teaches a routine"
    );
    let research_dir = test_assets_dir().join("research");
    let ron_file_count = std::fs::read_dir(&research_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("ron"))
        .count();
    assert_eq!(rows.len(), ron_file_count);
}
