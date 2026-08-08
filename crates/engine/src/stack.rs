//! The Stack's frame model and its generator.
//!
//! Pure data and pure functions — no ECS, no `Game`. A frame is never
//! persisted: it regenerates deterministically from `(world seed, depth)`,
//! exactly the way `world::WorldMap` regenerates terrain from its seed. The
//! save carries only where in the Stack the player is standing.

use std::collections::VecDeque;

use crate::tuning::{
    STACK_BREAKPOINTS_PER_FRAME, STACK_CACHES_PER_FRAME, STACK_CORRUPTION_PATCH_CELLS,
    STACK_CORRUPTION_PATCHES_PER_FRAME, STACK_DOORS_PER_FRAME, STACK_FAULTS_PER_FRAME,
    STACK_ORPHANS_PER_FRAME,
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

/// Side length of a generated frame, in cells. Odd, because the maze carver
/// below puts cells on odd coordinates and walls on even ones — 21 gives a
/// 10x10 lattice of cells inside a solid border.
const FRAME_SIZE: i32 = 21;

/// Percent of dead ends the braid pass opens back up into the rest of the
/// maze. A perfect maze is *all* dead ends, which is tedious to walk and
/// reads as noise from a first-person view; loops are what make a frame feel
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
/// than an ad-hoc list so a given seed always produces the same frame.
const DIRS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// Solid. Not walkable, and what every out-of-bounds read returns.
    Rock,
    Floor,
    /// Back the way you came — the frame's `entry`. On depth 1 this leads
    /// out to the surface.
    LinkUp,
    LinkDown,
    /// Something worth the walk, sitting in a dead end. Walking onto one
    /// empties it — see `Game::open_cache`. Whether a given cache has
    /// already been emptied is not part of the frame, which regenerates from
    /// its spec; it lives in `resources::FrameMemory::looted`.
    Cache,
    /// The deepest room of the stack, on the bottom frame only, where the
    /// way down would otherwise have been. Walking in starts the boss
    /// fight — see `Game::rouse_lair`. Whether it has already been cleared
    /// lives in `resources::FrameMemory::cleared`, not in the frame.
    Lair,
    /// A doorway. Walkable, but you cannot see past it — which is the whole
    /// reason it exists: a corridor that ends in a door reads as a decision
    /// rather than as more corridor.
    Door,
    /// A door heavy enough that it has to be shouldered open. Seals the lair
    /// off from the rest of the bottom frame, so the guardian is something
    /// you push your way into rather than stumble across.
    ///
    /// Costs nothing but noise (`TRACE_PER_SEAL`) — it is a barrier rather
    /// than a lock, and there is no key.
    ///
    /// Walkable as far as the frame is concerned — connectivity, dead-end
    /// detection and placing the way down all have to see through it, or the
    /// generator would treat a whole sealed wing as unreachable. Forcing one
    /// is `Game::step`'s business, and whether this one already stands open
    /// lives in `FrameMemory::opened`.
    SealedDoor,
    /// An exposed debug port. Walking onto one maps the whole frame at a
    /// stroke — and tells the stack exactly where you are, which is the
    /// single loudest thing the party can do (`TRACE_PER_BREAKPOINT`).
    ///
    /// One-shot; which ones have been used lives in
    /// `resources::FrameMemory::jacked`, not in the frame.
    Breakpoint,
    /// A hole in the floor. Walking onto one drops the party to the frame
    /// below, landing far from that frame's way up — so it is a descent you
    /// pay for with the walk back, rather than a free one.
    ///
    /// Never generated on the bottom frame, which has nothing below it to
    /// fall into. Not one-shot: it is terrain, and it works every time.
    Fault,
    /// Rotten substrate. Standing on it costs the player HP
    /// (`Game::bleed_corruption`), so a corrupted stretch of corridor is a
    /// route you can decide to walk around — the reason this exists at all,
    /// in a maze that otherwise has exactly one kind of walkable cell.
    Corruption,
    /// A program left running down here with nothing left to serve. Sits in
    /// a dead end, and joins the roster for a taming catalyst rather than
    /// for a won capture roll — see `Game::adopt_orphan`. Taken on a key,
    /// never on arrival: a cache costs nothing to walk into, and an orphan
    /// costs a consumable.
    ///
    /// There is no creature here until it is adopted. What species this one
    /// would be is a function of the frame spec (`Game::orphan_species`),
    /// and whether it has already been taken lives in
    /// `resources::FrameMemory::adopted`, not in the frame.
    Orphan,
}

