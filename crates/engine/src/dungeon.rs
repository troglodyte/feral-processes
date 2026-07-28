//! The dungeon map model and its generator.
//!
//! Pure data and pure functions — no ECS, no `Game`. A level is never
//! persisted: it regenerates deterministically from `(world seed, depth)`,
//! exactly the way `world::WorldMap` regenerates terrain from its seed. The
//! save carries only where in the dungeon the player is standing.

use std::collections::VecDeque;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

/// Side length of a generated level, in cells. Odd, because the maze carver
/// below puts cells on odd coordinates and walls on even ones — 21 gives a
/// 10x10 lattice of cells inside a solid border.
const LEVEL_SIZE: i32 = 21;

/// Percent of dead ends the braid pass opens back up into the rest of the
/// maze. A perfect maze is *all* dead ends, which is tedious to walk and
/// reads as noise from a first-person view; loops are what make a level feel
/// like a place rather than a puzzle box. Not in `tuning.rs` for the same
/// reason `world::WorldMap::classify` keeps its noise thresholds inline —
/// this is the shape of generated content, not a difficulty knob.
const BRAID_PERCENT: u32 = 50;

/// Which way the party is facing. North is `-y`, matching the top-down
/// renderer's convention that increasing `y` draws further down the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    pub fn turn_left(self) -> Self {
        match self {
            Dir::North => Dir::West,
            Dir::West => Dir::South,
            Dir::South => Dir::East,
            Dir::East => Dir::North,
        }
    }

    pub fn turn_right(self) -> Self {
        match self {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
        }
    }

    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::North => (0, -1),
            Dir::East => (1, 0),
            Dir::South => (0, 1),
            Dir::West => (-1, 0),
        }
    }

    /// The unit vector 90° clockwise from this one — the party's right hand.
    /// Used to walk the view cone sideways without another `match`.
    pub fn right_delta(self) -> (i32, i32) {
        self.turn_right().delta()
    }

    pub fn label(self) -> &'static str {
        match self {
            Dir::North => "N",
            Dir::East => "E",
            Dir::South => "S",
            Dir::West => "W",
        }
    }
}

/// Every direction, in a fixed order. The generator iterates this rather
/// than an ad-hoc list so a given seed always produces the same level.
const DIRS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// Solid. Not walkable, and what every out-of-bounds read returns.
    Rock,
    Floor,
    /// Back the way you came — the level's `entry`. On depth 1 this leads
    /// out to the surface.
    StairsUp,
    StairsDown,
}

impl CellKind {
    pub fn walkable(self) -> bool {
        !matches!(self, CellKind::Rock)
    }
}

#[derive(Clone, Debug)]
pub struct DungeonLevel {
    pub width: i32,
    pub height: i32,
    cells: Vec<CellKind>,
    /// Where the party arrives, and where `StairsUp` sits.
    pub entry: (i32, i32),
    pub stairs_down: (i32, i32),
}

impl DungeonLevel {
    /// Out of bounds reads as `Rock`, so callers walking a view cone past
    /// the edge of the level don't each need their own bounds check.
    pub fn cell(&self, x: i32, y: i32) -> CellKind {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return CellKind::Rock;
        }
        self.cells[(y * self.width + x) as usize]
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).walkable()
    }

    fn set(&mut self, x: i32, y: i32, kind: CellKind) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        self.cells[(y * self.width + x) as usize] = kind;
    }
}

/// Builds the level for `depth` under the world seeded with `world_seed`.
///
/// Deterministic in both arguments and nothing else. The RNG is seeded
/// locally rather than drawn from `resources::GameRng`: that generator's
/// stream position is not persisted, so drawing from it would regenerate a
/// *different* level after a save/load, and the party would find itself
/// inside solid rock.
pub fn generate(world_seed: u32, depth: u32) -> DungeonLevel {
    let mut rng = StdRng::seed_from_u64(((world_seed as u64) << 32) | depth as u64);

    let mut level = DungeonLevel {
        width: LEVEL_SIZE,
        height: LEVEL_SIZE,
        cells: vec![CellKind::Rock; (LEVEL_SIZE * LEVEL_SIZE) as usize],
        entry: (1, 1),
        stairs_down: (1, 1),
    };

    carve_maze(&mut level, &mut rng);
    braid(&mut level, &mut rng);

    let far = furthest_floor_from(&level, level.entry);
    level.stairs_down = far;
    level.set(far.0, far.1, CellKind::StairsDown);
    level.set(level.entry.0, level.entry.1, CellKind::StairsUp);

    level
}

