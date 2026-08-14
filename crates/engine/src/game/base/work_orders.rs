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
