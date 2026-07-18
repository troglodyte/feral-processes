use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::{GlyphColor, StatusKind};
use crate::items::ItemId;
use crate::world::Biome;

pub type SpeciesId = String;

/// A status condition a move has a chance to inflict on top of its direct
/// damage — see `components::StatusEffects`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveEffect {
    pub kind: StatusKind,
    /// Chance (0.0-1.0) this effect actually applies when the move lands.
    pub chance: f32,
    /// How many battle rounds the effect lasts.
    pub duration: u32,
    /// Bleed damage dealt per round; unused (but still required in the
    /// `.ron` file — use 0) for `Stun`.
    #[serde(default)]
    pub power: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveDef {
    pub name: String,
    pub power: i32,
    /// Optional status effect this move has a chance to inflict on the
    /// target when it lands, independent of its direct damage.
    /// `#[serde(default)]` so existing species files (including mods)
    /// without this field keep parsing as plain damage-only moves.
    #[serde(default)]
    pub effect: Option<MoveEffect>,
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
    /// Marks this species as a boss: excluded from the normal per-tile
    /// habitat spawn roll and instead spawned rarely in its place (see
    /// `Game::try_spawn_habitat_creature`), and guaranteed a cache of
    /// Portal Fragments on defeat instead of the flat drop chance every
    /// other species rolls (see `Game::award_loot`). A boss's stats are
    /// still whatever `base_hp`/`base_atk`/`base_def` are authored as —
    /// there's no separate engine-side multiplier, so make them tough in
    /// the `.ron` file itself. `#[serde(default)]` so existing species
    /// files (including mods) without this field keep parsing as ordinary,
    /// non-boss species.
    #[serde(default)]
    pub is_boss: bool,
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

    /// Ordinary (non-boss) species that can inhabit `biome` — the pool the
    /// normal per-tile spawn roll draws from.
    pub fn habitat_matches(&self, biome: Biome) -> Vec<&SpeciesDef> {
        self.species
            .values()
            .filter(|s| !s.is_boss && s.habitats.contains(&biome))
            .collect()
    }

    /// Boss species that can inhabit `biome` — a separate, much rarer pool
    /// than `habitat_matches`.
    pub fn boss_habitat_matches(&self, biome: Biome) -> Vec<&SpeciesDef> {
        self.species
            .values()
            .filter(|s| s.is_boss && s.habitats.contains(&biome))
            .collect()
    }

    pub fn all(&self) -> impl Iterator<Item = &SpeciesDef> {
        self.species.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn species_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species")
    }

    #[test]
    fn habitat_matches_excludes_bosses_and_boss_habitat_matches_includes_only_them() {
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
        assert!(warnings.is_empty(), "species assets should all load cleanly: {warnings:?}");

        let normal = db.habitat_matches(Biome::OpenGrid);
        assert!(!normal.is_empty(), "OpenGrid should have ordinary habitat species");
        assert!(normal.iter().all(|s| !s.is_boss), "habitat_matches should never include a boss species");

        let bosses = db.boss_habitat_matches(Biome::OpenGrid);
        assert!(!bosses.is_empty(), "at least one boss species should inhabit OpenGrid");
        assert!(bosses.iter().all(|s| s.is_boss), "boss_habitat_matches should only ever include boss species");
    }
}
