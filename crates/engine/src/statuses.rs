//! Status conditions, loaded from `assets/statuses/`.
//!
//! One def per status: what it is called, how it stacks, which of a closed
//! list of behaviours it composes, and the log lines it speaks. The
//! behaviours are Rust and the catalogue is data — modders compose, they do
//! not extend. `assets/statuses/README.md` is the schema reference.
//!
//! **An empty database is valid**: nothing can be armed. A missing directory
//! is not an error, `MemoryDb`'s rule.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A status's name in a `.ron` file. `transparent`, so a move names one as a
/// plain quoted string, and a mod's status needs no enum variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StatusId(pub String);

impl From<&str> for StatusId {
    fn from(s: &str) -> Self {
        StatusId(s.to_string())
    }
}

/// How re-arming a status already carried combines with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusStacking {
    /// One entry stays; re-arming keeps the larger of the remaining rounds and
    /// the power.
    Refresh,
    /// Each application adds a stack, up to `max`.
    Stack { max: u32 },
}

/// What a status does while it is carried. Closed on purpose; each acts per
/// stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusBehaviour {
    DamagePerRound,
    SkipTurn,
    EvasionCut(i32),
    AtkPercent(i32),
    MitigationPercent(i32),
    HealBlock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusDef {
    pub id: StatusId,
    /// Party-panel label.
    pub name: String,
    /// Battle-map tag.
    pub tag: String,
    pub stacking: StatusStacking,
    pub behaviours: Vec<StatusBehaviour>,
    /// Log line when armed; `{target}` is the only placeholder.
    pub inflict: String,
    /// Log line each round for `DamagePerRound`; adds `{n}`.
    #[serde(default)]
    pub tick: String,
    /// Log line when it runs out; `{target}` is the only placeholder.
    pub expire: String,
}

impl StatusDef {
    /// The line for arming this status on `target`.
    pub fn inflict_line(&self, target: &str) -> String {
        fill(&self.inflict, target, None)
    }

    /// The line for one round of `DamagePerRound` dealing `n`.
    pub fn tick_line(&self, target: &str, n: i32) -> String {
        fill(&self.tick, target, Some(n))
    }

    /// The line for this status running out on `target`.
    pub fn expire_line(&self, target: &str) -> String {
        fill(&self.expire, target, None)
    }

    pub fn has_behaviour(&self, pred: impl Fn(&StatusBehaviour) -> bool) -> bool {
        self.behaviours.iter().any(pred)
    }
}

/// The one place a log template's placeholders are substituted, so a move's
/// landing line and an ability's cannot come to read differently.
fn fill(template: &str, target: &str, n: Option<i32>) -> String {
    let line = template.replace("{target}", target);
    match n {
        Some(n) => line.replace("{n}", &n.to_string()),
        None => line,
    }
}

/// Every status the game knows, loaded from `assets/statuses/`.
#[derive(Resource, Default, Clone)]
pub struct StatusDb {
    defs: HashMap<StatusId, StatusDef>,
}

impl StatusDb {
    /// Loads every `*.ron` def in `dir`, in sorted file order so a duplicate
    /// id resolves the same way every run: the first file wins and the rest
    /// are warned about. A malformed file is skipped with a warning.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = StatusDb::default();
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
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<StatusDef>(&text) {
                Ok(def) if db.defs.contains_key(&def.id) => warnings.push(format!(
                    "skipped duplicate status {:?} in {path:?}: an earlier file already defines it",
                    def.id.0
                )),
                Ok(StatusDef {
                    stacking: StatusStacking::Stack { max: 0 },
                    id,
                    ..
                }) => warnings.push(format!(
                    "skipped status {:?} in {path:?}: Stack max must be at least 1",
                    id.0
                )),
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid status file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// Adds or replaces a def, so a test can carry a status no shipped file
    /// defines.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: StatusDef) {
        self.defs.insert(def.id.clone(), def);
    }

    pub fn get(&self, id: &StatusId) -> Option<&StatusDef> {
        self.defs.get(id)
    }

    /// The party-panel name of `id`, or the raw id for one with no definition
    /// — a label that still says *something* about a bad reference.
    pub fn name_of<'a>(&'a self, id: &'a StatusId) -> &'a str {
        self.get(id).map_or(id.0.as_str(), |def| def.name.as_str())
    }

    pub fn contains(&self, id: &StatusId) -> bool {
        self.defs.contains_key(id)
    }

    /// Every id, for the census over shipped content.
    #[cfg(test)]
    pub(crate) fn ids(&self) -> impl Iterator<Item = &StatusId> {
        self.defs.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_text(id: &str, name: &str) -> String {
        format!(
            "(\n    id: \"{id}\",\n    name: \"{name}\",\n    tag: \"TST\",\n    \
             stacking: Refresh,\n    behaviours: [SkipTurn],\n    \
             inflict: \"{{target}} is hit!\",\n    expire: \"{{target}} recovers.\",\n)\n"
        )
    }

    fn load(scratch: &str, files: &[(&str, String)]) -> (StatusDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir(scratch);
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        StatusDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn the_shipped_directory_loads_every_status_with_no_warnings() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/statuses");
        let (db, warnings) = StatusDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let bleed = db.get(&StatusId::from("bleed")).expect("bleed");
        assert_eq!(bleed.behaviours, [StatusBehaviour::DamagePerRound]);
        assert_eq!(bleed.stacking, StatusStacking::Refresh);
        assert!(bleed.tick.contains("{n}"));
        let stun = db.get(&StatusId::from("stun")).expect("stun");
        assert_eq!(stun.behaviours, [StatusBehaviour::SkipTurn]);
        assert!(stun.tick.is_empty(), "tick defaults to empty");
        let exposed = db.get(&StatusId::from("exposed")).expect("exposed");
        assert_eq!(exposed.behaviours, [StatusBehaviour::EvasionCut(50)]);
    }

    #[test]
    fn a_stack_def_parses() {
        let body = def_text("poison", "Poisoned").replace("Refresh", "Stack(max: 5)");
        let (db, warnings) = load("statuses_stack", &[("poison.ron", body)]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(&StatusId::from("poison")).expect("loaded");
        assert_eq!(def.stacking, StatusStacking::Stack { max: 5 });
    }

    #[test]
    fn a_stack_def_with_max_zero_is_skipped_and_warned() {
        let body = def_text("dud", "Dud").replace("Refresh", "Stack(max: 0)");
        let (db, warnings) = load("statuses_stack_zero", &[("dud.ron", body)]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(db.get(&StatusId::from("dud")).is_none());
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warned() {
        let (db, warnings) = load(
            "statuses_malformed",
            &[
                ("a_bad.ron", "(id: \"bad\"".to_string()),
                ("b_good.ron", def_text("good", "Good")),
            ],
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(db.get(&StatusId::from("bad")).is_none());
        assert!(db.get(&StatusId::from("good")).is_some());
    }

    #[test]
    fn a_duplicate_id_is_warned_and_the_first_file_wins() {
        let (db, warnings) = load(
            "statuses_duplicate",
            &[
                ("a.ron", def_text("dup", "First")),
                ("b.ron", def_text("dup", "Second")),
            ],
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(db.get(&StatusId::from("dup")).unwrap().name, "First");
    }

    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("statuses_absent");
        assert!(!dir.exists());
        let (db, warnings) = StatusDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(db.get(&StatusId::from("bleed")).is_none());
    }
}
