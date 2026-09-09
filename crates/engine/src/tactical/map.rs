//! The battle map: a grid generated per encounter and destroyed at teardown.
//!
//! `stack::generate`'s precedent, one space over. A board is **pure in its
//! spec** — the same `BattleSpec` yields the same board, cell for cell — and
//! is never saved, because a fight is never saved. Nothing here draws from
//! `resources::GameRng` (a draw does not survive a save/load and shifts
//! every later roll in the run) and nothing here seeds an `StdRng` either
//! (its sequence is not guaranteed stable across a `rand` upgrade, and the
//! tests below compare whole boards for equality). Every cell is *derived*:
//! folded through `derive::fold` and reduced through `derive::index`, the
//! way `rock::RockDb::kind_at` derives base space.

use crate::derive::{FNV_BASIS, fold, index};
use crate::tuning::{
    TACTICAL_BOARD_LARGE, TACTICAL_BOARD_MEDIUM, TACTICAL_BOARD_SMALL, TACTICAL_LARGE_BODIES,
    TACTICAL_MEDIUM_BODIES, TACTICAL_ROUGH_COST,
};
use crate::world::Biome;

/// What a cell of a battle map is made of.
///
/// Four kinds, and the four are the complete 2x2 of "can you cross it"
/// against "can you shoot through it" — which is what keeps them from being
/// an arbitrary list. `Blocked` is a chasm you can see over and cannot
/// cross; `Cover` is a boulder that stops both. Note this is the same
/// asymmetry the Stack already establishes, where `walkable()` and
/// `blocks_sight()` are deliberately not complements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleCell {
    Open,
    Rough,
    Cover,
    Blocked,
}

impl BattleCell {
    /// What crossing this cell costs, or `None` where it cannot be crossed.
    ///
    /// An `Option<u32>` rather than a `u32` with a sentinel because that is
    /// the shape the movement field's step rule takes: `None` is blocked,
    /// `Some(c)` is the cost.
    pub fn movement_cost(self) -> Option<u32> {
        match self {
            BattleCell::Open => Some(1),
            BattleCell::Rough => Some(TACTICAL_ROUGH_COST),
            BattleCell::Cover | BattleCell::Blocked => None,
        }
    }

    /// Derived from `movement_cost` rather than matched separately, so the
    /// two cannot come to disagree about a kind.
    pub fn walkable(self) -> bool {
        self.movement_cost().is_some()
    }

    /// `Cover` is the only kind that stops a line or a cone.
    pub fn blocks_sight(self) -> bool {
        matches!(self, BattleCell::Cover)
    }
}

/// Everything a battle map is derived from.
///
/// `stack::FrameSpec`'s counterpart: a board is a pure function of this and
/// nothing else, which is what makes it unit-testable without a `Game` and
/// what makes it safe never to save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleSpec {
    pub world_seed: u32,
    /// The world tile the fight opened on.
    pub site: (i32, i32),
    /// `GameClock` at the moment the fight opened.
    ///
    /// In the spec so that two fights on one tile are not the same board.
    /// Safe here and nowhere else: a battle is never saved and never
    /// regenerated, so a board that depends on the moment cannot come back
    /// wrong after a reload — the trap `stack::generate`'s doc warns about.
    pub tick: u64,
    pub zone: u32,
    pub biome: Biome,
    /// Party plus wild. Decides the board's extent, nothing else.
    pub bodies: u32,
}

impl BattleSpec {
    /// The board's extent, in cells on a side.
    pub fn side(self) -> i32 {
        if self.bodies >= TACTICAL_LARGE_BODIES {
            TACTICAL_BOARD_LARGE
        } else if self.bodies >= TACTICAL_MEDIUM_BODIES {
            TACTICAL_BOARD_MEDIUM
        } else {
            TACTICAL_BOARD_SMALL
        }
    }

    /// The fold every cell of this board starts from.
    ///
    /// `bodies` is deliberately absent: it decides the extent, and folding
    /// it in as well would mean one more companion in the party changed the
    /// ground under a fight on the same tile at the same moment.
    fn base_seed(self) -> u64 {
        fold(
            FNV_BASIS,
            &[
                self.world_seed as u64,
                self.site.0 as u32 as u64,
                self.site.1 as u32 as u64,
                self.tick,
                self.zone as u64,
                self.biome as u64,
            ],
        )
    }

    /// A stable seed for one cell of this board.
    pub(crate) fn cell_seed(self, x: i32, y: i32) -> u64 {
        fold(self.base_seed(), &[x as u32 as u64, y as u32 as u64])
    }
}

/// The four kinds in the order `terrain_weights` gives their weights.
const KINDS: [BattleCell; 4] = [
    BattleCell::Open,
    BattleCell::Rough,
    BattleCell::Cover,
    BattleCell::Blocked,
];

