//! `BaseGrid`: the player base's own coordinate space, separate from
//! `WorldMap`'s zone surface. A cell absent from the map is solid rock —
//! there is no `Solid` variant, because "not in the map" already says it,
//! and storing one for every untouched coordinate would make the sparse
//! map not sparse. Only `open` and `lay_floor` ever put a cell in.
//!
//! Keyed `BTreeMap<(i32, i32), BaseCell>` rather than `HashMap`, for the
//! same reason `Stock` keys by `ItemId` in a `BTreeMap`: whatever iterates
//! this map — a save encoder, once one exists — must see the same order
//! run to run, or the same base produces a different save file depending
//! on insertion history alone.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// One cell of base space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseCell {
    /// Carved out but not yet floored: walkable, but not a `Floor` for
    /// whatever a later task ties to build placement. `mined_at` is the
    /// tick it was opened — unread this slice, for slice 2's mining.
    Open { mined_at: u64 },
    /// Carved out and floored: the buildable, walkable base tile.
    Floor,
}

/// The player base's pocket-dimension coordinate space.
///
/// A `Resource` distinct from `WorldMap`: base space is not a zone, has no
/// biome, and is never generated from a seed. Every coordinate starts and
/// stays solid until `open` or `lay_floor` puts a cell there.
///
/// `Serialize`/`Deserialize` derive straight onto this struct — `cells`
/// stays private, since serde's derive is generated inside this module and
/// does not need a public field the way an external encoder would — and it
/// is embedded directly in `save::SaveData` as `base_grid`, the same way
/// `resources::StackMemory`/`PopulatedChunks` are: it is saved wholesale
/// rather than mirrored into a separate save-shaped type, because there is
/// no engine-internal reason to keep this type out of the save (unlike, say,
/// `components::TaskKind`, which stays un-derived and gets `save::CronjobKind`
/// as its mirror instead).
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BaseGrid {
    cells: BTreeMap<(i32, i32), BaseCell>,
}

impl BaseGrid {
    /// The cell at `(x, y)`, or `None` for solid rock.
    pub fn cell(&self, x: i32, y: i32) -> Option<BaseCell> {
        self.cells.get(&(x, y)).copied()
    }

    /// Whether `(x, y)` is laid floor. `Open` does not count — mined rock
    /// is walkable but not yet floored.
    pub fn is_floor(&self, x: i32, y: i32) -> bool {
        matches!(self.cell(x, y), Some(BaseCell::Floor))
    }

    /// Whether `(x, y)` is solid rock: absent from the map.
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).is_none()
    }

    /// Whether a program can stand on `(x, y)`: `Open` or `Floor`, either
    /// carved-out state.
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        matches!(
            self.cell(x, y),
            Some(BaseCell::Open { .. } | BaseCell::Floor)
        )
    }

    /// Lays floor at `(x, y)`, replacing whatever was there — `Open`
    /// included, so a mined tile that gets floored does not stack the two
    /// states.
    ///
    /// Unreached by production code this task, the same as `open` below:
    /// this task builds only `BaseGrid` and its predicates, and the tasks
    /// that stamp a base's floor and mine into its walls land later in the
    /// plan. `#[allow(dead_code)]` says so rather than leaving a standing
    /// warning on an otherwise-clean lib build.
    #[allow(dead_code)]
    pub(crate) fn lay_floor(&mut self, x: i32, y: i32) {
        self.cells.insert((x, y), BaseCell::Floor);
    }

    /// Carves `(x, y)` out of solid rock: walkable, not floored. Dead in
    /// gameplay this slice — nothing calls it before slice 2's mining —
    /// specified now so `BaseGrid` is complete, and covered by a test
    /// here for the same reason. `#[allow(dead_code)]` for the same
    /// reason `lay_floor` above carries it.
    #[allow(dead_code)]
    pub(crate) fn open(&mut self, x: i32, y: i32, tick: u64) {
        self.cells.insert((x, y), BaseCell::Open { mined_at: tick });
    }

    /// How many cells are laid floor — the cheapest assertion that a base
    /// was actually stamped into this space.
    pub fn floor_count(&self) -> usize {
        self.cells
            .values()
            .filter(|c| matches!(c, BaseCell::Floor))
            .count()
    }

    /// The map in key order. `#[cfg(test)]` the same way
    /// `DescriptionDb::subjects` is: only `tests::base_grid` calls this,
    /// checking the deterministic-iteration guarantee above, and nothing
    /// in play walks base space cell by cell yet — `pub(crate)` alone
    /// would leave a standing dead-code warning on an otherwise-clean lib
    /// build rather than stating the truth.
    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&(i32, i32), &BaseCell)> {
        self.cells.iter()
    }
}
