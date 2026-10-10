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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::derive::{FNV_BASIS, fold, index};
use crate::tactical::deploy;
use crate::tactical::props::{Blast, Leaves, PieceDef, PrefabDef, PropDb};
use crate::tuning::{
    TACTICAL_BOARD_LARGE, TACTICAL_BOARD_MEDIUM, TACTICAL_BOARD_SMALL, TACTICAL_DECOR_PER_MILLE,
    TACTICAL_LARGE_BODIES, TACTICAL_MEDIUM_BODIES, TACTICAL_PREFAB_ATTEMPTS,
    TACTICAL_PREFABS_BY_SIDE, TACTICAL_ROUGH_COST,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// A prop standing on one cell of a board.
///
/// **Copied from its `PieceDef` at placement**, so a reach query never
/// consults the `PropDb`: the board stays self-contained, like the terrain.
/// A decoration is copied with every gameplay flag cleared, which is what
/// makes "decoration blocks nothing" a property of the type rather than of
/// each piece file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropCell {
    pub piece: String,
    pub hp: Option<u32>,
    pub max_hp: Option<u32>,
    pub armour: u32,
    pub blocks_move: bool,
    pub blocks_sight: bool,
    pub cover: bool,
    pub volatile: Option<Blast>,
    pub decoration: bool,
    pub sprite: String,
    pub move_cost: Option<u32>,
    /// What destroying it puts on the cell, copied so the fight code needs
    /// no `PropDb` lookup to know.
    pub leaves: Leaves,
}

impl PropCell {
    pub fn from_def(def: &PieceDef) -> PropCell {
        if def.decoration {
            return PropCell {
                piece: def.id.clone(),
                hp: None,
                max_hp: None,
                armour: 0,
                blocks_move: false,
                blocks_sight: false,
                cover: false,
                volatile: None,
                decoration: true,
                sprite: def.sprite.clone(),
                move_cost: None,
                leaves: Leaves::Floor,
            };
        }
        PropCell {
            piece: def.id.clone(),
            hp: def.hp,
            max_hp: def.hp,
            armour: def.armour,
            blocks_move: def.blocks_move,
            blocks_sight: def.blocks_sight,
            cover: def.cover,
            volatile: def.volatile,
            decoration: false,
            sprite: def.sprite.clone(),
            move_cost: def.move_cost,
            leaves: def.leaves,
        }
    }

    /// Whether a blow can ever break it.
    pub fn destructible(&self) -> bool {
        !self.decoration && self.hp.is_some()
    }
}

/// What `Board::damage_prop` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropHit {
    /// Nothing stands on the cell.
    Missing,
    /// A decoration, or a piece with no hp: nothing changed.
    Indestructible,
    Damaged,
    /// Already removed from the board; the caller places what it leaves.
    Destroyed(PropCell),
}

/// A generated battle map. Never saved; discarded at teardown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub side: i32,
    cells: Vec<BattleCell>,
    /// Authored set pieces over the terrain. An overlay like `screens`, for
    /// the same reason: the cells are what `SiegeSave` stores and a prop
    /// is never saved, so a siege board has none.
    props: BTreeMap<(i32, i32), PropCell>,
    /// Cells a standing body blocks sight through — a barrier structure on
    /// a siege board. An overlay rather than a `Cover` cell because the
    /// cells are what `SiegeSave` stores, and a body cannot be seated back
    /// onto a cell that reads as unwalkable; the overlay is never saved,
    /// and `siege::board::seat_structures` derives it again on load.
    screens: BTreeSet<(i32, i32)>,
}

impl Board {
    /// A board of every cell `Blocked` — the siege board's starting point
    /// (`game::siege::board::build`), seeded before the flood fill opens
    /// whatever of it is actually walkable.
    pub(crate) fn solid(side: i32) -> Board {
        Board {
            side,
            cells: vec![BattleCell::Blocked; (side * side) as usize],
            screens: BTreeSet::new(),
            props: BTreeMap::new(),
        }
    }

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

    /// What entering the cell costs, or `None` where it cannot be entered.
    ///
    /// **The one movement read.** The ground's cost, raised to a crossable
    /// prop's own, and `None` under a prop that blocks movement — so the
    /// movement field, the step and the carve all see props without a line
    /// of their own.
    pub fn move_cost(&self, x: i32, y: i32) -> Option<u32> {
        let ground = self.cell(x, y).movement_cost()?;
        match self.props.get(&(x, y)) {
            None => Some(ground),
            Some(prop) if prop.blocks_move => None,
            Some(prop) => Some(ground.max(prop.move_cost.unwrap_or(1))),
        }
    }