/// Recursive-backtracker maze carver. Cells sit on odd coordinates; carving
/// to a neighbour two cells away also clears the wall between them, so the
/// result is a perfect maze — fully connected, with exactly one route
/// between any two cells. `braid` then relaxes that.
fn carve_maze(level: &mut DungeonLevel, rng: &mut StdRng) {
    let start = level.entry;
    level.set(start.0, start.1, CellKind::Floor);

    let mut stack = vec![start];
    while let Some(&(x, y)) = stack.last() {
        let mut candidates = Vec::new();
        for dir in DIRS {
            let (dx, dy) = dir.delta();
            let (nx, ny) = (x + dx * 2, y + dy * 2);
            if nx > 0
                && ny > 0
                && nx < level.width - 1
                && ny < level.height - 1
                && level.cell(nx, ny) == CellKind::Rock
            {
                candidates.push((dx, dy, nx, ny));
            }
        }

        if candidates.is_empty() {
            stack.pop();
            continue;
        }

        let (dx, dy, nx, ny) = candidates[rng.random_range(0..candidates.len())];
        level.set(x + dx, y + dy, CellKind::Floor);
        level.set(nx, ny, CellKind::Floor);
        stack.push((nx, ny));
    }
}

/// Opens `BRAID_PERCENT` of dead ends into a neighbouring corridor, turning
/// the perfect maze into a looping one. Walks cells in row-major order so
/// the pass is deterministic for a given RNG stream.
fn braid(level: &mut DungeonLevel, rng: &mut StdRng) {
    for y in (1..level.height - 1).step_by(2) {
        for x in (1..level.width - 1).step_by(2) {
            if level.cell(x, y) != CellKind::Floor || !is_dead_end(level, x, y) {
                continue;
            }
            if rng.random_range(0..100) >= BRAID_PERCENT {
                continue;
            }
            // Only walls with another carved cell on the far side are
            // candidates — knocking through to solid rock would carve a new
            // dead end rather than removing one.
            let mut walls = Vec::new();
            for dir in DIRS {
                let (dx, dy) = dir.delta();
                if level.cell(x + dx, y + dy) == CellKind::Rock
                    && level.cell(x + dx * 2, y + dy * 2) == CellKind::Floor
                {
                    walls.push((x + dx, y + dy));
                }
            }
            if walls.is_empty() {
                continue;
            }
            let (wx, wy) = walls[rng.random_range(0..walls.len())];
            level.set(wx, wy, CellKind::Floor);
        }
    }
}

fn is_dead_end(level: &DungeonLevel, x: i32, y: i32) -> bool {
    DIRS.iter()
        .filter(|d| {
            let (dx, dy) = d.delta();
            level.walkable(x + dx, y + dy)
        })
        .count()
        == 1
}

