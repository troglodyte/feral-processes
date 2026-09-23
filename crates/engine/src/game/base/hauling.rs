//! Posted programs walking: taking a post, carrying a clogged machine's
//! output to a depot, and coming back.
//!
//! `components::Carrying` is the only state this feature stores. Where a
//! worker is headed and whether it has arrived are both read off `Position`,
//! so the two cannot disagree with each other the way a hand-maintained
//! `HaulState` enum would.

use std::collections::HashSet;

use bevy_ecs::system::SystemParam;

use crate::alerts::{self, AlertKind};
use crate::base_grid::BaseGrid;
use crate::game::base::collect::ORTHOGONAL;
use crate::game::base::work_orders;
use crate::game::pursuit::walk_field;
use crate::items::ItemId;
use crate::systems::{intake_recipe, produced_item};
use crate::tuning::haul_walk_radius;
use crate::world::NEIGHBOURS;
use crate::*;

/// Takes up to `HAUL_CARRY_CAPACITY` units of one item out of `stock`'s
/// output, or `None` if there is nothing to take.
///
/// The item is the first key in `BTreeMap` order — `Stock` keys by `ItemId`
/// in a `BTreeMap` precisely so choices like this are stable run to run, and
/// picking deterministically is what lets a load be a single `(item, qty)`
/// pair rather than a map.
pub(crate) fn take_haul_load(
    stock: &mut Stock,
    somewhere_takes: impl Fn(&ItemId) -> bool,
) -> Option<Carrying> {
    // Cloned out before the map is touched: the borrow behind `.keys()` is
    // still live otherwise.
    //
    // **The predicate is inside the pick, not around it.** A buffer holding
    // two products with only the second one welcome anywhere would
    // otherwise sit clogged forever behind a head entry nobody will take.
    let item = stock.output.keys().find(|i| somewhere_takes(i)).cloned()?;
    let qty = take_from(stock, &item, tuning::HAUL_CARRY_CAPACITY);
    (qty > 0).then_some(Carrying { item, qty })
}

/// Takes up to `qty` of `item` out of `stock`'s output and reports how much
/// came. The inverse of `deposit`, and the one way units leave a buffer by
/// hand — a producer clearing its own and a worker drawing an ingredient
/// off a shelf differ only in which item they name.
///
/// `pub(crate)` for the third caller: `stock::spend_from_base` drains these
/// same buffers on the base's own behalf, and a second copy of the
/// remove-or-decrement is how a buffer ends up holding a zero entry that
/// every reader then has to know to skip.
pub(crate) fn take_from(stock: &mut Stock, item: &ItemId, qty: u32) -> u32 {
    take_from_buffer(&mut stock.output, item, qty)
}

/// `take_from`'s twin on the **input** hopper, for the one taker that reaches
/// into one from outside: `systems::burn_grid_upkeep` spends the Power Cell
/// a program fetched for a supplier, and that cell was delivered through
/// `Errand::Load` like any other ingredient.
///
/// The asymmetry is deliberate and is `Errand::Load`'s own: `Stock::output`
/// is the buffer a neighbour may pull from, and a machine's input belongs to
/// the machine. This is that machine spending it.
pub(crate) fn take_from_input(stock: &mut Stock, item: &ItemId, qty: u32) -> u32 {
    take_from_buffer(&mut stock.input, item, qty)
}

/// The remove-or-decrement both of the above are, written once. A buffer
/// that held a zero entry would be one every reader had to know to skip.
fn take_from_buffer(
    buffer: &mut std::collections::BTreeMap<ItemId, u32>,
    item: &ItemId,
    qty: u32,
) -> u32 {
    let held = buffer.get(item).copied().unwrap_or(0);
    let taken = qty.min(held);
    if taken == 0 {
        return 0;
    }
    if held == taken {
        buffer.remove(item);
    } else {
        buffer.insert(item.clone(), held - taken);
    }
    taken
}

/// Whether `a` is one of the four tiles orthogonally touching `b`.
///
/// `collect::ORTHOGONAL` rather than a second adjacency list: a worker's
/// arrival, a player's collect and a machine's reach into its neighbour all
/// ask this one question, and the moment they could differ the base stops
/// reading as a physical line. Movement itself stays 8-directional — only
/// touching is orthogonal.
fn touching(a: Position, b: Position) -> bool {
    ORTHOGONAL
        .iter()
        .any(|(dx, dy)| a.x == b.x + dx && a.y == b.y + dy)
}

/// True when `worker` stands on one of the tiles `structure`'s `side`-wide
/// footprint can be reached from — touching any one of its cells, anchor or
/// floor alike. `side: 1` is every structure but the Research Station, and
/// reduces to `touching(worker, structure)` exactly.
pub(crate) fn at_station(worker: Position, structure: Position, side: u8) -> bool {
    let footprint = crate::tactical::footprint_cells_at((structure.x, structure.y), side);
    // A worker standing *on* one of the footprint's own cells — reachable
    // only on a floor cell, since the anchor blocks movement — has not
    // arrived at a post: two adjacent footprint cells are each other's
    // orthogonal neighbour, so without this a floor cell would satisfy
    // `touching` against the cell beside it and read as "at station" while
    // still standing inside the structure. This is the other half of the
    // equivalence with `station_candidates`, which excludes the footprint's
    // own cells from the faces it offers for exactly this reason.
    if footprint.contains(&(worker.x, worker.y)) {
        return false;
    }
    footprint
        .into_iter()
        .any(|(x, y)| touching(worker, Position { x, y }))
}

/// A deployed machine and everything it wants hauled in, as
/// `haul_step_system` needs to see them: enough to answer whether the
/// machine beside a producer will ever pull its output.
///
/// An assembler and its ingredients, or a burning supplier and its fuel —
/// `systems::intake_recipe` is the one question, and it owns its answer for
/// that function's reason.
type Consumer = (Position, Vec<(ItemId, u32)>);

