//! Deriving the Affixes research tree from `AffixDb`.
//!
//! Nothing here is authored as a research file: an affix with a `research`
//! block is its own node, so a mod adds one by adding one affix file and
//! cannot forget the node. `routine_tree` is the counterpart for abilities.

use crate::affixes::{AffixDb, AffixDef};
use crate::research::{ResearchDef, ResearchTree};

/// The synthesised id for `affix`'s research node — `"affix:<id>"`, stable
/// across a rename of the affix's display words.
pub fn node_id(affix: &str) -> String {
    format!("affix:{affix}")
}

fn description(def: &AffixDef) -> String {
    let slots = match &def.slots {
        Some(slots) => slots
            .iter()
            .map(|s| s.label())
            .collect::<Vec<_>>()
            .join(" or "),
        None => "any gear".to_string(),
    };
    format!(
        "Lets you fit the {} affix to {slots} at a Mod Bench.",
        def.label()
    )
}

/// One `ResearchDef` per research-only affix, `ResearchDb::load_dir`'s
/// counterpart to `routine_tree::synthesise_nodes`. Sorted by id so the
/// merge order does not depend on the `HashMap` behind `AffixDb`.
pub fn synthesise_nodes(affixes: &AffixDb) -> Vec<ResearchDef> {
    let mut nodes: Vec<ResearchDef> = affixes
        .all()
        .filter_map(|def| {
            let research = def.research.as_ref()?;
            Some(ResearchDef {
                id: node_id(def.id.as_str()),
                name: def.label(),
                description: description(def),
                cost: research.cost,
                materials: research.materials.clone(),
                min_zone: research.min_zone,
                requires: research
                    .requires
                    .iter()
                    .map(|r| node_id(r.as_str()))
                    .collect(),
                recommended: false,
                unlocks_structures: Vec::new(),
                unlocks_recipes: Vec::new(),
                unlocks_tools: Vec::new(),
                tree: ResearchTree::Affixes,
                // Researched state is the plain `Research` id set, so
                // `node_researched` needs no branch for this tree.
                teaches: None,
                opens_routine_tree: false,
                opens_affix_tree: false,
                decompiler: None,
                requires_subject: false,
                // Hidden until a study attempt finds it, as a base bench is.
                discoverable: true,
                unlocks_fusion: false,
            })
        })
        .collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    nodes
}
