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

/// Rooms are placed by rejection sampling until they cover this much of the
/// frame — an area target rather than a count, because the frame has to hold
/// about as much *place* as a maze does. Encounters roll per step against a
/// fixed three caches, so a layout with half the walkable cells would
/// quietly halve the fights per cache. See
/// `every_layout_fills_the_frame_to_about_the_same_extent`.
///
/// Rejection sampling rather than a BSP split: a BSP leaf holds exactly one
/// room, so the rooms come out evenly spread and the frame reads as a grid.
/// Sampling clusters them, which is what makes one frame sit differently
/// from the next.
///
/// Inline rather than in `tuning.rs` for the reason `BRAID_PERCENT` is —
/// this is the shape of generated content, not a difficulty knob.
const ROOM_AREA_TARGET: i32 = 165;
const ROOM_ATTEMPTS: usize = 400;

/// Corridors carved between rooms the spanning tree already joined. These
/// are the loops: a spanning tree alone is a perfect maze with bigger cells,
/// and walking one is the weave this layout exists to stop. They are also
/// where the layout makes up the density its rooms cannot.
const EXTRA_ROOM_LINKS: usize = 5;

/// The `Chambers` layout's four halls, and the width of the corridors
/// joining them.
///
/// Six is what the density band allows once the corridors are paid for, and
/// it is also the smallest hall that still reads as one: the first-person
/// view reaches four cells, so a six-wide room is the point at which the far
/// wall stops being part of the same glance as the near one.
///
/// The corridors are three wide because a one-cell hole between two halls is
/// a doorway, and this is the layout you cross without looking for one. It
/// is also the width the halls' overlap always affords — quadrants are nine
/// cells across and hold a six-cell hall, so two neighbouring halls share at
/// least three rows however they jitter.
const HALL_SIZE: i32 = 6;
const CHAMBER_GAP_WIDTH: i32 = 3;

/// Percent of dead ends the braid pass opens back up into the rest of the
/// maze. A perfect maze is *all* dead ends, which is tedious to walk and
/// reads as noise from a first-person view; loops are what make a frame feel
/// like a place rather than a puzzle box. Not in `tuning.rs` for the same
/// reason `world::WorldMap::classify` keeps its noise thresholds inline —
/// this is the shape of generated content, not a difficulty knob.
///
/// Raising it was tried and reverted. At 100 the maze reaches 13 independent
/// loops instead of 8 — still nowhere near the hundred-odd the open layouts
/// carry — and it pays for them twice over: opening every dead end also
/// consumes the rock cells `stub_sites` needs, so the frame can no longer
/// carve the spurs its caches and orphan live in. The maze is the tight
/// layout of the three, and that is a choice rather than an oversight. See
/// `every_layout_offers_more_than_one_route`.
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

/// Which shape a frame is carved in.
///
/// Rolled per frame off `FrameSpec::rng_seed`, which is why it needs no
/// save field and no second seeding scheme: the layout is a property of the
/// spec, so it survives a save/load exactly the way the maze itself does.
///
/// Dispatched by an exhaustive `match` in `generate` rather than through a
/// trait or a table of function pointers, for the reason `render/stack.rs`'s
/// `cell_mark` is exhaustive: a fourth variant must fail to compile until
/// someone decides how it carves, and a wildcard or a lookup would let one
/// be added and silently never dispatched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameLayout {
    /// The original: a perfect maze, braided back open.
    Maze,
    /// Rectangular rooms joined by corridors, with extra corridors added
    /// between rooms that the spanning tree left unjoined.
    Rooms,
    /// A few large open chambers separated by short wall runs with wide
    /// gaps punched through them.
    Chambers,
}