impl CellKind {
    pub fn walkable(self) -> bool {
        !matches!(self, CellKind::Rock)
    }

    /// Whether this cell stops the view cone. Rock does the obvious way;
    /// a door does it by being shut.
    ///
    /// Phase 3's three kinds are all deliberately absent: a cell that is both
    /// walkable and sight-blocking fills the first-person view with its own
    /// face and truncates the map to the party's own row, which is the trap
    /// doors sprang and both cone consumers now carry an explicit `ahead == 0`
    /// exception for. See `the_new_cell_kinds_are_walkable_and_see_through`.
    pub fn blocks_sight(self) -> bool {
        matches!(self, CellKind::Rock | CellKind::Door | CellKind::SealedDoor)
    }
}

/// Everything a frame is a function of.
///
/// A struct rather than four positional arguments because the arguments are
/// all integers and three of them are interchangeable at a glance, which is
/// exactly the shape of call that ends up transposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSpec {
    pub world_seed: u32,
    /// The surface tile of the link this stack hangs from. Part of the
    /// seed, so two links in a sector are two different stacks rather
    /// than two doors onto the same maze.
    pub entrance: (i32, i32),
    /// 1 immediately below the surface, counting up as you descend.
    pub depth: u32,
    /// How many frames this stack runs before it bottoms out — see
    /// `frames_for`.
    pub frames: u32,
}

impl FrameSpec {
    /// The last frame of the stack, which has no way down.
    pub fn is_bottom(self) -> bool {
        self.depth >= self.frames
    }

    /// Mixes the whole spec down to one RNG seed.
    ///
    /// An FNV-1a pass rather than shifting the parts into disjoint bit
    /// ranges: adjacent links differ in a single low bit of one
    /// coordinate far more often than they differ anywhere else, and frames
    /// carved from seeds that close should not be able to rhyme.
    ///
    /// `pub(crate)` so anything else that has to be a stable property of a
    /// particular stack — which program guards its lair, say — can salt this
    /// rather than invent a second scheme that could collide with it.
    pub(crate) fn rng_seed(self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            self.world_seed as u64,
            self.entrance.0 as u32 as u64,
            self.entrance.1 as u32 as u64,
            self.depth as u64,
        ] {
            h ^= word;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: i32,
    pub height: i32,
    cells: Vec<CellKind>,
    /// Where the party arrives, and where `LinkUp` sits.
    pub entry: (i32, i32),
    /// `None` on the bottom frame of a stack — the point of a stack having
    /// a bottom is that there is nowhere further to go.
    pub link_down: Option<(i32, i32)>,
}

impl Frame {
    /// Out of bounds reads as `Rock`, so callers walking a view cone past
    /// the edge of the frame don't each need their own bounds check.
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

/// Builds the frame `spec` describes.
///
/// Deterministic in the spec and nothing else. The RNG is seeded locally
/// rather than drawn from `resources::GameRng`: that generator's stream
/// position is not persisted, so drawing from it would regenerate a
/// *different* frame after a save/load, and the party would find itself
/// inside solid rock.
pub fn generate(spec: FrameSpec) -> Frame {
    let mut rng = StdRng::seed_from_u64(spec.rng_seed());

    let mut level = Frame {
        width: FRAME_SIZE,
        height: FRAME_SIZE,
        cells: vec![CellKind::Rock; (FRAME_SIZE * FRAME_SIZE) as usize],
        entry: (1, 1),
        link_down: None,
    };

    carve_maze(&mut level, &mut rng);
    braid(&mut level, &mut rng);

    // The far cell earns its place either way: the way down on a frame that
    // has one, and on the bottom frame the lair — the deepest room of the
    // whole stack, and the only place the thing guarding it could sensibly
    // be. Placed before `place_caches`, which only ever builds on plain
    // floor and so cannot pave over either.
    let far = furthest_floor_from(&level, level.entry);
    if spec.is_bottom() {
        level.set(far.0, far.1, CellKind::Lair);
    } else {
        level.link_down = Some(far);
        level.set(far.0, far.1, CellKind::LinkDown);
    }
    level.set(level.entry.0, level.entry.1, CellKind::LinkUp);

    if spec.is_bottom() {
        seal_the_lair(&mut level, far);
    }
    place_doors(&mut level, &mut rng);
    // Phase 3's three, between the doors and the caches. Each takes its own
    // kind of site — a junction, then open floor — and none takes a dead
    // end, so `place_caches` still sees every dead end it would have seen
    // without them. All three are walkable, so they don't change what
    // `is_dead_end` reports either. See
    // `the_new_passes_leave_the_cache_count_alone`.
    place_breakpoint(&mut level, &mut rng);
    if !spec.is_bottom() {
        place_faults(&mut level, &mut rng);
    }
    place_corruption(&mut level, &mut rng);
    place_caches(&mut level, &mut rng);
    // After the caches, and the only pass that wants the same site type
    // they do. A cache is no longer `Floor`, so re-scanning excludes one
    // for free — the two passes stay uncoupled and neither has to know the
    // other's count. See `an_orphan_sits_in_a_dead_end_the_caches_left`.
    place_orphan(&mut level, &mut rng);

    level
}

/// Collects every plain `Floor` cell matching `wanted`, shuffled.
///
/// Shared by the three phase-3 passes so each one is its predicate and
/// nothing else. Fisher-Yates over a row-major scan, so which sites get
/// picked is a function of the seed rather than of iteration order — the
/// same guarantee `place_caches` and `place_doors` make, and the reason
/// `the_same_spec_places_every_kind_identically` holds.
fn shuffled_floor(
    level: &Frame,
    rng: &mut StdRng,
    wanted: impl Fn(&Frame, i32, i32) -> bool,
) -> Vec<(i32, i32)> {
    let mut sites: Vec<(i32, i32)> = Vec::new();
    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if level.cell(x, y) == CellKind::Floor && wanted(level, x, y) {
                sites.push((x, y));
            }
        }
    }
    for i in (1..sites.len()).rev() {
        sites.swap(i, rng.random_range(0..=i));
    }
    sites
}

