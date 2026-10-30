use crate::tests::support::*;
use crate::*;

/// A shipped structure must actually carry the flag, or every bench term
/// in `extraction_yield` is unreachable and the phase ships as a no-op.
#[test]
fn some_shipped_structure_extracts_programs() {
    let (db, warnings) =
        crate::structures::StructureDb::load_dir(&test_assets_dir().join("structures"))
            .expect("the shipped structures load");
    assert!(
        warnings.is_empty(),
        "shipped structures warned: {warnings:?}"
    );
    assert!(
        db.all().any(|def| def.extracts_programs),
        "no shipped structure sets extracts_programs"
    );
}

/// **A rig with no hopper is a machine that can never be loaded.** `strips`
/// is the whole feature's gate, and a zero there ships a structure that
/// builds, staffs, draws power and refuses every deposit.
#[test]
fn every_structure_that_strips_declares_a_hopper_and_an_output() {
    let game = Game::new(4110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();

    let mut checked = 0;
    for def in structures.all() {
        let Some(strips) = &def.strips else { continue };
        assert!(
            strips.hopper > 0,
            "structure {:?} strips programs but holds none",
            def.id
        );
        assert!(
            def.capacity > 0,
            "structure {:?} strips programs but has nowhere to put the yield",
            def.id
        );
        checked += 1;
    }
    assert!(checked > 0, "no shipped structure strips programs at all");
}

/// **Exactly one shipped structure declares `studies`.** The Research
/// Station is the whole of the feature so far — a second one shipping
/// unannounced would silently double a run's pace through it.
#[test]
fn exactly_one_shipped_structure_declares_studies() {
    let game = Game::new(4112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();
    let studying: Vec<&StructureId> = structures
        .all()
        .filter(|def| def.studies)
        .map(|def| &def.id)
        .collect();
    assert_eq!(
        studying.len(),
        1,
        "expected exactly one studies structure, found {studying:?}"
    );
    assert_eq!(studying[0], "research_node");
}

/// A pen needs a second cell to sit in — the anchor is spoken for by the
/// glyph, so a footprint-1 `studies` structure would derive a pen on top of
/// its own blocking cell.
#[test]
fn every_studies_structure_declares_a_footprint_of_at_least_two() {
    let game = Game::new(4113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();
    let mut checked = 0;
    for def in structures.all() {
        if !def.studies {
            continue;
        }
        assert!(
            def.footprint >= 2,
            "structure {:?} studies but its footprint is only {}",
            def.id,
            def.footprint
        );
        checked += 1;
    }
    assert!(checked > 0, "no shipped structure studies at all");
}

/// **What `offshift::in_reach` and every `step_to_post` call beside it rest
/// on.** `offshift.rs:340`/`:376`, `repair.rs:313` and `morale.rs:272` each
/// hardcode `1` for a structure's footprint rather than reading a def's own
/// `footprint`, on the strength of "no shipped amenity or Repair Bay
/// declares a footprint past 1" alone. A modded 2x2 amenity would parse
/// clean and walk bodies to the wrong faces with nothing failing to
/// compile — this is the census that closes it.
#[test]
fn every_amenity_and_repair_bay_declares_a_footprint_of_one() {
    let game = Game::new(4114, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();
    let mut checked = 0;
    for def in structures.all() {
        if def.services.is_empty() && def.recovery.is_none() {
            continue;
        }
        assert_eq!(
            def.footprint, 1,
            "structure {:?} is an amenity or a Repair Bay but its footprint is {} — \
             offshift::in_reach and every step_to_post call beside it hardcode 1",
            def.id, def.footprint
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no shipped structure is an amenity or a Repair Bay at all"
    );
}

/// The rig is a bench too, so manual extraction gains a second one. The
/// Compiler keeps its own flag — moving it would silently downgrade
/// manual extraction for a run in progress (spec 10.3).
#[test]
fn the_teardown_rig_is_also_an_extraction_bench_and_the_workbench_is() {
    let game = Game::new(4111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();

    let rig = structures
        .get("teardown_rig")
        .expect("teardown_rig should be a shipped structure");
    assert!(rig.strips.is_some(), "the rig should strip programs");
    assert!(
        rig.extracts_programs,
        "the rig should be a manual bench too"
    );

    let workbench = structures
        .get("workbench")
        .expect("workbench should be a shipped structure");
    assert!(workbench.extracts_programs && workbench.extracts_routines);
    assert_eq!(workbench.extracts_research.as_deref(), Some("automation"));
}

/// **`runs_a_job` is doubly load-bearing.** `spawn_structure` only inserts
/// a `MachineStatus` for a structure that runs a job, and
/// `idle_machine_system` skips one that does not. A rig missing from both
/// ships as a machine that can never say it is unstaffed — green, and
/// unreachable.
#[test]
fn a_structure_that_strips_runs_a_job() {
    let game = Game::new(4112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();
    let rig = structures
        .get("teardown_rig")
        .expect("teardown_rig should be a shipped structure");
    assert!(
        rig.runs_a_job(),
        "a rig that runs no job carries no MachineStatus and is skipped by idle_machine_system"
    );
}

/// The research node that hands the rig over resolves to it.
#[test]
fn the_teardown_node_unlocks_the_rig() {
    let game = Game::new(4113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<ResearchDb>();
    let node = research
        .get("teardown")
        .expect("teardown should be a shipped research node");
    assert!(
        node.unlocks_structures
            .contains(&"teardown_rig".to_string()),
        "the teardown node should unlock the rig"
    );
}

/// Every material a research node asks for must be something the base can
/// already be making by the time that node is reachable — the whole of
/// "don't require items that can't be unlocked first", and a census because
/// nothing in `ResearchDef` says a bill has to be payable.
///
/// The set is built from the node's own `requires` closure and nothing else:
/// structures no research gates at all, plus the structures its
/// prerequisites unlock, then the fixpoint of what that set can produce and
/// craft. `min_zone` is deliberately **not** a source of goods — a zone
/// number says the player breached, not that they took any particular node,
/// so a bill leaning on it would be an unstated prerequisite, which is
/// exactly the failure this exists to catch.
///
/// An extraction recipe (a `work` block or an Extractor's) makes its item out
/// of nothing on a timer and so seeds the set, once the node's prerequisites
/// hold the research it names; an assembling recipe does not, because a
/// machine runs its product's own `craftable.cost` and is therefore already
/// covered by the craft fixpoint. Treating it as a source is how an
/// assembler reads as a Trace Sniffer supply with no Logic Wafer in sight.
#[test]
fn every_research_material_is_reachable_through_that_nodes_own_prerequisites() {
    let game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let structures = game.structure_defs();
    let items = game.item_defs();

    let gated: std::collections::HashSet<&str> = research
        .all()
        .flat_map(|d| d.unlocks_structures.iter().map(|s| s.as_str()))
        .collect();

    let mut checked = 0;
    for def in research.all() {
        // The transitive `requires` closure, guarded on `seen` so a modded
        // cycle terminates — `ResearchDb::recommended_ids`' reason. An
        // Affixes-tree node is also behind its tree's gate, so the gate's own
        // closure counts: it is what the player has researched before one of
        // these nodes can so much as be found.
        let mut prereqs: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut stack: Vec<String> = def.requires.clone();
        if def.tree == crate::research::ResearchTree::Affixes {
            stack.extend(
                research
                    .all()
                    .filter(|d| d.opens_affix_tree)
                    .map(|d| d.id.clone()),
            );
        }
        while let Some(id) = stack.pop() {
            if !prereqs.insert(id.clone()) {
                continue;
            }
            if let Some(d) = research.get(&id) {
                stack.extend(d.requires.clone());
            }
        }

        let available: std::collections::HashSet<&str> = structures
            .iter()
            .map(|s| s.id.as_str())
            .filter(|id| {
                !gated.contains(id)
                    || prereqs.iter().any(|p| {
                        research
                            .get(p)
                            .is_some_and(|d| d.unlocks_structures.iter().any(|s| s == id))
                    })
            })
            .collect();

        // Open at this node: no research gate, or one this node's own
        // prerequisites hold.
        let open = |research_gate: Option<&str>| research_gate.is_none_or(|r| prereqs.contains(r));
        let mut makeable: std::collections::HashSet<ItemId> = structures
            .iter()
            .filter(|s| available.contains(s.id.as_str()))
            .flat_map(|s| s.productions())
            .filter(|p| p.extracts && open(p.recipe.research.as_deref()))
            .map(|p| p.recipe.produces.clone())
            .collect();
        loop {
            let grown: Vec<ItemId> = items
                .iter()
                .filter(|item| !makeable.contains(&item.id))
                .filter_map(|item| item.craftable.as_ref().map(|c| (&item.id, c)))
                .filter(|(_, c)| {
                    open(c.research.as_deref())
                        && c.requires_structure
                            .as_ref()
                            .is_none_or(|s| available.contains(s.as_str()))
                        && c.cost.iter().all(|(i, _)| makeable.contains(i))
                })
                .map(|(id, _)| id.clone())
                .collect();
            if grown.is_empty() {
                break;
            }
            makeable.extend(grown);
        }

        for (item, _) in &def.materials {
            assert!(
                makeable.contains(item),
                "research {:?} asks for {:?}, which nothing its own prerequisites unlock can make — \
                 the player would have to guess which other node to take first",
                def.id,
                item.as_str()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 40,
        "expected the shipped tree's material bills; {checked} lines is a tree that has \
         quietly gone back to costing Research Data alone"
    );
}

/// **The 25/5 split: every shipped node with `min_zone >= 2` declares
/// `requires_subject`, and the only nodes that do not are the five that get
/// a base running from turn one.** A node above zone 1 that forgot the field
/// would ship free of the study cost the feature exists to charge; one of the
/// five that gained it by mistake would silently gate a base's opening
/// machines. The zone rule is one-way now — the three benches
/// (`armor_bench`, `weapon_bench`, `routine_fabrication`) are gated at zone 1
/// deliberately, so the ungated set is named rather than derived.
#[test]
fn every_zone_gated_base_node_requires_a_subject_and_only_the_bootstrap_five_are_ungated() {
    /// What a fresh run may research before it has anyone to spend.
    const BOOTSTRAP: [&str; 5] = [
        "automation",
        "commerce",
        "fortification",
        "power_grid",
        "teardown",
    ];
    let game = Game::new(4114, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let mut checked = 0;
    let mut ungated: Vec<&str> = Vec::new();
    for def in research
        .all()
        .filter(|d| d.tree == crate::research::ResearchTree::Base)
    {
        if def.min_zone >= 2 {
            assert!(
                def.requires_subject,
                "{:?} has min_zone {} but does not require a subject",
                def.id, def.min_zone
            );
        }
        if !def.requires_subject {
            ungated.push(def.id.as_str());
        }
        checked += 1;
    }
    ungated.sort_unstable();
    assert_eq!(
        ungated, BOOTSTRAP,
        "the ungated set moved — a node was gated or ungated without this census being told"
    );
    assert_eq!(
        checked, 39,
        "expected the shipped base tree's 39 nodes; a count that moved means a node was \
         added, removed, or reclassified without this census being told"
    );
}

/// A routine node is synthesised, never authored, and must keep its own
/// economy — `routine_tree::synthesise_nodes` must leave `requires_subject`
/// at its default rather than gaining a second gate on top of "discover a
/// family by extracting a rung".
#[test]
fn no_synthesised_routine_node_requires_a_subject() {
    let mut game = Game::new(4115, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "routine_fabrication");
    let research = game.world.resource::<crate::research::ResearchDb>();
    let mut checked = 0;
    for def in research
        .all()
        .filter(|d| d.tree == crate::research::ResearchTree::Routines)
    {
        assert!(
            !def.requires_subject,
            "{:?} is a synthesised routine node and must not require a subject",
            def.id
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "the fixture is vacuous with no synthesised routine node loaded"
    );
}

/// No subject-gated node may sit upstream of an ungated one — a node that
/// cannot be selected without a subject must not gate content the eight
/// bootstrap nodes promise a fresh run with nobody pinned yet.
#[test]
fn no_subject_gated_node_is_a_prerequisite_of_an_ungated_one() {
    let game = Game::new(4116, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let mut checked = 0;
    for def in research.all().filter(|d| !d.requires_subject) {
        for req in &def.requires {
            let gated = research.get(req).is_some_and(|d| d.requires_subject);
            assert!(
                !gated,
                "{:?} is ungated but requires {:?}, which is subject-gated",
                def.id, req
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "the fixture is vacuous with nothing to check — every shipped node has at least one \
         `requires` edge somewhere in the tree"
    );
}

/// **Exactly one shipped node carries `unlocks_fusion`.** Zero would strand
/// the capability behind a check that always reads open (the lenient rule
/// makes that harmless, but silent); two would leave `fusion_opener_name`
/// naming an arbitrary one of them.
#[test]
fn exactly_one_shipped_research_node_unlocks_fusion() {
    let game = Game::new(4117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let openers: Vec<_> = research
        .all()
        .filter(|d| d.unlocks_fusion)
        .map(|d| d.id.clone())
        .collect();
    assert_eq!(
        openers,
        vec!["program_refactoring".to_string()],
        "expected exactly program_refactoring to unlock fusion, found {openers:?}"
    );
}

/// The other half: the shipped **base** tree actually *has* bills. The
/// census above passes vacuously against a tree with none, so a node whose
/// `materials` line was deleted by hand would read as free rather than as a
/// regression.
///
/// **Routine-tree nodes are excluded, by design rather than by omission.**
/// A synthesised node carries no material bill at all (spec §2 "Cost") —
/// there is nothing to check it against, since a derived node cannot
/// satisfy the "a bill may only name what its own prerequisites can make"
/// rule the way an authored node can.
#[test]
fn every_shipped_base_research_node_costs_materials() {
    let game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bare: Vec<&str> = game
        .world
        .resource::<crate::research::ResearchDb>()
        .all()
        .filter(|d| d.tree == crate::research::ResearchTree::Base)
        .filter(|d| d.materials.is_empty())
        .map(|d| d.id.as_str())
        .collect();
    assert!(
        bare.is_empty(),
        "research is a production run, not a purchase — these nodes cost Research Data alone: {bare:?}"
    );
}

/// The storage ladder: one Depot per sector, each holding twice what the
/// rung below it does, and every rung past the first standing behind
/// research.
///
/// A census rather than a formula because nothing in `StructureDef` knows
/// there is a ladder at all — the rungs are six independent files, and the
/// doubling is a content decision that reads as arbitrary the moment one
/// file is edited on its own. `Game::structure_unlocked` returns true for a
/// structure no research file names, so a rung whose node lost its
/// `unlocks_structures` line would ship buildable from turn one with nothing
/// failing to compile and nothing else failing to load.
#[test]
fn the_depot_ladder_doubles_and_every_rung_past_the_first_is_researched() {
    let game = Game::new(4114, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.structure_defs();
    let research = game.world.resource::<ResearchDb>();

    let rungs = [
        ("depot", 50),
        ("depot_mk2", 100),
        ("depot_mk3", 200),
        ("depot_mk4", 400),
        ("depot_mk5", 800),
        ("depot_mk6", 1600),
    ];

    for (i, (id, capacity)) in rungs.iter().enumerate() {
        let def = structures
            .iter()
            .find(|d| d.id == *id)
            .unwrap_or_else(|| panic!("{id} should be a shipped structure"));
        assert_eq!(
            def.capacity, *capacity,
            "{id} holds {} rather than the {capacity} its rung of the ladder is worth",
            def.capacity
        );
        assert!(
            def.stores,
            "{id} is a Depot and must be somewhere a hauler may empty into"
        );

        let gates: Vec<&str> = research
            .all()
            .filter(|node| node.unlocks_structures.iter().any(|s| s == id))
            .map(|node| node.id.as_str())
            .collect();
        if i == 0 {
            assert!(
                gates.is_empty(),
                "the first Depot carries a base through its opening hours and must stay \
                 unresearched; {gates:?} gate it"
            );
        } else {
            assert_eq!(
                gates.len(),
                1,
                "{id} should stand behind exactly one research node, not {gates:?}"
            );
            let node = research.get(gates[0]).expect("just enumerated");
            assert_eq!(
                node.min_zone as usize,
                i + 1,
                "{} gates {id} at sector {}, but the ladder is one rung a sector",
                node.id,
                node.min_zone
            );
        }
    }
}

/// **Nothing a research node gates may appear on the character-creation
/// shelf.** `ItemDb::creation_shelf` picks rows by `value <=
/// CREATION_SHELF_MAX_VALUE` and nothing else, so an item priced low enough
/// is offered to a character who has not been created yet — research-gated
/// content handed over for starting Credits, before the run that was
/// supposed to earn it exists.
///
/// This is `TOOL_CARRIER_VALUE`'s trap in a second place: a minted item joins
/// every pool that filters by *value* rather than by name, and that one was
/// closed by an explicit bar in `barred`. Here the price is what holds it,
/// which is exactly the kind of thing that rots under a retune — the shelf
/// also sorts by `(price, id)` and truncates, so a cheap new item does not
/// merely appear, it pushes an existing row off the end in silence.
#[test]
fn no_research_gated_recipe_is_offered_on_the_creation_shelf() {
    let game = Game::new(957, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated: Vec<&ItemId> = game
        .world
        .resource::<crate::research::ResearchDb>()
        .all()
        .flat_map(|def| def.unlocks_recipes.iter().map(|r| &r.result))
        .collect();
    assert!(
        !gated.is_empty(),
        "the tree unlocks no recipes at all, so this census proves nothing"
    );
    let shelf = game.world.resource::<ItemDb>().creation_shelf();
    for row in &shelf {
        assert!(
            !gated.contains(&&row.id),
            "{} is unlocked by research but sits on the creation shelf at {} \
             — under CREATION_SHELF_MAX_VALUE, so starting Credits buy what \
             the research tree is supposed to gate",
            row.id.as_str(),
            row.price
        );
    }
}

/// Emulate's Core is the one part barred from the creation shelf —
/// `Subroutine::shelf`: unlocking Emulate is what its research gates.
#[test]
fn the_creation_shelf_never_offers_the_emulate_core() {
    let game = Game::new(958, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let shelf = game.world.resource::<ItemDb>().creation_shelf();
    let emulate = crate::routine_craft::Subroutine::all()
        .into_iter()
        .filter(|p| !crate::routine_craft::Subroutine::shelf().contains(p))
        .map(|p| crate::items::ItemId::part(&p))
        .collect::<Vec<_>>();
    assert!(!emulate.is_empty(), "the shelf bars nothing");
    assert!(
        shelf.iter().all(|row| !emulate.contains(&row.id)),
        "a barred part must never be offered on the creation shelf"
    );
}

/// **The census.** `discoverable` is `#[serde(default)]`, so a shipped tree
/// that authors it nowhere is *invisible* rather than wrong — every node
/// lists, every test stays green, and the feature has silently not shipped.
/// This repo has already been bitten by exactly that.
///
/// `routine_fabrication` and `program_refactoring` are named on the other
/// side deliberately: they carry `requires_subject` like the base ones, and
/// leaving them visible is what keeps the routine tree and companion fusion
/// arriving when they do today rather than behind a dice roll.
#[test]
fn exactly_the_named_research_nodes_are_discoverable() {
    let game = Game::new(4118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut found: Vec<&str> = game
        .world
        .resource::<crate::research::ResearchDb>()
        .all()
        .filter(|d| d.discoverable)
        // The Weapons tree is synthesised from the part catalogue and every
        // node is discoverable by construction (`tests/weapon_tree.rs`).
        .filter(|d| d.tree != crate::research::ResearchTree::Weapons)
        .map(|d| d.id.as_str())
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            "ablative",
            "affix:deflecting",
            "affix:of_deflection",
            "affix:of_introspection",
            "armor_bench",
            "cache_coherence",
            "capacitance",
            "charge_density",
            "cold_archive",
            "cortex",
            "deep_analysis",
            "deep_splicing",
            "dispatch",
            "drop_pods",
            "firewall",
            "memory_mapping",
            "mod_bench",
            "model_inspection",
            "monofilament",
            "neural_amp",
            "overclock",
            "paging",
            "segmentation",
            "shard_mapping",
            "siphoning",
            "virtual_memory",
            "weapon_bench",
            "wetware_splicing",
        ],
        "the discoverable set is a content decision and nothing in ResearchDef states it"
    );
    for id in ["routine_fabrication", "program_refactoring"] {
        let def = game
            .world
            .resource::<crate::research::ResearchDb>()
            .get(id)
            .expect("both ship");
        assert!(
            def.requires_subject && !def.discoverable,
            "{id} costs a subject and must still be visible from turn one"
        );
    }
}

/// **A discoverable node must be reachable.** A node whose `requires`
/// closure bottoms out in something that can itself never be discovered is
/// a dead branch, and it ships silent: the menu simply never grows it.
///
/// Walked to a fixed point rather than recursively, so a cycle in the tree
/// terminates here instead of blowing the stack — `ResearchDb::load_dir`
/// does not reject one.
#[test]
fn every_discoverable_research_node_is_reachable_from_the_visible_tree() {
    let game = Game::new(4119, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::research::ResearchDb>();
    // Seed: everything visible from turn one is reachable by definition.
    let mut reachable: std::collections::HashSet<&str> = db
        .all()
        .filter(|d| !d.discoverable)
        .map(|d| d.id.as_str())
        .collect();
    loop {
        let grown: Vec<&str> = db
            .all()
            .filter(|d| !reachable.contains(d.id.as_str()))
            .filter(|d| d.requires.iter().all(|r| reachable.contains(r.as_str())))
            .map(|d| d.id.as_str())
            .collect();
        if grown.is_empty() {
            break;
        }
        reachable.extend(grown);
    }
    let stranded: Vec<&str> = db
        .all()
        .filter(|d| d.discoverable && !reachable.contains(d.id.as_str()))
        .map(|d| d.id.as_str())
        .collect();
    assert!(
        stranded.is_empty(),
        "these nodes can never enter the eligible pool, so they can never be found: {stranded:?}"
    );
    assert!(
        db.all().any(|d| d.discoverable),
        "the fixture is vacuous against a tree with nothing discoverable"
    );
}

/// The honeypot's whole chain in one place: research → Workbench → recipe →
/// `trap:`, and research → Assembler recipe. Three files author the links
/// between them and nothing in the compiler holds any of them.
///
/// The last assertion is the load-bearing one and the one nothing else
/// checks: without the Assembler listing the item, the recipe is
/// unassemblable, which reads at the keyboard as the work order screen
/// simply not listing it.
#[test]
fn the_honeypot_is_gated_by_deception_and_nothing_else() {
    let game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let honeypot = game
        .item_defs()
        .into_iter()
        .find(|d| d.id.as_str() == "honeypot")
        .expect("the honeypot ships");
    assert_eq!(
        honeypot.trap.map(|t| t.rarity_cap),
        Some(Rarity::Silver),
        "the shipped honeypot's ceiling is Optimized — a higher one is a balance change"
    );
    assert_eq!(
        honeypot
            .craftable
            .as_ref()
            .and_then(|c| c.requires_structure.as_ref())
            .map(|s| s.as_str()),
        Some("workbench"),
        "a honeypot is compiled at the bench, never by hand"
    );
    assert_eq!(
        honeypot
            .craftable
            .as_ref()
            .and_then(|c| c.research.as_deref()),
        Some("deception"),
        "and it keeps the gate of the Decoy Bench it replaced"
    );

    let assembler = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "assembler")
        .expect("the Assembler ships");
    let listed = assembler
        .machine_recipe(&ItemId("honeypot".to_string()))
        .expect("the Assembler must list the honeypot or it is unassemblable");
    assert_eq!(
        listed.research.as_deref(),
        Some("deception"),
        "the Assembler's honeypot recipe opens with the same node"
    );
}
