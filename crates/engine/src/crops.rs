//! A crop a farm plot can grow — `assets/crops/*.ron`, one file per crop,
//! loaded into `crops::CropDb` beside `floors::FloorDb`.
//!
//! **A crop names the item one harvest yields and how long it takes; it has
//! no cost or input field by design** (see `assets/crops/README.md`).
//!
//! **Loading follows `FloorDb::load_dir`**: an absent directory is an empty,
//! supported catalogue, a malformed file is skipped with a warning, and
//! `iter` is sorted by id because the plot's crop picker lists it in that
//! order. A zero `grow_ticks` or `stages` would divide by zero in the stage
//! maths and an empty glyph draws nothing, so each is skipped at load.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;
use crate::items_db::ItemDb;

/// A crop's id — a string newtype following `FloorId`'s shape, so a mod's
/// crop needs no Rust variant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CropId(pub String);

impl CropId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CropId {
    fn from(s: &str) -> Self {
        CropId(s.to_string())
    }
}

/// One crop, as authored in `assets/crops/`.
#[derive(Clone, Debug, Deserialize)]
pub struct CropDef {
    pub id: CropId,
    pub name: String,
    pub description: String,
    /// The item one harvest yields, one per tile.
    pub produces: ItemId,
    pub grow_ticks: u32,
    /// How many looks the crop passes through between sown and ripe.
    pub stages: u8,
    pub glyph: String,
    pub color: GlyphColor,
}

impl CropDef {
    /// Why this def cannot be used, if it cannot.
    fn defect(&self) -> Option<&'static str> {
        if self.grow_ticks == 0 {
            Some("grow_ticks must be above zero")
        } else if self.stages == 0 {
            Some("stages must be above zero")
        } else if self.glyph.is_empty() {
            Some("glyph must not be empty")
        } else {
            None
        }
    }
}

/// Every crop the install knows about, loaded from `assets/crops/`.
#[derive(Resource, Default, Clone)]
pub struct CropDb {
    defs: BTreeMap<CropId, CropDef>,
}

impl CropDb {
    /// Loads every `*.ron` in `dir`. `FloorDb::load_dir`'s shape: an absent
    /// directory is the empty catalogue; a file that fails to parse or
    /// validate is skipped with one warning.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = CropDb::default();
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
        // Sorted so two files claiming one id resolve the same way every run.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<CropDef>(&text) {
                Ok(def) => match def.defect() {
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                    Some(why) => {
                        warnings.push(format!("skipped invalid crop file {path:?}: {why}"))
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid crop file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// Drops every crop whose product `items` does not know, one warning
    /// each — a mod that removed an item must not leave a crop that
    /// harvests nothing.
    pub fn drop_unknown_products(&mut self, items: &ItemDb) -> Vec<String> {
        let mut warnings = Vec::new();
        self.defs.retain(|id, def| {
            let known = items.get(def.produces.0.as_str()).is_some();
            if !known {
                warnings.push(format!(
                    "dropped crop {:?}: it produces unknown item {:?}",
                    id.as_str(),
                    def.produces.0
                ));
            }
            known
        });
        warnings
    }

    pub fn get(&self, id: &CropId) -> Option<&CropDef> {
        self.defs.get(id)
    }

    /// Every def in id order — the crop picker's own order.
    pub fn iter(&self) -> impl Iterator<Item = &CropDef> {
        self.defs.values()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"(
    id: "test_crop",
    name: "Test Crop",
    description: "A test crop.",
    produces: "power_cell",
    grow_ticks: 100,
    stages: 4,
    glyph: "+",
    color: Yellow,
)"#;

    fn dir_with(name: &str, files: &[(&str, String)]) -> crate::tests::support::ScratchAssets {
        let dir = crate::tests::support::scratch_assets_dir(name);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, text) in files {
            std::fs::write(dir.join(file), text).unwrap();
        }
        dir
    }

    #[test]
    fn an_absent_directory_is_empty_and_silent() {
        let dir = crate::tests::support::scratch_assets_dir("crops_absent");
        let (db, warnings) = CropDb::load_dir(&dir).expect("absent is not an error");
        assert!(warnings.is_empty());
        assert!(db.is_empty());
    }

    #[test]
    fn a_malformed_file_is_skipped_with_the_directory_intact() {
        let dir = dir_with(
            "crops_bad",
            &[
                ("good.ron", GOOD.to_string()),
                ("broken.ron", "(id: \"x\", oh no".to_string()),
            ],
        );
        let (db, warnings) = CropDb::load_dir(&dir).unwrap();
        assert_eq!(db.iter().count(), 1);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("skipped invalid crop file"));
    }

    #[test]
    fn a_crop_with_zero_grow_time_or_zero_stages_or_an_empty_glyph_is_skipped() {
        for (label, bad) in [
            ("grow", GOOD.replace("grow_ticks: 100", "grow_ticks: 0")),
            ("stages", GOOD.replace("stages: 4", "stages: 0")),
            ("glyph", GOOD.replace("glyph: \"+\"", "glyph: \"\"")),
        ] {
            let dir = dir_with(
                &format!("crops_zero_{label}"),
                &[("good.ron", GOOD.to_string()), ("bad.ron", bad)],
            );
            let (db, warnings) = CropDb::load_dir(&dir).unwrap();
            assert_eq!(db.iter().count(), 1, "{label}");
            assert_eq!(warnings.len(), 1, "{label}: {warnings:?}");
            assert!(warnings[0].contains("skipped invalid crop file"), "{label}");
        }
    }

    #[test]
    fn a_crop_naming_an_unknown_item_is_dropped_with_a_warning() {
        let dir = dir_with(
            "crops_unknown_item",
            &[
                ("good.ron", GOOD.to_string()),
                (
                    "ghost.ron",
                    GOOD.replace("test_crop", "ghost_crop")
                        .replace("\"power_cell\"", "\"no_such_item\""),
                ),
            ],
        );
        let (mut db, load_warnings) = CropDb::load_dir(&dir).unwrap();
        assert!(load_warnings.is_empty(), "{load_warnings:?}");
        assert_eq!(db.iter().count(), 2);
        let assets = crate::tests::support::test_assets_dir();
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        let (items, _) = ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
        let warnings = db.drop_unknown_products(&items);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("ghost_crop"));
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["test_crop"]);
    }
}