/// How many walkable neighbours a cell has.
fn exits(level: &Frame, x: i32, y: i32) -> usize {
    DIRS.into_iter()
        .filter(|dir| {
            let (dx, dy) = dir.delta();
            level.walkable(x + dx, y + dy)
        })
        .count()
}

/// Puts the frame's breakpoint on a junction.
///
/// A junction rather than a dead end, which would have been the obvious home
/// for something worth walking to: `place_caches` owns dead ends, and taking
/// one would cost the frame a cache. A hub also reads better for a thing
/// that is exposed infrastructure rather than someone's stashed loot.
///
/// Falls back to nothing at all if the braid left no junction — a frame with
/// no breakpoint is a frame you map on foot, which is the game as it was.
fn place_breakpoint(level: &mut Frame, rng: &mut StdRng) {
    let sites = shuffled_floor(level, rng, |l, x, y| exits(l, x, y) >= 3);
    for &(x, y) in sites.iter().take(STACK_BREAKPOINTS_PER_FRAME) {
        level.set(x, y, CellKind::Breakpoint);
    }
}

/// Drops holes through the floor, on open corridor rather than in dead ends.
///
/// Never called on the bottom frame — see `generate`. A dead end is excluded
/// for the same reason `place_breakpoint` avoids one: those belong to caches.
fn place_faults(level: &mut Frame, rng: &mut StdRng) {
    let sites = shuffled_floor(level, rng, |l, x, y| !is_dead_end(l, x, y));
    for &(x, y) in sites.iter().take(STACK_FAULTS_PER_FRAME) {
        level.set(x, y, CellKind::Fault);
    }
}

