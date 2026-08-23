//! Posted programs walking: taking a post, carrying a clogged machine's
//! output to a depot, and coming back.
//!
//! `components::Carrying` is the only state this feature stores. Where a
//! worker is headed and whether it has arrived are both read off `Position`,
//! so the two cannot disagree with each other the way a hand-maintained
//! `HaulState` enum would.

use std::collections::HashSet;

use bevy_ecs::system::SystemParam;

use crate::base_grid::BaseGrid;
use crate::game::base::collect::ORTHOGONAL;
use crate::game::base::work_orders;
use crate::game::pursuit::walk_field;
use crate::items::ItemId;
use crate::systems::{assembly_recipe, produced_item};
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
pub(crate) fn take_haul_load(stock: &mut Stock) -> Option<Carrying> {
    // Cloned out before the map is touched: the borrow behind `.keys()` is
    // still live otherwise.
    let item = stock.output.keys().next().cloned()?;
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
    let held = stock.output.get(item).copied().unwrap_or(0);
    let taken = qty.min(held);
    if taken == 0 {
        return 0;
    }
    if held == taken {
        stock.output.remove(item);
    } else {
        stock.output.insert(item.clone(), held - taken);
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

/// True when `worker` stands on one of the four tiles `structure` can be
/// reached from.
pub(crate) fn at_station(worker: Position, structure: Position) -> bool {
    touching(worker, structure)
}

/// A deployed assembler and the ingredient list it runs, as
/// `haul_step_system` needs to see them: enough to answer whether the
/// machine beside a producer will ever pull its output.
type Consumer<'a> = (Position, &'a [(ItemId, u32)]);

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

/// The depot a worker at `from` should deliver to: fewest Chebyshev tiles
/// away, ties broken by the depot's `(x, y)`.
///
/// Deliberately not `walk_field` path cost. That would be a second field per
/// worker per tick for a difference only a wall between two near-equidistant
/// depots can produce, and the tie-break exists for the reason
/// `assembler_system` sorts by position: bevy's query iteration order is not
/// stable, and a base that picked a different depot after a reload is a
/// flaky test waiting to happen.
pub(crate) fn nearest_depot(
    depots: &[(Entity, Position)],
    from: Position,
) -> Option<(Entity, Position)> {
    depots
        .iter()
        .min_by_key(|(_, p)| (chebyshev(*p, from), p.x, p.y))
        .copied()
}

/// Every tile a deployed structure stands on. A worker may not walk over one,
/// for the reason the player may not: `move_player` refuses a tile
/// `find_blocking_structure_at` answers for, and a base that a program walks
/// through while its owner walks around stops reading as a physical place.
///
/// Built per tick from whatever positions the caller can see, rather than
/// cached — the same reasoning `haul_step_system` rebuilds its depot list on:
/// a demolished structure has to stop blocking without anything noticing it
/// changed.
pub(crate) fn structure_tiles(positions: impl Iterator<Item = Position>) -> HashSet<(i32, i32)> {
    positions.map(|p| (p.x, p.y)).collect()
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
/// `collect_adjacent` is orthogonal and they cannot stand on a building.
fn station_tiles(
    grid: &BaseGrid,
    structure: Position,
    from: Position,
    blocked: &HashSet<(i32, i32)>,
) -> Vec<Position> {
    let mut tiles = station_candidates(grid, structure, blocked);
    tiles.sort_by_key(|p| (chebyshev(*p, from), p.x, p.y));
    tiles
}

/// The same tiles unranked — what `station_tiles` sorts and what
/// `has_station` counts.
fn station_candidates(
    grid: &BaseGrid,
    structure: Position,
    blocked: &HashSet<(i32, i32)>,
) -> Vec<Position> {
    ORTHOGONAL
        .iter()
        .map(|(dx, dy)| Position {
            x: structure.x + dx,
            y: structure.y + dy,
        })
        .filter(|p| grid.walkable(p.x, p.y) && !blocked.contains(&(p.x, p.y)))
        .collect()
}

/// Whether anything could stand beside `structure` at all — `NoPost::BoxedIn`
/// asked without a worker, and therefore without a walk.
///
/// The existence half of `station_tiles` is the only half that does not
/// depend on who is asking: `from` ranks the faces and never adds or removes
/// one. That is what lets `dig_wants` drop the interior of a marked block
/// before the scheduler budgets for it, sharing this predicate rather than
/// keeping a second copy of what a face is.
pub(crate) fn has_station(
    grid: &BaseGrid,
    structure: Position,
    blocked: &HashSet<(i32, i32)>,
) -> bool {
    !station_candidates(grid, structure, blocked).is_empty()
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
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<PostRoute, NoPost> {
    let stations = station_tiles(grid, structure, from, blocked);
    if stations.is_empty() {
        return Err(NoPost::BoxedIn);
    }
    let start = (from.x, from.y);
    let reach = haul_walk_radius(pocket_radius);
    for station in stations {
        let field = walk_field((station.x, station.y), reach, |p| {
            grid.walkable(p.0, p.1) && (p == start || !blocked.contains(&p))
        });
        if let Some(&here) = field.get(&start) {
            return Ok((field, here));
        }
    }
    Err(NoPost::NoRoute)
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
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<(), NoPost> {
    if at_station(from, structure) {
        return Ok(());
    }
    post_field(grid, from, structure, blocked, pocket_radius).map(|_| ())
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
    blocked: &HashSet<(i32, i32)>,
    pocket_radius: i32,
) -> Result<Option<Position>, NoPost> {
    let (field, here) = post_field(grid, from, target, blocked, pocket_radius)?;
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
type Hauler = (
    Entity,
    &'static mut Position,
    &'static Task,
    Option<&'static Carrying>,
    Option<&'static Stranded>,
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
}

/// Everything `haul_step_system` asks before letting a load leave a machine:
/// has this one backed up, and does the base have any reason to run the
/// building beside it — an order somewhere in the queue naming what it makes,
/// or a standing job on it.
///
/// Bundled for the reason `HaulLookups` is, and the grouping is as real: the
/// three together are the whole of the departure decision, and nothing else
/// in the system reads any of them.
#[derive(SystemParam)]
pub struct HaulDeparture<'w, 's> {
    statuses: Query<'w, 's, &'static MachineStatus>,
    standing: Query<'w, 's, &'static StandingJob>,
    orders: Res<'w, resources::WorkOrders>,
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

/// The first ingredient `machine` cannot assemble a batch of out of its own
/// input or the machines touching it — the one an errand to a shelf would be
/// for. `None` when everything it needs is already within reach.
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

/// The nearest Depot actually holding `item`.
///
/// `stores` is every Depot whatever its state, not the `depots` list the
/// delivery leg uses — that one is filtered to those with *room*, and a full
/// shelf is still one to take something off.
fn nearest_store_holding(
    stores: &[(Entity, Position)],
    from: Position,
    item: &ItemId,
    structures: &HaulStructures,
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
    nearest_depot(&holding, from).map(|(e, _)| e)
}

/// What a posted program does with the tick: take a load off a clogged
/// machine, carry it toward a depot, put it down, or walk back to its post.
///
/// Everything here is derived rather than stored. A worker's destination is
/// the nearest depot with room while it is carrying and its own machine
/// otherwise, so dropping the load is all it takes to turn around, and there
/// is no arrival event to write. A depot demolished mid-walk, or filled up
/// by someone else, simply stops being the answer on the next tick.
pub(crate) fn haul_step_system(
    mut workers: Query<Hauler, (With<Tamed>, Without<Structure>)>,
    mut structures: Query<HaulStructure, Without<Tamed>>,
    departure: HaulDeparture,
    defs: HaulLookups,
    grid: Res<BaseGrid>,
    mut commands: Commands,
) {
    let HaulDeparture {
        statuses,
        standing,
        orders,
    } = departure;
    let HaulLookups {
        structures: db,
        items,
        clock,
    } = defs;
    let blocked = structure_tiles(structures.iter().map(|(_, p, _, _)| *p));
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
            let recipe = assembly_recipe(def, &items)?;
            let wanted =
                needed.contains(produced_item(def)?) || standing.get(e).is_ok_and(|job| job.work);
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
        let Ok((_, worker_pos, task, carrying, stranded)) = workers.get(worker) else {
            continue;
        };
        let (worker_pos, carrying, stranded) = (*worker_pos, carrying.cloned(), stranded.copied());
        let machine = task.target;

        // The whole of what this worker is doing with the tick, decided once
        // — see `Errand`. Scoped so every read of `structures` is finished
        // before the arrival below writes to it.
        let errand = {
            let recipe = structures
                .get(machine)
                .ok()
                .and_then(|(_, _, _, s)| db.get(&s.kind))
                .and_then(|def| assembly_recipe(def, &items));
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
                        Errand::Deposit(
                            nearest_depot(&depots, worker_pos)
                                .map(|(e, _)| e)
                                // Every depot full, or none built: the load
                                // goes back where it came from and re-clogs
                                // the machine. The base stalls loudly rather
                                // than the goods vanishing.
                                .unwrap_or(machine),
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
                        let depot = nearest_store_holding(&stores, worker_pos, &item, &structures)?;
                        (want > 0).then_some(Errand::Collect { depot, item, want })
                    })
                    .unwrap_or(Errand::Tend(machine)),
            }
        };
        let Ok((_, dest_pos, _, _)) = structures.get(errand.destination()) else {
            continue;
        };
        let dest_pos = *dest_pos;

        if at_station(worker_pos, dest_pos) {
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
                // it did.
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
                    if let Some(load) = take_haul_load(&mut stock) {
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
        let Ok(step) = step_to_post(&grid, worker_pos, dest_pos, &blocked, pocket_radius) else {
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
        }
    }
}
