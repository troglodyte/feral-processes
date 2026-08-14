//! Posted programs walking: taking a post, carrying a clogged machine's
//! output to a depot, and coming back.
//!
//! `components::Carrying` is the only state this feature stores. Where a
//! worker is headed and whether it has arrived are both read off `Position`,
//! so the two cannot disagree with each other the way a hand-maintained
//! `HaulState` enum would.

use std::collections::HashSet;

use crate::game::base::collect::ORTHOGONAL;
use crate::game::pursuit::walk_field;
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
    let held = stock.output.get(&item).copied().unwrap_or(0);
    let qty = held.min(tuning::HAUL_CARRY_CAPACITY);
    if qty == 0 {
        return None;
    }
    if held == qty {
        stock.output.remove(&item);
    } else {
        stock.output.insert(item.clone(), held - qty);
    }
    Some(Carrying { item, qty })
}

/// True when `worker` stands on one of the four tiles `structure` can be
/// reached from.
///
/// `collect::ORTHOGONAL` rather than a second adjacency list: a worker's
/// arrival and a player's collect ask the same question, and the moment they
/// could differ the base stops reading as a physical line. Movement itself
/// stays 8-directional — only arrival is orthogonal.
pub(crate) fn at_station(worker: Position, structure: Position) -> bool {
    ORTHOGONAL
        .iter()
        .any(|(dx, dy)| worker.x == structure.x + dx && worker.y == structure.y + dy)
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

/// The tile a worker must stand on to work or deliver to `structure`: the
/// walkable, unoccupied orthogonal neighbour nearest `from`, ties by
/// `(x, y)`. `None` when the structure is walled in.
///
/// A specific tile rather than "get within one step". Descending a cost
/// field until it reads 1 would let a worker park on a *diagonal* at cost 1,
/// never satisfy `at_station`, and spin there for the rest of the run.
///
/// `blocked` is applied here and not only in the walk: an occupied neighbour
/// nominated as a station is a tile the worker is being sent to stand *on*,
/// which is the one thing the walk refuses. A machine packed tightly enough
/// that all four of its neighbours hold buildings has no station at all —
/// and the player could never have collected from it either, since
/// `collect_adjacent` is orthogonal and they cannot stand on a building.
pub(crate) fn station_tile(
    map: &mut WorldMap,
    structure: Position,
    from: Position,
    blocked: &HashSet<(i32, i32)>,
) -> Option<Position> {
    ORTHOGONAL
        .iter()
        .map(|(dx, dy)| Position {
            x: structure.x + dx,
            y: structure.y + dy,
        })
        .filter(|p| map.tile(p.x, p.y).walkable && !blocked.contains(&(p.x, p.y)))
        .min_by_key(|p| (chebyshev(*p, from), p.x, p.y))
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
fn post_field(
    map: &mut WorldMap,
    from: Position,
    structure: Position,
    blocked: &HashSet<(i32, i32)>,
    build_radius: i32,
) -> Result<PostRoute, NoPost> {
    let station = station_tile(map, structure, from, blocked).ok_or(NoPost::BoxedIn)?;
    let start = (from.x, from.y);
    let reach = haul_walk_radius(build_radius);
    let field = walk_field(map, (station.x, station.y), reach, |t, p| {
        t.walkable && (p == start || !blocked.contains(&p))
    });
    let here = *field.get(&start).ok_or(NoPost::NoRoute)?;
    Ok((field, here))
}

/// Whether a worker standing at `from` could ever reach a post at
/// `structure`, and why not when it could not. See `post_field` for why this
/// is the same question the walker asks.
///
/// `at_station` first, in the order `haul_step_system` asks it: a worker
/// already on one of the four tiles it works from never walks, so it never
/// builds a field and cannot be refused for lacking a route through one.
pub(crate) fn post_reach(
    map: &mut WorldMap,
    from: Position,
    structure: Position,
    blocked: &HashSet<(i32, i32)>,
    build_radius: i32,
) -> Result<(), NoPost> {
    if at_station(from, structure) {
        return Ok(());
    }
    post_field(map, from, structure, blocked, build_radius).map(|_| ())
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
);

type HaulStructure = (
    Entity,
    &'static Position,
    &'static mut Stock,
    &'static Structure,
);

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
    statuses: Query<&MachineStatus>,
    db: Res<StructureDb>,
    platform: Res<Platform>,
    mut map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    let blocked = structure_tiles(structures.iter().map(|(_, p, _, _)| *p));
    // The cached radius rather than `Game::build_radius`: a system has no
    // `Game`, and `Platform` is where that derivation is kept readable from
    // a borrow this shallow.
    let build_radius = platform.radius;

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

    // Sorted for the reason `assembler_system` sorts its machines: two
    // workers competing for the last slot in a depot must resolve the same
    // way every run, and bevy's iteration order does not promise that.
    let mut order: Vec<(i32, i32, Entity)> = workers
        .iter()
        .filter(|(_, _, task, _)| matches!(task.kind, TaskKind::GatherResource))
        .map(|(e, p, _, _)| (p.x, p.y, e))
        .collect();
    order.sort_unstable();

    for (.., worker) in order {
        let Ok((_, worker_pos, task, carrying)) = workers.get(worker) else {
            continue;
        };
        let (worker_pos, carrying) = (*worker_pos, carrying.cloned());
        let machine = task.target;

        let destination = match &carrying {
            Some(_) => nearest_depot(&depots, worker_pos)
                .map(|(e, _)| e)
                // Every depot full, or none built: the load goes back where
                // it came from and re-clogs the machine. The base stalls
                // loudly rather than the goods vanishing.
                .unwrap_or(machine),
            None => machine,
        };
        let Ok((_, dest_pos, _, _)) = structures.get(destination) else {
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
            match carrying {
                Some(load) => {
                    let Ok((_, _, mut stock, _)) = structures.get_mut(destination) else {
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
                // At its post with empty hands. A clogged machine is where
                // the errand starts — the cycle is already lost, so the walk
                // costs nothing that was not lost anyway — and with nowhere
                // to take a load there is no errand to start, which is what
                // leaves a depot-less base behaving exactly as it did.
                None => {
                    if depots.is_empty() || statuses.get(machine) != Ok(&MachineStatus::Clogged) {
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
        // wall of new buildings, a depot demolished behind one, or terrain
        // that changed. The worker stands still, and the marker is what turns
        // its machine's status from `Unstaffed` into `Stranded`.
        let Ok((field, here)) = post_field(&mut map, worker_pos, dest_pos, &blocked, build_radius)
        else {
            commands.entity(worker).insert(Stranded);
            continue;
        };
        commands.entity(worker).remove::<Stranded>();
        let step = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (worker_pos.x + dx, worker_pos.y + dy))
            .filter_map(|n| field.get(&n).map(|&cost| (cost, n.0, n.1)))
            .min();
        if let Some((cost, x, y)) = step
            && cost < here
            && let Ok((_, mut pos, _, _)) = workers.get_mut(worker)
        {
            pos.x = x;
            pos.y = y;
        }
    }
}
