//! The traders that walk in, loaded from `assets/caravans/`.
//!
//! One def per trader: what it is called, what it looks like on the map, how
//! many rows its shelf holds, and how those rows are weighted between gear,
//! routine disks, programs and materials. *Which* trader visits and *what*
//! is on its shelf are both derived in `game/caravan.rs` from the base's own
//! seed — nothing here is rolled, and nothing here is saved.
//!
//! **An empty database is valid and inert**, the rule `MemoryDb` and
//! `NemesisDb` already follow: with no defs there is nothing for the
//! schedule to pick, so `Game::scheduled_visit` answers `None` forever and
//! deleting `assets/caravans/` restores the pre-caravan game rather than
//! breaking an install. That is why an absent directory is silent.
//!
//! `assets/caravans/README.md` is the schema reference.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

use crate::components::GlyphColor;

/// How a trader's shelf is split between the four kinds of row.
///
/// Relative weights, not counts: a def declares the *shape* of its stock and
/// `rows` decides how much of it there is, so raising a trader's row count
/// never silently changes what it deals in.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct CaravanWeights {
    #[serde(default)]
    pub gear: u32,
    #[serde(default)]
    pub routines: u32,
    #[serde(default)]
    pub programs: u32,
    #[serde(default)]
    pub materials: u32,
}

impl CaravanWeights {
    /// The four weights in the order `CaravanOfferKind` is chosen in. Shared
    /// rather than spelled out at each reader, so a fifth kind cannot be
    /// added to one site and missed at the other.
    pub(crate) fn total(&self) -> u32 {
        self.gear + self.routines + self.programs + self.materials
    }
}

/// One trader.
///
/// Every field is required — a def missing any of them is one that cannot be
/// drawn or stocked. Any field added *later* must be `#[serde(default)]`, per
/// the standing rule for `SpeciesDef`/`StructureDef`/`ItemDef`/`AbilityDef`.
#[derive(Clone, Debug, Deserialize)]
pub struct CaravanDef {
    pub id: String,
    /// What the arrival line and the screen header call it.
    pub name: String,
    /// One line of flavour, in the player's vocabulary.
    pub description: String,
    /// The glyph it wears on both maps.
    pub glyph: char,
    pub color: GlyphColor,
    /// How many rows its shelf holds.
    pub rows: u32,
    pub weights: CaravanWeights,
    /// What percentage of this trader's *gear* rows are standout stock —
    /// guaranteed an affix, likely above `Rarity::Ordinary`, and rolled off
    /// a raised quality floor — rather than the plain drop rates every other
    /// row uses.
    ///
    /// Content rather than a `tuning.rs` constant for `rows`' reason: how
    /// much stock a trader carries and what grade it is are the same
    /// question asked twice, and `weights` already settles the third. The
    /// *magnitudes* of a standout roll stay in `tuning.rs`, exactly as a
    /// perk's cost is data while its effect is not.
    ///
    /// Rounded **up**, so any non-zero share puts at least one standout row
    /// on a shelf that has any gear at all — a share that quietly rounds to
    /// nothing is a field that reads as broken.
    #[serde(default)]
    pub bonus_share: u32,
    /// The inclusive sector window this trader may visit in.
    pub min_zone: u32,
    pub max_zone: u32,
}

/// Every trader the game knows about.
#[derive(Resource, Default)]
pub struct CaravanDb {
    defs: BTreeMap<String, CaravanDef>,
}

impl CaravanDb {
    /// Loads every `*.ron` def in `dir`. Follows `MemoryDb::load_dir`: an
    /// absent directory is silent, the walk is sorted so two files claiming
    /// one id resolve the same way every run, and a malformed file costs the
    /// game that one trader and nothing else.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = CaravanDb::default();
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
            match ron::from_str::<CaravanDef>(&text) {
                Ok(def) => match complaint(&def) {
                    Some(why) => {
                        warnings.push(format!("skipped invalid caravan file {path:?}: {why}"))
                    }
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid caravan file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&CaravanDef> {
        self.defs.get(id)
    }

    /// Every loaded def, in id order.
    pub fn all(&self) -> impl Iterator<Item = &CaravanDef> {
        self.defs.values()
    }

    /// The traders whose authored window contains `zone`, sorted by id.
    ///
    /// **Sorted, and that is load-bearing.** This list feeds a derived pick
    /// in `game/caravan.rs`, so an unstable order would make one seed choose
    /// a different trader between two runs of the same save — the fault
    /// `assembler_system` sorts machines by `(x, y)` to avoid. The `BTreeMap`
    /// gives it for free; the ordering is asserted rather than assumed.
    pub fn for_zone(&self, zone: u32) -> Vec<&CaravanDef> {
        self.defs
            .values()
            .filter(|d| d.min_zone <= zone && zone <= d.max_zone)
            .collect()
    }
}

/// Why a def is unusable, or `None` if it is fine. Checked at load so a
/// broken trader is one logged warning at startup rather than an empty shelf
/// nobody can explain.
fn complaint(def: &CaravanDef) -> Option<String> {
    if def.id.trim().is_empty() {
        return Some("id is empty".into());
    }
    if def.rows == 0 {
        return Some("rows is 0, so the shelf would be empty".into());
    }
    if def.weights.total() == 0 {
        return Some("every weight is 0, so no row could be filled".into());
    }
    if def.bonus_share > 100 {
        return Some(format!(
            "bonus_share {} is a percentage and cannot exceed 100",
            def.bonus_share
        ));
    }
    if def.min_zone > def.max_zone {
        return Some(format!(
            "min_zone {} is above max_zone {}",
            def.min_zone, def.max_zone
        ));
    }
    None
}