/// Whether a machine at `machine` producing `item` has an **attached
/// building** — an orthogonal neighbour whose own recipe names that item.
///
/// This is what tells a feed buffer from a pile. A producer standing in a
/// line hands its output to the machine beside it, so hoarding a buffer's
/// worth is exactly right; a producer standing alone hands it to nobody, and
/// waiting for twenty units to accumulate before the first trip serves
/// nothing.
///
/// Still asked of the *recipe* rather than of whether the neighbour is
/// currently pulling — a consumer that is unstaffed, starved or clogged is
/// still the building this output belongs to. What it is **not** asked of is
/// every machine standing there: `haul_step_system` builds `consumers` from
/// the assemblers the base has a reason to run, so a Lathe nothing has been
/// ordered from is a bystander rather than a feed target.
fn consumer_beside(machine: Position, item: &ItemId, consumers: &[Consumer]) -> bool {
    consumers
        .iter()
        .any(|(pos, recipe)| touching(*pos, machine) && recipe.iter().any(|(want, _)| want == item))
}

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// The depot a worker at `from` should deliver to: the nearest one it can
/// walk to, fewest Chebyshev tiles away and ties broken by the depot's
/// `(x, y)`.
///
/// Ranked by Chebyshev rather than `walk_field` path cost. That would be a
/// second field per worker per tick for a difference only a wall between
/// two near-equidistant depots can produce, and the tie-break exists for the
/// reason `assembler_system` sorts by position: bevy's query iteration order
/// is not stable, and a base that picked a different depot after a reload is
/// a flaky test waiting to happen.
///
/// **Ranking is not choosing, though: a depot nothing can stand beside is
/// skipped.** Taken on distance alone, a Depot whose one free face was held
/// by an idle body — itself hemmed in by the worker waiting on it — stranded
/// two Mining Nodes for five hundred ticks beside a second Depot with three
/// open sides. `reachable` is asked in rank order and the first yes wins, so
/// the ranking still decides between depots that all work. With none
/// reachable the nearest is the answer anyway, and the worker reads
/// `Stranded` against it — the stall stays loud rather than going quiet.
/// A lone candidate is never walked: whatever the walk said, it would be the
/// answer.
pub(crate) fn nearest_depot(
    depots: &[(Entity, Position)],
    from: Position,
    reachable: impl Fn(Entity, Position) -> bool,
) -> Option<(Entity, Position)> {
    let mut ranked = depots.to_vec();
    ranked.sort_by_key(|(_, p)| (chebyshev(*p, from), p.x, p.y));
    if ranked.len() <= 1 {
        return ranked.first().copied();
    }
    ranked
        .iter()
        .copied()
        .find(|&(e, p)| reachable(e, p))
        .or_else(|| ranked.first().copied())
}

/// Every cell in base space that is already spoken for: the tile each
/// deployed structure stands on, and the tile each body the sim walks is
/// standing in.
///
/// A worker may not walk over a structure for the reason the player may not —
/// `move_player` refuses a tile `find_blocking_structure_at` answers for, and
/// a base a program walks through while its owner walks around stops reading
/// as a physical place. **A body is the same rule one step further**, and it
/// is what stops a crowd converging on one cell: every walk in base space
/// stops at the first tile its arrival test answers for, so with nothing in
/// the way that is the *same* tile for everyone approaching from the same
/// side. The save that forced this had 71 downed programs standing on one
/// cell outside a Repair Bay, drawn as a single glyph.
///
/// **Two iterators rather than one set the caller assembles**, which is the
/// whole of what keeps the two seams agreeing: `post_field` and `crew_reach`
/// have to answer the same question about which tiles are crossable, and a
/// caller that could pass the structures alone would silently be asking a
/// different one. `collect::feeders_by_tile`'s argument — the signature is
/// what holds it, since nothing here fails to compile when a half is
/// forgotten.
///
/// **Emits each structure's anchor only** — its footprint's floor cells stay
/// walkable — and takes `(Position, u8)` pairs rather than a bare
/// `Position` so this and `footprint_tiles` below are built from exactly the
/// same rows: a caller that could hand the two functions different lists
/// would silently let `structure_tiles` and `blocked_tiles` disagree about
/// what a structure's cells are.
///
/// Built per tick from whatever positions the caller can see, rather than
/// cached — the same reasoning `haul_step_system` rebuilds its depot list on:
/// a demolished structure, and a body that has moved on, both have to stop
/// blocking without anything noticing they changed. A cached one would also
/// be a new `Resource`, which shifts query iteration order across the whole
/// engine; `repair::Bays` refuses the same trade for the same reason.
pub(crate) fn blocked_tiles(
    structures: impl Iterator<Item = (Position, u8)>,
    bodies: impl Iterator<Item = Position>,
) -> HashSet<(i32, i32)> {
    structures
        .map(|(p, _)| (p.x, p.y))
        .chain(bodies.map(|p| (p.x, p.y)))
        .collect()
}

/// Every cell of every structure's footprint, anchor and floor alike —
/// `Game::structure_tiles`' body. `blocked_tiles`' pair, over the same
/// `(Position, u8)` rows.
pub(crate) fn footprint_tiles(
    structures: impl Iterator<Item = (Position, u8)>,
) -> HashSet<(i32, i32)> {
    structures
        .flat_map(|(p, side)| crate::tactical::footprint_cells_at((p.x, p.y), side))
        .collect()
}

/// Every tile a worker could stand on to work or deliver to `structure` —
/// the walkable, unoccupied orthogonal neighbours — nearest `from` first,
/// ties by `(x, y)`. Empty when the structure is walled in.
///
/// Specific tiles rather than "get within one step". Descending a cost
/// field until it reads 1 would let a worker park on a *diagonal* at cost 1,
/// never satisfy `at_station`, and spin there for the rest of the run.
///
/// **All four rather than only the nearest**, because the four faces of a
/// target are not always in the same part of the base. `post_field` tries
/// them in this order and stops at the first that routes, so a post that
/// resolved before resolves identically and through the same tile; what
/// changes is that a nearer face walled off from the worker no longer
/// stands for the whole answer. A machine's faces are rarely disconnected,
/// but a dig site's routinely are — a marked cell on a rock spur has the
/// corridor on one side and unbroken rock on the other, and the tie went to
/// the lower `x` regardless of which was which.
///
/// `blocked` is applied here and not only in the walk: an occupied neighbour
/// nominated as a station is a tile the worker is being sent to stand *on*,
/// which is the one thing the walk refuses. A machine packed tightly enough
/// that all four of its neighbours hold buildings has no station at all —
/// and the player could never have collected from it either, since
/// a take is orthogonal and they cannot stand on a building.
fn station_tiles(
    grid: &BaseGrid,
    structure: Position,
    side: u8,
    from: Position,
    blocked: &HashSet<(i32, i32)>,
) -> Vec<Position> {
    let mut tiles = station_candidates(grid, structure, side, blocked);
    tiles.sort_by_key(|p| (chebyshev(*p, from), p.x, p.y));
    tiles
}

