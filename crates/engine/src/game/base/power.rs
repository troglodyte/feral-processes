//! The base power ledger: supply against draw, recomputed from scratch on
//! every call — see the design spec's "The model: supply against draw,
//! recomputed every tick".
//!
//! `ledger` is one pure function with two callers, which is the whole
//! reason it exists as a function rather than as a rule written twice:
//! `systems::power_grid_system` (Task 3), which stores the result in
//! `resources::PowerGrid` for the tick, and `Game::base_power` (Task 5),
//! which the base pane reads directly so the header is correct on the first
//! frame after a load, before any tick has run.
//!
//! `PowerLedger::dark` holds `Entity` rather than a per-machine flag or a
//! bare count, because the eventual writer of `MachineStatus::Unpowered`
//! (`idle_machine_system`) needs to know *which* machines lost the cut, not
//! just how many. The flag itself lives on the structure; this is the fact
//! that decides it.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;

use crate::components::{Position, PowerFuel, Structure};
use crate::items::ItemId;
use crate::items_db::ItemDb;
use crate::structures::{StructureDb, StructureDef};

/// The result of one pass over every deployed `Structure`: what the base
/// supplies, what its machines draw, and which of those machines lost the
/// cut. See the module doc for why `dark` is a set of `Entity` rather than
/// a count or a flag folded into the sum.
pub(crate) struct PowerLedger {
    pub supply: u32,
    pub draw: u32,
    pub dark: HashSet<Entity>,
}

/// Whether a supplier is currently paying whatever `StructureDef::power_upkeep`
/// names, and therefore entitled to do anything at all this tick.
///
/// The one predicate two systems ask about two different resources:
/// `ledger` gates a burner's `power_supply` on it, and `systems::
/// power_regen_system` gates the personal trickle on it too — a dry
/// supplier does neither, not just the one the player happens to be
/// watching. A structure that names no fuel always answers yes; one that
/// does needs `components::PowerFuel` present *and* charged — absence reads
/// as dry, the same direction `PowerFuel`'s own doc argues for, since both
/// writers of a structure's component list insert it and a fixture that
/// hand-spawns a bare `Structure` should read as never having been wired up
/// rather than as running on free power.
pub(crate) fn is_fuelled(def: &StructureDef, fuel: Option<&PowerFuel>) -> bool {
    def.power_upkeep.is_none() || fuel.is_some_and(|f| f.ticks_left > 0)
}

/// Every item that can put charge back on the grid, and how many recipe
/// steps it sits from doing so: anything a supplier can burn
/// (`ItemDef::grid_fuel`) at `0`, and everything those are made out of below
/// them, through `work_orders::ingredient_depths`.
///
/// **Derived from the item catalogue, never authored on a structure.** A
/// `power_priority:` field would have said the same thing in twenty-five
/// files, and a mod's own mining node would have shipped with the default —
/// which reads at the keyboard exactly like the blackout this exists to
/// prevent, since a base dies quietly rather than refusing anything. Derived,
/// the rule *is* its own census: a structure outranks the rest precisely
/// when what it puts out can light the grid again.
pub(crate) fn fuel_chain(items: &ItemDb) -> HashMap<ItemId, u32> {
    crate::game::base::work_orders::ingredient_depths(
        items
            .all()
            .filter(|def| def.grid_fuel.is_some())
            .map(|def| def.id.clone()),
        items,
    )
}

/// How close what this structure puts out is to lighting the grid — the
/// first key of `ledger`'s cut, lowest first, and `None` for a machine whose
/// output is nowhere in `fuel_chain`.
///
/// **A rung, not a flag.** A Power Conduit makes cells out of nothing and
/// sits at `0`; a Mining Node makes what a cell is made of and sits at `1`,
/// and nothing on the grid assembles that into a cell without somebody's
/// hands. Treated as one tier, four nodes west of every Conduit take the
/// whole of the Home's bootstrap and the one machine that could relight the
/// grid is dark — and `Game::fuel_wants` never posts a body to a dark one.
///
/// **Both output fields, not just `work`.** Nothing shipped assembles a grid
/// fuel today, so a reader that asked `work.produces` alone would pass every
/// shipped census and quietly drop a mod's fuel bench out of the chain.
pub(crate) fn grid_rung(def: &StructureDef, chain: &HashMap<ItemId, u32>) -> Option<u32> {
    let produces = def.work.as_ref().map(|w| &w.produces);
    let assembles = def.assembles.as_ref().map(|a| &a.item);
    produces
        .into_iter()
        .chain(assembles)
        .filter_map(|id| chain.get(id).copied())
        .min()
}

