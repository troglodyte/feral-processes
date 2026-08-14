//! Work orders: what the base should be holding, and who it stands where
//! to close the gap.
//!
//! **A work order stores an item and a quantity. Nothing else.** No
//! per-machine plan, no unit targets, no progress counters. Which machines
//! a line needs, in what order, and who is on each is recomputed from live
//! world state every time it is asked — the same call `Game::build_radius`,
//! `Game::contract_board`, `descriptions.rs`, `Game::wielded_program` and
//! the Stack's regenerated frames all make, and it buys the same things
//! here: the derivation cannot go stale, needs no save field beyond the
//! order itself, and costs no migration when the base it describes moves.
//!
//! The alternative — multiplying the recipe tree through at queue time into
//! fixed per-machine targets and counting payouts against them — produces a
//! plan that is confidently wrong the moment a machine is demolished,
//! upgraded, or fed from stock the plan did not know about. That is the
//! second copy that drifts, which `CLAUDE.md` records biting this repo four
//! times.
//!
//! An order is a **target level, not a production run**: "3 Routine Disks"
//! means *have three*, and three already sitting in a Depot satisfy it
//! immediately.

use serde::{Deserialize, Serialize};

use crate::game::base::collect::ORTHOGONAL;
use crate::items::ItemId;
use crate::systems::{assembly_recipe, produced_item};
use crate::*;

/// One queue entry: an item and how many of it the base should hold.
///
/// A **named struct, never a tuple.** RON parses a `(` in a struct position
/// as the start of named fields, so a `Vec<(ItemId, u32)>` can never be
/// widened and cannot be converted to a named struct with defaulted
/// trailing fields either. Two fields shipped in that shape already
/// (`PlayerSave::fused_gear`, `SaveData::buyback`) and both had to be
/// drained into named successors; a third is not being added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    pub item: ItemId,
    pub qty: u32,
}

/// The deployed structure whose def puts `item` into an output buffer, or
/// `None` if nothing standing makes it.
///
/// Ties are broken by tile and then entity, so two machines making the same
/// thing resolve the same way on every run — bevy's query iteration order
/// is not stable, and `assembler_system` sorts its own machines for exactly
/// this reason.
pub(crate) fn producer_of(game: &Game, item: &ItemId) -> Option<Entity> {
    let db = game.world.resource::<StructureDb>();
    let mut found: Vec<(i32, i32, Entity)> = game
        .world
        .iter_entities()
        .filter_map(|e| {
            let kind = &e.get::<Structure>()?.kind;
            let pos = e.get::<Position>()?;
            let def = db.get(kind)?;
            (produced_item(def) == Some(item)).then_some((pos.x, pos.y, e.id()))
        })
        .collect();
    found.sort();
    found.first().map(|&(_, _, e)| e)
}

/// Whether any structure the game ships or a mod supplies could produce
/// `item` at all, deployed or not. Separates "you have not built it yet"
/// from "nothing in this game makes that", which are different errands.
fn makeable_by(game: &Game, item: &ItemId) -> Option<StructureDef> {
    game.structure_defs()
        .into_iter()
        .find(|def| produced_item(def) == Some(item))
}

/// The one sentence naming why a line for `item` can never move, or `None`
/// if it is whole.
///
/// Walks the recipe tree from the deployed machine that makes `item`, and
/// for every assembler in it checks that some orthogonal neighbour is
/// producing each ingredient — a machine can only ever take what a
/// neighbour has *finished* (`components::Stock`'s `output` is public and
/// its `input` is not), so a link with no feeder beside it can never run
/// however much of the ingredient the base holds elsewhere.
///
/// The banked exclusion falls out of this rather than being special-cased:
/// a banked payout goes straight to the player's bank and reaches no
/// `output` (`systems::deliver_payout`), so nothing can hold a stock of it
/// and nothing can be fed from it. `research_data` is refused on those
/// terms, with its Research Node standing.
pub(crate) fn chain_break(game: &Game, item: &ItemId) -> Option<String> {
    let items = game.world.resource::<ItemDb>();
    if items.get(item.as_str()).is_none() {
        return Some("No such item.".into());
    }
    if items.get(item.as_str()).is_some_and(|d| d.banked) {
        let name = game.item_name(item);
        return Some(format!(
            "{name} is banked as it is gathered — the base never holds a stock of it to order \
             against."
        ));
    }
    let Some(machine) = producer_of(game, item) else {
        let name = game.item_name(item);
        return Some(match makeable_by(game, item) {
            Some(def) => format!(
                "No {} deployed — that is what makes a {name}.",
                def.name.clone()
            ),
            None => format!("Nothing the base can build makes a {name}."),
        });
    };
    let mut seen = std::collections::HashSet::new();
    break_at(game, &structures_by_tile(game), machine, &mut seen)
}