    /// Derived from `move_cost`, as `BattleCell::walkable` is from
    /// `movement_cost`.
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.move_cost(x, y).is_some()
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).blocks_sight()
            || self.screens.contains(&(x, y))
            || self.props.get(&(x, y)).is_some_and(|p| p.blocks_sight)
    }

    /// Whether a shot passing this cell is partly covered by it —
    /// `reach::cover_between`'s one read.
    pub fn is_cover(&self, x: i32, y: i32) -> bool {
        self.cell(x, y) == BattleCell::Cover || self.props.get(&(x, y)).is_some_and(|p| p.cover)
    }

    pub fn prop_at(&self, x: i32, y: i32) -> Option<&PropCell> {
        self.props.get(&(x, y))
    }

    /// Every prop in reading order of the map's key, `(x, y)`.
    pub fn props(&self) -> impl Iterator<Item = (&(i32, i32), &PropCell)> {
        self.props.iter()
    }

    /// A no-op off the board, `set`'s rule.
    pub fn place_prop(&mut self, cell: (i32, i32), prop: PropCell) {
        if self.in_bounds(cell.0, cell.1) {
            self.props.insert(cell, prop);
        }
    }

    pub fn remove_prop(&mut self, cell: (i32, i32)) -> Option<PropCell> {
        self.props.remove(&cell)
    }

    /// Takes `amount` off the prop's hp. A prop taken to zero is removed
    /// before this returns, so a caller never sees a standing prop at 0.
    pub fn damage_prop(&mut self, cell: (i32, i32), amount: u32) -> PropHit {
        let Some(prop) = self.props.get_mut(&cell) else {
            return PropHit::Missing;
        };
        let Some(hp) = prop.hp.filter(|_| !prop.decoration) else {
            return PropHit::Indestructible;
        };
        let left = hp.saturating_sub(amount);
        if left == 0 {
            let gone = self.props.remove(&cell).expect("just read");
            return PropHit::Destroyed(gone);
        }
        prop.hp = Some(left);
        PropHit::Damaged
    }

    /// Screens `cell` from sight, or stops screening it — `TacticalBattle::
    /// screen` and `TacticalBattle::remove` are the two writers.
    pub(crate) fn set_screen(&mut self, cell: (i32, i32), screened: bool) {
        if screened {
            self.screens.insert(cell);
        } else {
            self.screens.remove(&cell);
        }
    }

    /// Puts `kind` on one cell — a no-op off the board, `cell`'s own
    /// "seen over, never stepped on" answer read the other way round: a
    /// write nobody can ever stand at is safe to ignore rather than a panic
    /// waiting for a flood fill's bounding box to be one cell out.
    pub(crate) fn set(&mut self, x: i32, y: i32, kind: BattleCell) {
        if self.in_bounds(x, y) {
            let i = (y * self.side + x) as usize;
            self.cells[i] = kind;
        }
    }

    /// Puts `kind` on one cell of an already-generated board, for tests that
    /// need a known cell under a known body. `from_rows` is the other half —
    /// use that where the whole layout matters and this where one cell does,
    /// since a fight opened by the engine runs on a *generated* board that no
    /// amount of reseeding will put cover on a chosen square of.
    #[cfg(test)]
    pub(crate) fn put(&mut self, x: i32, y: i32, kind: BattleCell) {
        self.set(x, y, kind);
    }

    /// A board written out by hand, one string per row, for tests that need
    /// a known layout rather than a generated one: `.` open, `~` rough, `#`
    /// cover, `X` blocked, and over open ground: `P` an indestructible prop
    /// that blocks sight and movement, `D` a destructible one (hp 10), `V` a
    /// volatile one (hp 5, radius 1, damage 12). Square, because `Board` has
    /// one `side`.
    #[cfg(test)]
    pub(crate) fn from_rows(rows: &[&str]) -> Board {
        let side = rows.len() as i32;
        let mut props = BTreeMap::new();
        let mut cells = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.chars().count() as i32, side, "a board is square");
            for (x, c) in row.chars().enumerate() {
                let (kind, prop) = match c {
                    '.' => (BattleCell::Open, None),
                    '~' => (BattleCell::Rough, None),
                    '#' => (BattleCell::Cover, None),
                    'X' => (BattleCell::Blocked, None),
                    'P' => (BattleCell::Open, Some(test_prop("P", None, None))),
                    'D' => (BattleCell::Open, Some(test_prop("D", Some(10), None))),
                    'V' => (
                        BattleCell::Open,
                        Some(test_prop(
                            "V",
                            Some(5),
                            Some(Blast {
                                radius: 1,
                                damage: 12,
                            }),
                        )),
                    ),
                    other => panic!("no such cell: {other}"),
                };
                cells.push(kind);
                if let Some(prop) = prop {
                    props.insert((x as i32, y as i32), prop);
                }
            }
        }
        Board {
            side,
            cells,
            screens: BTreeSet::new(),
            props,
        }
    }

    /// A board rebuilt from its own saved cells — `game::siege::persist::
    /// restore`'s door back in, `from_rows`'s shape but for real save data
    /// rather than a test fixture. `cells` is trusted to be `side * side`
    /// long and in the row-major order `cells()` itself yields, which is
    /// exactly what `game::siege::persist::assemble` wrote out; nothing
    /// else calls this back.
    pub(crate) fn from_cells(side: i32, cells: Vec<BattleCell>) -> Board {
        debug_assert_eq!(
            cells.len(),
            (side * side) as usize,
            "a saved siege board's cell count must match its own side"
        );
        Board {
            side,
            cells,
            screens: BTreeSet::new(),
            props: BTreeMap::new(),
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

#[cfg(test)]
fn test_prop(piece: &str, hp: Option<u32>, volatile: Option<Blast>) -> PropCell {
    PropCell {
        piece: piece.into(),
        hp,
        max_hp: hp,
        armour: 0,
        blocks_move: true,
        blocks_sight: true,
        cover: true,
        volatile,
        decoration: false,
        sprite: format!("prop_{piece}"),
        move_cost: None,
        leaves: Leaves::Rubble,
    }
}

/// Eight-way, in a fixed order so every walk over a board is deterministic.
pub(crate) const NEIGHBOURS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Every walkable cell reachable from `from`, eight-way.
///
/// **Cost-blind on purpose**, and not the movement field. This asks "can a
/// body get there at all", which `Rough`'s cost cannot change; the movement
/// field asks "how far can this body get this turn", which is a different
/// question with a different answer. Corner-cutting is allowed, matching
/// `walk_field`'s eight directions, so connectivity here and reachability
/// there agree.
fn region(board: &Board, from: (i32, i32)) -> BTreeSet<(i32, i32)> {
    let mut seen = BTreeSet::new();
    if !board.walkable(from.0, from.1) {
        return seen;
    }
    let mut queue = VecDeque::from([from]);
    seen.insert(from);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in NEIGHBOURS {
            let next = (x + dx, y + dy);
            if board.walkable(next.0, next.1) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

/// Opens blockers until the walkable ground is one piece.
///
/// A sealed pocket is rare at these blocker rates rather than impossible,
/// and rare is the worst kind: it survives every test run and then happens
/// to a player, who finds a body that cannot leave the cell it deployed on
/// and a fight that cannot finish. Each pass grows the mainland by at least
/// one cell, so this terminates in at most `side * side` passes.
///
/// **A prop in the way is removed only when no way round it exists.** The
/// corridor is first searched through noise and destructible props alone; an
/// indestructible prop is a set piece the player was meant to see, and is
/// opened only for a pocket that could not be reached any other way.
fn carve_to_connect(board: &mut Board) {
    loop {
        let Some(start) = board
            .cells()
            .map(|(c, _)| c)
            .find(|c| board.walkable(c.0, c.1))
        else {
            return;
        };
        let mainland = region(board, start);
        let stranded = board
            .cells()
            .map(|(c, _)| c)
            .find(|c| board.walkable(c.0, c.1) && !mainland.contains(c));
        let Some(stranded) = stranded else {
            return;
        };
        let mut path = corridor(board, &mainland, stranded, false);
        if path.is_empty() {
            path = corridor(board, &mainland, stranded, true);
        }
        for cell in path {
            board.set(cell.0, cell.1, BattleCell::Open);
            board.remove_prop(cell);
        }
    }
}

/// The shortest run of blockers between `from` and any cell of `mainland`.
///
/// A breadth-first walk that ignores walkability entirely and keeps
/// predecessors, so the path it reports back is the fewest cells that have
/// to be opened — the carve takes a corridor, not a demolition. Unless
/// `through_fixtures`, it will not step on an indestructible prop.
fn corridor(
    board: &Board,
    mainland: &BTreeSet<(i32, i32)>,
    from: (i32, i32),
    through_fixtures: bool,
) -> Vec<(i32, i32)> {
    let mut came_from: std::collections::BTreeMap<(i32, i32), (i32, i32)> =
        std::collections::BTreeMap::new();
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        if mainland.contains(&at) {
            let mut path = Vec::new();
            let mut step = at;
            while step != from {
                if !board.walkable(step.0, step.1) {
                    path.push(step);
                }
                step = came_from[&step];
            }
            return path;
        }
        for (dx, dy) in NEIGHBOURS {
            let next = (at.0 + dx, at.1 + dy);
            let fixture = board
                .prop_at(next.0, next.1)
                .is_some_and(|p| p.blocks_move && !p.destructible());
            if board.in_bounds(next.0, next.1)
                && (through_fixtures || !fixture)
                && seen.insert(next)
            {
                came_from.insert(next, at);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}

/// Salt for every fold prop placement draws from, so none of it can land on
/// a word the noise already folds to.
const PROP_SALT: u64 = 0x0070_726f_7073;

/// One placement's seed.
fn prop_seed(spec: BattleSpec, words: &[u64]) -> u64 {
    let mut all = vec![PROP_SALT];
    all.extend_from_slice(words);
    fold(spec.base_seed(), &all)
}

/// A prefab's cells as a grid of characters, turned `quarters` times
/// clockwise.
fn rotated(rows: &[String], quarters: u32) -> Vec<Vec<char>> {
    let mut grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    for _ in 0..quarters % 4 {
        let (h, w) = (grid.len(), grid[0].len());
        grid = (0..w)
            .map(|x| (0..h).rev().map(|y| grid[y][x]).collect())
            .collect();
    }
    grid
}

/// Picks one prefab by weight from `candidates`.
fn pick_weighted<'a>(seed: u64, candidates: &[&'a PrefabDef]) -> &'a PrefabDef {
    let total: u32 = candidates.iter().map(|p| p.weight).sum();
    let mut n = index(seed, total as usize) as u32;
    for p in candidates {
        if n < p.weight {
            return p;
        }
        n -= p.weight;
    }
    candidates[candidates.len() - 1]
}

/// Stamps the authored set pieces onto the noise, then scatters decoration.
///
/// Pure in `spec` and `props`, like the noise: every choice is a fold, no
/// `GameRng` is drawn. Each prefab is tried at `TACTICAL_PREFAB_ATTEMPTS`
/// hashed positions and rotations and dropped if none fits — a rectangle
/// clear of every earlier prefab and of the cells a deployment could seat
/// on. A cell under a solid piece becomes `Open` first, so destroying the
/// piece leaves floor and not whatever noise it happened to stand on.
fn place_props(board: &mut Board, spec: BattleSpec, props: &PropDb) {
    let side = board.side;
    let candidates: Vec<&PrefabDef> = props.prefabs_for(spec.biome).collect();
    if !candidates.is_empty() {
        let (_, min, max) = TACTICAL_PREFABS_BY_SIDE
            .iter()
            .find(|(s, _, _)| *s == side)
            .copied()
            .unwrap_or(TACTICAL_PREFABS_BY_SIDE[TACTICAL_PREFABS_BY_SIDE.len() - 1]);
        let count = min + index(prop_seed(spec, &[0]), (max - min + 1) as usize) as u32;
        let reserved = deploy::anchor_neighbourhoods(side);
        let mut claimed: BTreeSet<(i32, i32)> = BTreeSet::new();
        for slot in 0..count {
            let prefab = pick_weighted(prop_seed(spec, &[1, slot as u64]), &candidates);
            for attempt in 0..TACTICAL_PREFAB_ATTEMPTS {
                let at = |tag: u64| prop_seed(spec, &[2, slot as u64, attempt as u64, tag]);
                let grid = rotated(&prefab.rows, index(at(0), 4) as u32);
                let (h, w) = (grid.len() as i32, grid[0].len() as i32);
                if w > side || h > side {
                    continue;
                }
                let x0 = index(at(1), (side - w + 1) as usize) as i32;
                let y0 = index(at(2), (side - h + 1) as usize) as i32;
                let rect: Vec<(i32, i32)> = (0..h)
                    .flat_map(|y| (0..w).map(move |x| (x0 + x, y0 + y)))
                    .collect();
                let solid = |c: char| c != '.';
                let touches_reserved = grid.iter().enumerate().any(|(y, row)| {
                    row.iter().enumerate().any(|(x, &c)| {
                        solid(c) && reserved.contains(&(x0 + x as i32, y0 + y as i32))
                    })
                });
                if touches_reserved || rect.iter().any(|c| claimed.contains(c)) {
                    continue;
                }
                for (y, row) in grid.iter().enumerate() {
                    for (x, &c) in row.iter().enumerate() {
                        let Some(def) = prefab.legend.get(&c).and_then(|id| props.piece(id)) else {
                            continue;
                        };
                        let cell = (x0 + x as i32, y0 + y as i32);
                        if !def.decoration {
                            board.set(cell.0, cell.1, BattleCell::Open);
                        }
                        board.place_prop(cell, PropCell::from_def(def));
                    }
                }
                claimed.extend(rect);
                break;
            }
        }
    }

    let decor: Vec<&PieceDef> = props.pieces().filter(|p| p.decoration).collect();
    if decor.is_empty() {
        return;
    }
    for y in 0..side {
        for x in 0..side {
            if board.cell(x, y) != BattleCell::Open || board.prop_at(x, y).is_some() {
                continue;
            }
            let seed = prop_seed(spec, &[3, x as u32 as u64, y as u32 as u64]);
            if index(seed, 1000) as u32 >= TACTICAL_DECOR_PER_MILLE {
                continue;
            }
            let pick = index(
                prop_seed(spec, &[4, x as u32 as u64, y as u32 as u64]),
                decor.len(),
            );
            board.place_prop((x, y), PropCell::from_def(decor[pick]));
        }
    }
}

/// The whole generator: derive every cell, stamp the set pieces, then make
/// sure the walkable ground is one piece.
pub fn generate(spec: BattleSpec, props: &PropDb) -> Board {
    let side = spec.side();
    let cells = (0..side * side)
        .map(|i| kind_at(spec, i % side, i / side))
        .collect();
    let mut board = Board {
        side,
        cells,
        screens: BTreeSet::new(),
        props: BTreeMap::new(),
    };
    place_props(&mut board, spec, props);
    carve_to_connect(&mut board);
    board
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
        let a = generate(spec(4), &PropDb::default());
        let b = generate(spec(4), &PropDb::default());
        assert_eq!(a.side, b.side);
        assert_eq!(a.cells().collect::<Vec<_>>(), b.cells().collect::<Vec<_>>());
    }

    #[test]
    fn a_board_is_its_tier_square() {
        let board = generate(spec(9), &PropDb::default());
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
            blockers(&generate(city, &PropDb::default()))
                > blockers(&generate(plain, &PropDb::default())),
            "the biome does not reach the ground"
        );
    }

    #[test]
    fn out_of_bounds_is_seen_over_and_never_stepped_on() {
        let board = generate(spec(4), &PropDb::default());
        assert!(!board.in_bounds(-1, 0));
        assert!(!board.in_bounds(board.side, 0));
        assert_eq!(board.cell(-1, 0), BattleCell::Blocked);
        assert!(!board.walkable(-1, 0));
        assert!(!board.blocks_sight(-1, 0));
    }

    /// A hand-built board with a wall straight down the middle: two regions
    /// before the carve, one after.
    #[test]
    fn a_wall_across_the_board_is_carved_through() {
        let mut board = Board {
            side: 7,
            cells: vec![BattleCell::Open; 49],
            screens: BTreeSet::new(),
            props: BTreeMap::new(),
        };
        for y in 0..7 {
            board.set(3, y, BattleCell::Cover);
        }
        assert_eq!(
            region(&board, (0, 0)).len(),
            21,
            "the fixture is not actually split"
        );

        carve_to_connect(&mut board);

        assert_eq!(
            region(&board, (0, 0)).len(),
            board.cells().filter(|(_, k)| k.walkable()).count(),
            "the carve left ground the rest of the board cannot reach"
        );
    }

    /// The carve opens a way through and does not flatten the board.
    #[test]
    fn the_carve_spends_as_few_cells_as_it_can() {
        let mut board = Board {
            side: 7,
            cells: vec![BattleCell::Open; 49],
            screens: BTreeSet::new(),
            props: BTreeMap::new(),
        };
        for y in 0..7 {
            board.set(3, y, BattleCell::Cover);
        }
        carve_to_connect(&mut board);
        let opened = board
            .cells()
            .filter(|((x, _), k)| *x == 3 && k.walkable())
            .count();
        assert_eq!(
            opened, 1,
            "the carve took out more of the wall than it needed"
        );
    }

    /// The property the whole task exists for, over every shipped biome a
    /// fight can open on and every tier.
    #[test]
    fn no_generated_board_strands_anybody() {
        for biome in [
            Biome::OpenGrid,
            Biome::Deadlock,
            Biome::NullSector,
            Biome::Backplane,
        ] {
            for bodies in [2_u32, 5, 9] {
                for tick in 0..60_u64 {
                    let board = generate(
                        BattleSpec {
                            world_seed: 77,
                            site: (4, 4),
                            tick,
                            zone: 2,
                            biome,
                            bodies,
                        },
                        &PropDb::default(),
                    );
                    let walkable = board.cells().filter(|(_, k)| k.walkable()).count();
                    let start = board
                        .cells()
                        .find(|(_, k)| k.walkable())
                        .map(|(c, _)| c)
                        .expect("a board with no ground at all");
                    assert_eq!(
                        region(&board, start).len(),
                        walkable,
                        "{biome:?} at {bodies} bodies, tick {tick}: ground is in pieces"
                    );
                }
            }
        }
    }

    /// `Board::solid` — the siege board's starting point before the flood
    /// fill opens anything.
    #[test]
    fn a_solid_board_is_blocked_everywhere_in_bounds() {
        let board = Board::solid(4);
        assert_eq!(board.side, 4);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(board.cell(x, y), BattleCell::Blocked);
            }
        }
        assert!(board.in_bounds(3, 3));
        assert!(!board.in_bounds(4, 0));
    }

    #[test]
    fn set_off_the_board_changes_nothing_and_does_not_panic() {
        let mut board = Board::solid(3);
        board.set(-1, 0, BattleCell::Open);
        board.set(3, 3, BattleCell::Open);
        board.set(100, -100, BattleCell::Open);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(board.cell(x, y), BattleCell::Blocked);
            }
        }
    }

    /// `cells()` walks a hand-built board in the same row-major order
    /// `solid` and `set` put it together in — the order a save (Task 16)
    /// must round-trip through.
    #[test]
    fn cells_round_trips_through_solid_and_set_in_order() {
        let mut board = Board::solid(3);
        let writes = [
            (0, 0, BattleCell::Open),
            (1, 0, BattleCell::Rough),
            (2, 1, BattleCell::Cover),
        ];
        for &(x, y, kind) in &writes {
            board.set(x, y, kind);
        }
        let expected: Vec<((i32, i32), BattleCell)> = (0..3)
            .flat_map(|y| (0..3).map(move |x| (x, y)))
            .map(|(x, y)| {
                let kind = writes
                    .iter()
                    .find(|&&(wx, wy, _)| wx == x && wy == y)
                    .map(|&(_, _, k)| k)
                    .unwrap_or(BattleCell::Blocked);
                ((x, y), kind)
            })
            .collect();
        assert_eq!(board.cells().collect::<Vec<_>>(), expected);
    }

    /// `BattleCell` round-trips through RON — Task 16 saves a siege's board
    /// as a plain `Vec<BattleCell>`, and this is the derive that makes that
    /// legal.
    #[test]
    fn battle_cell_round_trips_through_ron() {
        for kind in [
            BattleCell::Open,
            BattleCell::Rough,
            BattleCell::Cover,
            BattleCell::Blocked,
        ] {
            let text = ron::to_string(&kind).expect("BattleCell must serialise");
            let back: BattleCell = ron::from_str(&text).expect("BattleCell must deserialise");
            assert_eq!(kind, back);
        }
    }
    fn rubble() -> PropCell {
        PropCell {
            piece: "rubble".into(),
            hp: None,
            max_hp: None,
            armour: 0,
            blocks_move: false,
            blocks_sight: false,
            cover: false,
            volatile: None,
            decoration: false,
            sprite: "prop_rubble".into(),
            move_cost: Some(TACTICAL_ROUGH_COST),
            leaves: Leaves::Floor,
        }
    }

    #[test]
    fn a_blocking_prop_stops_movement_and_sight_and_is_cover() {
        let board = Board::from_rows(&["...", ".P.", "..."]);
        assert_eq!(board.cell(1, 1), BattleCell::Open);
        assert!(!board.walkable(1, 1));
        assert_eq!(board.move_cost(1, 1), None);
        assert!(board.blocks_sight(1, 1));
        assert!(board.is_cover(1, 1));
        assert!(board.walkable(0, 0) && !board.is_cover(0, 0));
        assert_eq!(board.prop_at(1, 1).map(|p| p.piece.as_str()), Some("P"));
    }

    #[test]
    fn an_indestructible_prop_never_changes() {
        let mut board = Board::from_rows(&["...", ".P.", "..."]);
        assert_eq!(board.damage_prop((1, 1), 999), PropHit::Indestructible);
        assert!(board.prop_at(1, 1).is_some());
        assert_eq!(board.damage_prop((0, 0), 5), PropHit::Missing);
    }

    #[test]
    fn a_destructible_prop_is_removed_at_zero_and_opens_the_cell() {
        let mut board = Board::from_rows(&["...", ".D.", "..."]);
        assert_eq!(board.damage_prop((1, 1), 4), PropHit::Damaged);
        assert_eq!(board.prop_at(1, 1).and_then(|p| p.hp), Some(6));
        assert!(!board.walkable(1, 1));
        let PropHit::Destroyed(gone) = board.damage_prop((1, 1), 6) else {
            panic!("hp 6 less 6 should destroy it");
        };
        assert_eq!(gone.piece, "D");
        assert!(board.prop_at(1, 1).is_none());
        assert!(board.walkable(1, 1) && !board.blocks_sight(1, 1) && !board.is_cover(1, 1));
    }

    #[test]
    fn rubble_costs_the_rough_price_and_gives_no_cover() {
        let mut board = Board::from_rows(&["...", "...", "..."]);
        assert_eq!(board.move_cost(1, 1), Some(1));
        board.place_prop((1, 1), rubble());
        assert_eq!(board.move_cost(1, 1), Some(TACTICAL_ROUGH_COST));
        assert!(board.walkable(1, 1) && !board.is_cover(1, 1) && !board.blocks_sight(1, 1));
    }

    #[test]
    fn a_prop_never_makes_rough_ground_cheaper() {
        let mut board = Board::from_rows(&["~~~", "~~~", "~~~"]);
        let mut cheap = rubble();
        cheap.move_cost = None;
        board.place_prop((0, 0), cheap);
        assert_eq!(board.move_cost(0, 0), Some(TACTICAL_ROUGH_COST));
    }

    #[test]
    fn a_decoration_blocks_nothing_whatever_its_piece_says() {
        let def = PieceDef {
            id: "scorch".into(),
            blocks_move: true,
            blocks_sight: true,
            cover: true,
            hp: Some(5),
            armour: 0,
            volatile: Some(Blast {
                radius: 1,
                damage: 1,
            }),
            leaves: crate::tactical::props::Leaves::Floor,
            sprite: "prop_scorch".into(),
            decoration: true,
            move_cost: Some(9),
        };
        let mut board = Board::from_rows(&["...", "...", "..."]);
        board.place_prop((0, 0), PropCell::from_def(&def));
        assert_eq!(board.move_cost(0, 0), Some(1));
        assert!(!board.blocks_sight(0, 0) && !board.is_cover(0, 0));
        assert_eq!(board.damage_prop((0, 0), 99), PropHit::Indestructible);
    }

    #[test]
    fn a_prop_off_the_board_is_ignored() {
        let mut board = Board::from_rows(&["...", "...", "..."]);
        board.place_prop((5, 5), rubble());
        assert_eq!(board.props().count(), 0);
    }

    fn shipped() -> PropDb {
        PropDb::load_dir(&crate::tests::support::test_assets_dir().join("battle-props"))
            .unwrap()
            .0
    }

    const FIGHT_BIOMES: [Biome; 4] = [
        Biome::OpenGrid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Backplane,
    ];

    fn spec_at(biome: Biome, bodies: u32, tick: u64) -> BattleSpec {
        BattleSpec {
            world_seed: 77,
            site: (4, 4),
            tick,
            zone: 2,
            biome,
            bodies,
        }
    }

    #[test]
    fn an_empty_prop_db_leaves_the_noise_as_it_was() {
        for biome in FIGHT_BIOMES {
            for tick in 0..20 {
                let spec = spec_at(biome, 5, tick);
                let side = spec.side();
                let mut expected = Board {
                    side,
                    cells: (0..side * side)
                        .map(|i| kind_at(spec, i % side, i / side))
                        .collect(),
                    screens: BTreeSet::new(),
                    props: BTreeMap::new(),
                };
                carve_to_connect(&mut expected);
                assert_eq!(generate(spec, &PropDb::default()), expected);
            }
        }
    }

    #[test]
    fn the_same_spec_and_db_yield_the_same_props() {
        let db = shipped();
        for biome in FIGHT_BIOMES {
            let spec = spec_at(biome, 9, 3);
            assert_eq!(generate(spec, &db), generate(spec, &db));
        }
    }

    #[test]
    fn props_are_stamped_and_differ_between_fights() {
        let db = shipped();
        let a = generate(spec_at(Biome::Backplane, 9, 1), &db);
        let b = generate(spec_at(Biome::Backplane, 9, 2), &db);
        assert!(a.props().count() > 0);
        let cells = |b: &Board| {
            b.props()
                .map(|(c, p)| (*c, p.piece.clone()))
                .collect::<Vec<_>>()
        };
        assert_ne!(cells(&a), cells(&b));
    }

    /// Review focus 1: a long indestructible run must not wall a side in.
    #[test]
    fn props_never_strand_anybody_or_sit_on_a_deploy_cell() {
        let db = shipped();
        for biome in FIGHT_BIOMES {
            for bodies in [2_u32, 5, 9] {
                for tick in 0..70_u64 {
                    let board = generate(spec_at(biome, bodies, tick), &db);
                    let walkable: Vec<(i32, i32)> = board
                        .cells()
                        .map(|(c, _)| c)
                        .filter(|c| board.walkable(c.0, c.1))
                        .collect();
                    assert_eq!(
                        region(&board, walkable[0]).len(),
                        walkable.len(),
                        "{biome:?} at {bodies} bodies, tick {tick}: ground is in pieces"
                    );
                    for bearing in NEIGHBOURS {
                        let plan = deploy::plan(&board, bearing, 4, &[1; 4]);
                        for cell in plan.party.iter().chain(plan.wild.iter()) {
                            assert!(
                                board.prop_at(cell.0, cell.1).is_none_or(|p| !p.blocks_move),
                                "{biome:?} tick {tick}: deployed on a prop at {cell:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// The census, `cover_is_reachable_on_every_biome_a_fight_opens_on`'s
    /// pattern: a biome that never gets a set piece is a missing row.
    #[test]
    fn every_biome_a_fight_opens_on_gets_a_prefab_soon() {
        let db = shipped();
        for biome in FIGHT_BIOMES {
            let hit = (0..20_u64).any(|tick| {
                generate(spec_at(biome, 5, tick), &db)
                    .props()
                    .any(|(_, p)| !p.decoration)
            });
            assert!(hit, "{biome:?} never got a prefab in 20 fights");
        }
    }

    fn prop_def(id: &str, hp: Option<u32>) -> PieceDef {
        PieceDef {
            id: id.into(),
            blocks_move: true,
            blocks_sight: true,
            cover: true,
            hp,
            armour: 0,
            volatile: None,
            leaves: crate::tactical::props::Leaves::Floor,
            sprite: String::new(),
            decoration: false,
            move_cost: None,
        }
    }

    /// The carve prefers noise to a set piece: with a one-cell opening of
    /// cover in an otherwise indestructible wall, it opens the cover.
    #[test]
    fn the_carve_opens_noise_before_an_indestructible_prop() {
        let mut board = Board::from_rows(&[
            "...P...", "...P...", "...P...", "...#...", "...P...", "...P...", "...P...",
        ]);
        carve_to_connect(&mut board);
        assert!(board.walkable(3, 3));
        assert_eq!(board.props().count(), 6);
    }

    /// And never loops for want of a way round: a wall of fixtures edge to
    /// edge costs exactly one of them.
    #[test]
    fn the_carve_breaks_a_fixture_only_when_nothing_else_connects() {
        let mut board = Board::from_rows(&[
            "...P...", "...P...", "...P...", "...P...", "...P...", "...P...", "...P...",
        ]);
        carve_to_connect(&mut board);
        assert_eq!(board.props().count(), 6);
        assert_eq!(
            region(&board, (0, 0)).len(),
            board
                .cells()
                .filter(|(c, _)| board.walkable(c.0, c.1))
                .count()
        );
    }

    #[test]
    fn the_carve_removes_a_destructible_prop_to_connect() {
        let mut board = Board::from_rows(&[
            "...D...", "...D...", "...D...", "...D...", "...D...", "...D...", "...D...",
        ]);
        carve_to_connect(&mut board);
        assert_eq!(board.props().count(), 6);
    }

    #[test]
    fn a_prefab_rotates_through_four_quarters_back_to_itself() {
        let rows = vec!["ab.".to_string(), "c..".to_string()];
        let mut grid = rotated(&rows, 0);
        assert_eq!(grid, vec![vec!['a', 'b', '.'], vec!['c', '.', '.']]);
        grid = rotated(&rows, 1);
        assert_eq!(grid, vec![vec!['c', 'a'], vec!['.', 'b'], vec!['.', '.']]);
        assert_eq!(rotated(&rows, 4), rotated(&rows, 0));
    }

    #[test]
    fn a_prefab_too_big_for_the_board_is_dropped_not_a_panic() {
        let rows: Vec<String> = vec!["W".repeat(40)];
        let db = PropDb::from_parts(
            vec![prop_def("w", None)],
            vec![PrefabDef {
                id: "huge".into(),
                biomes: vec![],
                weight: 1,
                rows,
                legend: [('W', "w".to_string())].into(),
            }],
        );
        let board = generate(spec_at(Biome::OpenGrid, 2, 0), &db);
        assert_eq!(board.props().count(), 0);
    }
}
