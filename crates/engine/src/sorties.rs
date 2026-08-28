//! The catalogue of places a squad can be sent, loaded from
//! `assets/sorties/`.
//!
//! Data only. A site says what it is called, how far above the zone
//! baseline it sits, and how many fights getting through it takes — never
//! how long it takes, which `Game::sortie_duration` derives from those two,
//! nor what it pays, which falls out of the fights actually had.
//!
//! **An empty database is valid and inert**, exactly like `NeedDb` and
//! `MemoryDb`: `Game::sortie_board` offers nothing, no squad can be
//! dispatched and nothing panics. Deleting `assets/sorties/` restores the
//! pre-sortie game rather than breaking an install, which is why an absent
//! directory is silent here. Never gate a system or a screen on the
//! database being non-empty — that makes the property hold by accident at
//! one site and lapse at another.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// A site's id — a string newtype for `NeedId`'s reason: a mod's site
/// cannot be an enum variant. `transparent`, so a def names itself in a
/// `.ron` file as a plain quoted string rather than as `SortieId("...")`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SortieId(pub String);

impl SortieId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SortieId {
    fn from(s: &str) -> Self {
        SortieId(s.to_string())
    }
}

impl std::fmt::Display for SortieId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One site.
///
/// Five of the six fields are required; only `risk` defaults, because a
/// quiet site an author never thought about the danger of is the sane
/// reading of an omitted offset. Any field added *later* must be
/// `#[serde(default)]`, per the standing rule for
/// `SpeciesDef`/`StructureDef`/`ItemDef`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SortieDef {
    pub id: SortieId,
    /// What the board row leads with.
    pub name: String,
    /// One line of flavour under it, in the player's vocabulary.
    pub description: String,
    /// Steps **above the zone baseline**, never an absolute danger band.
    /// The opposition is drawn at `Game::danger_steps(None) + risk`, so a
    /// site stays as dangerous relative to the run in sector 9 as it was in
    /// sector 1 — and `Game::sortie_duration` reads this offset rather than
    /// the absolute band, or every trip late in a run would take enormously
    /// longer for no reason the player could name.
    #[serde(default)]
    pub risk: u32,
    /// Fewest fights the board may offer this site at. Refused at load if
    /// zero, or if `battles_max` is below it — see `SortieDb::load_dir`.
    pub battles_min: u32,
    /// Most fights the board may offer this site at, inclusive.
    pub battles_max: u32,
}

impl SortieDef {
    /// Whether this def is internally coherent. A site whose range is
    /// inverted or empty is a content fault refused at load, the way
    /// `field_buff_duration_mismatch` refuses its corners: a `battles_max`
    /// below `battles_min` would otherwise roll an empty range at board
    /// time, a long way from the file that caused it.
    fn fault(&self) -> Option<&'static str> {
        if self.battles_min == 0 {
            return Some("battles_min must be at least 1");
        }
        if self.battles_max < self.battles_min {
            return Some("battles_max is below battles_min");
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn test_stub() -> Self {
        Self {
            id: SortieId("stub".to_string()),
            name: "Stub Site".to_string(),
            description: "A fixture.".to_string(),
            risk: 0,
            battles_min: 1,
            battles_max: 1,
        }
    }
}

/// Every site the game knows about, loaded from `assets/sorties/`.
///
/// See the module doc for why an empty database is a supported state rather
/// than an install fault.
#[derive(Resource, Default)]
pub struct SortieDb {
    defs: BTreeMap<SortieId, SortieDef>,
}

impl SortieDb {
    /// Loads every `*.ron` def in `dir`. Follows `NeedDb::load_dir` line for
    /// line: an absent directory is silent, and a malformed file costs the
    /// game that one site and nothing else rather than stopping a player
    /// reaching the main menu over somebody else's mod.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = SortieDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, because two files claiming one id must resolve the same
        // way every run — `NeedDb::load_dir`'s rule.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<SortieDef>(&text) {
                Ok(def) => match def.fault() {
                    Some(why) => {
                        warnings.push(format!("skipped invalid site file {path:?}: {why}"))
                    }
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid site file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &SortieId) -> Option<&SortieDef> {
        self.defs.get(id)
    }

    /// **Sorted by id.** Every caller walks this, and an unsorted walk is
    /// where a board that was supposed to be reproducible stops being so.
    pub fn iter(&self) -> impl Iterator<Item = &SortieDef> {
        self.defs.values()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
