use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;
use crate::world::Biome;

pub type SpeciesId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveDef {
    pub name: String,
    pub power: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesDef {
    pub id: SpeciesId,
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub base_hp: i32,
    pub base_atk: i32,
    pub base_def: i32,
    /// 0.0 (trivial) .. 1.0 (very hard) to tame.
    pub taming_difficulty: f32,
    pub habitats: Vec<Biome>,
    pub moves: Vec<MoveDef>,
    /// If set, a tamed member of this species can work a matching resource node.
    pub work_resource: Option<ItemId>,
    /// If set, defeating/decompiling this species has a chance (0.0-1.0) to
    /// additionally drop one piece of equipment, independent of
    /// `work_resource`. `#[serde(default)]` so existing species files
    /// (including mods) without this field keep parsing.
    #[serde(default)]
    pub equipment_drop: Option<(ItemId, f32)>,
}

#[derive(Resource, Default)]
pub struct SpeciesDb {
    species: HashMap<SpeciesId, SpeciesDef>,
}

impl SpeciesDb {
    /// Loads every `*.ron` species definition in `dir`. Malformed files are
    /// skipped (with a returned warning) rather than aborting the whole
    /// load — a single bad custom/mod file shouldn't be able to crash
    /// startup for everything else.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = SpeciesDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<SpeciesDef>(&text) {
                Ok(def) => {
                    db.species.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid species file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&SpeciesDef> {
        self.species.get(id)
    }

    pub fn habitat_matches(&self, biome: Biome) -> Vec<&SpeciesDef> {
        self.species
            .values()
            .filter(|s| s.habitats.contains(&biome))
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &SpeciesDef> {
        self.species.values()
    }
}
