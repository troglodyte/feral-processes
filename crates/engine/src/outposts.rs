//! A one-tile fixture on the zone surface where posted programs extract
//! materials — see `docs/superpowers/specs/2026-09-23-outposts-design.md`.
//!
//! `OutpostDef` and `OutpostDb::load_dir` follow `NeedDb`'s absent-is-silent
//! pattern: no `assets/outposts/` directory means no outposts exist and the
//! kit refuses with a message, never a panic.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

use crate::items::ItemId;
use crate::world::Biome;

/// One growth tier's yield table, keyed by the biome under the outpost.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OutpostTierDef {
    /// The items this tier can produce in a given biome. A biome missing a
    /// row here falls back to this tier's first listed biome (`BTreeMap`
    /// order) — see `yields`.
    #[serde(default)]
    pub yields: BTreeMap<Biome, Vec<ItemId>>,
}

/// The one outpost def a v1 install ships, loaded by `OutpostDb`.
///
/// Every field but `tiers` is `#[serde(default)]` — a future second file
/// (a mod's own outpost variant, once `features:` grows past reserved) should
/// not need every field re-authored just to parse. `tiers` stays required:
/// a def with none would yield nothing at any tier, which is a content
/// mistake worth a load warning rather than a silently empty outpost.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OutpostDef {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub glyph: char,
    /// The item that founds an outpost when used on the zone surface — see
    /// `Game::found_outpost` and `Game::use_item`.
    #[serde(default)]
    pub kit: ItemId,
    /// Raw, then processed, then complex — index 0 is tier 1 (the tier a
    /// freshly founded outpost starts at). See `yields`.
    pub tiers: Vec<OutpostTierDef>,
    /// Reserved for the forward-camp/frontier-post extension points (design
    /// spec §10): `Rest`, `Stash`, `Reach(radius)`. Always empty in v1 —
    /// `()` is a placeholder element type, swapped out the day a feature
    /// actually lands here rather than designed for now.
    #[serde(default)]
    pub features: Vec<()>,
}

/// Every outpost def the game knows about, loaded from `assets/outposts/`.
///
/// An empty database is a supported state, `NeedDb`'s rule: deleting the
/// directory restores the pre-outpost game rather than breaking an install.
#[derive(Resource, Default)]
pub struct OutpostDb {
    def: Option<OutpostDef>,
}

impl OutpostDb {
    /// The v1 def, if this install ships one. `None` for a deleted
    /// `assets/outposts/` directory or one with no valid file in it.
    pub fn def(&self) -> Option<&OutpostDef> {
        self.def.as_ref()
    }

    /// Loads every `*.ron` file in `dir`, `NeedDb::load_dir`'s pattern: an
    /// absent directory is silent and a malformed file is skipped with a
    /// warning rather than aborting the whole load.
    ///
    /// **The first by filename wins.** v1 ships exactly one file; a second
    /// is a mod's problem to resolve, not a merge this loader attempts.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Self::default(), warnings));
            }
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, `NeedDb::load_dir`'s reason: which file wins must not
        // depend on the OS's own directory-listing order.
        paths.sort();
        let mut def = None;
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<OutpostDef>(&text) {
                Ok(parsed) => {
                    if def.is_none() {
                        def = Some(parsed);
                    }
                }
                Err(e) => warnings.push(format!("skipped invalid outpost file {path:?}: {e}")),
            }
        }
        Ok((Self { def }, warnings))
    }
}

/// The items a `tier`-th outpost (0-indexed: tier 0 is raw) can produce in
/// `biome` — the union of every tier up to and including it, design spec
/// §4. A tier missing a row for `biome` falls back to that tier's first
/// listed biome (`BTreeMap` order), so a new `Biome` variant with no
/// authored row still yields something rather than nothing. Sorted and
/// deduped, since the same item can appear in more than one tier's row.
pub fn yields(def: &OutpostDef, tier: usize, biome: Biome) -> Vec<ItemId> {
    let mut out: Vec<ItemId> = Vec::new();
    for t in def.tiers.iter().take(tier + 1) {
        let row = t.yields.get(&biome).or_else(|| t.yields.values().next());
        if let Some(row) = row {
            out.extend(row.iter().cloned());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(rows: &[(Biome, &[&str])]) -> OutpostTierDef {
        OutpostTierDef {
            yields: rows
                .iter()
                .map(|(b, items)| (*b, items.iter().map(|s| ItemId(s.to_string())).collect()))
                .collect(),
        }
    }

    fn ids(items: &[&str]) -> Vec<ItemId> {
        items.iter().map(|s| ItemId(s.to_string())).collect()
    }

    #[test]
    fn missing_directory_is_an_empty_db() {
        let dir =
            std::env::temp_dir().join(format!("feral_outposts_missing_{}", std::process::id()));
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_leaves_the_db_empty() {
        let dir =
            std::env::temp_dir().join(format!("feral_outposts_malformed_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.ron"), "not valid ron at all }{").unwrap();
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert_eq!(warnings.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_directory_is_an_empty_db_with_no_warnings() {
        let dir = std::env::temp_dir().join(format!("feral_outposts_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert!(warnings.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn yields_unions_every_tier_up_to_and_including_the_one_asked_for() {
        let def = OutpostDef {
            tiers: vec![
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
                tier(&[(Biome::Deadlock, &["static_mesh"])]),
                tier(&[(Biome::Deadlock, &["core_fragment"])]),
            ],
            ..Default::default()
        };
        assert_eq!(yields(&def, 0, Biome::Deadlock), ids(&["raw_trace"]));
        let mut two = ids(&["raw_trace", "static_mesh"]);
        two.sort();
        assert_eq!(yields(&def, 1, Biome::Deadlock), two);
        let mut three = ids(&["raw_trace", "static_mesh", "core_fragment"]);
        three.sort();
        assert_eq!(yields(&def, 2, Biome::Deadlock), three);
    }

    #[test]
    fn an_unlisted_biome_falls_back_to_the_tiers_first_listed_biome() {
        // `Biome` derives `Ord` off declaration order, so `DataVoid` sorts
        // before `Deadlock` and is the "first listed" row here.
        let def = OutpostDef {
            tiers: vec![tier(&[
                (Biome::DataVoid, &["raw_trace"]),
                (Biome::Deadlock, &["static_mesh"]),
            ])],
            ..Default::default()
        };
        // `NullSector` has no row of its own, so it falls back to
        // `DataVoid`'s — the lowest-sorted key present.
        assert_eq!(yields(&def, 0, Biome::NullSector), ids(&["raw_trace"]));
    }

    #[test]
    fn yields_is_sorted_and_deduped() {
        let def = OutpostDef {
            tiers: vec![
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
            ],
            ..Default::default()
        };
        assert_eq!(yields(&def, 1, Biome::Deadlock), ids(&["raw_trace"]));
    }

    #[test]
    fn an_empty_db_means_def_is_none() {
        let db = OutpostDb::default();
        assert!(db.def().is_none());
    }
}