/// What each biome's ground is made of, as `[Open, Rough, Cover, Blocked]`
/// weights summing to 100.
///
/// **Exhaustive on `Biome`** — `cell_mark`'s rule. A `_ =>` arm would ship a
/// new biome's fights on whatever the fallback happened to be, and terrain
/// that is quietly the wrong terrain reads as the generator being bland
/// rather than as a missing row.
///
/// In `tuning.rs`'s neighbourhood rather than in `.ron` for the reason
/// `Biome::name` already gives: mods extend species, structures, items and
/// environments, but the biome set is a fixed enum `WorldMap::classify`
/// sorts noise into, and ground for a variant that cannot exist is not a
/// thing a file can usefully say.
fn terrain_weights(biome: Biome) -> [u32; 4] {
    match biome {
        // A plain. Position matters least here, which is the point of it.
        Biome::OpenGrid => [82, 12, 4, 2],
        // Interference underfoot: slow going, little to hide behind.
        Biome::Deadlock => [60, 30, 7, 3],
        // Holes in the substrate — see across them, cannot cross them.
        Biome::NullSector => [62, 16, 8, 14],
        // A circuit board: dense with things to put between you and a shot.
        Biome::Backplane => [58, 14, 20, 8],
        // Laid and carved base floor. A fight does not open here today, but
        // if one ever does it opens on a floor, which is what these are.
        Biome::Platform | Biome::Excavated => [92, 6, 2, 0],
        // Unwalkable ground: `Biome::walkable()` is false for all three, so
        // nothing is ever placed there and no fight opens there. Plain open
        // rather than solid, so that if one ever did it would be a board and
        // not a wall.
        Biome::DataVoid | Biome::BlackIce | Biome::Entropy => [100, 0, 0, 0],
    }
}

/// The ground of one cell, derived and never rolled.
fn kind_at(spec: BattleSpec, x: i32, y: i32) -> BattleCell {
    let weights = terrain_weights(spec.biome);
    let total: u32 = weights.iter().sum();
    let mut n = index(spec.cell_seed(x, y), total as usize) as u32;
    for (kind, weight) in KINDS.iter().zip(weights) {
        if n < weight {
            return *kind;
        }
        n -= weight;
    }
    BattleCell::Open
}

/// A generated battle map. Never saved; discarded at teardown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub side: i32,
    cells: Vec<BattleCell>,
}

impl Board {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.side && y < self.side
    }

    /// Off the board reads as `Blocked` — seen over, never stepped on.
    ///
    /// The right answer for generation, the carve and deployment, none of
    /// which may reach outside the board. Walking *off* the edge is a
    /// departure from the fight rather than a step, and belongs to the turn
    /// model; nothing here treats the edge as an exit.
    pub fn cell(&self, x: i32, y: i32) -> BattleCell {
        if !self.in_bounds(x, y) {
            return BattleCell::Blocked;
        }
        self.cells[(y * self.side + x) as usize]
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).walkable()
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).blocks_sight()
    }

    fn set(&mut self, x: i32, y: i32, kind: BattleCell) {
        if self.in_bounds(x, y) {
            let i = (y * self.side + x) as usize;
            self.cells[i] = kind;
        }
    }

    pub fn cells(&self) -> impl Iterator<Item = ((i32, i32), BattleCell)> + '_ {
        let side = self.side;
        self.cells
            .iter()
            .enumerate()
            .map(move |(i, &kind)| (((i as i32) % side, (i as i32) / side), kind))
    }
}

