//! `BaseGrid`: the base's laid floor, an overlay on the field keyed by
//! field coordinates. It answers only "is this cell floor" (plus the two
//! marks painted over floor, finishes and stockpiles); whether a cell can be
//! stood on is the field's question, asked through `Ground`.
//!
//! Keyed by `BTreeSet`/`BTreeMap` rather than hash containers, for the same
//! reason `Stock` keys by `ItemId` in a `BTreeMap`: whatever iterates it —
//! the save encoder, `rooms::detect`, `siege_spawn_cells` — must see the same
//! order run to run.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::floors::{FloorDb, FloorId};
use crate::world::WorldMap;

/// The base's floor overlay on the field.
///
/// Saved wholesale in `save::SaveData` as `base_grid`. A cell is floor or it
/// is not; mined ground is a `WorldMap` override (`Biome::Excavated`), not a
/// state of this store.
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "SavedBaseGrid")]
pub struct BaseGrid {
    floor: BTreeSet<(i32, i32)>,
    /// A decorative finish the dig crew has painted over laid floor — kept
    /// apart from `floor` so a finished cell stays plain floor to every
    /// reader that asks `is_floor`.
    #[serde(default)]
    finishes: BTreeMap<(i32, i32), FloorId>,
    /// Cells the player has zoned as a stockpile: a standing preference over
    /// floor, not a property of the cell. Building over a marked cell leaves
    /// the mark in place, and the floor-item search simply finds no free tile.
    #[serde(default)]
    stockpiles: BTreeSet<(i32, i32)>,
}

/// `BaseGrid` as the save spells it, which before ADR 0032 was base space's
/// `cells` map (`Floor` and mined `Open` cells, both walkable) and a `seed`.
///
/// Temporary — it keeps the checked-in `dev-saves/` loading with their base
/// intact until they are recaptured, and goes with the `SAVE_FORMAT_VERSION`
/// bump. Base-space coordinates are the field's now, and every old cell a
/// body could stand on loads as floor, the one ground the crew walks.
#[derive(Deserialize)]
struct SavedBaseGrid {
    #[serde(default)]
    floor: BTreeSet<(i32, i32)>,
    #[serde(default)]
    cells: BTreeMap<(i32, i32), serde::de::IgnoredAny>,
    #[serde(default)]
    finishes: BTreeMap<(i32, i32), FloorId>,
    #[serde(default)]
    stockpiles: BTreeSet<(i32, i32)>,
}

impl From<SavedBaseGrid> for BaseGrid {
    fn from(saved: SavedBaseGrid) -> Self {
        let mut floor = saved.floor;
        floor.extend(saved.cells.into_keys());
        BaseGrid {
            floor,
            finishes: saved.finishes,
            stockpiles: saved.stockpiles,
        }
    }
}

impl BaseGrid {
    /// Whether `(x, y)` is laid floor.
    pub fn is_floor(&self, x: i32, y: i32) -> bool {
        self.floor.contains(&(x, y))
    }

    /// Lays floor at `(x, y)`. `Game::floor_cell` is the one door a cell
    /// becomes floor through in play; founding is the other.
    pub(crate) fn lay_floor(&mut self, x: i32, y: i32) {
        self.floor.insert((x, y));
    }

