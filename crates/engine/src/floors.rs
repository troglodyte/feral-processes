//! A decorative finish painted onto laid base floor — `assets/floors/*.ron`,
//! one file per finish, loaded into `floors::FloorDb` beside `rock::RockDb`.
//!
//! **A finish is a shade plus an optional carved sprite, never an RGB
//! triple.** `FloorShade` is a fixed, engine-side set so a mod cannot paint a
//! carpet that reads as rock — the map's one colour rule elsewhere is hue
//! answers "can I walk here", and gui's separation census (`terrain.rs`) is
//! what holds every shade far enough from every rock reading to keep that
//! rule legible even though a finish only ever sits on already-laid floor.
//!
//! **Loading follows `MemoryDb::load_dir`**: an absent directory is an empty,
//! supported catalogue (no brush is offered and the game is exactly today's),
//! a malformed file — including one naming a shade `FloorShade` does not
//! have — is skipped with a warning, and `iter` is sorted by id because the
//! brush cycle walks it in that order.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// A finish's id — a string newtype following `ItemId`'s shape, so a mod's
/// finish needs no Rust variant. `#[serde(transparent)]` so a `.ron` file
/// spells it as a plain quoted string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FloorId(pub String);

impl FloorId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FloorId {
    fn from(s: &str) -> Self {
        FloorId(s.to_string())
    }
}

/// A finish's tint, resolved to a colour in gui (`terrain::shade_color`) and
/// never authored as a free RGB triple — see the module doc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloorShade {
    Cobalt,
    Teal,
    Moss,
    Olive,
    Ochre,
    Umber,
    Wine,
    Plum,
    Violet,
    Slate,
}

impl FloorShade {
    /// Every shade, for the censuses that must fail the day an eleventh is
    /// added rather than passing because they name ten files by hand.
    pub const ALL: [FloorShade; 10] = [
        FloorShade::Cobalt,
        FloorShade::Teal,
        FloorShade::Moss,
        FloorShade::Olive,
        FloorShade::Ochre,
        FloorShade::Umber,
        FloorShade::Wine,
        FloorShade::Plum,
        FloorShade::Violet,
        FloorShade::Slate,
    ];
}

/// One finish, as authored in `assets/floors/`.
#[derive(Clone, Debug, Deserialize)]
pub struct FloorDef {
    pub id: FloorId,
    /// Brush label and examine line.
    pub name: String,
    /// One line of flavour.
    pub description: String,
    pub shade: FloorShade,
    /// Sprite key override; falls back to `id`, `@`-prefixed ignored — see
    /// `sprite_name`.
    #[serde(default)]
    pub sprite: Option<String>,
    /// The memory a program writes by lingering on this finish
    /// (`Game::note_comforts`). `None` is purely cosmetic.
    #[serde(default)]
    pub comfort: Option<String>,
}

impl FloorDef {
    /// The sprite key this finish draws under, `SpeciesDef::sprite_name`'s
    /// rule exactly: an override starting with `@` is treated as absent, so
    /// a finish can never read the player's own drawn-icon slot.
    pub fn sprite_name(&self) -> &str {
        self.sprite
            .as_deref()
            .filter(|s| !s.starts_with('@'))
            .unwrap_or(self.id.as_str())
    }
}

/// Every finish the install knows about, loaded from `assets/floors/`.
///
/// `BTreeMap` rather than `MemoryDb`'s `HashMap`: the brush cycle walks
/// `iter` in id order, and keying the store itself sorted is simpler than
/// sorting a `HashMap`'s values on every call.
#[derive(Resource, Default, Clone)]
pub struct FloorDb {
    defs: BTreeMap<FloorId, FloorDef>,
}

impl FloorDb {
    /// Loads every `*.ron` in `dir`. `MemoryDb::load_dir`'s shape: an absent
    /// directory is the empty, supported catalogue; a file that fails to
    /// parse — a syntax fault or a shade name `FloorShade` does not have —
    /// is skipped with one warning and costs the install nothing else.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = FloorDb::default();
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
        // Sorted for the reason `TalentDb::load_dir` sorts: two files
        // claiming one id must resolve the same way every run.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<FloorDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid floor file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &FloorId) -> Option<&FloorDef> {
        self.defs.get(id)
    }

    /// Every def in id order — the brush cycle's own order.
    pub fn iter(&self) -> impl Iterator<Item = &FloorDef> {
        self.defs.values()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_directory_is_empty_and_silent() {
        let dir = crate::tests::support::scratch_assets_dir("floors_absent");
        let (db, warnings) = FloorDb::load_dir(&dir).expect("absent is not an error");
        assert!(warnings.is_empty());
        assert!(db.is_empty());
    }

    const GOOD: &str = r#"(
    id: "test_finish",
    name: "Test Finish",
    description: "A test finish.",
    shade: Cobalt,
)"#;

    #[test]
    fn a_malformed_file_is_skipped_with_the_directory_intact() {
        let dir = crate::tests::support::scratch_assets_dir("floors_bad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("good.ron"), GOOD).unwrap();
        std::fs::write(dir.join("broken.ron"), "(id: \"x\", oh no").unwrap();
        let (db, warnings) = FloorDb::load_dir(&dir).unwrap();
        assert_eq!(db.iter().count(), 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn an_unknown_shade_name_is_skipped_with_the_directory_intact() {
        let dir = crate::tests::support::scratch_assets_dir("floors_shade");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("good.ron"), GOOD).unwrap();
        std::fs::write(
            dir.join("bogus.ron"),
            GOOD.replace("\"test_finish\"", "\"bogus\"")
                .replace("Cobalt", "Chartreuse"),
        )
        .unwrap();
        let (db, warnings) = FloorDb::load_dir(&dir).unwrap();
        assert_eq!(db.iter().count(), 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn iter_is_id_sorted_even_when_files_were_written_in_reverse_order() {
        let dir = crate::tests::support::scratch_assets_dir("floors_sort");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("z.ron"),
            GOOD.replace("\"test_finish\"", "\"zebra\""),
        )
        .unwrap();
        std::fs::write(
            dir.join("a.ron"),
            GOOD.replace("\"test_finish\"", "\"aardvark\""),
        )
        .unwrap();
        let (db, warnings) = FloorDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["aardvark", "zebra"]);
    }

    #[test]
    fn sprite_name_falls_back_to_id_uses_override_and_ignores_at_prefix() {
        let plain: FloorDef = ron::from_str(GOOD).unwrap();
        assert_eq!(plain.sprite_name(), "test_finish");

        let overridden: FloorDef =
            ron::from_str(&GOOD.replace(")", "    sprite: Some(\"override\"),\n)")).unwrap();
        assert_eq!(overridden.sprite_name(), "override");

        let at_prefixed: FloorDef =
            ron::from_str(&GOOD.replace(")", "    sprite: Some(\"@drawn\"),\n)")).unwrap();
        assert_eq!(at_prefixed.sprite_name(), "test_finish");
    }
}
