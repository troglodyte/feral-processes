//! What a program makes of where it is standing, loaded from
//! `assets/thoughts/`.
//!
//! **Half data, like perks.** Wording and weight are a file each; what fires
//! a thought is the closed `Trigger` enum, so a modder can reword, reweight
//! or delete a thought but not invent a trigger.
//!
//! **An empty database is valid and inert**, `MemoryDb`'s rule and for its
//! reason: a trigger with no def never fires, so deleting `assets/thoughts/`
//! restores the game as it was before situations existed. An absent
//! directory is therefore silent.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;

/// What can make a program think something about its surroundings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Trigger {
    BesideRival,
    BesideFriend,
    Unpowered,
    MachineRunning,
    NoAmenity,
}

impl Trigger {
    /// Every variant, for the shipped-asset census. The `match` in
    /// `Trigger::index` is exhaustive, so a new variant fails to compile
    /// until it is placed here too.
    pub const ALL: [Trigger; 5] = [
        Trigger::BesideRival,
        Trigger::BesideFriend,
        Trigger::Unpowered,
        Trigger::MachineRunning,
        Trigger::NoAmenity,
    ];

    #[cfg(test)]
    fn index(self) -> usize {
        match self {
            Trigger::BesideRival => 0,
            Trigger::BesideFriend => 1,
            Trigger::Unpowered => 2,
            Trigger::MachineRunning => 3,
            Trigger::NoAmenity => 4,
        }
    }
}

/// One thought: how a trigger is worded and how hard it weighs.
#[derive(Clone, Debug, Deserialize)]
pub struct ThoughtDef {
    pub trigger: Trigger,
    /// What a screen row leads with.
    pub name: String,
    /// One line of flavour under it, in the player's vocabulary.
    pub blurb: String,
    /// Signed; positive lifts morale.
    pub intensity: f32,
}

/// Every thought the game knows about, one per trigger.
#[derive(Resource, Default)]
pub struct ThoughtDb {
    defs: BTreeMap<Trigger, ThoughtDef>,
}

impl ThoughtDb {
    /// Loads every `*.ron` def in `dir`, `MemoryDb::load_dir`'s shape. Two
    /// files naming one trigger: the first by file name wins and the second
    /// is warned about, so a trigger never counts twice.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ThoughtDb::default();
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
        // Sorted, so "the first by file name wins" is the same every run.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ThoughtDef>(&text) {
                Ok(def) => {
                    if db.defs.contains_key(&def.trigger) {
                        warnings.push(format!(
                            "skipped thought file {path:?}: {:?} already has a thought",
                            def.trigger
                        ));
                        continue;
                    }
                    db.defs.insert(def.trigger, def);
                }
                Err(e) => warnings.push(format!("skipped invalid thought file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, trigger: Trigger) -> Option<&ThoughtDef> {
        self.defs.get(&trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_text(trigger: &str, name: &str) -> String {
        format!(
            "(\n    trigger: {trigger},\n    name: \"{name}\",\n    blurb: \"b\",\n    \
             intensity: -3.0,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (ThoughtDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("thoughts");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        ThoughtDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn a_well_formed_def_loads() {
        let (db, warnings) = load(&[("a.ron", def_text("BesideRival", "Beside a rival"))]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(Trigger::BesideRival).expect("loaded");
        assert_eq!(def.name, "Beside a rival");
        assert_eq!(def.intensity, -3.0);
        assert!(db.get(Trigger::Unpowered).is_none());
    }

    #[test]
    fn a_malformed_file_is_skipped_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("a_bad.ron", "(trigger: Nope".to_string()),
            ("b_good.ron", def_text("Unpowered", "Dark")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("a_bad.ron"), "{warnings:?}");
        assert!(db.get(Trigger::Unpowered).is_some());
    }

    #[test]
    fn a_second_def_for_one_trigger_is_warned_and_the_first_kept() {
        let (db, warnings) = load(&[
            ("a.ron", def_text("NoAmenity", "First")),
            ("b.ron", def_text("NoAmenity", "Second")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("b.ron"), "{warnings:?}");
        assert_eq!(db.get(Trigger::NoAmenity).unwrap().name, "First");
    }

    #[test]
    fn an_absent_directory_is_empty_and_silent() {
        let dir = crate::tests::support::scratch_assets_dir("thoughts_absent");
        let (db, warnings) = ThoughtDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty());
        assert!(Trigger::ALL.iter().all(|t| db.get(*t).is_none()));
    }

    #[test]
    fn all_lists_every_variant_once_in_index_order() {
        for (i, t) in Trigger::ALL.iter().enumerate() {
            assert_eq!(t.index(), i);
        }
    }
}
