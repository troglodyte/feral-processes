//! Authored set pieces for a battle map: what a prop is made of (`PieceDef`)
//! and the grids that stamp them onto the noise (`PrefabDef`).
//!
//! Content, not code: both live in `assets/battle-props/`, schema in that
//! directory's `README.md`. `PropDb::load_dir` follows `MemoryDb::load_dir`
//! — an absent directory is the empty, supported catalogue (the pre-prop
//! game), and a file that fails to parse, or a prefab that names a piece
//! that does not exist, is skipped with one warning and costs the install
//! nothing else.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::world::Biome;

/// The id of the piece destruction leaves behind when a piece says
/// `leaves: Rubble`. Required in data; the loader warns when it is absent.
pub const RUBBLE_PIECE: &str = "rubble";

/// What a volatile piece does when it is destroyed: damage to every body and
/// prop within `radius` cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blast {
    pub radius: u32,
    pub damage: u32,
}

/// What destroying a piece leaves on its cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leaves {
    Rubble,
    #[default]
    Floor,
}

/// One kind of prop. Every field but `id` is optional in a file, so an
/// existing or modded piece keeps parsing when a field is added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceDef {
    pub id: String,
    #[serde(default)]
    pub blocks_move: bool,
    #[serde(default)]
    pub blocks_sight: bool,
    /// Counts for `reach::cover_between`.
    #[serde(default)]
    pub cover: bool,
    /// `None` is indestructible.
    #[serde(default)]
    pub hp: Option<u32>,
    #[serde(default)]
    pub armour: u32,
    #[serde(default)]
    pub volatile: Option<Blast>,
    #[serde(default)]
    pub leaves: Leaves,
    /// Sprite key, `prop_<piece>` by convention.
    #[serde(default)]
    pub sprite: String,
    /// Visual only: never blocks, never takes damage.
    #[serde(default)]
    pub decoration: bool,
    /// What crossing the cell costs, where it can be crossed. `None` is
    /// "no more than the ground under it".
    #[serde(default)]
    pub move_cost: Option<u32>,
}

/// A grid of pieces to stamp onto the noise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabDef {
    pub id: String,
    /// Empty is every biome.
    #[serde(default)]
    pub biomes: Vec<Biome>,
    #[serde(default = "default_weight")]
    pub weight: u32,
    pub rows: Vec<String>,
    /// Row character to piece id. `.` is "leave the noise alone" and is
    /// never a legend key.
    #[serde(default)]
    pub legend: BTreeMap<char, String>,
}

fn default_weight() -> u32 {
    1
}

impl PrefabDef {
    /// Why this prefab cannot be stamped, if it cannot. Checked against the
    /// loaded pieces, so it runs after every piece file has been read.
    fn fault(&self, pieces: &BTreeMap<String, PieceDef>) -> Option<String> {
        if self.weight == 0 {
            return Some("its weight is zero".into());
        }
        let Some(first) = self.rows.first() else {
            return Some("it has no rows".into());
        };
        let width = first.chars().count();
        if width == 0 || self.rows.iter().any(|r| r.chars().count() != width) {
            return Some("its rows are not one non-empty width".into());
        }
        if self.legend.contains_key(&'.') {
            return Some("'.' is reserved for \"leave the noise alone\"".into());
        }
        for (key, piece) in &self.legend {
            if !pieces.contains_key(piece) {
                return Some(format!("legend '{key}' names unknown piece \"{piece}\""));
            }
        }
        self.rows
            .iter()
            .flat_map(|r| r.chars())
            .find(|c| *c != '.' && !self.legend.contains_key(c))
            .map(|c| format!("row character '{c}' is not in its legend"))
    }

    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, |r| r.chars().count())
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct PropDb {
    pieces: BTreeMap<String, PieceDef>,
    prefabs: Vec<PrefabDef>,
}

/// Every `*.ron` in `dir`, sorted so two files claiming one id resolve the
/// same way every run. An absent directory is no files.
fn ron_paths(dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .collect();
    paths.sort();
    Ok(paths)
}