/// Sums `power_supply` over every deployed structure and `power_draw` over
/// every one whose def `StructureDef::runs_a_job()` — a machine draws
/// whether or not anyone is posted to it (see the design spec's "A machine
/// draws whether or not anyone is posted to it"). Machines beyond what the
/// supply covers are cut in `(x, y)` order until the rest fit.
///
/// **The cut favours the machines that can restart the grid**, closest to
/// the fuel first, and only then falls back to position. A base whose supply has collapsed to the Home's
/// free 4 recovers by mining Core Fragments and running the Power Conduit
/// that turns them into Power Cells; cut in tile order alone, that supply
/// goes to whichever machines happen to sit at the lowest `(x, y)` — so a
/// Compiler drawing 3 in the corner of the base could darken a working
/// Conduit and leave the run dead with its own way out standing idle.
/// `grid_rung` is the first sort key and `fuel_chain` is what it reads; an
/// empty `ItemDb` authors no fuel, gives every machine the same `None`, and
/// leaves the order exactly the tile order it was.
///
/// Machines are sorted by position *within* a tier: bevy's query
/// iteration order is not stable, so two machines competing for the last
/// unit of supply would resolve differently between runs otherwise. Same
/// reason, and the same order, `systems::assembler_system` sorts by.
///
/// The cut loop does **not** stop at the first machine that doesn't fit — a
/// 3-draw machine that can't fit a 2-unit budget goes dark while a 1-draw
/// machine behind it in the order still runs. Stopping early would darken
/// an arbitrary tail behind the first machine that happened not to fit,
/// rather than exactly the machines that don't.
///
/// A structure declaring `StructureDef::power_upkeep` counts toward `supply`
/// only while `is_fuelled` says yes — which is the whole of what a burning
/// supplier's fuel buys. `systems::power_grid_system` spends and refuels
/// **before** calling this, so the figure is always this tick's.
///
/// A structure whose def is missing from `db` contributes nothing to either
/// sum and is never dark — the same "an unknown kind is inert" shape the
/// neighbouring base systems already use, rather than a panic.
pub(crate) fn ledger(world: &World, db: &StructureDb, items: &ItemDb) -> PowerLedger {
    let chain = fuel_chain(items);
    let mut supply = 0u32;
    // (entity, rung, (x, y), draw) for every deployed machine, collected
    // before the cut runs so the sort sees the whole base at once.
    let mut machines: Vec<(Entity, u32, (i32, i32), u32)> = Vec::new();

    for entity_ref in world.iter_entities() {
        let Some(structure) = entity_ref.get::<Structure>() else {
            continue;
        };
        let Some(def) = db.get(&structure.kind) else {
            continue;
        };
        // A burner supplies nothing while it is dry — see `is_fuelled`, the
        // one predicate this and `systems::power_regen_system` both ask.
        if is_fuelled(def, entity_ref.get::<PowerFuel>()) {
            supply += def.power_supply;
        }
        if def.runs_a_job() {
            let pos = entity_ref
                .get::<Position>()
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            machines.push((
                entity_ref.id(),
                // `MAX` for no rung: behind every machine that has one.
                grid_rung(def, &chain).unwrap_or(u32::MAX),
                (pos.x, pos.y),
                def.power_draw,
            ));
        }
    }
    // The entity rides along in the key, not just the tile: `place_structure`
    // refuses an occupied tile, but a hand-edited or corrupt save can still
    // land two machines on one, and a stable sort on `tile` alone would then
    // break the tie on `world.iter_entities()` order — the exact instability
    // this sort exists to remove.
    // The rung first — the fuel chain keeps the supply, nearest the fuel
    // soonest — and the tile is what still decides between two on one rung.
    machines.sort_by_key(|(e, rung, tile, _)| (*rung, *tile, *e));

    let draw: u32 = machines
        .iter()
        .map(|(_, _, _, machine_draw)| machine_draw)
        .sum();

    let mut dark = HashSet::new();
    let mut budget = supply;
    for (entity, _, _, machine_draw) in machines {
        if budget >= machine_draw {
            budget -= machine_draw;
        } else {
            dark.insert(entity);
        }
    }

    PowerLedger { supply, draw, dark }
}