/// Every deployed structure by the tile it stands on. Built once per walk
/// rather than scanned per neighbour, which is the shape `assembler_system`
/// already uses to answer the same adjacency question.
pub(crate) fn structures_by_tile(game: &Game) -> std::collections::HashMap<(i32, i32), Entity> {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .filter_map(|e| e.get::<Position>().map(|p| ((p.x, p.y), e.id())))
        .collect()
}

/// The recursive half of `chain_break`, walking *from the deployed
/// machine* rather than re-asking `producer_of` per ingredient — the
/// question is whether this machine's own neighbours can feed it, and a
/// producer standing somewhere else in the base is no answer to that.
///
/// `seen` is not an optimisation. Two machines that assemble each other's
/// ingredients are unreachable in the shipped assets but expressible by a
/// mod, and without it the walk would not terminate.
fn break_at(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    seen: &mut std::collections::HashSet<Entity>,
) -> Option<String> {
    if !seen.insert(machine) {
        return None;
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let kind = &game.world.get::<Structure>(machine)?.kind;
    let def = db.get(kind)?;
    let pos = *game.world.get::<Position>(machine)?;
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor makes its product out of nothing on a timer, so the
        // walk terminates here rather than at a depth limit.
        return None;
    };
    let recipe: Vec<ItemId> = recipe.iter().map(|(item, _)| item.clone()).collect();
    for ingredient in recipe {
        let feeder = ORTHOGONAL.into_iter().find_map(|(dx, dy)| {
            by_tile
                .get(&(pos.x + dx, pos.y + dy))
                .copied()
                .filter(|&e| {
                    game.world
                        .get::<Structure>(e)
                        .and_then(|s| db.get(&s.kind))
                        .and_then(produced_item)
                        == Some(&ingredient)
                })
        });
        let Some(feeder) = feeder else {
            let want = game.item_name(&ingredient);
            return Some(format!(
                "Nothing beside the {} is making {want} — a machine can only take what a \
                 neighbour has finished.",
                def.name
            ));
        };
        if let Some(deeper) = break_at(game, by_tile, feeder, seen) {
            return Some(deeper);
        }
    }
    None
}

/// Whether staffing `machine` right now would actually move something.
///
/// Output has room, **and** for an assembler, its input holds at least one
/// batch of each ingredient *or* every shortfall is sitting in an
/// orthogonally adjacent feeder's output.
///
/// The second half is the load-bearing one. `assembler_system`'s pull phase
/// is *behind* the "is anyone posted here" gate, so a machine with nobody on
/// it never fills its own input — "the input is empty" is therefore not the
/// same question as "this machine has nothing to do", and reading it as one
/// would leave every empty bench permanently unstaffed.
///
/// This is also the predicate that *releases* a worker: a clogged machine
/// cannot progress, so it stops wanting a body, so the body goes somewhere
/// useful. That is how "work the deepest requirement until it is made, then
/// move on" falls out without the scheduler sequencing any phases.
pub(crate) fn can_progress(game: &Game, machine: Entity) -> bool {
    let Some(stock) = game.world.get::<Stock>(machine) else {
        return false;
    };
    if stock.output_room() == 0 {
        return false;
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let Some(def) = game
        .world
        .get::<Structure>(machine)
        .and_then(|s| db.get(&s.kind))
    else {
        return false;
    };
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor makes its product out of nothing on a timer, so room
        // in the output is the whole question for it.
        return produced_item(def).is_some();
    };
    let Some(pos) = game.world.get::<Position>(machine).copied() else {
        return false;
    };
    let by_tile = structures_by_tile(game);
    recipe.iter().all(|(item, per_batch)| {
        let want = per_batch * crate::tuning::INPUT_STOCK_BATCHES;
        let held = stock.input.get(item).copied().unwrap_or(0);
        if held >= *per_batch {
            return true;
        }
        let short = want - held.min(want);
        let beside: u32 = ORTHOGONAL
            .into_iter()
            .filter_map(|(dx, dy)| by_tile.get(&(pos.x + dx, pos.y + dy)).copied())
            .filter_map(|feeder| game.world.get::<Stock>(feeder))
            .map(|s| s.output.get(item).copied().unwrap_or(0))
            .sum();
        beside.min(short) + held >= *per_batch
    })
}

