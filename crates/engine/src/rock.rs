//! What a cell of base-space rock is made of.
//!
//! Content, like species and items — `assets/rock/*.ron`, one file per kind,
//! so an ore later is a file drop rather than a code change.
//!
//! **A wall's kind is a property of the place, and is never stored.**
//! `base_grid` opens by explaining that there is no `Solid` variant because
//! "not in the map" already says it, and writing a kind out for every
//! untouched coordinate would make the sparse map not sparse. So the kind is
//! derived instead: `RockDb::kind_at` answers for any coordinate whether or
//! not anything has ever touched it, which is what makes drawing an unmined
//! wall face possible at all.
//!
//! Selection is an FNV-1a fold reduced through `derive::index`, for
//! `descriptions.rs`'s three reasons: `resources::GameRng` does not survive
//! a save/load and a draw from it shifts every later roll in the run;
//! `StdRng`'s sequence is not guaranteed stable across a `rand` upgrade; and
//! `%` reads only the seed's lowest bit for a small pool, which
//! anti-correlates neighbouring pools while looking perfectly reasonable.
//!
//! **The seed is base space's own**, `BaseGrid::seed`, and not
//! `WorldMap::seed()`. The world seed changes on every breach
//! (`enter_next_zone` mints the next zone from `wrapping_add(0x9E37_79B9)`)
//! while `BaseGrid` *travels* with the run — so salted off the world seed,
//! every seam in the base would reshuffle each time the player portalled and
//! a half-cut wall would come back a different kind under its saved
//! `Durability`.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

use crate::derive::index;

/// How bright a kind may be drawn against `Biome::Entropy`'s own colour.
///
/// A *brightness* and not a hue, because the map's one colour rule is that
/// hue answers "can I walk here" — `render/base.rs` records the consequence
/// for anything told apart within a band: "brightness rather than hue, since
/// hue is already spoken for". An authored hue would also fight
/// `biome_tint`'s per-sector rotation, so a seam would change appearance on
/// a breach.
///
/// Bounded below at 1.0 so no kind can be *darker* than plain rock (an
/// exposed face must never be harder to see than the wall around it) and
/// above at a factor that keeps the darkest walkable ground brighter still.
pub const SHADE_BAND: (f32, f32) = (1.0, 4.0);

/// One kind of base-space rock.
#[derive(Clone, Debug, Deserialize)]
pub struct RockDef {
    /// A string id following the `ItemId` precedent rather than an enum —
    /// that is the whole expansion seam, since an ore is a file with a new
    /// id and no Rust behind it.
    pub id: String,
    pub name: String,
    /// Relative share of base space this kind covers. Zero is refused at
    /// load: a kind nothing can ever roll is an authoring mistake, not a
    /// way to disable a file.
    pub weight: u32,
    /// How much damage one cell of this kind absorbs before it opens.
    pub durability: u32,
    /// The fewest swings this kind can ever be opened in, however hard the
    /// swinger hits — `Game::strike_rock` caps one swing at
    /// `durability.div_ceil(min_swings)`.
    ///
    /// This is what makes levelling speed digging up without trivialising
    /// it, and it is deliberately level-independent: a durability that grew
    /// with the player would make digging cost the same forever, which is
    /// the one thing it must not do.
    pub min_swings: u32,
    /// Brightness against `Biome::Entropy`, inside `SHADE_BAND`.
    pub shade: f32,
}

/// What a swing at one cell needs to know, in a `Copy` shape.
///
/// A struct rather than a tuple of two numbers, for `SaveData`'s reason
/// stated the other way round: these are read at three sites that each want
/// a different pair of them, and a positional tuple is the shape that lets
/// two of them silently swap. `Copy` because `view_tiles` asks per tile per
/// frame, where cloning a `RockDef`'s strings would be a per-frame
/// allocation for a number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    /// The cell's `Durability::max_hp`.
    pub durability: u32,
    /// The most damage one swing at it may land.
    pub swing_cap: u32,
    /// How bright its exposed face is drawn. See `SHADE_BAND`.
    pub shade: f32,
}