/// Breadth-first walk from `from`, returning the walkable cell at the
/// greatest step distance. Ties break toward the lowest row-major index, so
/// the result is a pure function of the level rather than of hash order.
fn furthest_floor_from(level: &DungeonLevel, from: (i32, i32)) -> (i32, i32) {
    let mut dist = vec![u32::MAX; (level.width * level.height) as usize];
    let idx = |x: i32, y: i32| (y * level.width + x) as usize;

    dist[idx(from.0, from.1)] = 0;
    let mut queue = VecDeque::from([from]);
    let (mut best, mut best_dist) = (from, 0);

    while let Some((x, y)) = queue.pop_front() {
        let d = dist[idx(x, y)];
        if d > best_dist {
            best = (x, y);
            best_dist = d;
        }
        for dir in DIRS {
            let (dx, dy) = dir.delta();
            let (nx, ny) = (x + dx, y + dy);
            if !level.walkable(nx, ny) || dist[idx(nx, ny)] != u32::MAX {
                continue;
            }
            dist[idx(nx, ny)] = d + 1;
            queue.push_back((nx, ny));
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floors(level: &DungeonLevel) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in 0..level.height {
            for x in 0..level.width {
                if level.walkable(x, y) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    fn reachable_from(level: &DungeonLevel, from: (i32, i32)) -> Vec<(i32, i32)> {
        let mut seen = vec![from];
        let mut queue = VecDeque::from([from]);
        while let Some((x, y)) = queue.pop_front() {
            for dir in DIRS {
                let (dx, dy) = dir.delta();
                let (nx, ny) = (x + dx, y + dy);
                if level.walkable(nx, ny) && !seen.contains(&(nx, ny)) {
                    seen.push((nx, ny));
                    queue.push_back((nx, ny));
                }
            }
        }
        seen
    }

    fn dead_ends(level: &DungeonLevel) -> usize {
        floors(level)
            .into_iter()
            .filter(|&(x, y)| is_dead_end(level, x, y))
            .count()
    }

    #[test]
    fn the_same_seed_and_depth_yield_an_identical_level() {
        let a = generate(1234, 3);
        let b = generate(1234, 3);
        assert_eq!(floors(&a), floors(&b));
        assert_eq!(a.entry, b.entry);
        assert_eq!(a.stairs_down, b.stairs_down);
    }

    #[test]
    fn different_depths_of_the_same_world_diverge() {
        let a = generate(1234, 1);
        let b = generate(1234, 2);
        assert_ne!(
            floors(&a),
            floors(&b),
            "each depth must be its own level, not the same maze restated"
        );
    }

    #[test]
    fn different_worlds_diverge_at_the_same_depth() {
        assert_ne!(floors(&generate(1, 1)), floors(&generate(2, 1)));
    }

    #[test]
    fn every_walkable_cell_is_reachable_from_the_entry() {
        for depth in 1..=5 {
            let level = generate(99, depth);
            let reached = reachable_from(&level, level.entry);
            let all = floors(&level);
            assert_eq!(
                reached.len(),
                all.len(),
                "depth {depth} stranded {} cells behind solid rock",
                all.len() - reached.len()
            );
        }
    }

    #[test]
    fn the_stairs_down_are_placed_and_reachable() {
        for depth in 1..=5 {
            let level = generate(7, depth);
            assert_eq!(
                level.cell(level.stairs_down.0, level.stairs_down.1),
                CellKind::StairsDown
            );
            assert_ne!(
                level.stairs_down, level.entry,
                "depth {depth} put the way down on top of the way in"
            );
            assert!(reachable_from(&level, level.entry).contains(&level.stairs_down));
        }
    }

    #[test]
    fn the_entry_holds_the_stairs_up() {
        let level = generate(7, 2);
        assert_eq!(level.cell(level.entry.0, level.entry.1), CellKind::StairsUp);
    }

    #[test]
    fn braiding_removes_dead_ends_a_perfect_maze_would_leave() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut level = DungeonLevel {
            width: LEVEL_SIZE,
            height: LEVEL_SIZE,
            cells: vec![CellKind::Rock; (LEVEL_SIZE * LEVEL_SIZE) as usize],
            entry: (1, 1),
            stairs_down: (1, 1),
        };
        carve_maze(&mut level, &mut rng);
        let before = dead_ends(&level);
        braid(&mut level, &mut rng);
        let after = dead_ends(&level);
        assert!(before > 0, "a perfect maze should have dead ends to remove");
        assert!(
            after < before,
            "braiding left {after} of {before} dead ends — no loops were opened"
        );
    }

    #[test]
    fn out_of_bounds_reads_as_solid_rock() {
        let level = generate(1, 1);
        assert_eq!(level.cell(-1, 5), CellKind::Rock);
        assert_eq!(level.cell(5, -1), CellKind::Rock);
        assert_eq!(level.cell(level.width, 5), CellKind::Rock);
        assert_eq!(level.cell(5, level.height), CellKind::Rock);
        assert!(!level.walkable(-1, -1));
    }

    #[test]
    fn the_level_is_walled_in() {
        let level = generate(5, 1);
        for i in 0..level.width {
            assert!(!level.walkable(i, 0), "top edge leaks at {i}");
            assert!(
                !level.walkable(i, level.height - 1),
                "bottom edge leaks at {i}"
            );
            assert!(!level.walkable(0, i), "left edge leaks at {i}");
            assert!(
                !level.walkable(level.width - 1, i),
                "right edge leaks at {i}"
            );
        }
    }

    #[test]
    fn turning_four_times_returns_the_original_facing() {
        for dir in DIRS {
            assert_eq!(dir.turn_left().turn_left().turn_left().turn_left(), dir);
            assert_eq!(dir.turn_right().turn_right().turn_right().turn_right(), dir);
        }
    }

    #[test]
    fn turning_left_and_right_are_inverses() {
        for dir in DIRS {
            assert_eq!(dir.turn_left().turn_right(), dir);
            assert_eq!(dir.turn_right().turn_left(), dir);
        }
    }

    #[test]
    fn north_is_negative_y_matching_the_top_down_renderer() {
        assert_eq!(Dir::North.delta(), (0, -1));
        assert_eq!(Dir::South.delta(), (0, 1));
        assert_eq!(Dir::East.delta(), (1, 0));
        assert_eq!(Dir::West.delta(), (-1, 0));
    }

    #[test]
    fn the_right_hand_is_ninety_degrees_clockwise() {
        assert_eq!(Dir::North.right_delta(), Dir::East.delta());
        assert_eq!(Dir::East.right_delta(), Dir::South.delta());
        assert_eq!(Dir::South.right_delta(), Dir::West.delta());
        assert_eq!(Dir::West.right_delta(), Dir::North.delta());
    }
}
