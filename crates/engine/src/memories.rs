//! What an owned program can remember, loaded from `assets/memories/`.
//!
//! One def per memory kind: what it is called, how strongly it lands, how
//! fast it fades, and what it must be *about*. The catalogue is **data** and
//! the triggers that write memories are Rust, which is the same half-data
//! seam `perks::Perk` sits on — a `MemoryDef` deliberately carries no
//! `trigger` field, because a trigger vocabulary invented from the first
//! handful of samples is the speculative abstraction the code principles
//! forbid. `assets/memories/README.md` is the schema reference.
//!
//! **An empty database is valid and inert**, exactly like `NemesisDb` and
//! `SectorDb`: `Game::remember` finds no def and writes nothing, and every
//! reader skips an entry it cannot resolve, so deleting `assets/memories/`
//! restores the pre-memory game rather than breaking an install. That is why
//! an absent directory is silent here — the opposite call from `TalentDb`,
//! whose `?` on a missing directory is deliberate because every level a
//! Kernel Ring buys is spent in one of those trees. Do not "fix" this one to
//! match that neighbour.

use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// A memory kind's id — a string newtype for `talents::TalentId`'s reason: a
/// mod's memory cannot be an enum variant. `transparent`, so a def names
/// itself in a `.ron` file as a plain quoted string rather than as
/// `MemoryId("...")`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(String);

impl MemoryId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MemoryId {
    fn from(s: &str) -> Self {
        MemoryId(s.to_string())
    }
}

impl std::fmt::Display for MemoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The authorable half of `components::MemorySubject`: what a def declares it
/// is *about*, without the payload a record carries. A `.ron` file cannot
/// name a particular program or tile — those come from the trigger — so the
/// def declares the kind and the write validates the pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySubjectKind {
    Nothing,
    Program,
    Species,
    Structure,
    BaseTile,
    Activity,
}

/// One memory kind.
///
/// These seven fields are the initial schema and every one of them is
/// **required**: a def missing any of them is a def that cannot be scored or
/// drawn. Any field added *later* must be `#[serde(default)]`, per the
/// standing rule for `SpeciesDef`/`StructureDef`/`ItemDef`/`AbilityDef`, so a
/// mod's existing files keep parsing untouched — but do not retroactively
/// default these.
#[derive(Clone, Debug, Deserialize)]
pub struct MemoryDef {
    pub id: MemoryId,
    /// What a screen row leads with.
    pub name: String,
    /// One line of flavour under it, in the player's vocabulary.
    pub blurb: String,
    /// Signed, and the intensity of one undecayed strike. Negative is a
    /// grudge, positive a fondness.
    pub valence: f32,
    /// Ticks until intensity halves.
    pub half_life: u64,
    /// Which `MemorySubject` kind a record of this def must carry.
    pub subject: MemorySubjectKind,
    /// How far reinforcement compounds before it stops.
    pub strike_cap: u32,
}

/// Every memory kind the game knows about, loaded from `assets/memories/`.
///
/// See the module doc for why an empty database is a supported state rather
/// than an install fault.
#[derive(Resource, Default)]
pub struct MemoryDb {
    defs: HashMap<MemoryId, MemoryDef>,
}

impl MemoryDb {
    /// Loads every `*.ron` def in `dir`. Follows `AffixDb::load_dir` for the
    /// absent-directory rule and `TalentDb::load_dir` for the sorted walk and
    /// the per-file skip-and-warn: a malformed file costs the game that one
    /// memory kind and nothing else, and never stops a player reaching the
    /// main menu over somebody else's mod.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = MemoryDb::default();
        let mut warnings = Vec::new();
        // A missing directory is not an error: memories are optional content
        // and an install without them is the pre-memory game.
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
        // every run — the same reason `TalentDb::load_dir` sorts.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<MemoryDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid memory file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &MemoryId) -> Option<&MemoryDef> {
        self.defs.get(id)
    }

    /// Every loaded def, for the censuses. Order is map iteration order and
    /// so unstable; nothing may read it expecting an order.
    pub fn all(&self) -> impl Iterator<Item = &MemoryDef> {
        self.defs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed def, differing only in the fields a test cares about.
    fn def_text(id: &str, name: &str) -> String {
        format!(
            "(\n    id: \"{id}\",\n    name: \"{name}\",\n    blurb: \"b\",\n    \
             valence: -6.0,\n    half_life: 3000,\n    subject: BaseTile,\n    \
             strike_cap: 3,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (MemoryDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("memories");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        MemoryDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn a_well_formed_def_loads_with_no_warnings() {
        let (db, warnings) = load(&[("stranded.ron", def_text("stranded_at", "Left stranded"))]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(&MemoryId::from("stranded_at")).expect("loaded");
        assert_eq!(def.name, "Left stranded");
        assert_eq!(def.subject, MemorySubjectKind::BaseTile);
        assert_eq!(def.half_life, 3000);
        assert_eq!(def.strike_cap, 3);
        assert!(def.valence < 0.0);
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("bad.ron", "(id: \"broken\", name:".to_string()),
            ("good.ron", def_text("stranded_at", "Left stranded")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"), "{warnings:?}");
        assert!(
            db.get(&MemoryId::from("stranded_at")).is_some(),
            "the good file still loads"
        );
        assert!(db.get(&MemoryId::from("broken")).is_none());
    }

    /// Deleting `assets/memories/` is a supported way to play, so an absent
    /// directory is not even a warning.
    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("memories_absent");
        assert!(!dir.exists(), "the fixture must not create the directory");
        let (db, warnings) = MemoryDb::load_dir(&dir).expect("an absent directory is not an error");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(db.all().count(), 0);
    }

    /// Two files claiming one id must resolve the same way every run, so the
    /// walk is sorted and the alphabetically last file wins.
    ///
    /// The fixture is built forwards on purpose: this filesystem hands
    /// directory entries back in reverse creation order, so an unsorted walk
    /// would insert them alphabetically *backwards* and the first file would
    /// win instead. The guard below is what keeps that from being luck — on a
    /// filesystem that happens to enumerate in sorted order the fixture could
    /// not tell the two walks apart, and it says so rather than passing.
    #[test]
    fn two_files_claiming_one_id_resolve_the_same_way_every_run() {
        let names: Vec<String> = (0..12).map(|i| format!("f{i:02}.ron")).collect();
        let dir = crate::tests::support::scratch_assets_dir("memories_duplicate");
        std::fs::create_dir_all(&*dir).unwrap();
        for name in &names {
            std::fs::write(dir.join(name), def_text("stranded_at", name)).unwrap();
        }

        let raw: Vec<String> = std::fs::read_dir(&*dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_ne!(
            raw.last().map(String::as_str),
            names.last().map(String::as_str),
            "this filesystem enumerates the fixture in an order an unsorted walk \
             would resolve the same way a sorted one does, so the fixture cannot \
             prove anything — reorder it before trusting this test"
        );

        let (db, warnings) = MemoryDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(&MemoryId::from("stranded_at")).expect("loaded");
        assert_eq!(
            def.name, "f11.ron",
            "the alphabetically last file must win, whatever order the \
             directory hands its entries back in"
        );
    }
}