/// Which machines this order needs a body on, deepest first.
///
/// Walks the recipe tree from the machine that makes the ordered item.
/// **A machine earns a place only if it `can_progress`** — staffing one that
/// would stand there doing nothing is what a scheduler must not do, and a
/// clogged machine dropping off this list is exactly how a body is released
/// downstream. The walk recurses *through* a machine either way, so an
/// upstream feeder that can work is found whether or not its consumer can.
///
/// The consequence worth knowing: on a base with every buffer empty this
/// returns the one extractor at the top of the line and nothing else,
/// because nothing else has anything to do yet. The full deepest-first line
/// appears as stock reaches each stage, which is also what lets two and
/// three staff spread down a chain without ever being posted somewhere
/// idle.
///
/// **A machine reached twice is kept once, at its deepest position** —
/// otherwise a shared feeder is staffed second on behalf of one branch while
/// the other still needs it first. No shipped assembler recipe has more than
/// one ingredient so the case is unreachable against the real assets, but
/// the engine's multi-input support is real and mods may ship one.
///
/// The sort is **total and stable** — `(depth, x, y, entity)` — for the
/// reason `assembler_system` sorts its machines: bevy's query iteration
/// order is not stable, and two machines at equal depth resolving
/// differently between runs is a flaky test and a base that behaves
/// differently after a reload.
pub(crate) fn wants(game: &Game, order: &WorkOrder) -> Vec<(Entity, u32)> {
    let mut deepest: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    let by_tile = structures_by_tile(game);
    if let Some(machine) = producer_of(game, &order.item) {
        let mut seen = std::collections::HashSet::new();
        walk_wants(game, &by_tile, machine, 0, &mut deepest, &mut seen);
    }
    let mut list: Vec<(u32, Option<i32>, Option<i32>, Entity)> = deepest
        .into_iter()
        .map(|(e, depth)| {
            let pos = game.world.get::<Position>(e).map(|p| (p.x, p.y));
            (depth, pos.map(|p| p.0), pos.map(|p| p.1), e)
        })
        .collect();
    // Deepest first, then by tile, then by entity — a total order, so two
    // machines at equal depth never swap between runs.
    list.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| (a.1, a.2, a.3).cmp(&(b.1, b.2, b.3)))
    });
    list.into_iter().map(|(d, _, _, e)| (e, d)).collect()
}

/// The recursive half of `wants`. `seen` guards a mod's mutually-feeding
/// pair, which would not terminate; `deepest` is what keeps a shared feeder
/// to one entry at the furthest depth it was reached.
fn walk_wants(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    depth: u32,
    deepest: &mut std::collections::HashMap<Entity, u32>,
    seen: &mut std::collections::HashSet<Entity>,
) {
    // Guards a mod's mutually-feeding pair *along one path*, which is what
    // would not terminate. Lifted again on the way out, so a shared feeder
    // reached down a second, longer branch is still measured at that depth
    // — which is exactly the case the guard must not swallow.
    if !seen.insert(machine) {
        return;
    }
    walk_feeders(game, by_tile, machine, depth, deepest, seen);
    seen.remove(&machine);
}