impl RockDef {
    /// The kind base space is made of when `assets/rock/` is empty or holds
    /// nothing loadable.
    ///
    /// An empty directory is a supported install: it restores *uniform*
    /// rock, the same supported way deleting `assets/sectors/` restores
    /// undifferentiated zones. It does **not** restore the one-shot — the
    /// fallback carries the same floor of 2 that ordinary rock does, because
    /// the swing floor is a bug fix and not content, and someone deleting
    /// this directory is asking for one kind of rock rather than for their
    /// base back to being demolishable by a misstep.
    fn fallback() -> Self {
        RockDef {
            id: "entropy".to_string(),
            name: "Entropy".to_string(),
            weight: 1,
            durability: crate::tuning::BASE_ROCK_DURABILITY,
            min_swings: crate::tuning::BASE_ROCK_MIN_SWINGS,
            shade: 1.0,
        }
    }

    /// The most damage one swing at this kind may land.
    pub fn swing_cap(&self) -> u32 {
        self.durability.div_ceil(self.min_swings.max(1))
    }

    /// This kind's numbers, without its strings.
    pub fn wall(&self) -> Wall {
        Wall {
            durability: self.durability,
            swing_cap: self.swing_cap(),
            shade: self.shade,
        }
    }
}

/// Every kind of rock the install knows about.
///
/// `BTreeMap` for the reason `Stock` keys by `ItemId` in one: selection
/// walks the map in key order, so a `HashMap` would make the same seed and
/// the same coordinate produce a different kind between runs.
#[derive(Resource, Debug)]
pub struct RockDb {
    defs: BTreeMap<String, RockDef>,
    fallback: RockDef,
}

impl Default for RockDb {
    fn default() -> Self {
        RockDb {
            defs: BTreeMap::new(),
            fallback: RockDef::fallback(),
        }
    }
}