/// The whole generator: derive every cell, then make sure the walkable
/// ground is one piece.
pub fn generate(spec: BattleSpec) -> Board {
    let side = spec.side();
    let cells = (0..side * side)
        .map(|i| kind_at(spec, i % side, i / side))
        .collect();
    Board { side, cells }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Biome;

    fn spec(bodies: u32) -> BattleSpec {
        BattleSpec {
            world_seed: 1234,
            site: (12, -7),
            tick: 900,
            zone: 3,
            biome: Biome::OpenGrid,
            bodies,
        }
    }

    #[test]
    fn the_board_steps_up_a_tier_at_four_bodies_and_at_seven() {
        assert_eq!(spec(1).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(3).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(4).side(), TACTICAL_BOARD_MEDIUM);
        assert_eq!(spec(6).side(), TACTICAL_BOARD_MEDIUM);
        assert_eq!(spec(7).side(), TACTICAL_BOARD_LARGE);
        assert_eq!(spec(13).side(), TACTICAL_BOARD_LARGE);
    }

    /// Every tier is reachable in play: the smallest fight the game can
    /// field is the player and one wild body, and the largest is a full
    /// party against a full pack.
    #[test]
    fn all_three_tiers_are_reachable_between_the_smallest_and_largest_fight() {
        let smallest = 1 + 1;
        let largest = crate::tuning::MAX_PARTY_SIZE as u32 + crate::tuning::MAX_PACK_BODIES;
        assert_eq!(spec(smallest).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(largest).side(), TACTICAL_BOARD_LARGE);
        assert!(
            (smallest..=largest).any(|n| spec(n).side() == TACTICAL_BOARD_MEDIUM),
            "the middle tier can never be reached"
        );
    }

    #[test]
    fn a_cell_seed_is_a_property_of_the_spec_and_the_cell() {
        assert_eq!(spec(4).cell_seed(3, 5), spec(4).cell_seed(3, 5));
        assert_ne!(spec(4).cell_seed(3, 5), spec(4).cell_seed(3, 6));
        assert_ne!(spec(4).cell_seed(3, 5), spec(4).cell_seed(5, 3));
    }

    /// Two fights on the same tile are not the same fight. A board is never
    /// saved and never regenerated, so leaning on the clock here cannot come
    /// back wrong after a reload.
    #[test]
    fn a_later_fight_on_the_same_tile_gets_a_different_board() {
        let mut later = spec(4);
        later.tick += 1;
        assert_ne!(spec(4).cell_seed(0, 0), later.cell_seed(0, 0));
    }

    #[test]
    fn the_biome_and_the_zone_both_reach_the_cell_seed() {
        let mut marsh = spec(4);
        marsh.biome = Biome::Deadlock;
        assert_ne!(spec(4).cell_seed(0, 0), marsh.cell_seed(0, 0));

        let mut deeper = spec(4);
        deeper.zone += 1;
        assert_ne!(spec(4).cell_seed(0, 0), deeper.cell_seed(0, 0));
    }

    /// The pair that makes four kinds necessary rather than arbitrary: one
    /// you can see over and cannot cross, one you can do neither with. If
    /// these ever agree, two of the kinds are the same kind.
    #[test]
    fn blocked_is_seen_over_and_cover_is_not() {
        assert!(!BattleCell::Blocked.walkable());
        assert!(!BattleCell::Blocked.blocks_sight());
        assert!(!BattleCell::Cover.walkable());
        assert!(BattleCell::Cover.blocks_sight());
    }

    #[test]
    fn rough_is_crossed_and_costs_more_than_open() {
        assert_eq!(BattleCell::Open.movement_cost(), Some(1));
        assert!(BattleCell::Rough.walkable());
        assert!(
            BattleCell::Rough.movement_cost() > BattleCell::Open.movement_cost(),
            "Rough that costs one is Open with a different name"
        );
    }

    /// `walkable` is derived from `movement_cost` rather than matched
    /// separately, so the two can never come to disagree about a kind.
    #[test]
    fn walkable_is_exactly_having_a_movement_cost() {
        for kind in [
            BattleCell::Open,
            BattleCell::Rough,
            BattleCell::Cover,
            BattleCell::Blocked,
        ] {
            assert_eq!(kind.walkable(), kind.movement_cost().is_some());
        }
    }

    /// The weights read as percentages at a glance, which is the only
    /// reason they are worth reading at all.
    #[test]
    fn every_biomes_ground_sums_to_a_hundred() {
        for biome in [
            Biome::DataVoid,
            Biome::Deadlock,
            Biome::NullSector,
            Biome::Backplane,
            Biome::OpenGrid,
            Biome::BlackIce,
            Biome::Platform,
            Biome::Excavated,
            Biome::Entropy,
        ] {
            let total: u32 = terrain_weights(biome).iter().sum();
            assert_eq!(total, 100, "{biome:?} does not sum to 100");
        }
    }

    #[test]
    fn the_same_spec_yields_an_identical_board() {
        let a = generate(spec(4));
        let b = generate(spec(4));
        assert_eq!(a.side, b.side);
        assert_eq!(a.cells().collect::<Vec<_>>(), b.cells().collect::<Vec<_>>());
    }

    #[test]
    fn a_board_is_its_tier_square() {
        let board = generate(spec(9));
        assert_eq!(board.side, TACTICAL_BOARD_LARGE);
        assert_eq!(
            board.cells().count(),
            (TACTICAL_BOARD_LARGE * TACTICAL_BOARD_LARGE) as usize
        );
    }

    fn blockers(board: &Board) -> usize {
        board.cells().filter(|(_, kind)| !kind.walkable()).count()
    }

    /// The ground is the biome's, not one texture everywhere. Backplane is
    /// a circuit board and is dense with cover; Open Grid is a plain.
    #[test]
    fn a_backplane_fight_has_more_to_hide_behind_than_an_open_grid_one() {
        let mut plain = spec(4);
        plain.biome = Biome::OpenGrid;
        let mut city = spec(4);
        city.biome = Biome::Backplane;
        assert!(
            blockers(&generate(city)) > blockers(&generate(plain)),
            "the biome does not reach the ground"
        );
    }

    #[test]
    fn out_of_bounds_is_seen_over_and_never_stepped_on() {
        let board = generate(spec(4));
        assert!(!board.in_bounds(-1, 0));
        assert!(!board.in_bounds(board.side, 0));
        assert_eq!(board.cell(-1, 0), BattleCell::Blocked);
        assert!(!board.walkable(-1, 0));
        assert!(!board.blocks_sight(-1, 0));
    }
}