/// Grows `STACK_CORRUPTION_PATCHES_PER_FRAME` stretches of rotten substrate,
/// each `STACK_CORRUPTION_PATCH_CELLS` long.
///
/// Contiguous, and that is the entire point rather than a detail of how it
/// is written: a single corrupted cell is a toll you pay without a decision,
/// where a stretch of three is something a player can look at and route
/// around. See `corruption_arrives_in_contiguous_patches`.
///
/// Each patch is a seed cell plus a walk along plain-floor neighbours,
/// preferring the neighbour the shuffled order offers first. A patch that
/// runs out of room short of its full length is abandoned whole rather than
/// left stunted, so the count test means what it says.
fn place_corruption(level: &mut Frame, rng: &mut StdRng) {
    let seeds = shuffled_floor(level, rng, |l, x, y| !is_dead_end(l, x, y));

    let mut grown = 0;
    for &seed in &seeds {
        if grown == STACK_CORRUPTION_PATCHES_PER_FRAME {
            break;
        }
        // Re-checked because an earlier patch may have grown over this seed.
        if level.cell(seed.0, seed.1) != CellKind::Floor {
            continue;
        }

        let mut patch = vec![seed];
        while patch.len() < STACK_CORRUPTION_PATCH_CELLS {
            let mut order = DIRS;
            for i in (1..order.len()).rev() {
                order.swap(i, rng.random_range(0..=i));
            }
            let next = patch.iter().find_map(|&(x, y)| {
                order.iter().find_map(|dir| {
                    let (dx, dy) = dir.delta();
                    let cell = (x + dx, y + dy);
                    let free = level.cell(cell.0, cell.1) == CellKind::Floor
                        && !patch.contains(&cell)
                        && !is_dead_end(level, cell.0, cell.1);
                    free.then_some(cell)
                })
            });
            match next {
                Some(cell) => patch.push(cell),
                // Boxed in. Abandon this patch rather than ship a short one.
                None => break,
            }
        }
        if patch.len() < STACK_CORRUPTION_PATCH_CELLS {
            continue;
        }
        for (x, y) in patch {
            level.set(x, y, CellKind::Corruption);
        }
        grown += 1;
    }
}

/// Walls the lair off behind sealed doors.
///
/// Every open neighbour gets one, rather than picking a single cut vertex on
/// the route in: `braid` leaves loops, so one door on the shortest path is
/// no guarantee there isn't a way round it, and the analysis to find a true
/// cut vertex would be a lot of machinery to reach the same place. The lair
/// sits at the end of the longest walk in the frame, so it rarely has more
/// than one or two ways in anyway.
fn seal_the_lair(level: &mut Frame, lair: (i32, i32)) {
    for dir in DIRS {
        let (dx, dy) = dir.delta();
        let (nx, ny) = (lair.0 + dx, lair.1 + dy);
        if level.cell(nx, ny) == CellKind::Floor {
            level.set(nx, ny, CellKind::SealedDoor);
        }
    }
}

/// Hangs plain doors in corridors — cells with exactly two exits, opposite
/// each other, which is what a doorway looks like.
///
/// A junction never gets one: a door you cannot see past, in a cell with
/// three ways out of it, hides two choices behind one wall.
fn place_doors(level: &mut Frame, rng: &mut StdRng) {
    let mut corridors: Vec<(i32, i32)> = Vec::new();
    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if level.cell(x, y) != CellKind::Floor {
                continue;
            }
            let north = level.walkable(x, y - 1);
            let south = level.walkable(x, y + 1);
            let east = level.walkable(x + 1, y);
            let west = level.walkable(x - 1, y);
            let vertical = north && south && !east && !west;
            let horizontal = east && west && !north && !south;
            if vertical || horizontal {
                corridors.push((x, y));
            }
        }
    }

    for i in (1..corridors.len()).rev() {
        corridors.swap(i, rng.random_range(0..=i));
    }
    for &(x, y) in corridors.iter().take(STACK_DOORS_PER_FRAME) {
        level.set(x, y, CellKind::Door);
    }
}

/// Puts caches in dead ends.
///
/// Dead ends specifically, and not any spare floor cell: `braid` deliberately
/// leaves half of them in place, and a dead end with nothing at the end of it
/// is a corridor that wasted your time. This gives the braid's leftovers the
/// reason to exist that they were missing — walking one is now a bet rather
/// than a mistake.
///
/// Runs last so it can see the way down, which it will not build over: a
/// cache on the way down would be picked up by anyone descending, for free.
fn place_caches(level: &mut Frame, rng: &mut StdRng) {
    let mut ends: Vec<(i32, i32)> = Vec::new();
    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if level.cell(x, y) == CellKind::Floor && is_dead_end(level, x, y) {
                ends.push((x, y));
            }
        }
    }

    // Fisher-Yates over a row-major list, so which dead ends get picked is a
    // pure function of the seed rather than of iteration order.
    for i in (1..ends.len()).rev() {
        ends.swap(i, rng.random_range(0..=i));
    }
    for &(x, y) in ends.iter().take(STACK_CACHES_PER_FRAME) {
        level.set(x, y, CellKind::Cache);
    }
}