impl FrameLayout {
    /// The frame's layout, and deliberately the generator's *first* draw —
    /// so the layouts do not each carve from a stream position that depends
    /// on which one was chosen.
    fn roll(rng: &mut StdRng) -> Self {
        match rng.random_range(0..3) {
            0 => FrameLayout::Maze,
            1 => FrameLayout::Rooms,
            _ => FrameLayout::Chambers,
        }
    }
}

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

    /// Continues `rng_seed`'s FNV-1a fold with further words, so anything
    /// that must be a stable property of a *cell* of a stack salts off the
    /// one scheme rather than inventing a second that could collide with it.
    ///
    /// Each word is multiplied through the FNV prime rather than XOR-ed in
    /// once, so `[a, b]` and `[b, a]` diverge and two adjacent cells cannot
    /// rhyme — the same argument `rng_seed` makes about adjacent links.
    ///
    /// `LAIR_SALT`, `ORPHAN_SALT` and `FALL_SALT` are deliberately **not**
    /// migrated onto this: each answers one question per frame, a single XOR
    /// is sufficient there, and all three are pinned by tests.
    pub(crate) fn salted(self, words: &[u64]) -> u64 {
        let mut h = self.rng_seed();
        for &word in words {
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
    /// Which shape this frame was carved in. Rolled from the spec, so it is
    /// regenerated rather than saved.
    pub layout: FrameLayout,
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
    let layout = FrameLayout::roll(&mut rng);

    let mut level = Frame {
        width: FRAME_SIZE,
        height: FRAME_SIZE,
        cells: vec![CellKind::Rock; (FRAME_SIZE * FRAME_SIZE) as usize],
        entry: (1, 1),
        link_down: None,
        layout,
    };

    match layout {
        FrameLayout::Maze => {
            carve_maze(&mut level, &mut rng);
            braid(&mut level, &mut rng);
        }
        FrameLayout::Rooms => carve_rooms(&mut level, &mut rng),
        FrameLayout::Chambers => carve_chambers(&mut level, &mut rng),
    }

    // The way up goes down first, before anything counts dead ends. A maze
    // entry is the lattice corner and usually *is* a dead end, so a stub
    // pass that ran first would count it as a cache site and then watch this
    // line pave over it — one frame in several quietly losing its orphan.
    level.set(level.entry.0, level.entry.1, CellKind::LinkUp);

    // Runs before anything is sited, so every pass below measures the frame
    // the player will actually walk. Carving spurs afterwards would move the
    // distances the far-half pick is made from, and
    // `the_way_down_is_far_without_always_being_the_furthest_cell` measures
    // the finished frame.
    guarantee_loot_stubs(&mut level, &mut rng);

    // The far cell earns its place either way: the way down on a frame that
    // has one, and on the bottom frame the lair — the deepest room of the
    // whole stack, and the only place the thing guarding it could sensibly
    // be. Placed before `place_caches`, which only ever builds on plain
    // floor and so cannot pave over either.
    //
    // Dead ends are excluded from both. A spur exists to hold a cache, and
    // the way down at the end of one is a blind alley that turns out to be
    // the exit; for the lair it would also cost `place_caches` a site.
    let far = far_site(&level, &mut rng, spec.is_bottom());
    if spec.is_bottom() {
        level.set(far.0, far.1, CellKind::Lair);
    } else {
        level.link_down = Some(far);
        level.set(far.0, far.1, CellKind::LinkDown);
    }

    if spec.is_bottom() {
        seal_the_lair(&mut level, far);
        // Topped up, because the seal turns every open neighbour of the lair
        // into a `SealedDoor` and one of those can be a spur tip — which
        // costs the bottom frame the cache or orphan that spur was carved
        // for. A no-op on the frames where it took none.
        guarantee_loot_stubs(&mut level, &mut rng);
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

/// A rectangle of floor, in frame coordinates.
#[derive(Clone, Copy)]
struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Room {
    fn centre(self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// Whether the two rooms would meet with no wall between them. Grown by
    /// a cell on every side, so two rooms always have at least one rock cell
    /// separating them — a corridor has to be carved to join them, which is
    /// what keeps the layout from collapsing into one open blob.
    fn touches(self, other: Room) -> bool {
        self.x <= other.x + other.w
            && other.x <= self.x + self.w
            && self.y <= other.y + other.h
            && other.y <= self.y + self.h
    }
}

fn manhattan(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Carves a straight run of floor. One of the two axes must not change —
/// `carve_corridor` is what guarantees that by splitting every route into
/// two legs.
fn carve_line(level: &mut Frame, from: (i32, i32), to: (i32, i32)) {
    let (dx, dy) = ((to.0 - from.0).signum(), (to.1 - from.1).signum());
    let (mut x, mut y) = from;
    loop {
        level.set(x, y, CellKind::Floor);
        if (x, y) == to {
            return;
        }
        x += dx;
        y += dy;
    }
}

/// Joins two points with an L, turning either at the start or at the end.
/// Which one is a coin flip, so a frame's corridors don't all elbow the
/// same way.
fn carve_corridor(level: &mut Frame, from: (i32, i32), to: (i32, i32), rng: &mut StdRng) {
    let elbow = if rng.random_range(0..2) == 0 {
        (to.0, from.1)
    } else {
        (from.0, to.1)
    };
    carve_line(level, from, elbow);
    carve_line(level, elbow, to);
}

/// Rectangular rooms joined by corridors, with extra corridors on top.
///
/// The spanning tree is what makes `every_walkable_cell_is_reachable_from_
/// the_entry` hold without a repair pass: every room is joined to one
/// already joined, so the frame is connected by construction rather than by
/// inspection afterwards.
fn carve_rooms(level: &mut Frame, rng: &mut StdRng) {
    let mut rooms: Vec<Room> = Vec::new();
    let mut area = 0;
    for _ in 0..ROOM_ATTEMPTS {
        if area >= ROOM_AREA_TARGET {
            break;
        }
        let (w, h) = (rng.random_range(3..=7), rng.random_range(3..=7));
        let room = Room {
            x: rng.random_range(1..=level.width - 1 - w),
            y: rng.random_range(1..=level.height - 1 - h),
            w,
            h,
        };
        if rooms.iter().any(|&other| room.touches(other)) {
            continue;
        }
        area += w * h;
        rooms.push(room);
    }

    for room in &rooms {
        for y in room.y..room.y + room.h {
            for x in room.x..room.x + room.w {
                level.set(x, y, CellKind::Floor);
            }
        }
    }

    // Prim over room centres, joining the closest loose room to the tree
    // each pass. Ties break to the first found in a fixed scan, so the
    // result is a function of the seed and not of iteration order.
    let mut joined = vec![0usize];
    let mut loose: Vec<usize> = (1..rooms.len()).collect();
    while !loose.is_empty() {
        let mut best = (0usize, 0usize, i32::MAX);
        for (slot, &l) in loose.iter().enumerate() {
            for &j in &joined {
                let d = manhattan(rooms[l].centre(), rooms[j].centre());
                if d < best.2 {
                    best = (slot, j, d);
                }
            }
        }
        let l = loose.remove(best.0);
        carve_corridor(level, rooms[l].centre(), rooms[best.1].centre(), rng);
        joined.push(l);
    }

    // A seed whose rooms packed badly makes the shortfall up in corridors
    // rather than in attempts geometry was never going to satisfy. Each
    // extra link nets roughly a dozen cells, and buys another loop with
    // them — which is the one place this layout can spend area well.
    let extra = EXTRA_ROOM_LINKS + ((ROOM_AREA_TARGET - area).max(0) / 12) as usize;
    for _ in 0..extra {
        if rooms.len() < 2 {
            break;
        }
        let a = rng.random_range(0..rooms.len());
        let b = rng.random_range(0..rooms.len());
        if a != b {
            carve_corridor(level, rooms[a].centre(), rooms[b].centre(), rng);
        }
    }

    level.entry = rooms[0].centre();
}

/// Joins two halls with a corridor as wide as the layout's gaps.
///
/// Straight, never an L: the halls sit in quadrants, so any two neighbours
/// are either side by side or stacked, and always share at least
/// `CHAMBER_GAP_WIDTH` rows or columns to run the corridor along.
fn join_halls(level: &mut Frame, a: Room, b: Room, rng: &mut StdRng) {
    let w = CHAMBER_GAP_WIDTH;
    if a.x + a.w <= b.x || b.x + b.w <= a.x {
        let lo = a.y.max(b.y);
        let hi = ((a.y + a.h).min(b.y + b.h) - w).max(lo);
        let y = rng.random_range(lo..=hi);
        let (from, to) = if a.x < b.x {
            (a.x + a.w - 1, b.x)
        } else {
            (b.x + b.w - 1, a.x)
        };
        for d in 0..w {
            carve_line(level, (from, y + d), (to, y + d));
        }
    } else {
        let lo = a.x.max(b.x);
        let hi = ((a.x + a.w).min(b.x + b.w) - w).max(lo);
        let x = rng.random_range(lo..=hi);
        let (from, to) = if a.y < b.y {
            (a.y + a.h - 1, b.y)
        } else {
            (b.y + b.h - 1, a.y)
        };
        for d in 0..w {
            carve_line(level, (x + d, from), (x + d, to));
        }
    }
}

/// Four open halls, one per quadrant, joined in a ring by wide corridors.
///
/// A ring rather than a tree: four joins for four halls is exactly one loop,
/// so there are always two ways round to anywhere. That is the whole
/// difference between this and four big rooms.
///
/// The halls are left clear rather than dressed with pillars. Pillars were
/// the obvious way to spend the density this layout would otherwise run over
/// budget on, but they take it out of the one thing the layout is for: a
/// hall broken up by standing rock is a room with an awkward shape, and
/// `the_chambers_layout_carves_halls` stops passing. The density comes out
/// of `HALL_SIZE` instead, where it costs nothing that matters.
fn carve_chambers(level: &mut Frame, rng: &mut StdRng) {
    let (split_x, split_y) = (level.width / 2, level.height / 2);
    let quadrants = [
        (1, 1, split_x - 1, split_y - 1),
        (split_x + 1, 1, level.width - 2, split_y - 1),
        (1, split_y + 1, split_x - 1, level.height - 2),
        (split_x + 1, split_y + 1, level.width - 2, level.height - 2),
    ];

    let mut halls = Vec::new();
    for (x0, y0, x1, y1) in quadrants {
        let (w, h) = (HALL_SIZE.min(x1 - x0 + 1), HALL_SIZE.min(y1 - y0 + 1));
        let hall = Room {
            x: x0 + rng.random_range(0..=x1 - x0 + 1 - w),
            y: y0 + rng.random_range(0..=y1 - y0 + 1 - h),
            w,
            h,
        };
        for y in hall.y..hall.y + hall.h {
            for x in hall.x..hall.x + hall.w {
                level.set(x, y, CellKind::Floor);
            }
        }
        halls.push(hall);
    }

    for (a, b) in [(0, 1), (1, 3), (3, 2), (2, 0)] {
        join_halls(level, halls[a], halls[b], rng);
    }

    level.entry = halls[rng.random_range(0..halls.len())].centre();
}

/// How many plain-floor dead ends the frame currently holds — the site type
/// `place_caches` and `place_orphan` compete for.
fn plain_dead_ends(level: &Frame) -> usize {
    let mut count = 0;
    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if level.cell(x, y) == CellKind::Floor && is_dead_end(level, x, y) {
                count += 1;
            }
        }
    }
    count
}

/// Every rock cell a spur could open from, shuffled.
///
/// A candidate has exactly one walkable neighbour, so carving it makes a
/// dead end rather than a shortcut. That neighbour has to be plain `Floor`,
/// which keeps a spur off the lair, either link and a sealed door — and it
/// must not itself be a dead end, or carving would merely move one and the
/// loop below would never terminate.
fn stub_sites(level: &Frame, rng: &mut StdRng) -> Vec<(i32, i32)> {
    let mut sites = Vec::new();
    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if level.cell(x, y) != CellKind::Rock {
                continue;
            }
            let mut open = DIRS.into_iter().filter_map(|dir| {
                let (dx, dy) = dir.delta();
                level.walkable(x + dx, y + dy).then_some((x + dx, y + dy))
            });
            let Some(neighbour) = open.next() else {
                continue;
            };
            if open.next().is_some() {
                continue;
            }
            if level.cell(neighbour.0, neighbour.1) == CellKind::Floor
                && !is_dead_end(level, neighbour.0, neighbour.1)
            {
                sites.push((x, y));
            }
        }
    }
    for i in (1..sites.len()).rev() {
        sites.swap(i, rng.random_range(0..=i));
    }
    sites
}

/// Pushes a freshly carved spur one cell further where there is room.
///
/// Two cells reads as a side passage rather than as an alcove, and hands
/// `place_doors` a corridor site into the bargain — a door on the neck of a
/// spur is exactly the "this ends in a decision" the kind exists for.
fn extend_stub(level: &mut Frame, neck: (i32, i32), rng: &mut StdRng) {
    let mut order = DIRS;
    for i in (1..order.len()).rev() {
        order.swap(i, rng.random_range(0..=i));
    }
    for dir in order {
        let (dx, dy) = dir.delta();
        let (nx, ny) = (neck.0 + dx, neck.1 + dy);
        if nx <= 0 || ny <= 0 || nx >= level.width - 1 || ny >= level.height - 1 {
            continue;
        }
        if level.cell(nx, ny) != CellKind::Rock || exits(level, nx, ny) != 1 {
            continue;
        }
        level.set(nx, ny, CellKind::Floor);
        return;
    }
}

/// Carves spurs until the frame holds enough plain-floor dead ends for
/// `place_caches` and `place_orphan` to both fill.
///
/// One site rule across all three layouts, and that is the whole reason the
/// feature passes below need no layout awareness at all: rooms and chambers
/// have almost no dead ends of their own, and even the maze came up short of
/// the four they want about one frame in four. Without this, a frame quietly
/// ships two caches instead of three and reads as a payout curve that moved.
///
/// Runs after `seal_the_lair` so a spur cannot open inside the sealed
/// region. Nothing between here and `place_caches` consumes a dead end —
/// doors want two opposite exits, breakpoints want three, faults and
/// corruption exclude dead ends outright — so what this carves is what
/// those two passes get. A frame with nowhere left to carve places fewer,
/// which is the same graceful shortfall `.take()` already gives them.
fn guarantee_loot_stubs(level: &mut Frame, rng: &mut StdRng) {
    let wanted = STACK_CACHES_PER_FRAME + STACK_ORPHANS_PER_FRAME;
    while plain_dead_ends(level) < wanted {
        let Some(&(x, y)) = stub_sites(level, rng).first() else {
            return;
        };
        level.set(x, y, CellKind::Floor);
        extend_stub(level, (x, y), rng);
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

/// Every plain-floor cell in the far half of the frame, measured from
/// `from` in steps.
///
/// The one definition of "far", shared by the three things that need one:
/// where the way down goes, where the lair goes, and where a party falling
/// through a fault comes down. It replaced `furthest_floor_from` for the
/// first two — the single furthest cell is the opposite corner every time,
/// which is half of what made a frame read as one long weave — and it was
/// already written out longhand inside `fault_landing`, which is the second
/// copy CLAUDE.md's mirroring rule is about.
///
/// `Floor` specifically, so nothing lands on a cache, the lair, a fault or
/// either link.
fn far_half_floor(level: &Frame, from: (i32, i32)) -> Vec<(i32, i32)> {
    let dist = distances_from(level, from);
    let Some(reach) = dist.iter().filter(|&&d| d != u32::MAX).max().copied() else {
        return Vec::new();
    };

    let mut far = Vec::new();
    for y in 0..level.height {
        for x in 0..level.width {
            let d = dist[(y * level.width + x) as usize];
            if level.cell(x, y) == CellKind::Floor && d != u32::MAX && d * 2 >= reach {
                far.push((x, y));
            }
        }
    }
    far
}

/// Picks the cell the frame's way down — or its lair — is built on.
///
/// Drawn from `far_half_floor` rather than taken as the single furthest
/// cell, which was the opposite corner every time. Dead ends are excluded
/// from both: a spur is there to hold a cache, and the way down at the end
/// of one is a blind alley that turns out to be the exit.
///
/// The lair takes the *least connected* of the candidates, because
/// `seal_the_lair` puts a sealed door on every open neighbour — on a cell
/// with four of them that is a ring of doors standing in the middle of a
/// room, which seals correctly and reads as nonsense. Fewest neighbours
/// makes the seal a doorway.
///
/// Falls back to the furthest cell when a frame offers no far-half floor at
/// all, which keeps the old behaviour as the degenerate case rather than
/// leaving the frame without a way on.
fn far_site(level: &Frame, rng: &mut StdRng, is_lair: bool) -> (i32, i32) {
    let mut sites: Vec<(i32, i32)> = far_half_floor(level, level.entry)
        .into_iter()
        .filter(|&(x, y)| !is_dead_end(level, x, y))
        .collect();
    if sites.is_empty() {
        return furthest_floor_from(level, level.entry);
    }

    if is_lair {
        let fewest = sites
            .iter()
            .map(|&(x, y)| exits(level, x, y))
            .min()
            .expect("sites is not empty");
        sites.retain(|&(x, y)| exits(level, x, y) == fewest);
    }
    sites[rng.random_range(0..sites.len())]
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

    let far = far_half_floor(level, level.entry);
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

    /// Every frame of one layout, over a sweep wide enough to field a good
    /// number of each. Depth stays clear of the bottom so shape tests are
    /// not also testing the lair's seal.
    fn frames_of(layout: FrameLayout) -> Vec<Frame> {
        let mut out = Vec::new();
        for world_seed in 0..90 {
            for depth in 1..=5 {
                let level = generate(spec(world_seed, depth));
                if level.layout == layout {
                    out.push(level);
                }
            }
        }
        assert!(!out.is_empty(), "the sweep produced no {layout:?} frames");
        out
    }

    /// Whether the frame holds a `size` x `size` block of walkable cells.
    fn has_open_block(level: &Frame, size: i32) -> bool {
        (0..=level.height - size).any(|y| {
            (0..=level.width - size)
                .any(|x| (0..size).all(|dy| (0..size).all(|dx| level.walkable(x + dx, y + dy))))
        })
    }

    /// A room is the one thing the maze carver can never make. It touches
    /// only odd cells and the single wall between two of them, so a cell at
    /// even-even coordinates is always rock and a 3x3 block of open floor is
    /// unreachable no matter how generously the braid runs. Finding one is
    /// therefore proof the rooms carver ran, rather than proof the braid had
    /// a good day.
    #[test]
    fn the_rooms_layout_carves_rooms_the_maze_never_could() {
        for level in frames_of(FrameLayout::Rooms) {
            assert!(has_open_block(&level, 3), "a Rooms frame carved no room");
        }
        for level in frames_of(FrameLayout::Maze) {
            assert!(
                !has_open_block(&level, 3),
                "the maze opened a 3x3 block — this test no longer proves \
                 anything about the rooms carver"
            );
        }
    }

    /// The shape changed and the amount of place did not, and holding those
    /// two apart is the whole reconciliation this change rests on.
    ///
    /// The complaint the layouts answer is the *weave* — how far you walk
    /// between two points. It is not that there is too much frame. But
    /// encounters roll per step against a fixed `STACK_CACHES_PER_FRAME`, so
    /// a layout carving half the walkable cells of a maze would quietly
    /// double the loot per fight without anyone editing `tuning.rs`. Every
    /// layout therefore has to fill the frame to roughly the same extent.
    ///
    /// The band is wide because it spans three generators — measured, the
    /// maze runs 201-210, chambers 200-211 and rooms 153-225, the last being
    /// looser because rejection sampling has cramped seeds. It is here to
    /// catch a carver that halves or doubles the frame, not to freeze any of
    /// their exact output. `a_frames_shape_still_matches_what_trace_was_
    /// tuned_against` holds the same band from the balance side.
    #[test]
    fn every_layout_fills_the_frame_to_about_the_same_extent() {
        for layout in [FrameLayout::Maze, FrameLayout::Rooms, FrameLayout::Chambers] {
            for level in frames_of(layout) {
                let walkable = floors(&level).len();
                assert!(
                    (150..=230).contains(&walkable),
                    "a {layout:?} frame carved {walkable} walkable cells, \
                     outside the 150-230 every layout shares"
                );
            }
        }
    }

    /// A chamber is a hall — open enough that crossing it is a straight
    /// walk rather than a route. Six cells square is past anything the
    /// rooms carver reaches often and flatly impossible in the maze, whose
    /// even-even cells are always rock.
    #[test]
    fn the_chambers_layout_carves_halls() {
        for level in frames_of(FrameLayout::Chambers) {
            assert!(
                has_open_block(&level, 6),
                "a Chambers frame carved nothing bigger than a room"
            );
        }
    }

    /// The way down used to sit on the single furthest cell from the entry.
    /// That is the opposite corner every time, and it is half of what made a
    /// frame read as one long weave — the topology was only the other half.
    /// It is drawn from the far half now: still a real walk, and still never
    /// the near half, but not a maximal one and not the same shape of walk
    /// twice.
    #[test]
    fn the_way_down_is_far_without_always_being_the_furthest_cell() {
        let mut maximal = 0;
        let mut frames = 0;
        for world_seed in 0..90 {
            for depth in 1..=5 {
                let level = generate(spec(world_seed, depth));
                let down = level.link_down.expect("these frames have room below");
                let dist = distances_from(&level, level.entry);
                let reach = dist.iter().filter(|&&d| d != u32::MAX).max().unwrap();
                let at_down = dist[(down.1 * level.width + down.0) as usize];
                assert!(
                    at_down * 2 >= *reach,
                    "seed {world_seed} depth {depth}: the way down sits in the \
                     near half, {at_down} steps out of {reach}"
                );
                if at_down == *reach {
                    maximal += 1;
                }
                frames += 1;
            }
        }
        assert!(
            maximal * 2 < frames,
            "{maximal} of {frames} frames put the way down on the single \
             furthest cell — it is still the opposite corner every time"
        );
    }

    /// Walkable adjacencies, counted once per pair.
    fn edges(level: &Frame) -> usize {
        let mut n = 0;
        for y in 0..level.height {
            for x in 0..level.width {
                if !level.walkable(x, y) {
                    continue;
                }
                n += usize::from(level.walkable(x + 1, y));
                n += usize::from(level.walkable(x, y + 1));
            }
        }
        n
    }

    /// The complaint, stated as a number.
    ///
    /// A tree over N cells has N-1 edges and exactly one route between any
    /// two of them, so every wrong turn has to be walked back — that is what
    /// weaving through the whole map *is*. Independent loops are
    /// `edges - cells + 1`.
    ///
    /// The floors differ per layout because the layouts genuinely do, and
    /// flattening them to one number would either be vacuous for the open
    /// two or unreachable for the maze. A maze on a 21x21 lattice cannot be
    /// made much loopier without spending density it has not got — see
    /// `BRAID_PERCENT`, where raising it was tried and reverted — so it
    /// stays the tight layout, and the answer to weaving is that two frames
    /// in three are not one.
    #[test]
    fn every_layout_offers_more_than_one_route() {
        for (layout, floor) in [
            (FrameLayout::Maze, 2),
            (FrameLayout::Rooms, 40),
            (FrameLayout::Chambers, 40),
        ] {
            for level in frames_of(layout) {
                let (cells, edges) = (floors(&level).len(), edges(&level));
                let loops = edges + 1 - cells;
                assert!(
                    loops >= floor,
                    "a {layout:?} frame offers {loops} ways round its \
                     {cells} cells, under its floor of {floor} — a tree \
                     would offer 1"
                );
            }
        }
    }

    /// The lair has to stay something you shoulder your way into, on every
    /// layout. `seal_the_lair` puts a sealed door on every open neighbour,
    /// so the enclosure is only as good as the *cell chosen* — and
    /// `far_site` now chooses it, taking the least connected far-half
    /// candidate for exactly this reason. Dropped into the middle of a hall
    /// it would still seal correctly and read as four doors standing in open
    /// space, so the door count is asserted beside the enclosure.
    #[test]
    fn the_lair_is_sealed_off_on_every_layout() {
        let mut seen = Vec::new();
        for world_seed in 0..90 {
            let level = generate(FrameSpec {
                world_seed,
                entrance: (0, 0),
                depth: 4,
                frames: 4,
            });
            let lair = *cells_of(&level, CellKind::Lair)
                .first()
                .expect("a bottom frame has a lair");

            // Flood from the entry treating the seal as solid: the lair must
            // be out of reach.
            let mut seen_cells = vec![level.entry];
            let mut queue = VecDeque::from([level.entry]);
            while let Some((x, y)) = queue.pop_front() {
                for dir in DIRS {
                    let (dx, dy) = dir.delta();
                    let next = (x + dx, y + dy);
                    if level.cell(next.0, next.1) == CellKind::SealedDoor
                        || !level.walkable(next.0, next.1)
                        || seen_cells.contains(&next)
                    {
                        continue;
                    }
                    seen_cells.push(next);
                    queue.push_back(next);
                }
            }
            assert!(
                !seen_cells.contains(&lair),
                "seed {world_seed} ({:?}) leaves a way into the lair that \
                 crosses no seal",
                level.layout
            );

            let doors = DIRS
                .into_iter()
                .filter(|dir| {
                    let (dx, dy) = dir.delta();
                    level.cell(lair.0 + dx, lair.1 + dy) == CellKind::SealedDoor
                })
                .count();
            assert!(
                (1..=2).contains(&doors),
                "seed {world_seed} ({:?}) sealed the lair behind {doors} \
                 doors — `far_site` is meant to pick a cell that takes one \
                 or two",
                level.layout
            );
            if !seen.contains(&level.layout) {
                seen.push(level.layout);
            }
        }
        assert_eq!(seen.len(), 3, "only {seen:?} bottom frames in the sweep");
    }

    /// Every frame carving the same shape is the complaint this change
    /// exists to answer. A stack the party walks down has to offer more than
    /// one kind of place, so all three layouts must turn up over a sweep.
    #[test]
    fn every_layout_turns_up_across_a_sweep_of_frames() {
        let mut seen: Vec<FrameLayout> = Vec::new();
        for world_seed in 0..60 {
            for depth in 1..=5 {
                let layout = generate(spec(world_seed, depth)).layout;
                if !seen.contains(&layout) {
                    seen.push(layout);
                }
            }
        }
        assert_eq!(seen.len(), 3, "only {seen:?} ever generated");
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

    /// Swept per layout rather than down one seed's stack. The maze is
    /// connected by construction and always was; the rooms carver relies on
    /// its spanning tree and the chambers carver on its ring, and a bug in
    /// either would strand a wing behind rock on some seeds and not others.
    #[test]
    fn every_walkable_cell_is_reachable_from_the_entry() {
        for layout in [FrameLayout::Maze, FrameLayout::Rooms, FrameLayout::Chambers] {
            for level in frames_of(layout) {
                let reached = reachable_from(&level, level.entry);
                let all = floors(&level);
                assert_eq!(
                    reached.len(),
                    all.len(),
                    "a {layout:?} frame stranded {} cells behind solid rock",
                    all.len() - reached.len()
                );
            }
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
            layout: FrameLayout::Maze,
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
        for layout in [FrameLayout::Maze, FrameLayout::Rooms, FrameLayout::Chambers] {
            for level in frames_of(layout) {
                assert_walled_in(&level);
            }
        }
    }

    fn assert_walled_in(level: &Frame) {
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

    /// The full set is a promise now, not a ceiling, and `guarantee_loot_stubs`
    /// is what makes it one. `place_caches` and `place_orphan` both want plain-
    /// floor dead ends; the rooms and chambers layouts have almost none of
    /// their own, and the maze itself ran short of the four they need about
    /// one frame in four. A frame silently shipping two caches instead of
    /// three reads as a payout curve that moved rather than as a generator
    /// that came up short, which is why this sweeps rather than sampling.
    #[test]
    fn every_frame_fields_a_full_set_of_caches_and_an_orphan() {
        for world_seed in 0..80 {
            for depth in 1..=6 {
                let level = generate(FrameSpec {
                    frames: 6,
                    ..spec(world_seed, depth)
                });
                let layout = level.layout;
                assert_eq!(
                    cells_of(&level, CellKind::Cache).len(),
                    STACK_CACHES_PER_FRAME,
                    "seed {world_seed} depth {depth} ({layout:?}) is short a cache"
                );
                assert_eq!(
                    cells_of(&level, CellKind::Orphan).len(),
                    STACK_ORPHANS_PER_FRAME,
                    "seed {world_seed} depth {depth} ({layout:?}) left no orphan"
                );
            }
        }
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

    /// The whole point of continuing the fold rather than XOR-ing once: two
    /// cells of one frame, and the three lengths of one cell, have to
    /// diverge robustly rather than by luck.
    #[test]
    fn salting_the_frame_seed_diverges_on_every_word() {
        let spec = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        let seeds = [
            spec.salted(&[]),
            spec.salted(&[1]),
            spec.salted(&[2]),
            spec.salted(&[1, 0]),
            spec.salted(&[0, 1]),
            spec.salted(&[1, 1]),
        ];
        for (i, a) in seeds.iter().enumerate() {
            for (j, b) in seeds.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "salted words {i} and {j} collided on {a}");
            }
        }
    }

    /// Salting is a continuation of `rng_seed`, so two frames that already
    /// differ still differ after any number of words.
    #[test]
    fn salting_keeps_two_frames_apart() {
        let a = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        let b = FrameSpec { depth: 3, ..a };
        assert_ne!(a.salted(&[9, 9, 9]), b.salted(&[9, 9, 9]));
    }

    /// Same inputs, same answer — across calls and, because the mix is
    /// fixed arithmetic rather than a hasher, across builds.
    #[test]
    fn salting_is_stable() {
        let spec = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        assert_eq!(spec.salted(&[4, 5]), spec.salted(&[4, 5]));
        assert_eq!(spec.salted(&[]), spec.rng_seed());
    }
}
