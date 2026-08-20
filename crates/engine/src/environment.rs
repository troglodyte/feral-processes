//! Ambient environment effects: what the ground does to you for walking on
//! it.
//!
//! Keyed to the biome and loaded from `assets/environment/*.ron`, on the
//! same `load_dir`-with-warnings pattern every other asset db follows. An
//! absent directory is silent and leaves the db empty, which is the
//! pre-environment game — deleting the directory restores it exactly, the
//! same supported way deleting `assets/sectors/` does.
//!
//! Data and loading only: nothing here knows about a `Game`. The one reader
//! that resolves a tile to an effect, and the zone-1 gate it holds, live in
//! `game/environment.rs`.

use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::tuning::{MAX_ENVIRONMENT_ATTRITION, MAX_ENVIRONMENT_DRAG_TICKS};
use crate::world::Biome;

/// What standing on this ground costs.
///
/// Two shapes rather than one, so the vocabulary is not all damage: ground
/// that slows you down is a different kind of pressure from ground that
/// hurts you, and a sector made entirely of the first still reads as
/// hostile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EnvironmentEffect {
    /// A bite out of the player's Integrity, as a fraction of their maximum
    /// with a floor. A fraction rather than a flat figure for the reason
    /// `bleed_corruption` records: terrain is uncorrelated with player
    /// level, so any constant is lethal at level 1 and free by mid-run.
    Attrition { hp_percent: f32, min_damage: i32 },
    /// Extra ticks the step costs, on top of the one every step costs.
    Drag { extra_ticks: u32 },
}

/// One ambient environment as authored. See `assets/environment/README.md`
/// for the schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentDef {
    pub id: String,
    /// What the player calls this ground's condition, distinct from the
    /// biome's own name.
    pub name: String,
    /// The sentence under that name.
    pub description: String,
    /// Every biome this effect claims. A def may claim several; two defs
    /// may not claim the same one.
    pub biomes: Vec<Biome>,
    pub effect: EnvironmentEffect,
}

impl EnvironmentDef {
    /// Why this file is unusable, or `None` if it is fine.
    ///
    /// Three refusals, each protecting something a file has no business
    /// revoking: the base's own floor is the one safe ground in the game, an
    /// authored attrition of `0.5` is death in two steps, and an authored
    /// drag of `10_000` is a hang the player cannot tell from a crash.
    fn fault(&self) -> Option<String> {
        if self.biomes.contains(&Biome::Platform) {
            return Some(
                "claims Platform, which is the base's own floor — nothing spawns there and \
                 no ambush fires there, and that is not a file's decision to revoke"
                    .to_string(),
            );
        }
        match self.effect {
            EnvironmentEffect::Attrition {
                hp_percent,
                min_damage,
            } => {
                if !(0.0..=MAX_ENVIRONMENT_ATTRITION).contains(&hp_percent) {
                    return Some(format!(
                        "hp_percent {hp_percent} is outside 0.0..={MAX_ENVIRONMENT_ATTRITION}"
                    ));
                }
                if min_damage < 0 {
                    return Some(format!("min_damage {min_damage} would heal, not hurt"));
                }
            }
            EnvironmentEffect::Drag { extra_ticks } => {
                if extra_ticks > MAX_ENVIRONMENT_DRAG_TICKS {
                    return Some(format!(
                        "extra_ticks {extra_ticks} is above the ceiling of \
                         {MAX_ENVIRONMENT_DRAG_TICKS}"
                    ));
                }
            }
        }
        None
    }
}

#[derive(Resource, Default)]
pub struct EnvironmentDb {
    defs: Vec<EnvironmentDef>,
    by_biome: HashMap<Biome, usize>,
}

impl EnvironmentDb {
    /// Loads every `.ron` file in `dir`, skipping any that is malformed or
    /// that `EnvironmentDef::fault` refuses, and returning the warnings for
    /// the caller to push into the message log. Never panics: a malformed
    /// mod file must not crash startup.
    ///
    /// Files are read in sorted order rather than the order the filesystem
    /// hands them back, so which of two files claiming one biome is the one
    /// refused is the same on every platform.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = EnvironmentDb::default();
        let mut warnings = Vec::new();
        // A missing directory is not an error: ambient effects are optional
        // content and an install without them is the pre-environment game.
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            let def = match ron::from_str::<EnvironmentDef>(&text) {
                Ok(def) => def,
                Err(e) => {
                    warnings.push(format!("skipped invalid environment file {path:?}: {e}"));
                    continue;
                }
            };
            if let Some(fault) = def.fault() {
                warnings.push(format!(
                    "skipped invalid environment file {path:?}: {fault}"
                ));
                continue;
            }
            if let Some(&taken) = def.biomes.iter().find_map(|b| db.by_biome.get(b)) {
                warnings.push(format!(
                    "skipped environment file {path:?}: its biome is already claimed by {:?} — \
                     one biome, one ambient effect",
                    db.defs[taken].id
                ));
                continue;
            }
            let index = db.defs.len();
            for biome in &def.biomes {
                db.by_biome.insert(*biome, index);
            }
            db.defs.push(def);
        }
        Ok((db, warnings))
    }

    /// The ambient effect claiming `biome`, if any. Unclaimed ground is
    /// neutral, which is what makes an empty db the pre-environment game.
    pub fn for_biome(&self, biome: Biome) -> Option<&EnvironmentDef> {
        self.by_biome.get(&biome).map(|&i| &self.defs[i])
    }

    pub fn all(&self) -> impl Iterator<Item = &EnvironmentDef> {
        self.defs.iter()
    }
}
