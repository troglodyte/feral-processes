//! What an owned program needs kept topped up, loaded from `assets/needs/`.
//!
//! One def per need: what it is called, how fast it falls, the two thresholds
//! that decide when a program leaves its post and when it comes back, and
//! what an empty reserve is worth to morale. The catalogue is **data** and
//! the drain, the errand and the teeth are Rust — the same half-data seam
//! `memories::MemoryDef` sits on, and for the same reason: a need's effect is
//! a hook into a particular formula, not something a `.ron` file can express.
//! `assets/needs/README.md` is the schema reference.
//!
//! **An empty database is valid and inert**, exactly like `MemoryDb`: nothing
//! is seeded, nothing drains, no program ever goes off shift and `strain`
//! answers zero without a branch. Deleting `assets/needs/` restores the
//! pre-needs game rather than breaking an install, which is why an absent
//! directory is silent here. Never gate a system or a screen on the database
//! being non-empty — that makes the property hold by accident at one site and
//! lapse at another.

use crate::components::Needs;
use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// A reserve runs 0..100, `PowerReserve`'s range and for its readability
/// reason: `critical`, `content` and every authored threshold are then
/// plainly percentages of a bar the player can be shown.
pub const NEED_MIN: f32 = 0.0;
pub const NEED_MAX: f32 = 100.0;

/// A need's id — a string newtype for `MemoryId`'s reason: a mod's need
/// cannot be an enum variant. `transparent`, so a def names itself in a
/// `.ron` file as a plain quoted string rather than as `NeedId("...")`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NeedId(String);

impl NeedId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NeedId {
    fn from(s: &str) -> Self {
        NeedId(s.to_string())
    }
}

impl std::fmt::Display for NeedId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One need.
///
/// These nine fields are the initial schema and every one of them is
/// **required**: a def missing any of them is a def that cannot be drained,
/// serviced or drawn. Any field added *later* must be `#[serde(default)]`,
/// per the standing rule for `SpeciesDef`/`StructureDef`/`ItemDef`, so a
/// mod's existing files keep parsing untouched — but do not retroactively
/// default these.
#[derive(Clone, Debug, Deserialize)]
pub struct NeedDef {
    pub id: NeedId,
    /// What a manifest row leads with.
    pub name: String,
    /// One line of flavour under it, in the player's vocabulary.
    pub blurb: String,
    /// The player's verb for the errand: what the program is *doing* while it
    /// is off shift servicing this need.
    pub servicing: String,
    /// How much the reserve falls per tick with the program idle.
    pub drain_per_tick: f32,
    /// Multiplies `drain_per_tick` while the program holds a `Task`.
    pub working_multiplier: f32,
    /// Below this the program leaves its post.
    pub critical: f32,
    /// At this the program is done and goes back on shift.
    pub content: f32,
    /// The morale contribution at empty, scaled linearly to nothing at
    /// `content` — so a satisfied need is worth zero rather than worth a
    /// little. Signed; negative is a drag.
    pub morale_weight: f32,
}

/// Every need the game knows about, loaded from `assets/needs/`.
///
/// See the module doc for why an empty database is a supported state rather
/// than an install fault.
#[derive(Resource, Default)]
pub struct NeedDb {
    defs: BTreeMap<NeedId, NeedDef>,
}

impl NeedDb {
    /// Loads every `*.ron` def in `dir`. Follows `MemoryDb::load_dir` line for
    /// line: an absent directory is silent, and a malformed file costs the
    /// game that one need and nothing else rather than stopping a player
    /// reaching the main menu over somebody else's mod.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = NeedDb::default();
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
        // Sorted, because two files claiming one id must resolve the same way
        // every run — `MemoryDb::load_dir`'s rule.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<NeedDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid need file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &NeedId) -> Option<&NeedDef> {
        self.defs.get(id)
    }

    /// **Sorted by id.** Every caller iterates this; an unsorted walk is where
    /// a nondeterministic tie-break gets in.
    pub fn iter(&self) -> impl Iterator<Item = &NeedDef> {
        self.defs.values()
    }
}

/// What a program's reserves are worth to morale — signed, baseline **zero**.
///
/// Each def's `morale_weight` scaled linearly from full at `NEED_MIN` to
/// nothing at `content`, so a satisfied need contributes exactly zero and a
/// program with no `Needs` at all contributes exactly zero, without a branch
/// at either site. `base_int`'s idiom, in the same expression.
///
/// **A free function for `party::role_of`'s reason.** A bevy system has no
/// `Game` to ask, and two folds would eventually disagree about whether an
/// unresolvable def counts — which is the property the whole empty-catalogue
/// guarantee rests on. `Game::need_strain` is a caller of this.
///
/// **An entry whose def no file defines is skipped**, contributing nothing,
/// exactly as every `Memories` reader skips one.
pub fn strain(needs: &Needs, db: &NeedDb) -> f32 {
    needs
        .iter()
        .filter_map(|(id, value)| {
            let def = db.get(id)?;
            // At or above `content` the need is satisfied and worth nothing.
            // Below it, the weight ramps linearly to full at `NEED_MIN`.
            let span = def.content - NEED_MIN;
            if span <= 0.0 {
                return None;
            }
            let short = (def.content - value).clamp(0.0, span);
            Some(def.morale_weight * (short / span))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed def, differing only in the fields a test cares about.
    fn def_text(id: &str, name: &str) -> String {
        format!(
            "(\n    id: \"{id}\",\n    name: \"{name}\",\n    blurb: \"b\",\n    \
             servicing: \"Defragmenting\",\n    drain_per_tick: 0.02,\n    \
             working_multiplier: 2.0,\n    critical: 20.0,\n    content: 60.0,\n    \
             morale_weight: -4.0,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (NeedDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("needs");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        NeedDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn the_shipped_defs_load_and_are_found_by_id() {
        let (db, warnings) =
            NeedDb::load_dir(&crate::tests::support::test_assets_dir().join("needs")).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        for id in ["coherence", "slack"] {
            let def = db.get(&NeedId::from(id)).unwrap_or_else(|| panic!("{id}"));
            assert!(def.drain_per_tick > 0.0, "{id} must actually fall");
            assert!(
                def.critical < def.content,
                "{id}: leaving a post and being done are two thresholds"
            );
        }
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("bad.ron", "(id: \"broken\", name:".to_string()),
            ("good.ron", def_text("coherence", "Coherence")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"), "{warnings:?}");
        assert!(db.get(&NeedId::from("coherence")).is_some());
        assert!(db.get(&NeedId::from("broken")).is_none());
    }

    /// Deleting `assets/needs/` is a supported way to play, so an absent
    /// directory is not even a warning.
    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("needs_absent");
        assert!(!dir.exists(), "the fixture must not create the directory");
        let (db, warnings) = NeedDb::load_dir(&dir).expect("an absent directory is not an error");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(db.iter().count(), 0);
    }

    /// Every caller iterates `iter`, so it is sorted by id whatever order the
    /// directory hands its entries back in.
    #[test]
    fn iteration_is_in_id_order_however_the_files_were_written() {
        let (db, warnings) = load(&[
            ("z.ron", def_text("slack", "Slack")),
            ("a.ron", def_text("coherence", "Coherence")),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["coherence", "slack"]);
    }
}
