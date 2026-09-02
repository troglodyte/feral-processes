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

use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::components::{Position, PowerFuel, Structure};
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

/// Sums `power_supply` over every deployed structure and `power_draw` over
/// every one whose def `StructureDef::runs_a_job()` — a machine draws
/// whether or not anyone is posted to it (see the design spec's "A machine
/// draws whether or not anyone is posted to it"). Machines beyond what the
/// supply covers are cut in `(x, y)` order until the rest fit.
///
/// Machines are sorted by position before the cut runs: bevy's query
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
pub(crate) fn ledger(world: &World, db: &StructureDb) -> PowerLedger {
    let mut supply = 0u32;
    // (entity, (x, y), draw) for every deployed machine, collected before
    // the cut runs so the sort sees the whole base at once.
    let mut machines: Vec<(Entity, (i32, i32), u32)> = Vec::new();

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
            machines.push((entity_ref.id(), (pos.x, pos.y), def.power_draw));
        }
    }
    // The entity rides along in the key, not just the tile: `place_structure`
    // refuses an occupied tile, but a hand-edited or corrupt save can still
    // land two machines on one, and a stable sort on `tile` alone would then
    // break the tie on `world.iter_entities()` order — the exact instability
    // this sort exists to remove.
    machines.sort_by_key(|(e, tile, _)| (*tile, *e));

    let draw: u32 = machines
        .iter()
        .map(|(_, _, machine_draw)| machine_draw)
        .sum();

    let mut dark = HashSet::new();
    let mut budget = supply;
    for (entity, _, machine_draw) in machines {
        if budget >= machine_draw {
            budget -= machine_draw;
        } else {
            dark.insert(entity);
        }
    }

    PowerLedger { supply, draw, dark }
}
