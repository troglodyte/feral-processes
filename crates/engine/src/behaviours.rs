//! Tactical behaviour profiles: how a species' hostiles weigh their choices
//! on a battle map. Data in `assets/behaviours/`, schema in that
//! directory's README.
//!
//! `BehaviourDb::load_dir` follows the `*Db::load_dir` pattern — a malformed
//! file is skipped with a warning — with one difference: `default` must
//! exist, so its absence is a load error rather than an empty catalogue.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Game;
use crate::components::Creature;
use crate::resources::Behaviours;
use crate::species::SpeciesDb;
use crate::tactical::policy::{CellWeights, TacticalFeature};
use bevy_ecs::prelude::Entity;

const DEFAULT_ID: &str = "default";

/// The side of zero a trained weight may not cross.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sign {
    Positive,
    Negative,
}

/// One personality. Features are named by string and resolved to
/// `TacticalFeature` where they are scored, so a profile naming a feature
/// this build does not know is a warning and not a parse failure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BehaviourDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Authored character; the trainer never writes these.
    #[serde(default)]
    pub pinned: BTreeMap<String, f32>,
    /// Directions the trainer may not reverse.
    #[serde(default)]
    pub signs: BTreeMap<String, Sign>,
    /// Free weights, the trainer's to write.
    #[serde(default)]
    pub weights: BTreeMap<String, f32>,
    #[serde(default)]
    pub cells: CellWeights,
}

#[derive(Clone, Debug)]
pub struct BehaviourDb {
    profiles: BTreeMap<String, BehaviourDef>,
    /// A fixed fight's profile for every hostile, set by the arena.
    forced: Option<String>,
}

impl BehaviourDb {
    /// Loads every `*.ron` profile in `dir`. A missing `default` is an
    /// error: every species without a `behaviour` plays as it.
    pub fn load_dir(dir: &Path) -> io::Result<(Self, Vec<String>)> {
        let mut warnings = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, so which of two files claiming one id wins does not depend
        // on the OS's directory order.
        paths.sort();
        let mut profiles = BTreeMap::new();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<BehaviourDef>(&text) {
                Ok(mut def) => {
                    let doubled: Vec<String> = def
                        .weights
                        .keys()
                        .filter(|k| def.pinned.contains_key(*k))
                        .cloned()
                        .collect();
                    for feature in doubled {
                        warnings.push(format!(
                            "behaviour {:?}: {feature:?} is both pinned and a free weight — \
                             pinned wins",
                            def.id
                        ));
                        def.weights.remove(&feature);
                    }
                    let unknown: Vec<&String> = def
                        .pinned
                        .keys()
                        .chain(def.weights.keys())
                        .chain(def.signs.keys())
                        .filter(|k| TacticalFeature::from_name(k).is_none())
                        .collect();
                    for feature in unknown {
                        warnings.push(format!(
                            "behaviour {:?}: unknown feature {feature:?} — ignored",
                            def.id
                        ));
                    }
                    let against_sign: Vec<String> = def
                        .weights
                        .iter()
                        .filter(|(k, w)| match def.signs.get(*k) {
                            Some(Sign::Positive) => **w < 0.0,
                            Some(Sign::Negative) => **w > 0.0,
                            None => false,
                        })
                        .map(|(k, _)| k.clone())
                        .collect();
                    for feature in against_sign {
                        warnings.push(format!(
                            "behaviour {:?}: {feature:?} is weighted against its own sign",
                            def.id
                        ));
                    }
                    if profiles.contains_key(&def.id) {
                        warnings.push(format!(
                            "behaviour {:?} in {path:?}: id already defined — first file wins",
                            def.id
                        ));
                    } else {
                        profiles.insert(def.id.clone(), def);
                    }
                }
                Err(e) => warnings.push(format!("skipped invalid behaviour file {path:?}: {e}")),
            }
        }
        if !profiles.contains_key(DEFAULT_ID) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{dir:?} has no valid {DEFAULT_ID:?} behaviour profile"),
            ));
        }
        Ok((
            Self {
                profiles,
                forced: None,
            },
            warnings,
        ))
    }

    pub fn get(&self, id: &str) -> Option<&BehaviourDef> {
        self.profiles.get(id)
    }

    pub fn default_profile(&self) -> &BehaviourDef {
        self.profiles
            .get(DEFAULT_ID)
            .expect("load_dir refuses a db with no default profile")
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }

    /// Adds or replaces a profile, for a test that needs one of its own.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: BehaviourDef) {
        self.profiles.insert(def.id.clone(), def);
    }

    /// Makes every hostile play `id`, or lifts the override. An id with no
    /// profile is ignored on read, like a species naming one.
    pub fn set_override(&mut self, id: Option<String>) {
        self.forced = id;
    }

    /// The profile a species names, `default` when it names none or one this
    /// db lacks; an override beats both.
    pub fn for_species(&self, behaviour: Option<&str>) -> &BehaviourDef {
        self.forced
            .as_deref()
            .into_iter()
            .chain(behaviour)
            .find_map(|id| self.get(id))
            .unwrap_or_else(|| self.default_profile())
    }

    /// One warning per species naming a profile this db does not hold.
    pub fn unknown_species_references(&self, species: &SpeciesDb) -> Vec<String> {
        let mut named: Vec<(&str, &str)> = species
            .all()
            .filter_map(|d| Some((d.id.as_str(), d.behaviour.as_deref()?)))
            .filter(|(_, b)| self.get(b).is_none())
            .collect();
        named.sort();
        named
            .into_iter()
            .map(|(id, b)| {
                format!("species {id:?} names unknown behaviour {b:?} — plays as default")
            })
            .collect()
    }
}

/// Rewrites `path` whole from `def`. Comments in the old file are lost.
pub fn write_file(path: &Path, def: &BehaviourDef) -> io::Result<()> {
    let text = ron::ser::to_string_pretty(def, ron::ser::PrettyConfig::default())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, text)
}

impl Game {
    /// The profile `body` decides by: an arena override if set, else its
    /// species' `behaviour`, else `default`.
    pub fn behaviour_of(&self, body: Entity) -> &BehaviourDef {
        let named = self
            .world
            .get::<Creature>(body)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .and_then(|def| def.behaviour.as_deref());
        self.world.resource::<Behaviours>().0.for_species(named)
    }
}
