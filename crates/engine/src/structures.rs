use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;

pub type StructureId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkDef {
    pub produces: ItemId,
    pub ticks_per_unit: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassiveProcessDef {
    pub consumes: ItemId,
    pub produces: ItemId,
    pub ticks_per_unit: u32,
    /// Chebyshev distance (in tiles) the player must be within for this to run.
    pub radius: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructureDef {
    pub id: StructureId,
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub build_cost: Vec<(ItemId, u32)>,
    /// If set, a tamed creature can be assigned to work this structure,
    /// producing `produces` every `ticks_per_unit` ticks.
    pub work: Option<WorkDef>,
    /// If set, this structure automatically converts `consumes` into
    /// `produces` whenever the player is standing within `radius` tiles —
    /// no assigned worker needed, unlike `work`. `#[serde(default)]` so
    /// existing structure files written before this field existed still
    /// parse (defaulting to no passive processing).
    #[serde(default)]
    pub passive_process: Option<PassiveProcessDef>,
    /// If set, this structure is a symlink target: `Game::use_symlink` can
    /// teleport the player to it for this item cost, from anywhere on the
    /// map. `#[serde(default)]` so existing structure files written before
    /// this field existed still parse (defaulting to no symlink).
    #[serde(default)]
    pub teleport_cost: Option<Vec<(ItemId, u32)>>,
}

#[derive(Resource, Default)]
pub struct StructureDb {
    structures: HashMap<StructureId, StructureDef>,
}

impl StructureDb {
    /// Loads every `*.ron` structure definition in `dir`. Malformed files
    /// are skipped (with a returned warning) rather than aborting the whole
    /// load — a single bad custom/mod file shouldn't be able to crash
    /// startup for everything else.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = StructureDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<StructureDef>(&text) {
                Ok(def) => {
                    db.structures.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid structure file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&StructureDef> {
        self.structures.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &StructureDef> {
        self.structures.values()
    }
}
