use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::structures::{StructureDb, StructureId};

pub type ResearchId = String;

/// A craft recipe a research node unlocks. Recipe *data* lives in the
/// research `.ron` files rather than in Rust so a mod can ship a structure,
/// its research node and its recipes as pure data — `ItemId` itself stays a
/// Rust enum (see `CLAUDE.md`), but nothing about a recipe has to be.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
    /// If set, the recipe only appears in the compile menu while a structure
    /// of this kind is actually deployed — researching the blueprint isn't
    /// enough on its own, you still need the bench. `#[serde(default)]` so a
    /// recipe with no bench requirement can just omit the field.
    #[serde(default)]
    pub requires_structure: Option<StructureId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchDef {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    /// Research Data spent to unlock this node.
    pub cost: u32,
    #[serde(default)]
    pub requires: Vec<ResearchId>,
    #[serde(default)]
    pub unlocks_structures: Vec<StructureId>,
    #[serde(default)]
    pub unlocks_recipes: Vec<ResearchRecipe>,
}

#[derive(Resource, Default)]
pub struct ResearchDb {
    nodes: HashMap<ResearchId, ResearchDef>,
}

impl ResearchDb {
    /// Loads every `*.ron` research node in `dir`, then drops any node that
    /// can never be reached or acted on: one naming an unknown prereq, or an
    /// unknown structure in `unlocks_structures`. Both would otherwise sit in
    /// the menu forever with no explanation. Dropping cascades — a node whose
    /// prereq was itself dropped is equally unreachable — so validation runs
    /// to a fixpoint. Malformed files are skipped with a warning rather than
    /// aborting the load, so one bad mod file can't crash startup.
    pub fn load_dir(dir: &Path, structures: &StructureDb) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ResearchDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ResearchDef>(&text) {
                Ok(def) => {
                    db.nodes.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid research file {path:?}: {e}")),
            }
        }

        loop {
            let mut dropped = Vec::new();
            for def in db.nodes.values() {
                if let Some(missing) = def
                    .unlocks_structures
                    .iter()
                    .find(|s| structures.get(s).is_none())
                {
                    dropped.push((
                        def.id.clone(),
                        format!(
                            "skipped research {:?}: unlocks unknown structure {missing:?}",
                            def.id
                        ),
                    ));
                    continue;
                }
                if let Some(missing) = def.requires.iter().find(|r| !db.nodes.contains_key(*r)) {
                    dropped.push((
                        def.id.clone(),
                        format!(
                            "skipped research {:?}: unknown prerequisite {missing:?}",
                            def.id
                        ),
                    ));
                }
            }
            if dropped.is_empty() {
                break;
            }
            for (id, warning) in dropped {
                db.nodes.remove(&id);
                warnings.push(warning);
            }
        }

        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&ResearchDef> {
        self.nodes.get(id)
    }

