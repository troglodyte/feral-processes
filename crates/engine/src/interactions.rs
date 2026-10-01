//! What two idle programs can say to each other, loaded from
//! `assets/interactions/`.
//!
//! One def per kind of exchange: which memory it leaves on the listener and
//! the speaker, and how likely it is given the speaker's disposition and the
//! band of the speaker's opinion of the listener. The catalogue is **data**;
//! the pass that rolls and writes is `Game::note_interactions`.
//!
//! **An empty database is valid and inert**, `MemoryDb`'s rule: an absent
//! `assets/interactions/` is silent and means no program ever talks.

use crate::bonds::Bond;
use crate::disposition::Disposition;
use crate::memories::{MemoryDb, MemoryId};
use bevy_ecs::prelude::Resource;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// One kind of exchange. `assets/interactions/README.md` is the schema.
#[derive(Clone, Debug, Deserialize)]
pub struct InteractionDef {
    pub id: String,
    /// For the README table and a later screen; not shown yet.
    pub name: String,
    /// Written on the listener, about the speaker. Ignored when `gossip`:
    /// the hearsay def comes from the speaker's own memory's `spreads_as`.
    pub listener_memory: String,
    /// Written on the speaker, about the listener.
    #[serde(default)]
    pub speaker_memory: Option<String>,
    /// Base pick weight; finite and `>= 0`.
    pub weight: f32,
    /// Multiplier on the **speaker's** disposition; absent is `1.0`.
    #[serde(default)]
    pub by_disposition: BTreeMap<Disposition, f32>,
    /// Multiplier on the band of the speaker's opinion of the listener;
    /// absent is `1.0`.
    #[serde(default)]
    pub by_band: BTreeMap<Bond, f32>,
    /// Tells the speaker's strongest tellable memory instead of writing a
    /// fixed one.
    #[serde(default)]
    pub gossip: bool,
}

impl InteractionDef {
    fn non_finite_field(&self) -> Option<&'static str> {
        let ok = |v: &f32| v.is_finite() && *v >= 0.0;
        if !ok(&self.weight) {
            Some("weight")
        } else if !self.by_disposition.values().all(ok) {
            Some("by_disposition")
        } else if !self.by_band.values().all(ok) {
            Some("by_band")
        } else {
            None
        }
    }

    /// The pick weight for this speaker and band. Gossip availability is the
    /// caller's to apply, because it needs the speaker's memories.
    pub fn weight_for(&self, speaker: Disposition, band: Bond) -> f32 {
        self.weight
            * self.by_disposition.get(&speaker).copied().unwrap_or(1.0)
            * self.by_band.get(&band).copied().unwrap_or(1.0)
    }
}

/// Every interaction the game knows about, in id order.
#[derive(Resource, Default)]
pub struct InteractionDb {
    defs: BTreeMap<String, InteractionDef>,
}