/// The body of `walk_wants`, split out solely so every early return leaves
/// the `seen` guard to one caller rather than each remembering to lift it.
fn walk_feeders(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    depth: u32,
    deepest: &mut std::collections::HashMap<Entity, u32>,
    seen: &mut std::collections::HashSet<Entity>,
) {
    if can_progress(game, machine) {
        let entry = deepest.entry(machine).or_insert(depth);
        *entry = (*entry).max(depth);
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let Some(def) = game
        .world
        .get::<Structure>(machine)
        .and_then(|s| db.get(&s.kind))
    else {
        return;
    };
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor has no upstream, so the walk terminates here. A
        // clogged one simply did not earn its place above, and something
        // else has to drain it before it wants a body again.
        return;
    };
    let recipe: Vec<ItemId> = recipe.iter().map(|(item, _)| item.clone()).collect();
    let Some(pos) = game.world.get::<Position>(machine).copied() else {
        return;
    };
    for ingredient in recipe {
        for (dx, dy) in ORTHOGONAL {
            let Some(&feeder) = by_tile.get(&(pos.x + dx, pos.y + dy)) else {
                continue;
            };
            let makes = game
                .world
                .get::<Structure>(feeder)
                .and_then(|s| db.get(&s.kind))
                .and_then(produced_item)
                == Some(&ingredient);
            if makes {
                walk_wants(game, by_tile, feeder, depth + 1, deepest, seen);
            }
        }
    }
}

/// How many of `item` the **base** is holding — every Depot
/// (`StructureDef::stores`) and every machine output buffer.
///
/// Deliberately not the player's inventory: an order says what the base
/// should hold, and where it holds it does not matter. What you are carrying
/// is yours.
pub(crate) fn base_holding(game: &Game, item: &ItemId) -> u32 {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .filter_map(|e| e.get::<Stock>())
        .map(|s| s.output.get(item).copied().unwrap_or(0))
        .sum()
}

impl Game {
    /// Queues an order for `qty` of `item`, or names why the line for it can
    /// never move.
    ///
    /// Every refusal runs **before** anything is pushed — the same ordering
    /// argument `use_symlink` makes about `clear_stack` and
    /// `install_routine` makes about the disk. A refused order leaves the
    /// queue exactly as it was.
    pub fn queue_work_order(&mut self, item: ItemId, qty: u32) -> Result<(), String> {
        if qty == 0 {
            return Err("An order for nothing is not an order.".into());
        }
        // The same `Position` trap `find_target_in_direction` fell into: a
        // party underground has its `Position` pinned to the surface
        // entrance tile, so anything claiming something about where the base
        // is has to refuse rather than answer from a tile four frames up.
        self.require_surface()?;
        if let Some(reason) = chain_break(self, &item) {
            return Err(reason);
        }
        let name = self.item_name(&item).to_string();
        self.world
            .resource_mut::<resources::WorkOrders>()
            .0
            .push(WorkOrder { item, qty });
        self.log_base(format!("Work order filed: {qty} x {name}."));
        Ok(())
    }

    /// Drops the order at `index`, shifting the ones behind it up.
    ///
    /// **Unwinds nothing, because nothing was wound.** There are no
    /// per-machine targets to roll back and no reserved stock to release —
    /// the next tick simply derives a different answer. That is the
    /// derived-never-stored decision paying out somewhere it was not
    /// designed for.
    pub fn cancel_work_order(&mut self, index: usize) -> Result<(), String> {
        let dropped = {
            let mut orders = self.world.resource_mut::<resources::WorkOrders>();
            if index >= orders.0.len() {
                return Err("No such work order.".into());
            }
            orders.0.remove(index)
        };
        let name = self.item_name(&dropped.item).to_string();
        self.log_base(format!("Work order cancelled: {name}."));
        Ok(())
    }

    pub fn work_orders(&self) -> &[WorkOrder] {
        &self.world.resource::<resources::WorkOrders>().0
    }
}