    /// Every loaded node, cheapest first and ties broken by `id`. `HashMap`
    /// iteration order is randomized per instance, so without this the tree's
    /// `[1]`, `[2]`, ... numbering would shuffle between sessions even though
    /// nothing about the files changed — the same digit could mean an 8-cost
    /// node one session and a 45-cost node the next.
    pub fn all(&self) -> impl Iterator<Item = &ResearchDef> {
        let mut defs: Vec<&ResearchDef> = self.nodes.values().collect();
        defs.sort_by(|a, b| a.cost.cmp(&b.cost).then_with(|| a.id.cmp(&b.id)));
        defs.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads a `ResearchDb`
    /// from it against a `StructureDb` built from the real assets — so
    /// `unlocks_structures` validation runs against real structure ids.
    fn load(tag: &str, files: &[(&str, &str)]) -> (ResearchDb, Vec<String>) {
        let dir = std::env::temp_dir().join(format!("feral_research_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let structures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("structures");
        let (structures, _) = StructureDb::load_dir(&structures_dir).unwrap();
        let result = ResearchDb::load_dir(&dir, &structures).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "automation",
        name: "Automation",
        description: "Self-running compile jobs.",
        cost: 8,
        unlocks_structures: ["compiler"],
    )"#;

    #[test]
    fn a_valid_node_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load("valid", &[("automation", VALID)]);
        let def = db.get("automation").expect("valid node should load");
        assert_eq!(def.name, "Automation");
        assert_eq!(def.cost, 8);
        assert!(def.requires.is_empty(), "requires defaults to empty");
        assert!(def.unlocks_recipes.is_empty(), "recipes default to empty");
        assert_eq!(def.unlocks_structures, vec!["compiler".to_string()]);
        assert!(warnings.is_empty(), "a valid node warns about nothing");
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(
            "malformed",
            &[("automation", VALID), ("broken", "(this is not ron")],
        );
        assert!(
            db.get("automation").is_some(),
            "one bad mod file must not take the others down"
        );
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn a_node_requiring_an_unknown_node_is_skipped_with_a_warning() {
        let dangling = r#"(
            id: "dangling",
            name: "Dangling",
            description: "Requires something that does not exist.",
            cost: 5,
            requires: ["nonexistent"],
        )"#;
        let (db, warnings) = load("dangling", &[("automation", VALID), ("dangling", dangling)]);
        assert!(
            db.get("dangling").is_none(),
            "a node with an unreachable prereq would be permanently unresearchable"
        );
        assert!(db.get("automation").is_some());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nonexistent"));
    }

    #[test]
    fn a_node_unlocking_an_unknown_structure_is_skipped_with_a_warning() {
        let bad = r#"(
            id: "ghost_bench",
            name: "Ghost Bench",
            description: "Unlocks a structure that does not exist.",
            cost: 5,
            unlocks_structures: ["not_a_structure"],
        )"#;
        let (db, warnings) = load("ghost", &[("ghost_bench", bad)]);
        assert!(db.get("ghost_bench").is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not_a_structure"));
    }

    #[test]
    fn removing_an_invalid_node_also_removes_whatever_depended_on_it() {
        let bad = r#"(
            id: "ghost_bench",
            name: "Ghost Bench",
            description: "Unlocks a structure that does not exist.",
            cost: 5,
            unlocks_structures: ["not_a_structure"],
        )"#;
        let dependent = r#"(
            id: "dependent",
            name: "Dependent",
            description: "Hangs off a node that gets dropped.",
            cost: 9,
            requires: ["ghost_bench"],
        )"#;
        let (db, warnings) = load("cascade", &[("ghost_bench", bad), ("dependent", dependent)]);
        assert!(db.get("ghost_bench").is_none());
        assert!(
            db.get("dependent").is_none(),
            "a node whose prereq was dropped is just as unreachable"
        );
        assert_eq!(warnings.len(), 2, "each dropped node explains itself");
    }

    #[test]
    fn the_shipped_tree_loads_clean() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
        let (db, warnings) = ResearchDb::load_dir(&assets.join("research"), &structures).unwrap();
        assert!(
            warnings.is_empty(),
            "the shipped tree must not warn: {warnings:?}"
        );
        assert_eq!(db.all().count(), 13, "13 nodes ship with the game");
        assert_eq!(
            db.get("cortex").map(|d| d.cost),
            Some(45),
            "cortex is the deepest node"
        );
    }

    #[test]
    fn all_is_ordered_by_cost_then_id() {
        let cheap = r#"(id: "cheap", name: "Cheap", description: "d", cost: 1)"#;
        let mid_b = r#"(id: "b_mid", name: "B", description: "d", cost: 5)"#;
        let mid_a = r#"(id: "a_mid", name: "A", description: "d", cost: 5)"#;
        let (db, _) = load(
            "order",
            &[("cheap", cheap), ("b_mid", mid_b), ("a_mid", mid_a)],
        );
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["cheap", "a_mid", "b_mid"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }
}