/// The same tiles unranked — what `station_tiles` sorts and what
/// `has_station` counts.
///
/// **The asking body's own cell is deliberately not exempted**, and it does
/// not need to be: a body already standing on one of its target's faces never
/// consults this list, because every caller answers `at_station` first —
/// `post_reach` and `reaches` short-circuit on it, and the four walkers
/// (`haul_step_system`, `run_dig_crew`, `step_to_repair` and the caravan) each
/// guard their own call. That short-circuit became load-bearing when bodies
/// started counting as occupied, and seven existing tests in `tests::building`,
/// `tests::chains` and `tests::power` fail if it is taken out. An exemption
/// here was written first and removed again: instrumented, it never fired
/// once across the whole suite or a 1,200-tick run of a real 106-body save.
pub(crate) fn station_candidates(
    grid: &BaseGrid,
    structure: Position,
    side: u8,
    blocked: &HashSet<(i32, i32)>,
) -> Vec<Position> {
    let footprint = crate::tactical::footprint_cells_at((structure.x, structure.y), side);
    let footprint_set: HashSet<(i32, i32)> = footprint.iter().copied().collect();
    let mut seen: HashSet<(i32, i32)> = HashSet::new();
    let mut candidates: Vec<Position> = footprint
        .iter()
        .flat_map(|&(fx, fy)| {
            ORTHOGONAL.iter().map(move |(dx, dy)| Position {
                x: fx + dx,
                y: fy + dy,
            })
        })
        // A cell that is itself part of the footprint is not a face to post
        // at — the worker posts from outside the whole footprint, never on
        // one of its own floor cells.
        .filter(|p| !footprint_set.contains(&(p.x, p.y)))
        .filter(|p| seen.insert((p.x, p.y)))
        .filter(|p| grid.walkable(p.x, p.y) && !blocked.contains(&(p.x, p.y)))
        .collect();
    candidates.sort_by_key(|p| (p.x, p.y));
    candidates
}

/// Whether anything could stand beside `structure` at all — `NoPost::BoxedIn`
/// asked without a worker, and therefore without a walk.
///
/// The existence half of `station_tiles` is the only half that does not
/// depend on who is asking: `from` ranks the faces and never adds or removes
/// one. That is what lets `dig_wants` drop the interior of a marked block
/// before the scheduler budgets for it, sharing this predicate rather than
/// keeping a second copy of what a face is.
///
/// **`structures`, and deliberately not the walk's `blocked` set.** A body
/// standing on the only face of a marked cell is *proof* that something can
/// stand there, so counting bodies here answers the wrong question — and
/// answers it in the one direction that deadlocks: the want is dropped
/// because its own digger is standing at it, the digger is freed and wanders
/// off, the want comes back, the digger walks back, and the base never cuts
/// anything again. Who may take a *particular* face is `station_tiles`'
/// question, asked with the body doing the asking and answered against the
/// full set.
pub(crate) fn has_station(
    grid: &BaseGrid,
    structure: Position,
    side: u8,
    structures: &HashSet<(i32, i32)>,
) -> bool {
    !station_candidates(grid, structure, side, structures).is_empty()
}

/// A route to a post: the walk field, and the worker's own cost in it.
/// Aliased for the same `type_complexity` reason `Hauler` below is.
type PostRoute = (HashMap<(i32, i32), u32>, u32);

/// Why a worker cannot walk to a post. The two are worth telling apart
/// because they leave the player different errands: a boxed-in machine has to
/// be dug out of its own base, while an unroutable one may just need you to
/// stand closer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoPost {
    /// Not one of the structure's four neighbours is both walkable and
    /// unoccupied — nothing can stand next to it at all.
    BoxedIn,
    /// A station exists, but no route reaches it from `from` within
    /// `haul_walk_radius`: too far, or something is in the way.
    NoRoute,
}