    /// Half the larger side of the floor's bounding box, rounded up, and 0
    /// with no floor: how far apart two cells of the base can be, halved.
    ///
    /// Bounds the Dijkstra field a posted worker walks (`tuning::
    /// haul_walk_radius` doubles it), which is why it is measured rather
    /// than fixed: a walk bounded by the size the base *started* at would
    /// refuse postings across a base grown since. Walked rather than cached:
    /// every laid cell changes it.
    pub(crate) fn radius(&self) -> i32 {
        let Some(&(x0, y0)) = self.floor.first() else {
            return 0;
        };
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (x0, x0, y0, y0);
        for &(x, y) in &self.floor {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        ((max_x - min_x).max(max_y - min_y) + 1) / 2
    }

    /// How many cells are laid floor.
    pub fn floor_count(&self) -> usize {
        self.floor.len()
    }

    /// Takes `(x, y)` back off the floor, and any finish with it. Nothing in
    /// play un-floors a cell; test fixtures do.
    #[cfg(test)]
    pub(crate) fn revert(&mut self, x: i32, y: i32) {
        self.floor.remove(&(x, y));
        self.finishes.remove(&(x, y));
    }

    /// The finish painted over `(x, y)`, if any.
    pub fn finish_at(&self, x: i32, y: i32) -> Option<&FloorId> {
        self.finishes.get(&(x, y))
    }

    /// Paints `id` over `(x, y)`, refusing — and changing nothing — unless
    /// the cell is laid floor. Overwrites whatever finish was already there.
    pub(crate) fn set_finish(&mut self, x: i32, y: i32, id: FloorId) -> bool {
        if !self.is_floor(x, y) {
            return false;
        }
        self.finishes.insert((x, y), id);
        true
    }

    /// Whether `(x, y)` is zoned as a stockpile.
    pub fn is_stockpile(&self, x: i32, y: i32) -> bool {
        self.stockpiles.contains(&(x, y))
    }

    /// Zones or un-zones `(x, y)`. Marking is refused — and changes nothing —
    /// unless the cell is laid floor; clearing always succeeds, so a mark
    /// left under a structure can still be taken off.
    pub(crate) fn set_stockpile(&mut self, x: i32, y: i32, on: bool) -> bool {
        if !on {
            self.stockpiles.remove(&(x, y));
            return true;
        }
        if !self.is_floor(x, y) {
            return false;
        }
        self.stockpiles.insert((x, y));
        true
    }

    /// Every stockpile cell, in `(x, y)` order.
    pub fn stockpile_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.stockpiles.iter().copied()
    }

    /// Strips `(x, y)`'s finish, if it has one, leaving the cell `Floor`.
    /// Reports whether a finish was actually removed.
    pub(crate) fn clear_finish(&mut self, x: i32, y: i32) -> bool {
        self.finishes.remove(&(x, y)).is_some()
    }

    /// Drops every finish `db` no longer resolves and every finish on a cell
    /// that is not `Floor`, returning one warning line per drop — called
    /// once at load, right before this grid becomes the live resource, so a
    /// mod that renamed or deleted a finish (or a save from before this
    /// feature ran `set_finish`'s own check) cannot leave a dangling id
    /// behind.
    pub(crate) fn prune_finishes(&mut self, db: &FloorDb) -> Vec<String> {
        let mut warnings = Vec::new();
        self.finishes.retain(|&(x, y), id| {
            if db.get(id).is_none() {
                warnings.push(format!(
                    "dropped floor finish {id:?} at ({x}, {y}): no such finish is loaded"
                ));
                return false;
            }
            if !self.floor.contains(&(x, y)) {
                warnings.push(format!(
                    "dropped floor finish {id:?} at ({x}, {y}): the cell is not laid floor"
                ));
                return false;
            }
            true
        });
        warnings
    }

    /// Every floor cell in `(x, y)` order — the deterministic iteration
    /// this type exists to guarantee.
    pub(crate) fn floor_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.floor.iter().copied()
    }
}

/// The field as the base walks it: the floor overlay over the `WorldMap`.
///
/// One borrow of each resource, so the pure crew and siege rules can ask
/// walkability without `&mut WorldMap` (`WorldMap::peek` generates nothing).
#[derive(Clone, Copy)]
pub(crate) struct Ground<'a> {
    pub floor: &'a BaseGrid,
    pub map: &'a WorldMap,
}

impl Ground<'_> {
    /// Whether a body can stand on `(x, y)`: laid floor, or a walkable field
    /// tile. Floor counts on its own so a fixture that lays floor needs no
    /// override under it; in play floor only goes on walkable ground.
    pub(crate) fn walkable(&self, x: i32, y: i32) -> bool {
        self.floor.is_floor(x, y) || self.map.peek(x, y).walkable
    }

    pub(crate) fn is_floor(&self, x: i32, y: i32) -> bool {
        self.floor.is_floor(x, y)
    }
}
