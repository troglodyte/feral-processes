//! Deriving the routine research tree — family, rung and prerequisite.

use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use crate::research::ResearchTree;
use crate::routine_tree::{family, gets_node, routine_prereq, scope_rank, version};
use crate::species::SpeciesDb;
use crate::tests::support::{
    base_with_a_research_node, discover_all_research, set_zone, stand_in_base, test_assets_dir,
    unlock_research_chain,
};
use crate::views::{GraphDir, ResearchState};
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

/// Spec §1 "Researched means known": a rung counts as satisfied when a
/// *higher version at the same scope* is known, even though the lower one
/// itself never was. Without this, a starter routine minted at Single v2.0
/// would leave Patch Party v1.0 (whose own prerequisite is `hot_patch`,
/// Single v1.0) stranded forever, since nobody would ever know v1.0 itself.
#[test]
fn rung_satisfied_counts_a_higher_version_at_the_same_scope() {
    let db = shipped_abilities();
    let mut known = std::collections::BTreeSet::new();
    known.insert("checksum_repair".to_string()); // Patch Single v2.0
    assert!(
        crate::routine_tree::rung_satisfied(&db, &known, "hot_patch"),
        "knowing Single v2.0 must satisfy the Single v1.0 rung"
    );
    assert!(
        !crate::routine_tree::rung_satisfied(&db, &known, "mirror_restore"),
        "a higher version at a DIFFERENT scope (Party) must not count — sanity check"
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

/// Spec §4 "Unlocking": `routine/emulate` is synthesised with no research
/// file, and `family`/`routine_prereq` must give it a single root rung —
/// todo #100 Task 4's own check against #101 as built.
#[test]
fn emulate_is_its_familys_own_parentless_root() {
    let abilities = shipped_abilities();
    let def = abilities.get("emulate").expect("emulate.ron ships");
    assert_eq!(family(def), "Emulate");
    assert_eq!(
        routine_prereq(&abilities, def),
        None,
        "Emulate has no cheaper sibling scope, so it roots its own family"
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

/// One assertion per row of the design spec's §2 "Zone gate" table
/// (`docs/superpowers/specs/2026-09-16-routine-research-tree-design.md`),
/// including the four extra Patch rungs the spec amended in alongside this
/// test — `no_research_node_is_gated_below_its_own_prerequisite`
/// (`tests/research.rs`) is what forces a whole version chain to carry at
/// least its root's gate, so `checksum_repair`/`cold_boot`/
/// `mirror_restore`/`redundancy_sync` are zone 2 too, though nothing ever
/// granted them at that zone under the old base tree.
#[test]
fn every_row_of_the_spec_zone_gate_table_matches_the_shipped_assets() {
    let db = shipped_abilities();
    let zone_of = |id: &str| -> u32 {
        let z = db
            .get(id)
            .unwrap_or_else(|| panic!("missing ability {id}"))
            .research_zone;
        if z == 0 { 1 } else { z }
    };
    let rows: &[(&str, u32)] = &[
        ("deep_scan", 3),
        ("trace_analysis", 3),
        ("stealth_protocol", 3),
        ("salvage_routine", 3),
        ("buffer_overrun", 3),
        ("wild_jump", 3),
        // The eleven `model_inspection` routines.
        ("backprop", 3),
        ("dropout_group", 3),
        ("hallucination", 3),
        ("gradient_descent", 3),
        ("inference_probe", 3),
        ("dropout", 3),
        ("cold_sample", 3),
        ("data_poisoning", 3),
        ("heat_injection", 3),
        ("fine_tune", 3),
        ("prompt_injection", 3),
        ("hardened_shell_party", 3),
        ("null_route", 3),
        ("hardened_shell", 2),
        ("overclock", 2),
        ("ablative_layer", 2),
        ("hot_patch", 2),
        // The four extra Patch rungs, forced to hot_patch's own gate.
        ("checksum_repair", 2),
        ("cold_boot", 2),
        ("mirror_restore", 2),
        ("redundancy_sync", 2),
        // Everything else defaults to zone 1.
        ("symlink", 1),
        ("detach", 1),
        ("priority_boost", 1),
        ("repair_loop", 1),
        ("trickle_charge", 1),
    ];
    for (id, want) in rows {
        assert_eq!(zone_of(id), *want, "{id}'s research_zone");
    }
}

/// Spec §2 "The first discovery door": `routine_reader` moved off `cortex`
/// (zone 3) onto `routine_fabrication`, so recovering from downed programs
/// opens at the same moment as the routine tree rather than waiting for a
/// deep-zone base node. `deep_analysis`'s `requires` follows it too — it
/// used to hang off `field_ops`, one of the nine deleted nodes, and now
/// roots directly off `routine_fabrication`.
#[test]
fn routine_reader_is_unlocked_by_routine_fabrication_and_deep_analysis_requires_it() {
    let (_, research) = shipped_research();
    let fabrication = research.get("routine_fabrication").unwrap();
    assert!(
        fabrication
            .unlocks_tools
            .iter()
            .any(|t| t.as_str() == "routine_reader"),
        "routine_fabrication must unlock routine_reader"
    );
    let cortex = research.get("cortex").unwrap();
    assert!(
        !cortex
            .unlocks_tools
            .iter()
            .any(|t| t.as_str() == "routine_reader"),
        "cortex must no longer unlock routine_reader"
    );
    let deep_analysis = research.get("deep_analysis").unwrap();
    assert!(
        deep_analysis
            .requires
            .iter()
            .any(|r| r == "routine_fabrication"),
        "deep_analysis must require routine_fabrication, the root the deleted \
         field_ops chain hung from"
    );
}

/// Every synthesised routine node's prerequisite chain must terminate at a
/// parentless root that is itself a synthesised node — never a dangling id,
/// and never a cycle. `routine_prereq` only ever names a peer that itself
/// `gets_node`, so this ought to hold by construction; this census is the
/// one place that construction is checked against the real shipped tree
/// rather than trusted.
#[test]
fn every_synthesised_routine_node_is_reachable_from_a_parentless_root() {
    let (_, research) = shipped_research();
    let routines: Vec<&crate::research::ResearchDef> = research
        .all()
        .filter(|d| d.tree == ResearchTree::Routines)
        .collect();
    for start in &routines {
        let mut current = start.id.clone();
        let mut seen = std::collections::HashSet::new();
        loop {
            assert!(
                seen.insert(current.clone()),
                "{}'s prerequisite chain cycles back to {current}",
                start.id
            );
            let def = research
                .get(&current)
                .unwrap_or_else(|| panic!("{}'s chain names a dangling id {current}", start.id));
            match def.requires.as_slice() {
                [] => break,
                [only] => current = only.clone(),
                many => panic!(
                    "{} has {} prerequisites, but a routine node names at most one",
                    def.id,
                    many.len()
                ),
            }
        }
    }
}

/// Spec's own testing list, "Save": a save made before this feature
/// existed carries no `discovered_routines` key at all, but a routine it
/// already knew must read exactly as it does in a fresh save — listed,
/// `Unlocked`, and its family discovered off `KnownRoutines` alone
/// (`family_discovered`'s OR of `DiscoveredRoutines` and `KnownRoutines`).
/// The round trip alone cannot prove this — a `#[serde(default)]` field
/// passes a round trip even when the save on disk never carried it — so the
/// key is stripped from the real file rather than trusted to skip itself.
#[test]
fn a_pre_change_save_knowing_a_group_rung_loads_it_unlocked_and_discovered() {
    let mut game = Game::new(9160, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "routine_fabrication");
    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert("hardened_shell_party".to_string());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_pre_change_group_rung_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("discovered_routines"),
        "the fixture must actually have the key to strip, or this proves nothing"
    );
    // `DiscoveredRoutines` stays empty in this fixture (only `KnownRoutines`
    // is seeded), so RON prints it on one line — `discovered_routines: [],`
    // — unlike the multi-line non-empty form the sibling save test strips.
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("discovered_routines"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).expect("a pre-feature save still loads");
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.family_discovered("Hardened Shell"),
        "a known rung must read as discovered with no DiscoveredRoutines entry at all"
    );
    assert!(loaded.is_researched("routine/hardened_shell_party"));
    let rows = loaded.research_nodes(ResearchTree::Routines);
    let row = rows
        .iter()
        .find(|n| n.id == "routine/hardened_shell_party")
        .expect("a known node must still be listed");
    assert_eq!(row.state, ResearchState::Unlocked);
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
fn routine_research_cost_is_monotonic_in_scope_zone_and_version() {
    use crate::tuning::routine_research_cost;
    assert!(routine_research_cost(1, (1, 0), 1) > routine_research_cost(0, (1, 0), 1));
    assert!(routine_research_cost(2, (1, 0), 1) > routine_research_cost(1, (1, 0), 1));
    assert!(routine_research_cost(0, (2, 0), 1) > routine_research_cost(0, (1, 0), 1));
    assert!(routine_research_cost(0, (3, 0), 1) > routine_research_cost(0, (2, 0), 1));
    assert!(routine_research_cost(0, (1, 0), 2) > routine_research_cost(0, (1, 0), 1));
    assert!(routine_research_cost(0, (1, 0), 3) > routine_research_cost(0, (1, 0), 2));
    assert_eq!(
        routine_research_cost(0, (1, 0), 0),
        routine_research_cost(0, (1, 0), 1),
        "an ability authoring no research_zone (0) prices exactly as zone 1"
    );
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
    assert!(
        !listed.contains("routine/cold_boot"),
        "cold_boot (Single v3.0) requires checksum_repair (Single v2.0), which is \
         only discovered so far, not researched — a third rung must not leak in \
         alongside the two real children"
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

/// `emulate.ron` sets `research_zone: 2` (constraints.md decision 9) — this
/// is the zone-visibility half of the same design check
/// `emulate_is_its_familys_own_parentless_root` covers structurally.
#[test]
fn emulate_is_visible_starting_at_zone_two() {
    let mut game = Game::new(9141, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    assert!(
        !listed_routine_ids(&game).contains("routine/emulate"),
        "zone 1: emulate's own gate is zone 2"
    );
    set_zone(&mut game, 2);
    assert!(listed_routine_ids(&game).contains("routine/emulate"));
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

/// Review decision (2026-09-16, todo #101): the spec's own "Nothing is
/// listed until the tree is open" is taken literally — a closed tree hides
/// even an already-known rung, reverting the "known bypasses tree-open"
/// ordering an earlier pass shipped. See `.claude/skills/seams/references/
/// base.md`'s routine-tree entry for the argument.
#[test]
fn the_closed_tree_lists_nothing_even_when_a_rung_is_known() {
    let mut game = Game::new(9135, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert("mirror_restore".to_string());
    assert!(!game.routine_tree_open());
    assert!(
        listed_routine_ids(&game).is_empty(),
        "a closed tree lists nothing at all, known rungs included"
    );
}

/// The "hidden parent" case `research_graph` has to treat as absent
/// (spec §2 "Visibility") is reachable with the tree **open**: a known
/// rung is listed unconditionally once the tree is open, even past a zone
/// gate its own (unknown) parent is still waiting on. "Hardened Shell" is
/// always-visible (fixture assumption shared with
/// `an_always_visible_family_lists_its_nodes_at_the_right_zone_and_none_below_it`),
/// so at zone 1 its Single root (`hardened_shell`, gate zone 2) is hidden
/// by the zone gate while a known Party rung stays listed regardless.
#[test]
fn a_known_child_with_a_hidden_parent_still_gets_a_graph_cell() {
    let mut game = Game::new(9136, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_routine_tree(&mut game);
    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert("hardened_shell_party".to_string());
    let listed = listed_routine_ids(&game);
    assert!(
        listed.contains("routine/hardened_shell_party"),
        "already known, listed unconditionally once the tree is open"
    );
    assert!(
        !listed.contains("routine/hardened_shell"),
        "its own prerequisite is still behind a zone gate the party hasn't met"
    );

    let graph = game.research_graph(ResearchTree::Routines);
    assert!(
        graph
            .cells
            .iter()
            .any(|c| c.id == "routine/hardened_shell_party"),
        "a listed node with an unlisted parent must still get a cell"
    );
    assert!(
        !graph
            .edges
            .iter()
            .any(|(_, to)| to == "routine/hardened_shell_party"),
        "the hidden parent contributes no edge"
    );

    // A cell with no edges into it is still a cell the cursor can reach —
    // `ResearchGraph::step` walks `cells` by tier and slot, not by edge, so
    // the hidden parent cannot orphan it from keyboard navigation either.
    // `routine/symlink` (always-visible, zone 1, its own root) lands at
    // tier 0 alongside it, since the Kahn pass treats the unlisted parent
    // as absent and lays this node out as if it were a root too.
    let symlink_cell = graph
        .cell("routine/symlink")
        .expect("symlink is always-visible and must be listed");
    let target_cell = graph
        .cell("routine/hardened_shell_party")
        .expect("checked above");
    assert_eq!(
        symlink_cell.tier, 0,
        "fixture assumption: symlink is a root, tier 0"
    );
    assert_eq!(
        target_cell.tier, 0,
        "the hidden-parent node must land at tier 0 too"
    );
    let dir = if target_cell.slot >= symlink_cell.slot {
        GraphDir::Down
    } else {
        GraphDir::Up
    };
    let mut cursor = "routine/symlink".to_string();
    for _ in 0..graph.cells.len() {
        if cursor == "routine/hardened_shell_party" {
            break;
        }
        cursor = graph.step(&cursor, dir);
    }
    assert_eq!(
        cursor, "routine/hardened_shell_party",
        "stepping within tier 0 must be able to land on the hidden-parent node"
    );
}

#[test]
fn the_closed_tree_lists_nothing_and_select_research_refuses() {
    let mut game = Game::new(9136, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    assert!(listed_routine_ids(&game).is_empty());
    let err = game.select_research("routine/symlink").unwrap_err();
    assert!(err.contains("Routine Fabrication"), "got: {err}");
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .is_none(),
        "the closed-tree refusal must file nothing"
    );
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

/// Review fix (2026-09-16, todo #101): `research_nodes(tree)` walks
/// `listed_research(tree)`, so a project belonging to the *other* tree
/// never shows as `ResearchState::Active` among either tree's own rows —
/// there is no id collision between `routine/*` and a base `.ron` id to
/// make it match by accident. A header built by scanning rows therefore
/// used to read "No research project" on whichever screen was not running
/// the active project. `Game::active_research_progress` is the unfiltered
/// door both screens' headers must use instead.
#[test]
fn active_research_progress_is_unfiltered_by_tree_and_shows_on_the_other_screen() {
    let mut game = Game::new(9150, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    open_routine_tree(&mut game);
    game.select_research("routine/symlink").unwrap();

    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .all(|n| n.state != ResearchState::Active),
        "a routine project must not read Active among the base tree's own rows"
    );

    let (name, earned, cost) = game
        .active_research_progress()
        .expect("a routine project is running");
    assert_eq!(name, "Symlink Party");
    assert_eq!(earned, 0);
    assert!(cost > 0);

    // The cross-tree refusal already existed in `select_research`
    // (unconditional on `ActiveResearch::id`, never asking which tree it
    // belongs to) — pin it here alongside the row state, so a base row
    // stays `Available` (never specially blocked just because the running
    // project is the other tree's) exactly as an in-tree "another project
    // active" row already does, and the refusal names the routine project.
    let base_row = game
        .research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.state == ResearchState::Available)
        .expect("some base node must still read Available with a routine project running");
    let err = game.select_research(&base_row.id).unwrap_err();
    assert_eq!(
        err,
        "The base is already working on Symlink Party — abandon it first."
    );
}

#[test]
fn the_base_trees_output_is_unchanged_apart_from_the_deleted_nodes() {
    let mut game = Game::new(9139, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
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

/// `field_ops` is one of the nine nodes task 3 deleted — a save made before
/// that deletion (or before a mod's own tree change) could carry it as the
/// active project forever, with no door that ever re-selects on its own.
#[test]
fn a_stale_active_research_clears_on_load_with_its_work_orders() {
    let mut game = Game::new(9140, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .id = Some("field_ops".to_string());
    game.world
        .resource_mut::<crate::resources::WorkOrders>()
        .0
        .push(
            crate::game::base::work_orders::WorkOrder::batch(
                crate::items::ItemId::from("bytecode_block"),
                1,
            )
            .with_research(),
        );

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
    assert!(
        loaded
            .world
            .resource::<crate::resources::WorkOrders>()
            .0
            .iter()
            .all(|o| !o.for_research),
        "its work orders must clear with it"
    );
}
