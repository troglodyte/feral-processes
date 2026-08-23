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
    /// tick it was opened, and `base::entropy::base_entropy_system` reads it
    /// every tick: an `Open` cell left unfloored past
    /// `BASE_ENTROPY_REFILL_TICKS` goes back to solid.
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
    /// Base space's own seed, which `rock::RockDb::kind_at` folds to decide
    /// what any coordinate is made of.
    ///
    /// **Its own, and deliberately not `WorldMap::seed()`.** The world seed
    /// changes on every breach — `enter_next_zone` mints the next zone from
    /// `wrapping_add(0x9E37_79B9)` — while this grid *travels* with the run.
    /// Salted off the world seed, every seam in the base would reshuffle
    /// each time the player portalled, and a half-cut wall would come back a
    /// different kind under its already-saved `Durability`.
    ///
    /// `#[serde(default)]`, so a save written before rock kinds existed
    /// loads at 0 — a valid, deterministic layout rather than a special
    /// case, and additive, so this costs no `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    seed: u32,
}

impl BaseGrid {
    /// The cell at `(x, y)`, or `None` for solid rock.
    pub fn cell(&self, x: i32, y: i32) -> Option<BaseCell> {
        self.cells.get(&(x, y)).copied()
    }

    /// Mints base space's seed. `Game::new` is the one caller — a base has
    /// one layout for the whole run, and re-seeding it mid-run would move
    /// the seams under a base already dug into them.
    pub(crate) fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    /// The seed `rock::RockDb::kind_at` folds. See the field.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Whether `(x, y)` is a rock face with air against it: solid, with at
    /// least one **orthogonal** walkable neighbour.
    ///
    /// Orthogonal rather than 8-way — a diagonal neighbour does not expose a
    /// face. Derived per lookup and never cached, and
    /// `base::entropy::base_entropy_system` is the argument for that: a
    /// re-knitting cell changes the exposure of its four neighbours, and a
    /// cached flag would need keeping in step with every open, floor and
    /// revert.
    pub fn is_exposed(&self, x: i32, y: i32) -> bool {
        self.is_solid(x, y)
            && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| self.walkable(x + dx, y + dy))
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
    /// Two callers in gameplay: `Game::lay_starting_pocket`, the pocket the
    /// run opens with when its first Home goes down, and `Game::floor_cell`,
    /// which is the one door a cell becomes floor through in play.
    pub(crate) fn lay_floor(&mut self, x: i32, y: i32) {
        self.cells.insert((x, y), BaseCell::Floor);
    }

    /// Carves `(x, y)` out of solid rock: walkable, not floored.
    ///
    /// `Game::strike_rock` is the one caller in play — the swing that takes
    /// a wall's last durability — which is what makes `mined_at` the tick
    /// the wall came down and so the tick entropy measures its window from.
    pub(crate) fn open(&mut self, x: i32, y: i32, tick: u64) {
        self.cells.insert((x, y), BaseCell::Open { mined_at: tick });
    }

    /// How far the carved-out space reaches from base space's origin: the
    /// largest Chebyshev distance of any cell in it, and 0 while everything
    /// is still solid.
    ///
    /// Bounds the Dijkstra field a posted worker walks (`tuning::
    /// haul_walk_radius`), which is why it is measured rather than taken
    /// from `STARTING_POCKET_RADIUS`: a walk bounded by the size the pocket
    /// *started* at would refuse postings across a base the player has since
    /// mined outward, which is a machine you can see from your Home and
    /// cannot staff.
    ///
    /// Walked rather than cached, the same argument `Game::build_radius`
    /// makes about deriving over stored state: a cell opened, floored or
    /// reclaimed by entropy changes this answer, and a cached one would need
    /// keeping in step with all three.
    pub(crate) fn radius(&self) -> i32 {
        self.cells
            .keys()
            .map(|&(x, y)| x.abs().max(y.abs()))
            .max()
            .unwrap_or(0)
    }

    /// How many cells are laid floor — the cheapest assertion that a base
    /// was actually stamped into this space.
    pub fn floor_count(&self) -> usize {
        self.cells
            .values()
            .filter(|c| matches!(c, BaseCell::Floor))
            .count()
    }

    /// Takes `(x, y)` back out of the map: solid rock again, and absent
    /// rather than chipped, because absent is what this module means by
    /// solid. `game::base::entropy` is the one caller — nothing else in the
    /// game ever *shrinks* base space.
    pub(crate) fn revert(&mut self, x: i32, y: i32) {
        self.cells.remove(&(x, y));
    }

    /// The map in key order — the deterministic iteration this type exists
    /// to guarantee. `base_entropy_system` is the one thing in play that
    /// walks base space cell by cell; `tests::base_grid` walks it to check
    /// the order itself.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&(i32, i32), &BaseCell)> {
        self.cells.iter()
    }
}