/// The walk field a worker at `from` follows to reach `structure`, paired
/// with `from`'s own cost in it — or why there is no such walk.
///
/// One function rather than a distance check beside the walk: `assign_cronjob`
/// refuses exactly what `haul_step_system` cannot deliver, so a posting the
/// menu accepts is a posting that arrives. Returning `from`'s cost with the
/// field is what makes "the worker is in it" a fact the caller is handed
/// rather than one it has to re-establish.
///
/// The worker's own tile is admitted whatever `blocked` says. `place_structure`
/// checks terrain and other structures but never whether a program is standing
/// there, so a building can go up on top of a hauler mid-walk; since the walk
/// filters successors, that worker would otherwise be absent from its own
/// field and frozen for the rest of the run. You may step *off* an occupied
/// tile, never onto one.
///
/// **The faces are tried in `station_tiles`' order and the first that routes
/// wins**, so a post that already resolved resolves through the same tile at
/// the same cost — the extra walks are paid only where the old code was
/// about to answer `NoRoute`, and at most three of them.
fn post_field(
    grid: &BaseGrid,
    from: Position,
    structure: Position,
    side: u8,
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<PostRoute, NoPost> {
    let stations = station_tiles(grid, structure, side, from, blocked);
    if stations.is_empty() {
        return Err(NoPost::BoxedIn);
    }
    let start = (from.x, from.y);
    let reach = haul_walk_radius(pocket_radius);
    for station in stations {
        let field = walk_field((station.x, station.y), reach, |p| {
            (grid.walkable(p.0, p.1) && (p == start || !blocked.contains(&p))).then_some(1)
        });
        if let Some(&here) = field.get(&start) {
            return Ok((field, here));
        }
    }
    Err(NoPost::NoRoute)
}

/// A body's own reach: the tile it set off from, and every tile it can walk
/// to from there. Aliased for the same `type_complexity` reason `PostRoute`
/// above is.
pub(crate) type CrewReach = (Position, HashMap<(i32, i32), u32>);

/// Every tile a worker at `from` could walk to, in one field.
///
/// `post_field` above answers "can this one body reach this one post" and
/// costs a Dijkstra walk per face to do it. `schedule_base_labour` has to
/// ask that of **every** dig want before it budgets bodies for them — a
/// plan can be a hundred cells — so asking it one post at a time is a
/// hundred walks a tick, every tick, for as long as the plan stands.
/// Turned around, one walk answers all of them: build the field from the
/// *body* instead of from the post, and each want is a set lookup through
/// `reaches`.
///
/// The step rule is `post_field`'s, character for character, because the
/// two have to agree about which tiles are crossable. What differs is which
/// end the search is bounded around: `walk_field` bounds successors to a box
/// centred on its origin, so this one is centred on the worker where
/// `post_field`'s is centred on the face. Both boxes are
/// `haul_walk_radius(pocket_radius)` half-width and every cell of the base
/// is within `pocket_radius` of the origin, so up to a base radius of
/// `HAUL_WALK_MAX_TILES / 2` each box contains the whole base and the two
/// agree by construction. Past that the boxes can bind differently — which
/// is why this is a *filter* and `post_reach` stays the authority at the
/// posting itself: a want this keeps is still refused there, and the only
/// thing at stake is a want it drops on a base wider than the walk cap was
/// ever sized for.
pub(crate) fn crew_reach(
    grid: &BaseGrid,
    from: Position,
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> HashMap<(i32, i32), u32> {
    let start = (from.x, from.y);
    walk_field(start, haul_walk_radius(pocket_radius), |p| {
        (grid.walkable(p.0, p.1) && (p == start || !blocked.contains(&p))).then_some(1)
    })
}

/// Whether a body whose `crew_reach` field is `reach` could stand at a post
/// on `structure` — `post_reach` asked of a field already built.
///
/// `at_station` first and for `post_reach`'s reason: a body already touching
/// the post never walks, so it can never be refused for want of a route
/// through a field.
pub(crate) fn reaches(
    grid: &BaseGrid,
    reach: &HashMap<(i32, i32), u32>,
    from: Position,
    structure: Position,
    side: u8,
    blocked: &HashSet<(i32, i32)>,
) -> bool {
    at_station(from, structure, side)
        || station_candidates(grid, structure, side, blocked)
            .iter()
            .any(|s| reach.contains_key(&(s.x, s.y)))
}

/// Whether a worker standing at `from` could ever reach a post at
/// `structure`, and why not when it could not. See `post_field` for why this
/// is the same question the walker asks.
///
/// `at_station` first, in the order `haul_step_system` asks it: a worker
/// already on one of the four tiles it works from never walks, so it never
/// builds a field and cannot be refused for lacking a route through one.
pub(crate) fn post_reach(
    grid: &BaseGrid,
    from: Position,
    structure: Position,
    side: u8,
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<(), NoPost> {
    if at_station(from, structure, side) {
        return Ok(());
    }
    post_field(grid, from, structure, side, blocked, pocket_radius).map(|_| ())
}

/// The one step a worker at `from` takes toward a post at `target` this
/// tick, or `Err` when no route to it exists at all.
///
/// `Ok(None)` means the field admits nowhere better than the tile the worker
/// is already standing on — it waits rather than moving.
///
/// Split out of `haul_step_system`'s tail so `Game::run_dig_crew` walks the
/// *same* walk to a dig site rather than keeping a second copy of it. The
/// crew cannot ride `haul_step_system` itself: that resolves every
/// destination through its `Structure` query, and a `DigSite` is the one
/// `Task` target that is not a structure. `post_reach` is already the shared
/// question "does this posting arrive"; this is the shared answer to "which
/// way".
pub(crate) fn step_to_post(
    grid: &BaseGrid,
    from: Position,
    target: Position,
    side: u8,
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<Option<Position>, NoPost> {
    let (field, here) = post_field(grid, from, target, side, blocked, pocket_radius)?;
    Ok(NEIGHBOURS
        .iter()
        .map(|(dx, dy)| (from.x + dx, from.y + dy))
        .filter_map(|n| field.get(&n).map(|&cost| (cost, n.0, n.1)))
        .min()
        .filter(|&(cost, ..)| cost < here)
        .map(|(_, x, y)| Position { x, y }))
}

/// Moves as much of `load` into `stock`'s output as fits, and reports how
/// much landed. Never past `capacity` — an over-capacity write would make
/// that field a suggestion, and a full depot is a decided failure mode
/// rather than an exception to it.
fn deposit(stock: &mut Stock, load: &Carrying) -> u32 {
    let moved = load.qty.min(stock.output_room());
    if moved > 0 {
        *stock.output.entry(load.item.clone()).or_default() += moved;
    }
    moved
}

/// The worker side of `haul_step_system`. Aliased for the same
/// `type_complexity` reason `systems::CronjobWorker` is. `Without<Structure>`
/// is what lets this hold `Position` mutably while `HaulStructure` below
/// reads it — bevy proves the two disjoint from the filters, not from the
/// fact that nothing is both a program and a building.
///
/// The trailing `Tamed` is read for its `owner` alone, which is what
/// `party::walks_the_base` needs to tell a posted body from a party member
/// holding the tile it was beaten on. A posted worker is one of the bodies a
/// *second* worker may not walk over, and this query holds `Position`
/// mutably — so the posted half of that set cannot come from anywhere else.
type Hauler = (
    Entity,
    &'static mut Position,
    &'static Task,
    Option<&'static Carrying>,
    Option<&'static Stranded>,
    &'static Tamed,
);

type HaulStructure = (
    Entity,
    &'static Position,
    &'static mut Stock,
    &'static Structure,
);

/// The read-only reference data `haul_step_system` consults, bundled so
/// bevy's one-param-per-resource injection doesn't push the system past
/// clippy's argument-count threshold. Bundled rather than `#[allow]`ed for
/// the reason `systems::CronjobLookups` is: the grouping is real, since both
/// are the def tables an errand is decided against.
#[derive(SystemParam)]
pub struct HaulLookups<'w> {
    structures: Res<'w, StructureDb>,
    items: Res<'w, ItemDb>,
    /// Read for one thing only: the tick a stranding *began* on. See
    /// `components::Stranded`. Bundled here rather than added as a bare
    /// system parameter for the reason the two def tables are — the argument
    /// list is already at clippy's threshold.
    clock: Res<'w, resources::GameClock>,
    /// The alert board, bundled for the same reason: `haul_step_system` is
    /// already at clippy's argument-count threshold.
    board: ResMut<'w, crate::alerts::AlertBoard>,
}

/// A body standing in base space that holds no post — what
/// `HaulGround::idle` reads off each one, aliased for `Hauler`'s
/// `type_complexity` reason.
type Bystander = (Entity, &'static Position, &'static Tamed);

/// And which bodies those are. `Without<Task>` is load-bearing twice over:
/// it is what proves this query disjoint from `Hauler`'s `&mut Position` to
/// bevy, and it is the half of the body set `haul_step_system`'s own query
/// cannot see.
type NotPosted = (Without<Task>, Without<Structure>);

/// The ground a walk is measured against, and who is already standing on it.
///
/// Bundled for `HaulLookups`' reason — the argument list is at clippy's
/// threshold — and the grouping is real: all three answer the one question
/// `step_to_post` is about, which cells this body may step into.
#[derive(SystemParam)]
pub struct HaulGround<'w, 's> {
    grid: Res<'w, BaseGrid>,
    /// Every body the sim walks that this system's own query cannot see:
    /// staff between postings, a program off shift on a need, a patient on
    /// its way to a Bay. `Without<Task>` is what proves it disjoint from
    /// `Hauler`'s `&mut Position` to bevy — the two halves of the body set
    /// are split by exactly that filter and by nothing else.
    idle: Query<'w, 's, Bystander, NotPosted>,
    roles: crate::game::party::Roles<'w, 's>,
}

/// Everything `haul_step_system` asks before letting a load leave a machine:
/// has this one backed up, does the base have any reason to run the building
/// beside it — an order somewhere in the queue naming what it makes, or a
/// standing job on it — and is there a shelf that will take what it is
/// holding.
///
/// Bundled for the reason `HaulLookups` is, and the grouping is as real: the
/// four together are the whole of the departure decision. `filters` is the
/// one that is read twice, because where a load may land is the same
/// question asked again once the worker is carrying it — see
/// `Game::depot_accepts`, which is this rule's other reader.
///
/// A separate query rather than a fifth column on `HaulStructure`: the
/// structure query is taken mutably to move stock, and every `get_mut`
/// pattern in the system would have to widen for a component only the
/// deposit legs read.
#[derive(SystemParam)]
pub struct HaulDeparture<'w, 's> {
    statuses: Query<'w, 's, &'static MachineStatus>,
    standing: Query<'w, 's, &'static StandingJob>,
    orders: Res<'w, resources::WorkOrders>,
    filters: Query<'w, 's, &'static crate::components::DepotFilter>,
}

/// The structure side of `haul_step_system`, named so the helpers that read
/// it can say so in a signature.
type HaulStructures<'w, 's> = Query<'w, 's, HaulStructure, Without<Tamed>>;

/// What a posted program is doing with this tick.
///
/// **Derived every tick and never stored**, which is what keeps `Carrying`
/// the feature's only state: a worker's whole plan falls out of where it is
/// standing, what it is holding, and what its machine is short of. A depot
/// demolished mid-walk, a shelf someone else emptied, or a load that turned
/// out not to fit simply produces a different answer on the next tick, with
/// no arrival event to write and nothing to unwind.
///
/// One enum rather than a destination and a separate arrival branch. There
/// are four of each and they have to agree — a worker sent to a depot and
/// then asked to unload into a machine spins there forever — so they are one
/// decision, made once, at the top of the loop.
///
/// Every variant carries **owned** data. That is not incidental: deriving
/// the errand reads the structures query, applying it writes to it, and a
/// variant holding a borrow would keep the read alive across the write.
enum Errand {
    /// Put the held load into this structure's *output*. A depot normally;
    /// the worker's own machine when no depot has room, which puts the goods
    /// back where they came from rather than riding forever.
    Deposit(Entity),
    /// Put the held load into this machine's *input* — a fetched ingredient
    /// arriving home. `room` is this tick's headroom under the same
    /// `INPUT_STOCK_BATCHES` ceiling `assembler_system` pulls to, so a
    /// fetched batch and a pulled one leave the hopper in the same state.
    Load { machine: Entity, room: u32 },
    /// Walk to this depot and draw `want` of `item` off it. The errand a
    /// machine starts when it cannot assemble a batch from its own input or
    /// from anything touching it, but the base has the ingredient in store.
    Collect {
        depot: Entity,
        item: ItemId,
        want: u32,
    },
    /// Nothing to move. Stand at the post — and pick a load up if this is a
    /// machine that has one to shed.
    Tend(Entity),
}

impl Errand {
    /// Where the worker is walking. Every errand has exactly one, which is
    /// what lets the walk below be written once.
    fn destination(&self) -> Entity {
        match self {
            Errand::Deposit(e)
            | Errand::Load { machine: e, .. }
            | Errand::Collect { depot: e, .. }
            | Errand::Tend(e) => *e,
        }
    }
}

/// How much more of `item` a machine's input may hold before it is stocked
/// to `INPUT_STOCK_BATCHES` batches. Zero for an item its recipe does not
/// name, which is what makes a load of *product* fall through to `Deposit`
/// even on a machine whose recipe happens to mention it.
fn input_room(stock: &Stock, recipe: &[(ItemId, u32)], item: &ItemId) -> u32 {
    let Some((_, per_batch)) = recipe.iter().find(|(id, _)| id == item) else {
        return 0;
    };
    (per_batch * tuning::INPUT_STOCK_BATCHES)
        .saturating_sub(stock.input.get(item).copied().unwrap_or(0))
}

/// The first thing on `machine`'s intake it cannot cover a batch of out of
/// its own input or the machines touching it — the one an errand to a shelf
/// would be for. `None` when everything it needs is already within reach.
///
/// A batch is an assembler's recipe quantity or a supplier's
/// `tuning::POWER_UPKEEP_CELLS_PER_WINDOW`, whichever `intake_recipe`
/// reported; nothing here knows which it is holding.
///
/// The store term is deliberately **zero**: this is the question of whether
/// the local chain can cover it, and the shelf is the answer being
/// considered rather than part of the question.
/// `work_orders::batch_within_reach` is the shared rule, so the machine the
/// scheduler staffs off store is the machine this fetches for.
fn missing_ingredient(
    machine: Entity,
    recipe: &[(ItemId, u32)],
    structures: &HaulStructures,
    by_tile: &HashMap<(i32, i32), Entity>,
) -> Option<ItemId> {
    let (_, pos, stock, _) = structures.get(machine).ok()?;
    recipe.iter().find_map(|(item, per_batch)| {
        let held = stock.input.get(item).copied().unwrap_or(0);
        let beside: u32 = ORTHOGONAL
            .into_iter()
            .filter_map(|(dx, dy)| by_tile.get(&(pos.x + dx, pos.y + dy)).copied())
            .filter_map(|feeder| structures.get(feeder).ok())
            .map(|(_, _, s, _)| s.output.get(item).copied().unwrap_or(0))
            .sum();
        (!crate::game::base::work_orders::batch_within_reach(held, beside, 0, *per_batch))
            .then(|| item.clone())
    })
}

/// The nearest of `stores` actually holding `item` in its output.
///
/// `stores` is every Depot whatever its state — or, for a burning supplier,
/// every structure — not the `depots` list the delivery leg uses — that one is filtered to those with *room*, and a full
/// shelf is still one to take something off.
fn nearest_store_holding(
    stores: &[(Entity, Position)],
    from: Position,
    item: &ItemId,
    structures: &HaulStructures,
    reachable: impl Fn(Entity, Position) -> bool,
) -> Option<Entity> {
    let holding: Vec<(Entity, Position)> = stores
        .iter()
        .copied()
        .filter(|(e, _)| {
            structures
                .get(*e)
                .is_ok_and(|(_, _, s, _)| s.output.get(item).copied().unwrap_or(0) > 0)
        })
        .collect();
    nearest_depot(&holding, from, reachable).map(|(e, _)| e)
}

/// What a posted program does with the tick: take a load off a clogged
/// machine, carry it toward a depot, put it down, or walk back to its post.
///
/// Everything here is derived rather than stored. A worker's destination is
/// the nearest depot with room while it is carrying and its own machine
/// otherwise, so dropping the load is all it takes to turn around, and there
/// is no arrival event to write. A depot demolished mid-walk, or filled up
/// by someone else, simply stops being the answer on the next tick.
/// One `Record::Haul`, when a leg actually moved something.
///
/// A zero is dropped rather than recorded: an errand that moved nothing is a
/// Depot that filled or emptied while the worker walked, and it is already
/// visible as the stall the machine reports. Recording it would put a row in
/// the log for every tick a full base spends re-deciding the same errand.
#[allow(clippy::too_many_arguments)]
fn note_haul(
    telemetry: &mut crate::resources::BattleTelemetry,
    tick: u64,
    post: (Position, &str),
    errand: &str,
    item: &ItemId,
    qty: u32,
    distance: u32,
) {
    if qty == 0 {
        return;
    }
    let (pos, kind) = post;
    crate::base_ledger::record_in_system(telemetry, || crate::telemetry::Record::Haul {
        tick,
        machine: (pos.x, pos.y),
        kind: kind.to_string(),
        errand: errand.to_string(),
        item: item.0.clone(),
        qty,
        distance,
    });
}

pub(crate) fn haul_step_system(
    mut workers: Query<Hauler, (With<Tamed>, Without<Structure>)>,
    mut structures: Query<HaulStructure, Without<Tamed>>,
    departure: HaulDeparture,
    defs: HaulLookups,
    ground: HaulGround,
    mut telemetry: ResMut<crate::resources::BattleTelemetry>,
    mut commands: Commands,
) {
    let HaulGround { grid, idle, roles } = ground;
    let HaulDeparture {
        statuses,
        standing,
        orders,
        filters,
    } = departure;
    // The crew's half of the Depot filter rule. A refusal reads exactly as
    // a full shelf everywhere below, which is what lets the whole feature
    // be two `filter` calls rather than a state a worker has to carry.
    let accepts = |depot: Entity, item: &ItemId| {
        filters
            .get(depot)
            .map_or(true, |f| !f.denied.contains(item))
    };
    let HaulLookups {
        structures: db,
        items,
        clock,
        mut board,
    } = defs;
    // **Grown as bodies move, never shrunk** — `drift_idle_staff`'s `held`
    // rule, and for its reason: a vacated cell stays spoken for until the
    // next tick, which costs a body one step it will be offered again and
    // saves this from depending on the order the pool is walked in. Built
    // once and read for the rest of the tick, two haulers heading for the
    // same free cell would both be told it was free.
    let mut blocked = blocked_tiles(
        structures
            .iter()
            .map(|(_, p, _, s)| (*p, db.get(&s.kind).map(|d| d.footprint).unwrap_or(1))),
        workers
            .iter()
            .filter(|(entity, _, task, _, _, tamed)| {
                crate::game::party::walks_the_base(roles.of(*entity, tamed.owner), Some(task.kind))
            })
            .map(|(_, p, ..)| *p)
            .chain(
                idle.iter()
                    .filter(|(entity, _, tamed)| {
                        crate::game::party::walks_the_base(roles.of(*entity, tamed.owner), None)
                    })
                    .map(|(_, p, _)| *p),
            ),
    );
    // What bounds the Dijkstra field a walker rebuilds each tick: how far
    // the base actually reaches, measured off the grid rather than taken
    // from the size the pocket started at. See `BaseGrid::radius`.
    let pocket_radius = grid.radius();

    // Rebuilt every tick rather than cached: this is the list that makes a
    // demolished or newly-filled depot stop being a destination without
    // anything having to notice it changed.
    let depots: Vec<(Entity, Position)> = structures
        .iter()
        .filter(|(_, _, stock, s)| {
            stock.output_room() > 0 && db.get(&s.kind).is_some_and(|d| d.stores)
        })
        .map(|(e, p, _, _)| (e, *p))
        .collect();

    // Every assembler the base has a reason to run, and what it takes — so a
    // producer can be asked whether the building beside it will ever pull its
    // output. Built per tick for the same reason `depots` is: a demolished
    // consumer, or one whose order has just been filled, has to stop being
    // one without anything noticing it changed.
    //
    // The reason is either the queue naming what it makes, however far down a
    // recipe tree, or a standing work job — the player saying "keep this
    // running" outside any order. Without one it pulls nothing at all
    // (`assembler_system` returns before its pull phase with no program
    // posted), so counting it would reserve a whole buffer for a machine that
    // will never take it.
    let needed = work_orders::queue_needs(&orders.0, &items);
    let consumers: Vec<Consumer> = structures
        .iter()
        .filter_map(|(e, p, _, s)| {
            let def = db.get(&s.kind)?;
            let recipe = intake_recipe(def, &items)?;
            // **A burning supplier is always a consumer.** There is no order
            // that names its fuel and no program to post to it, so the two
            // reasons an assembler counts cannot apply — and it is off the
            // grid without the cell either way. Left out, a Conduit standing
            // beside a Recharger Node would have its cells hauled off to a
            // depot for somebody to walk back.
            let wanted = def.power_upkeep.is_some()
                || produced_item(def).is_some_and(|item| needed.contains(item))
                || standing.get(e).is_ok_and(|job| job.work);
            wanted.then_some((*p, recipe))
        })
        .collect();

    // Every Depot whatever its state — the *collect* leg's list, as against
    // `depots` above, which is filtered to those with room to take a
    // delivery. A full shelf is still one to draw an ingredient off.
    let stores: Vec<(Entity, Position)> = structures
        .iter()
        .filter(|(_, _, _, s)| db.get(&s.kind).is_some_and(|d| d.stores))
        .map(|(e, p, _, _)| (e, *p))
        .collect();

    // A burning supplier's collect list: every structure's output, not the
    // shelves alone. Its fuel is the grid, and a Conduit's cells stranded
    // behind a full Depot kept a real base dark for good — see
    // `Game::fuel_wants`, whose stock gate counts the same buffers.
    let every_structure: Vec<(Entity, Position)> =
        structures.iter().map(|(e, p, _, _)| (e, *p)).collect();

    let by_tile: HashMap<(i32, i32), Entity> = structures
        .iter()
        .map(|(e, p, _, _)| ((p.x, p.y), e))
        .collect();

    // Sorted for the reason `assembler_system` sorts its machines: two
    // workers competing for the last slot in a depot must resolve the same
    // way every run, and bevy's iteration order does not promise that.
    let mut order: Vec<(i32, i32, Entity)> = workers
        .iter()
        .filter(|(_, _, task, ..)| matches!(task.kind, TaskKind::GatherResource))
        .map(|(e, p, ..)| (p.x, p.y, e))
        .collect();
    order.sort_unstable();

    for (.., worker) in order {
        let Ok((_, worker_pos, task, carrying, stranded, _)) = workers.get(worker) else {
            continue;
        };
        let (worker_pos, carrying, stranded) = (*worker_pos, carrying.cloned(), stranded.copied());
        let machine = task.target;

        // The whole of what this worker is doing with the tick, decided once
        // — see `Errand`. Scoped so every read of `structures` is finished
        // before the arrival below writes to it.
        let errand = {
            // `post_reach` asked of a candidate depot, for `nearest_depot`.
            let reachable = |depot: Entity, at: Position| {
                let side = structures
                    .get(depot)
                    .ok()
                    .and_then(|(_, _, _, s)| db.get(&s.kind))
                    .map(|d| d.footprint)
                    .unwrap_or(1);
                post_reach(&grid, worker_pos, at, side, &blocked, pocket_radius).is_ok()
            };
            let def = structures
                .get(machine)
                .ok()
                .and_then(|(_, _, _, s)| db.get(&s.kind));
            let recipe = def.and_then(|def| intake_recipe(def, &items));
            // The burner itself is in `every_structure`, harmlessly: a
            // fetched cell lands in its *input*, never its output.
            let sources = if def.is_some_and(|d| d.power_upkeep.is_some()) {
                &every_structure
            } else {
                &stores
            };
            let recipe = recipe.as_deref();
            match &carrying {
                Some(load) => {
                    // A load the machine's own recipe has room for is an
                    // ingredient coming home; anything else is product being
                    // cleared. That is the whole of the direction test, and
                    // it is why `Carrying` needs no field saying which way
                    // the worker is walking — which matters, since
                    // `CreatureSave::carrying` is a positional tuple RON
                    // cannot widen.
                    let room = recipe
                        .and_then(|r| {
                            structures
                                .get(machine)
                                .ok()
                                .map(|(_, _, stock, _)| input_room(stock, r, &load.item))
                        })
                        .unwrap_or(0);
                    if room > 0 {
                        Errand::Load { machine, room }
                    } else {
                        let taking: Vec<(Entity, Position)> = depots
                            .iter()
                            .copied()
                            .filter(|(e, _)| accepts(*e, &load.item))
                            .collect();
                        Errand::Deposit(
                            nearest_depot(&taking, worker_pos, reachable)
                                .map(|(e, _)| e)
                                .unwrap_or_else(|| {
                                    // Every depot full, refusing this item, or
                                    // none built: the load goes back where it
                                    // came from and re-clogs the machine. The
                                    // base stalls loudly rather than the goods
                                    // vanishing.
                                    //
                                    // The false→true edge only — the branch
                                    // runs for every worker with nowhere to
                                    // deposit, every tick, and `count` exists
                                    // to say how many times this happened,
                                    // not how many workers hit it this tick.
                                    if !board.depots_full {
                                        board.depots_full = true;
                                        alerts::post(
                                            &mut board,
                                            AlertKind::DepotsFull,
                                            "depots",
                                            "A hauled load has nowhere to go — no Depot will take it."
                                                .to_string(),
                                        );
                                    }
                                    machine
                                }),
                        )
                    }
                }
                None => recipe
                    .and_then(|r| missing_ingredient(machine, r, &structures, &by_tile))
                    .and_then(|item| {
                        let want = recipe
                            .and_then(|r| {
                                structures
                                    .get(machine)
                                    .ok()
                                    .map(|(_, _, stock, _)| input_room(stock, r, &item))
                            })
                            .unwrap_or(0)
                            .min(tuning::HAUL_CARRY_CAPACITY);
                        let depot = nearest_store_holding(
                            sources,
                            worker_pos,
                            &item,
                            &structures,
                            reachable,
                        )?;
                        (want > 0).then_some(Errand::Collect { depot, item, want })
                    })
                    .unwrap_or(Errand::Tend(machine)),
            }
        };
        let Ok((_, dest_pos, _, dest_structure)) = structures.get(errand.destination()) else {
            continue;
        };
        let dest_pos = *dest_pos;
        let dest_side = db
            .get(&dest_structure.kind)
            .map(|d| d.footprint)
            .unwrap_or(1);
        // Whether this errand's destination is a real Depot rather than the
        // fallback `Errand::Deposit(machine)` bouncing a load back into its
        // own machine — read here, off `dest_structure`, before it goes out
        // of scope, since the latch below must clear only on the former.
        let dest_is_depot = db.get(&dest_structure.kind).is_some_and(|d| d.stores);
        // Read before the arms, which take `structures` mutably. The post
        // and not the worker's own tile: by the time an errand acts the two
        // are the same place, and what the analysis groups by is the
        // machine.
        let (post, post_kind) = structures
            .get(machine)
            .map(|(_, p, _, s)| (*p, s.kind.clone()))
            .unwrap_or((worker_pos, String::new()));
        let legs = chebyshev(post, dest_pos).max(0) as u32;

        if at_station(worker_pos, dest_pos, dest_side) {
            // A worker standing where it meant to stand is not stranded,
            // whatever it was a tick ago. Cleared here as well as after a
            // successful field because this branch returns before one is
            // ever built — a depot deployed beside a stranded worker would
            // otherwise leave the marker set while it went back to work.
            commands.entity(worker).remove::<Stranded>();
            match errand {
                Errand::Deposit(depot) => {
                    let Some(load) = carrying else {
                        continue;
                    };
                    let Ok((_, _, mut stock, _)) = structures.get_mut(depot) else {
                        continue;
                    };
                    let moved = deposit(&mut stock, &load);
                    // Cleared by *any* successful deposit into a real
                    // Depot, not only one into the depot that was full — a
                    // base with several Depots is unstuck the moment any of
                    // them has room again. **Not** by the fallback bounce
                    // into the worker's own machine (`dest_is_depot` is
                    // false there): that only puts the goods back where they
                    // came from, so clearing on it let every bounced load
                    // re-arm the latch and repost.
                    if moved > 0 && dest_is_depot {
                        board.depots_full = false;
                    }
                    note_haul(
                        &mut telemetry,
                        clock.tick,
                        (post, &post_kind),
                        "deposit",
                        &load.item,
                        moved,
                        legs,
                    );
                    if moved == load.qty {
                        commands.entity(worker).remove::<Carrying>();
                    } else if moved > 0 {
                        commands.entity(worker).insert(Carrying {
                            item: load.item,
                            qty: load.qty - moved,
                        });
                    }
                }
                // An ingredient arriving home. The one write to a machine's
                // `input` outside `assembler_system`, and it is not the
                // exception to that asymmetry it looks like: `Stock::output`
                // is public so a *neighbour* can pull from it, and nothing
                // outside a machine may reach into its input. This is the
                // machine's own posted program loading its own hopper.
                Errand::Load { machine, room } => {
                    let Some(load) = carrying else {
                        continue;
                    };
                    let moved = load.qty.min(room);
                    if moved > 0 {
                        let Ok((_, _, mut stock, _)) = structures.get_mut(machine) else {
                            continue;
                        };
                        *stock.input.entry(load.item.clone()).or_default() += moved;
                    }
                    note_haul(
                        &mut telemetry,
                        clock.tick,
                        (post, &post_kind),
                        "load",
                        &load.item,
                        moved,
                        legs,
                    );
                    // Nothing landed means the hopper filled while the worker
                    // walked. It keeps the load, and next tick's errand is a
                    // `Deposit` — there is no stuck state to write.
                    if moved == load.qty {
                        commands.entity(worker).remove::<Carrying>();
                    } else if moved > 0 {
                        commands.entity(worker).insert(Carrying {
                            item: load.item,
                            qty: load.qty - moved,
                        });
                    }
                }
                Errand::Collect { depot, item, want } => {
                    let Ok((_, _, mut stock, _)) = structures.get_mut(depot) else {
                        continue;
                    };
                    let taken = take_from(&mut stock, &item, want);
                    note_haul(
                        &mut telemetry,
                        clock.tick,
                        (post, &post_kind),
                        "collect",
                        &item,
                        taken,
                        legs,
                    );
                    if taken > 0 {
                        commands
                            .entity(worker)
                            .insert(Carrying { item, qty: taken });
                    }
                }
                // At its post with empty hands and nothing to fetch, which is
                // where the outbound errands start. Two of them do.
                //
                // A **clogged** machine is the original: the cycle is already
                // lost, so the walk costs nothing that was not lost anyway.
                //
                // A machine with **no attached building** is the second, and
                // it fires on whatever the last cycle put in the buffer
                // rather than waiting for twenty units to pile up. Nobody
                // downstream is going to take them, so the buffer is not a
                // buffer — it is a heap sitting where the base cannot count
                // it. The cost is real and deliberate: the worker is away
                // walking for most of the round trip and produces nothing
                // while it is (`task_progress_system` gates on `at_station`),
                // so where the depot stands is now what paces a lone
                // extractor.
                //
                // "No attached building" counts a **bystander** as well as an
                // empty tile: a machine whose recipe names this product but
                // which nothing has asked to run is not downstream of
                // anything, and it pulls nothing at all while unstaffed. See
                // `consumers` above for what makes one count.
                //
                // With nowhere to take a load there is no errand at all,
                // which is what leaves a depot-less base behaving exactly as
                // it did — and a base whose every shelf refuses this
                // product behaving exactly like a depot-less one.
                Errand::Tend(machine) => {
                    if depots.is_empty() {
                        continue;
                    }
                    let clogged = statuses.get(machine) == Ok(&MachineStatus::Clogged);
                    let attached = structures.get(machine).ok().is_some_and(|(_, p, _, s)| {
                        db.get(&s.kind)
                            .and_then(produced_item)
                            .is_some_and(|item| consumer_beside(*p, item, &consumers))
                    });
                    if !clogged && attached {
                        continue;
                    }
                    let Ok((_, _, mut stock, _)) = structures.get_mut(machine) else {
                        continue;
                    };
                    if let Some(load) = take_haul_load(&mut stock, |item| {
                        depots.iter().any(|(e, _)| accepts(*e, item))
                    }) {
                        commands.entity(worker).insert(load);
                    }
                }
            }
            continue;
        }

        // No field means no route. `assign_cronjob` refuses that case up
        // front, so what is left here is a route lost after the posting — a
        // wall of new buildings, a depot demolished behind one, or ground
        // that changed. The worker stands still, and the marker is what turns
        // its machine's status from `Unstaffed` into `Stranded`.
        let Ok(step) = step_to_post(
            &grid,
            worker_pos,
            dest_pos,
            dest_side,
            &blocked,
            pocket_radius,
        ) else {
            // Only on entry: `since` is the start of the episode, and
            // rewriting it every tick would leave nothing able to tell a
            // route that has just broken from one broken an hour ago. See
            // `components::Stranded`.
            if stranded.is_none() {
                commands
                    .entity(worker)
                    .insert(Stranded { since: clock.tick });
            }
            continue;
        };
        commands.entity(worker).remove::<Stranded>();
        if let Some(next) = step
            && let Ok((_, mut pos, ..)) = workers.get_mut(worker)
        {
            *pos = next;
            blocked.insert((next.x, next.y));
        }
    }
}