/// Leaves a program running in a dead end.
///
/// A dead end for the same reason a cache gets one — a corridor with
/// nothing at the end of it wasted your time — and one the caches did not
/// take, which is what running after them buys. A frame short of dead ends
/// places fewer, exactly as `place_caches` degrades through `.take()`: a
/// missing orphan is a quiet frame, not a bug worth a panic.
fn place_orphan(level: &mut Frame, rng: &mut StdRng) {
    let ends = shuffled_floor(level, rng, is_dead_end);
    for &(x, y) in ends.iter().take(STACK_ORPHANS_PER_FRAME) {
        level.set(x, y, CellKind::Orphan);
    }
}

/// Recursive-backtracker maze carver. Cells sit on odd coordinates; carving
/// to a neighbour two cells away also clears the wall between them, so the
/// result is a perfect maze — fully connected, with exactly one route
/// between any two cells. `braid` then relaxes that.
fn carve_maze(level: &mut Frame, rng: &mut StdRng) {
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
fn braid(level: &mut Frame, rng: &mut StdRng) {
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

fn is_dead_end(level: &Frame, x: i32, y: i32) -> bool {
    DIRS.iter()
        .filter(|d| {
            let (dx, dy) = d.delta();
            level.walkable(x + dx, y + dy)
        })
        .count()
        == 1
}

/// Step distance from `from` to every cell, row-major, `u32::MAX` where
/// unreachable.
///
/// Extracted so `furthest_floor_from` and `fault_landing` share one walk
/// rather than each keeping its own copy of the same breadth-first search —
/// two BFS bodies over the same graph is precisely the drift CLAUDE.md's
/// mirroring rule is about.
fn distances_from(level: &Frame, from: (i32, i32)) -> Vec<u32> {
    let mut dist = vec![u32::MAX; (level.width * level.height) as usize];
    let idx = |x: i32, y: i32| (y * level.width + x) as usize;

    dist[idx(from.0, from.1)] = 0;
    let mut queue = VecDeque::from([from]);
    while let Some((x, y)) = queue.pop_front() {
        let d = dist[idx(x, y)];
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
    dist
}

/// The walkable cell at the greatest step distance from `from`. Ties break
/// toward the lowest row-major index, so the result is a pure function of
/// the frame rather than of hash order.
fn furthest_floor_from(level: &Frame, from: (i32, i32)) -> (i32, i32) {
    let dist = distances_from(level, from);
    let mut best = (from, 0);
    for y in 0..level.height {
        for x in 0..level.width {
            let d = dist[(y * level.width + x) as usize];
            if d != u32::MAX && d > best.1 {
                best = ((x, y), d);
            }
        }
    }
    best.0
}

/// Where a party falling into this frame through a fault comes down.
///
/// Plain `Floor` in the **far half** of the frame, measured from `entry` —
/// which is the frame's way up, so a fall always costs a walk back. `Floor`
/// specifically, so a fall can never deposit the party on the lair, a cache,
/// or another fault.
///
/// Picked from a stream salted off the frame's own seed rather than taken as
/// "the furthest cell", which would land every fall in the same corner and
/// always beside the way further down. `None` if the frame somehow offers no
/// far-half floor, which leaves the caller to fall back on the entry.
pub(crate) fn fault_landing(level: &Frame, spec: FrameSpec) -> Option<(i32, i32)> {
    const FALL_SALT: u64 = 0xFA11_1E15;

    let dist = distances_from(level, level.entry);
    let reach = dist.iter().filter(|&&d| d != u32::MAX).max().copied()?;

    let mut far: Vec<(i32, i32)> = Vec::new();
    for y in 0..level.height {
        for x in 0..level.width {
            let d = dist[(y * level.width + x) as usize];
            if level.cell(x, y) == CellKind::Floor && d != u32::MAX && d * 2 >= reach {
                far.push((x, y));
            }
        }
    }

    let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ FALL_SALT);
    (!far.is_empty()).then(|| far[rng.random_range(0..far.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floors(level: &Frame) -> Vec<(i32, i32)> {
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

    fn reachable_from(level: &Frame, from: (i32, i32)) -> Vec<(i32, i32)> {
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

    fn dead_ends(level: &Frame) -> usize {
        floors(level)
            .into_iter()
            .filter(|&(x, y)| is_dead_end(level, x, y))
            .count()
    }

    /// A spec deep in a stack with plenty of room left below it, so tests
    /// that aren't about the bottom don't accidentally land on one.
    fn spec(world_seed: u32, depth: u32) -> FrameSpec {
        FrameSpec {
            world_seed,
            entrance: (0, 0),
            depth,
            frames: 9,
        }
    }

    #[test]
    fn the_same_spec_yields_an_identical_frame() {
        let a = generate(spec(1234, 3));
        let b = generate(spec(1234, 3));
        assert_eq!(floors(&a), floors(&b));
        assert_eq!(a.entry, b.entry);
        assert_eq!(a.link_down, b.link_down);
    }

    #[test]
    fn different_depths_of_the_same_world_diverge() {
        let a = generate(spec(1234, 1));
        let b = generate(spec(1234, 2));
        assert_ne!(
            floors(&a),
            floors(&b),
            "each depth must be its own frame, not the same maze restated"
        );
    }

    #[test]
    fn different_worlds_diverge_at_the_same_depth() {
        assert_ne!(floors(&generate(spec(1, 1))), floors(&generate(spec(2, 1))));
    }

    /// Two links in one sector must be two stacks. Without the
    /// entrance tile in the seed every hole in the ground opened onto the
    /// same maze, and walking to a distant one bought nothing.
    #[test]
    fn links_on_different_tiles_diverge_at_the_same_depth() {
        let here = FrameSpec {
            entrance: (12, -40),
            ..spec(5, 1)
        };
        let there = FrameSpec {
            entrance: (13, -40),
            ..spec(5, 1)
        };
        assert_ne!(
            floors(&generate(here)),
            floors(&generate(there)),
            "adjacent links carved the same maze"
        );
    }

    #[test]
    fn every_walkable_cell_is_reachable_from_the_entry() {
        for depth in 1..=5 {
            let level = generate(spec(99, depth));
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
    fn the_link_down_is_placed_and_reachable() {
        for depth in 1..=5 {
            let level = generate(spec(7, depth));
            let down = level.link_down.expect("depth {depth} has room below it");
            assert_eq!(level.cell(down.0, down.1), CellKind::LinkDown);
            assert_ne!(
                down, level.entry,
                "depth {depth} put the way down on top of the way in"
            );
            assert!(reachable_from(&level, level.entry).contains(&down));
        }
    }

    #[test]
    fn the_bottom_frame_of_a_stack_has_no_way_down() {
        let level = generate(FrameSpec {
            world_seed: 7,
            entrance: (3, 4),
            depth: 4,
            frames: 4,
        });
        assert_eq!(level.link_down, None);
        assert!(
            !floors(&level)
                .into_iter()
                .any(|(x, y)| level.cell(x, y) == CellKind::LinkDown),
            "the bottom frame laid a way down into nothing"
        );
    }

    #[test]
    fn the_entry_holds_the_link_up() {
        let level = generate(spec(7, 2));
        assert_eq!(level.cell(level.entry.0, level.entry.1), CellKind::LinkUp);
    }

    #[test]
    fn braiding_removes_dead_ends_a_perfect_maze_would_leave() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut level = Frame {
            width: FRAME_SIZE,
            height: FRAME_SIZE,
            cells: vec![CellKind::Rock; (FRAME_SIZE * FRAME_SIZE) as usize],
            entry: (1, 1),
            link_down: None,
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
        let level = generate(spec(1, 1));
        assert_eq!(level.cell(-1, 5), CellKind::Rock);
        assert_eq!(level.cell(5, -1), CellKind::Rock);
        assert_eq!(level.cell(level.width, 5), CellKind::Rock);
        assert_eq!(level.cell(5, level.height), CellKind::Rock);
        assert!(!level.walkable(-1, -1));
    }

    #[test]
    fn the_frame_is_walled_in() {
        let level = generate(spec(5, 1));
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

    fn cells_of(level: &Frame, kind: CellKind) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in 0..level.height {
            for x in 0..level.width {
                if level.cell(x, y) == kind {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// Every cell of the frame, so determinism can be asserted on the whole
    /// grid rather than only on which cells are walkable. `floors` was
    /// enough while every walkable cell was interchangeable; with three new
    /// kinds it would pass while two frames disagreed about all of them.
    fn grid(level: &Frame) -> Vec<CellKind> {
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .map(|(x, y)| level.cell(x, y))
            .collect()
    }

    #[test]
    fn each_new_kind_generates_within_its_tuning_count() {
        for depth in 1..=5 {
            let level = generate(spec(31, depth));
            assert_eq!(
                cells_of(&level, CellKind::Breakpoint).len(),
                STACK_BREAKPOINTS_PER_FRAME,
                "depth {depth} breakpoints"
            );
            assert_eq!(
                cells_of(&level, CellKind::Fault).len(),
                STACK_FAULTS_PER_FRAME,
                "depth {depth} faults"
            );
            assert_eq!(
                cells_of(&level, CellKind::Corruption).len(),
                STACK_CORRUPTION_PATCHES_PER_FRAME * STACK_CORRUPTION_PATCH_CELLS,
                "depth {depth} corruption"
            );
        }
    }

    /// The whole-grid version of `the_same_spec_yields_an_identical_frame`.
    /// Placement drawing from `GameRng` instead of the local stream would
    /// pass that test and fail this one.
    #[test]
    fn the_same_spec_places_every_kind_identically() {
        assert_eq!(grid(&generate(spec(88, 2))), grid(&generate(spec(88, 2))));
    }

    /// The claim the placement order rests on: each kind has its own site
    /// type, so no new pass can pave over something that was already there.
    #[test]
    fn no_new_kind_lands_on_something_that_was_already_there() {
        for depth in 1..=6 {
            let level = generate(FrameSpec {
                frames: 6,
                ..spec(64, depth)
            });
            let taken: Vec<(i32, i32)> = [
                CellKind::Cache,
                CellKind::Lair,
                CellKind::LinkUp,
                CellKind::LinkDown,
                CellKind::Door,
                CellKind::SealedDoor,
            ]
            .into_iter()
            .flat_map(|k| cells_of(&level, k))
            .collect();
            for kind in [
                CellKind::Breakpoint,
                CellKind::Fault,
                CellKind::Corruption,
                CellKind::Orphan,
            ] {
                for cell in cells_of(&level, kind) {
                    assert!(
                        !taken.contains(&cell),
                        "depth {depth}: {kind:?} at {cell:?} paved over an existing feature"
                    );
                }
            }
        }
    }

    /// Dead ends stay whole. The three phase-3 passes run *before*
    /// `place_caches` and none of them takes a dead end; `place_orphan`
    /// takes one but runs *after*, so it can only ever have the dead ends
    /// the caches left. Either way the cache count is exactly what it was
    /// before phase 3 existed — if a new pass started eating dead ends
    /// first, frames would quietly start shipping two caches instead of
    /// three.
    #[test]
    fn the_new_passes_leave_the_cache_count_alone() {
        for depth in 1..=5 {
            let level = generate(spec(17, depth));
            assert_eq!(
                cells_of(&level, CellKind::Cache).len(),
                STACK_CACHES_PER_FRAME,
                "depth {depth} lost a cache to one of the new passes"
            );
        }
    }

    /// The count is a ceiling, not a promise, and the gap is wide enough to
    /// be worth pinning rather than waving at. `place_orphan` runs after
    /// `place_caches` and wants the same site type, so a frame needs
    /// `STACK_CACHES_PER_FRAME + STACK_ORPHANS_PER_FRAME` plain-floor dead
    /// ends to field one — and measured over this sample, **about three
    /// frames in four do**. A count test asserting one per frame would be
    /// asserting something the generator has never done.
    ///
    /// The floor is deliberately well under the measured rate: this exists
    /// to catch a generator change that quietly stops placing orphans at
    /// all, not to freeze the braid's exact output.
    #[test]
    fn most_frames_place_an_orphan_and_none_places_two() {
        let mut placed = 0;
        let mut frames = 0;
        for world_seed in 0..100 {
            for depth in 1..=6 {
                let level = generate(FrameSpec {
                    frames: 6,
                    ..spec(world_seed, depth)
                });
                let orphans = cells_of(&level, CellKind::Orphan).len();
                assert!(
                    orphans <= STACK_ORPHANS_PER_FRAME,
                    "seed {world_seed} depth {depth} placed {orphans} orphans"
                );
                placed += orphans;
                frames += 1;
            }
        }
        assert!(
            placed * 10 >= frames * 6,
            "only {placed} of {frames} frames left an orphan running — the \
             dead ends the caches leave have dried up"
        );
    }

    /// The claim `place_orphan` running last rests on, and the one a count
    /// test alone would pass while getting wrong: an orphan takes a dead
    /// end, which is the site type `place_caches` owns — so it can only
    /// have the ones the caches did not. A cache is no longer `Floor` by
    /// the time this pass scans, which is the whole mechanism.
    #[test]
    fn an_orphan_sits_in_a_dead_end_the_caches_left() {
        for depth in 1..=6 {
            let level = generate(FrameSpec {
                frames: 6,
                ..spec(64, depth)
            });
            for cell in cells_of(&level, CellKind::Orphan) {
                assert!(
                    is_dead_end(&level, cell.0, cell.1),
                    "depth {depth}: the orphan at {cell:?} is not in a dead end"
                );
            }
        }
    }

    /// A fault on the bottom frame would drop the party into nothing.
    #[test]
    fn the_bottom_frame_generates_no_faults() {
        let level = generate(FrameSpec {
            world_seed: 21,
            entrance: (2, 2),
            depth: 4,
            frames: 4,
        });
        assert!(
            cells_of(&level, CellKind::Fault).is_empty(),
            "the bottom frame laid a hole down into nothing"
        );
    }

    /// Corruption has to arrive as contiguous stretches, not as scattered
    /// cells that merely happen to number six. A count test alone passes
    /// either way, and the scattered version is a toll booth rather than the
    /// routing decision this kind exists to create.
    #[test]
    fn corruption_arrives_in_contiguous_patches() {
        let level = generate(spec(43, 3));
        let cells = cells_of(&level, CellKind::Corruption);
        assert!(!cells.is_empty(), "nothing to check");

        // Flood-fill through corruption only; the patches it finds must each
        // be the tuned size.
        let mut unvisited = cells.clone();
        let mut patches = Vec::new();
        while let Some(start) = unvisited.pop() {
            let mut patch = vec![start];
            let mut queue = VecDeque::from([start]);
            while let Some((x, y)) = queue.pop_front() {
                for dir in DIRS {
                    let (dx, dy) = dir.delta();
                    let next = (x + dx, y + dy);
                    if let Some(i) = unvisited.iter().position(|&c| c == next) {
                        unvisited.remove(i);
                        patch.push(next);
                        queue.push_back(next);
                    }
                }
            }
            patches.push(patch);
        }

        assert_eq!(
            patches.len(),
            STACK_CORRUPTION_PATCHES_PER_FRAME,
            "expected {STACK_CORRUPTION_PATCHES_PER_FRAME} patches, got {:?}",
            patches.iter().map(|p| p.len()).collect::<Vec<_>>()
        );
        for patch in &patches {
            assert_eq!(
                patch.len(),
                STACK_CORRUPTION_PATCH_CELLS,
                "a patch came out the wrong size: {patch:?}"
            );
        }
    }

    /// The door trap, guarded. A cell that is both walkable and
    /// sight-blocking fills the first-person view with its own face and
    /// truncates the map to the party's row — the bug doors shipped with,
    /// which both cone consumers now carry an `ahead == 0` exception for.
    /// Neither phase 3's three kinds nor phase 4's orphan is allowed to
    /// reopen it.
    #[test]
    fn the_new_cell_kinds_are_walkable_and_see_through() {
        for kind in [
            CellKind::Breakpoint,
            CellKind::Fault,
            CellKind::Corruption,
            CellKind::Orphan,
        ] {
            assert!(kind.walkable(), "{kind:?} is not walkable");
            assert!(
                !kind.blocks_sight(),
                "{kind:?} blocks sight — it inherits the door trap, and both \
                 remember_view and draws_as_face need the ahead == 0 exception \
                 before it can ship"
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