impl RockDb {
    /// Loads every `.ron` in `dir`, skipping a malformed or out-of-range
    /// file with a warning rather than panicking — `PerkDb::load_dir`'s
    /// pattern, and the rule every asset directory in the game follows.
    ///
    /// An absent directory is not an error, for `MemoryDb`'s reason: it is
    /// the empty catalogue, and the empty catalogue is a supported install.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = RockDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<RockDef>(&text) {
                Ok(def) => match def_fault(&def) {
                    Some(fault) => {
                        warnings.push(format!("skipped invalid rock file {path:?}: {fault}"))
                    }
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid rock file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// What the cell at `(x, y)` in base space is made of.
    ///
    /// Total over the roster rather than the first match, so a mod adding a
    /// kind widens the pool instead of displacing one. Falls back to the
    /// built-in kind when nothing loaded — every reader tolerates a
    /// one-kind world, which is the half of "an empty directory is
    /// supported" that has to hold at the *reader* end.
    pub fn kind_at(&self, seed: u32, x: i32, y: i32) -> &RockDef {
        let total: u32 = self.defs.values().map(|d| d.weight).sum();
        if total == 0 {
            return &self.fallback;
        }
        let mut n = index(block_seed(seed, x, y), total as usize) as u32;
        for def in self.defs.values() {
            if n < def.weight {
                return def;
            }
            n -= def.weight;
        }
        // Unreachable while the weights sum to `total`, and cheaper to state
        // than to prove at every call site.
        &self.fallback
    }

    /// What a swing at `(x, y)` needs to know. `kind_at` without the
    /// strings, which is what every caller in play actually wants.
    pub fn wall_at(&self, seed: u32, x: i32, y: i32) -> Wall {
        self.kind_at(seed, x, y).wall()
    }

    /// Every kind in key order — what the asset censuses walk.
    pub fn defs(&self) -> impl Iterator<Item = &RockDef> {
        self.defs.values()
    }
}

/// Why this def cannot be loaded, or `None`.
fn def_fault(def: &RockDef) -> Option<String> {
    if def.id.is_empty() || def.name.is_empty() {
        return Some("has no id or no name".into());
    }
    if def.weight == 0 {
        return Some("weight must be at least 1, or the kind could never be rolled".into());
    }
    if def.durability == 0 {
        return Some("durability must be at least 1, or the cell would open to no swing".into());
    }
    if def.min_swings == 0 {
        return Some("min_swings must be at least 1".into());
    }
    let (lo, hi) = SHADE_BAND;
    if !(lo..=hi).contains(&def.shade) {
        return Some(format!("shade {} is outside {lo}..={hi}", def.shade));
    }
    None
}

/// How many tiles across one patch of a single kind is.
///
/// Folded per *block* rather than per tile so a run of adjacent cells
/// resolves to the same kind and a dense kind reads as a seam with an
/// inside. Per tile it would be salt and pepper: every wall a different
/// colour, no seam to follow, and an exposed face conveying nothing about
/// what is behind it.
const VEIN_BLOCK: i32 = 4;

/// The value `index` reduces: base space's seed and the block a coordinate
/// falls in.
///
/// **Every input goes in a byte at a time.** One XOR-then-multiply round
/// carries a difference only about the prime's own width upward, so folding
/// a block coordinate as a single word would leave neighbouring blocks
/// differing nowhere near bit 63 — which is the bit `derive::index` reads.
/// That is the measured failure `descriptions::Slot::tags` documents, reached
/// here by the same route.
fn block_seed(seed: u32, x: i32, y: i32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for word in [
        seed as u64,
        x.div_euclid(VEIN_BLOCK) as i64 as u64,
        y.div_euclid(VEIN_BLOCK) as i64 as u64,
    ] {
        for byte in word.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> RockDb {
        let (db, warnings) =
            RockDb::load_dir(&crate::tests::support::test_assets_dir().join("rock")).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped rock assets warned: {warnings:?}"
        );
        db
    }

    /// The shipped roster must be loadable and hold more than one kind, or
    /// every wall in the game is the same wall and the feature is inert.
    #[test]
    fn the_shipped_kinds_load() {
        let db = db();
        assert!(
            db.defs().count() >= 2,
            "a single shipped kind makes base space uniform"
        );
    }

    /// The census the design promises: no shipped kind may be softer than
    /// the fallback, and none may be openable in one swing.
    #[test]
    fn every_shipped_kind_clears_the_floor() {
        let fallback = RockDef::fallback();
        for def in db().defs() {
            assert!(
                def.min_swings >= 2,
                "{}: min_swings {} lets a developed player open it in one",
                def.id,
                def.min_swings
            );
            assert!(
                def.durability >= fallback.durability,
                "{}: durability {} is under the fallback's {}",
                def.id,
                def.durability,
                fallback.durability
            );
            let (lo, hi) = SHADE_BAND;
            assert!(
                (lo..=hi).contains(&def.shade),
                "{}: shade {} is outside the band",
                def.id,
                def.shade
            );
        }
    }

    /// An empty directory is a supported install — and it restores *uniform*
    /// rock, not the one-shot. The second assertion is the half the obvious
    /// reading of "restores the pre-feature game" gets backwards.
    #[test]
    fn an_empty_directory_is_uniform_rock_and_still_takes_two_swings() {
        let dir = crate::tests::support::scratch_assets_dir("rock_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let (db, warnings) = RockDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty());
        let kinds: std::collections::BTreeSet<&str> = (0..40)
            .map(|i| db.kind_at(7, i * VEIN_BLOCK, 0).id.as_str())
            .collect();
        assert_eq!(kinds.len(), 1, "an empty directory must be uniform");
        let only = db.kind_at(7, 0, 0);
        assert!(
            only.min_swings >= 2 && only.swing_cap() < only.durability,
            "the fallback kind must keep the swing floor: the floor is the bug \
             fix, not content"
        );
    }

    /// An absent directory is not an error, the same way an absent
    /// `assets/memories/` is not.
    #[test]
    fn an_absent_directory_is_not_an_error() {
        let dir = crate::tests::support::scratch_assets_dir("rock_absent");
        let (db, warnings) = RockDb::load_dir(&dir).expect("absent is not an error");
        assert!(warnings.is_empty());
        assert_eq!(db.defs().count(), 0);
    }

    /// A broken file costs that file and nothing else.
    #[test]
    fn a_malformed_file_is_skipped_with_the_directory_intact() {
        let dir = crate::tests::support::scratch_assets_dir("rock_bad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("good.ron"), GOOD).unwrap();
        std::fs::write(dir.join("broken.ron"), "(id: \"x\", oh no").unwrap();
        let (db, warnings) = RockDb::load_dir(&dir).unwrap();
        assert_eq!(db.defs().count(), 1);
        assert_eq!(warnings.len(), 1);
    }

    /// A shade outside the band is refused, for `SectorDef`'s reason: the
    /// map's colour rule is not a per-file opinion.
    #[test]
    fn a_shade_outside_the_band_is_refused() {
        let dir = crate::tests::support::scratch_assets_dir("rock_shade");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("garish.ron"),
            GOOD.replace("shade: 1.0", "shade: 40.0"),
        )
        .unwrap();
        let (db, warnings) = RockDb::load_dir(&dir).unwrap();
        assert_eq!(db.defs().count(), 0);
        assert!(warnings[0].contains("shade"), "{:?}", warnings);
    }

    const GOOD: &str = r#"(
    id: "test",
    name: "Test Rock",
    weight: 10,
    durability: 24,
    min_swings: 2,
    shade: 1.0,
)"#;

    /// A patch has an inside: the cells of one block agree, so an exposed
    /// face says something about what is behind it.
    #[test]
    fn one_block_is_one_kind() {
        let db = db();
        for bx in -3..3 {
            for by in -3..3 {
                let anchor = db.kind_at(11, bx * VEIN_BLOCK, by * VEIN_BLOCK).id.clone();
                for dx in 0..VEIN_BLOCK {
                    for dy in 0..VEIN_BLOCK {
                        assert_eq!(
                            db.kind_at(11, bx * VEIN_BLOCK + dx, by * VEIN_BLOCK + dy)
                                .id,
                            anchor,
                            "block ({bx},{by}) is not one kind"
                        );
                    }
                }
            }
        }
    }

    /// The shipped weights must actually produce a mix over a region the
    /// size of a base, or the roster is decorative.
    #[test]
    fn a_base_sized_region_holds_more_than_one_kind() {
        let db = db();
        let kinds: std::collections::BTreeSet<&str> = (-24..24)
            .flat_map(|x| (-24..24).map(move |y| (x, y)))
            .map(|(x, y)| db.kind_at(4242, x, y).id.as_str())
            .collect();
        assert!(kinds.len() >= 2, "one kind over a whole base: {kinds:?}");
    }

    /// Two different base seeds lay their seams out differently, or the
    /// seed is decorative and every run digs the same base.
    #[test]
    fn two_seeds_lay_out_differently() {
        let db = db();
        let layout = |seed: u32| -> Vec<String> {
            (-8..8)
                .flat_map(|x| (-8..8).map(move |y| (x, y)))
                .map(|(x, y)| db.kind_at(seed, x, y).id.clone())
                .collect()
        };
        assert_ne!(layout(1), layout(2));
    }

    /// Neighbouring blocks decorrelate — the property `block_seed`'s
    /// byte-at-a-time fold exists to hold, and the one a shortened fold
    /// would silently break while every individual answer still looked
    /// arbitrary.
    #[test]
    fn adjacent_blocks_do_not_all_agree() {
        let db = db();
        let row: Vec<&str> = (0..40)
            .map(|b| db.kind_at(9, b * VEIN_BLOCK, 0).id.as_str())
            .collect();
        assert!(
            row.iter().collect::<std::collections::BTreeSet<_>>().len() >= 2,
            "forty adjacent blocks are all one kind: {row:?}"
        );
    }
}
