//! The settlement catalogue: one `.ron` file per town or city.
//!
//! `MemoryDb::load_dir`'s shape exactly — a malformed file is skipped with
//! a warning rather than panicking startup, and an **absent directory loads
//! empty**, which is the supported install with no settlements on the map
//! at all. Nothing may gate a draw or a walk on the catalogue being
//! non-empty: `placement::settlement_at` returns `None` for every region
//! when it is, and that is the pre-settlement game.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::{SettlementKind, Specialty, Temperament};

/// One authored settlement.
///
/// Every field is required. There is no `#[serde(default)]` here on
/// purpose: a settlement with no specialty or no temperament is not a
/// neutral settlement, it is one whose behaviour hooks have nothing to read
/// — and a file that silently loads half-authored is worse than one that is
/// skipped loudly.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SettlementDef {
    pub id: String,
    pub name: String,
    /// One or two sentences, shown on the settlement's own screen.
    pub blurb: String,
    pub kind: SettlementKind,
    pub specialty: Specialty,
    pub temperament: Temperament,
}

/// Every settlement the install can place, in id order.
#[derive(Default, Debug)]
pub struct SettlementDb {
    /// `BTreeMap` rather than `HashMap` because `placement` indexes into
    /// this pool by a derived number: a `HashMap`'s iteration order would
    /// make which town stands where differ between runs of the same seed.
    defs: BTreeMap<String, SettlementDef>,
}

impl SettlementDb {
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = SettlementDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<SettlementDef>(&text) {
                Ok(def) if def.id.is_empty() => warnings.push(format!(
                    "skipped invalid settlement file {path:?}: id must not be empty"
                )),
                Ok(def) if def.name.trim().is_empty() => warnings.push(format!(
                    "skipped invalid settlement file {path:?}: a settlement the map draws must \
                     have a name to answer with"
                )),
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid settlement file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&SettlementDef> {
        self.defs.get(id)
    }

    /// Every settlement in id order — what `placement` indexes and what the
    /// asset censuses walk.
    pub fn iter(&self) -> impl Iterator<Item = &SettlementDef> {
        self.defs.values()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