impl InteractionDb {
    /// Loads every `*.ron` def in `dir`, `ThoughtDb::load_dir`'s shape. A def
    /// whose memory ids do not resolve in `memories` is skipped with a
    /// warning, so a mod deleting a memory costs the interactions that wrote
    /// it and nothing else.
    pub fn load_dir(dir: &Path, memories: &MemoryDb) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = InteractionDb::default();
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
        // Sorted, so "the first wins" is the same every run.
        paths.sort();
        let resolves = |id: &str| memories.get(&MemoryId::from(id)).is_some();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            let def = match ron::from_str::<InteractionDef>(&text) {
                Ok(def) => def,
                Err(e) => {
                    warnings.push(format!("skipped invalid interaction file {path:?}: {e}"));
                    continue;
                }
            };
            if let Some(field) = def.non_finite_field() {
                warnings.push(format!(
                    "skipped invalid interaction file {path:?}: {field} is negative or not finite"
                ));
                continue;
            }
            if !def.gossip && !resolves(&def.listener_memory) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: listener_memory {:?} is not a memory",
                    def.listener_memory
                ));
                continue;
            }
            if let Some(m) = def.speaker_memory.as_deref().filter(|m| !resolves(m)) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: speaker_memory {m:?} is not a memory"
                ));
                continue;
            }
            if db.defs.contains_key(&def.id) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: {:?} is already defined",
                    def.id
                ));
                continue;
            }
            db.defs.insert(def.id.clone(), def);
        }
        Ok((db, warnings))
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Every def in id order, which the weighted pick depends on being
    /// stable.
    pub fn iter(&self) -> impl Iterator<Item = &InteractionDef> {
        self.defs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_text(id: &str) -> String {
        format!(
            "(id: \"{id}\", name: \"n\", blurb: \"b\", valence: 1.0, half_life: 100, \
             subject: Program, strike_cap: 3)"
        )
    }

    fn memories() -> MemoryDb {
        let dir = crate::tests::support::scratch_assets_dir("interactions_memories");
        std::fs::create_dir_all(&*dir).unwrap();
        for id in ["chatted_with", "insulted_by"] {
            std::fs::write(dir.join(format!("{id}.ron")), memory_text(id)).unwrap();
        }
        let (db, w) = MemoryDb::load_dir(&dir).unwrap();
        assert!(w.is_empty(), "{w:?}");
        db
    }

    fn def_text(id: &str, extra: &str) -> String {
        format!(
            "(id: \"{id}\", name: \"n\", listener_memory: \"chatted_with\", weight: 1.0, {extra})"
        )
    }

    fn load(files: &[(&str, String)]) -> (InteractionDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("interactions");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        InteractionDb::load_dir(&dir, &memories()).unwrap()
    }

    #[test]
    fn a_well_formed_def_loads_and_iterates_in_id_order() {
        let (db, w) = load(&[
            ("z.ron", def_text("zeta", "")),
            (
                "a.ron",
                def_text(
                    "alpha",
                    "speaker_memory: Some(\"insulted_by\"), by_band: {Enemy: 4.0}",
                ),
            ),
        ]);
        assert!(w.is_empty(), "{w:?}");
        let ids: Vec<_> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "zeta"]);
        let alpha = db.iter().next().unwrap();
        assert_eq!(alpha.weight_for(Disposition::Steady, Bond::Enemy), 4.0);
        assert_eq!(alpha.weight_for(Disposition::Steady, Bond::Close), 1.0);
    }

    #[test]
    fn an_absent_directory_is_an_empty_db_without_warnings() {
        let dir = crate::tests::support::scratch_assets_dir("interactions_absent");
        let (db, w) = InteractionDb::load_dir(&dir.join("nope"), &memories()).unwrap();
        assert!(db.is_empty() && w.is_empty());
    }

    #[test]
    fn a_malformed_file_is_skipped_without_losing_its_neighbours() {
        let (db, w) = load(&[
            ("bad.ron", "(id: \"x\", name:".to_string()),
            ("good.ron", def_text("good", "")),
        ]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(db.iter().count(), 1);
    }

    #[test]
    fn a_bad_weight_or_multiplier_is_skipped() {
        let (db, w) = load(&[
            (
                "a.ron",
                def_text("a", "").replace("weight: 1.0", "weight: -1.0"),
            ),
            (
                "b.ron",
                def_text("b", "").replace("weight: 1.0", "weight: inf"),
            ),
            ("c.ron", def_text("c", "by_band: {Close: -0.5}")),
            ("d.ron", def_text("d", "by_disposition: {Amiable: NaN}")),
        ]);
        assert_eq!(w.len(), 4, "{w:?}");
        assert!(db.is_empty());
    }

    #[test]
    fn a_duplicate_id_warns_and_the_first_file_wins() {
        let (db, w) = load(&[
            (
                "a.ron",
                def_text("same", "").replace("weight: 1.0", "weight: 2.0"),
            ),
            ("b.ron", def_text("same", "")),
        ]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(db.iter().next().unwrap().weight, 2.0);
    }

    #[test]
    fn an_unresolved_memory_id_is_skipped() {
        let (db, w) = load(&[
            ("a.ron", def_text("a", "").replace("chatted_with", "nope")),
            ("b.ron", def_text("b", "speaker_memory: Some(\"nope\")")),
        ]);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(db.is_empty());
    }

    #[test]
    fn a_gossip_defs_listener_memory_is_not_resolved() {
        let (db, w) = load(&[(
            "g.ron",
            def_text("gossip", "gossip: true").replace("chatted_with", "ignored"),
        )]);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(db.iter().count(), 1);
    }
}