impl PropDb {
    /// Loads `pieces/` and `prefabs/` under `dir`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = PropDb::default();
        let mut warnings = Vec::new();
        for path in ron_paths(&dir.join("pieces"))? {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<PieceDef>(&text) {
                Ok(def) => {
                    db.pieces.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid prop piece file {path:?}: {e}")),
            }
        }
        if !db.pieces.is_empty() && !db.pieces.contains_key(RUBBLE_PIECE) {
            warnings.push(format!(
                "no \"{RUBBLE_PIECE}\" prop piece: destroyed props that leave rubble will leave floor"
            ));
        }
        for path in ron_paths(&dir.join("prefabs"))? {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<PrefabDef>(&text) {
                Ok(def) => match def.fault(&db.pieces) {
                    None => db.prefabs.push(def),
                    Some(why) => {
                        warnings.push(format!("skipped invalid prop prefab file {path:?}: {why}"))
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid prop prefab file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn piece(&self, id: &str) -> Option<&PieceDef> {
        self.pieces.get(id)
    }

    pub fn pieces(&self) -> impl Iterator<Item = &PieceDef> {
        self.pieces.values()
    }

    /// The prefabs that may be stamped on `biome`, in file order.
    pub fn prefabs_for(&self, biome: Biome) -> impl Iterator<Item = &PrefabDef> {
        self.prefabs
            .iter()
            .filter(move |p| p.biomes.is_empty() || p.biomes.contains(&biome))
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty() && self.prefabs.is_empty()
    }

    /// A db built in code, for tests that need a known piece set.
    #[cfg(test)]
    pub(crate) fn from_parts(pieces: Vec<PieceDef>, prefabs: Vec<PrefabDef>) -> PropDb {
        PropDb {
            pieces: pieces.into_iter().map(|p| (p.id.clone(), p)).collect(),
            prefabs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> (PropDb, Vec<String>) {
        PropDb::load_dir(&crate::tests::support::test_assets_dir().join("battle-props")).unwrap()
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("feral-props-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("pieces")).unwrap();
        std::fs::create_dir_all(dir.join("prefabs")).unwrap();
        dir
    }

    #[test]
    fn the_shipped_dir_loads_with_no_warnings() {
        let (db, warnings) = shipped();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(db.prefabs.len() >= 6);
        assert!(db.piece(RUBBLE_PIECE).is_some());
    }

    #[test]
    fn rubble_is_walkable_rough_and_not_cover() {
        let (db, _) = shipped();
        let rubble = db.piece(RUBBLE_PIECE).unwrap();
        assert!(!rubble.blocks_move && !rubble.cover && rubble.hp.is_none());
        assert_eq!(rubble.move_cost, Some(crate::tuning::TACTICAL_ROUGH_COST));
    }

    #[test]
    fn every_shipped_prefab_legend_resolves_and_every_sprite_is_named() {
        let (db, _) = shipped();
        for prefab in &db.prefabs {
            for piece in prefab.legend.values() {
                assert!(db.piece(piece).is_some(), "{} names {piece}", prefab.id);
            }
        }
        for piece in db.pieces() {
            assert_eq!(piece.sprite, format!("prop_{}", piece.id));
        }
    }

    #[test]
    fn a_biome_filter_is_honoured_and_empty_means_every_biome() {
        let prefab = |id: &str, biomes: Vec<Biome>| PrefabDef {
            id: id.into(),
            biomes,
            weight: 1,
            rows: vec!["x".into()],
            legend: BTreeMap::new(),
        };
        let db = PropDb::from_parts(
            vec![],
            vec![
                prefab("any", vec![]),
                prefab("only_deadlock", vec![Biome::Deadlock]),
            ],
        );
        let ids = |b| db.prefabs_for(b).map(|p| p.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(Biome::OpenGrid), ["any"]);
        assert_eq!(ids(Biome::Deadlock), ["any", "only_deadlock"]);
    }

    #[test]
    fn bad_files_are_skipped_and_their_siblings_load() {
        let dir = scratch("bad");
        let w = |sub: &str, name: &str, text: &str| {
            std::fs::write(dir.join(sub).join(name), text).unwrap();
        };
        w("pieces", "a.ron", r#"(id: "crate", blocks_move: true)"#);
        w("pieces", "rubble.ron", r#"(id: "rubble")"#);
        w("pieces", "broken.ron", "(id: ");
        w(
            "prefabs",
            "good.ron",
            r#"(id: "good", rows: ["C."], legend: {'C': "crate"})"#,
        );
        w(
            "prefabs",
            "missing_piece.ron",
            r#"(id: "m", rows: ["Z"], legend: {'Z': "nope"})"#,
        );
        w(
            "prefabs",
            "unknown_biome.ron",
            r#"(id: "b", biomes: [Nowhere], rows: ["C"], legend: {'C': "crate"})"#,
        );
        w(
            "prefabs",
            "unlisted_char.ron",
            r#"(id: "u", rows: ["Q"], legend: {})"#,
        );
        w("prefabs", "malformed.ron", "nonsense");
        let (db, warnings) = PropDb::load_dir(&dir).unwrap();
        assert_eq!(db.prefabs.len(), 1);
        assert_eq!(db.prefabs[0].id, "good");
        assert!(db.piece("crate").is_some());
        assert_eq!(warnings.len(), 5, "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_piece_omitting_optional_fields_gets_defaults() {
        let def: PieceDef = ron::from_str(r#"(id: "bare")"#).unwrap();
        assert!(!def.blocks_move && !def.blocks_sight && !def.cover && !def.decoration);
        assert_eq!(def.hp, None);
        assert_eq!(def.leaves, Leaves::Floor);
        assert_eq!(def.volatile, None);
        assert_eq!(def.move_cost, None);
    }

    #[test]
    fn an_absent_dir_is_an_empty_catalogue() {
        let (db, warnings) = PropDb::load_dir(Path::new("/nonexistent/battle-props")).unwrap();
        assert!(db.is_empty() && warnings.is_empty());
    }

    #[test]
    fn a_missing_rubble_piece_warns() {
        let dir = scratch("norubble");
        std::fs::write(dir.join("pieces/a.ron"), r#"(id: "crate")"#).unwrap();
        let (_, warnings) = PropDb::load_dir(&dir).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
