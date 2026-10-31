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

use crate::components::{BatteryCharge, Position, PowerFuel, Siphoned, Structure};
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
    /// What each structure that touches the grid contributes to `supply` and
    /// `draw`, summed in the same pass: the per-structure list the Power
    /// page prints cannot disagree with the totals because it is where they
    /// come from. A supplier that is dry or unsiphoned is listed at zero.
    pub entries: Vec<PowerEntry>,
    /// What each Battery does this tick, `battery_flow`'s answer keyed by
    /// entity: positive is charge taken in, negative given out. The ledger
    /// only reads; `systems::power_grid_system` applies it.
    pub battery_flow: Vec<(Entity, i32)>,
}

/// One structure's share of the grid — see `PowerLedger::entries`.
pub(crate) struct PowerEntry {
    pub entity: Entity,
    pub supply: u32,
    pub draw: u32,
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
///
/// Siphon occupancy is deliberately not part of this predicate: it gates
/// `supply` alone, in `ledger`.
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
/// **A rung, not a flag.** An Extractor set to Power Cell makes cells out
/// of nothing and sits at `0`; one set to what a cell is made of sits at `1`,
/// and nothing on the grid assembles that into a cell without somebody's
/// hands. Treated as one tier, four nodes west of every Conduit take the
/// whole of the Home's bootstrap and the one machine that could relight the
/// grid is dark — and `Game::fuel_wants` never posts a body to a dark one.
///
/// **Whatever the instance produces, not just `work`.** Nothing shipped
/// assembles a grid fuel today, so a reader that asked `work.produces` alone
/// would pass every shipped census and quietly drop a mod's fuel bench out of
/// the chain. A machine with nothing set produces nothing, so it has no rung.
pub(crate) fn grid_rung(
    placed: crate::structures::Placed<'_>,
    chain: &HashMap<ItemId, u32>,
) -> Option<u32> {
    placed
        .production()
        .and_then(|p| chain.get(&p.recipe.produces).copied())
}

/// One Battery's state going into `battery_flow`, in the supply-ticks
/// `BatteryDef` documents.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub charge: u32,
    pub capacity: u32,
    pub rate: u32,
}

/// What each Battery does this tick, in the order `cells` is given: positive
/// is charge taken in, negative is charge given out (ADR 0037).
///
/// Spare supply is shared out in order, each cell taking up to
/// `min(rate, capacity - charge)`; a shortfall is covered in order, each
/// giving up to `min(rate, charge)`, and the cells together never give more
/// than the shortfall. A pure function of plain numbers so the grid's whole
/// storage rule is testable without a world; the caller sorts the cells by
/// `(x, y)` so the answer never depends on query order.
pub(crate) fn battery_flow(supply: u32, draw: u32, cells: &[Cell]) -> Vec<i32> {
    let mut surplus = supply.saturating_sub(draw);
    let mut shortfall = draw.saturating_sub(supply);
    cells
        .iter()
        .map(|c| {
            if surplus > 0 {
                let taken = surplus.min(c.rate).min(c.capacity.saturating_sub(c.charge));
                surplus -= taken;
                taken as i32
            } else {
                let given = shortfall.min(c.rate).min(c.charge);
                shortfall -= given;
                -(given as i32)
            }
        })
        .collect()
}

/// The total `battery_flow` gives out, as a positive figure.
fn discharge(flow: &[i32]) -> u32 {
    flow.iter()
        .filter(|f| **f < 0)
        .map(|f| f.unsigned_abs())
        .sum()
}

/// Sums `power_supply` over every deployed structure and `power_draw` over
/// every one whose def `StructureDef::runs_a_job()` — a machine draws
/// whether or not anyone is posted to it (see the design spec's "A machine
/// draws whether or not anyone is posted to it"). Machines beyond what the
/// supply covers are cut in `(x, y)` order until the rest fit.
///
/// **The cut favours the machines that can restart the grid**, closest to
/// the fuel first, and only then falls back to position. A base whose supply has collapsed to the Home's
/// free 4 recovers by mining Core Fragments and running the Extractor
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
/// A structure with `StructureDef::siphons` counts toward `supply` only
/// while some program's `Siphoned` points at it, so an empty siphon supplies
/// 0 and draws 0.
///
/// A Battery's discharge is added to `supply` before the cut (ADR 0037), so
/// the figure returned already includes it; `battery_flow` is the per-cell
/// answer `power_grid_system` writes back.
///
/// A structure whose def is missing from `db` contributes nothing to either
/// sum and is never dark — the same "an unknown kind is inert" shape the
/// neighbouring base systems already use, rather than a panic.
pub(crate) fn ledger(world: &World, db: &StructureDb, items: &ItemDb) -> PowerLedger {
    let chain = fuel_chain(items);
    let mut supply = 0u32;
    let mut entries: Vec<PowerEntry> = Vec::new();
    // (entity, rung, (x, y), draw) for every deployed machine, collected
    // before the cut runs so the sort sees the whole base at once.
    let mut machines: Vec<(Entity, u32, (i32, i32), u32)> = Vec::new();
    // Every Battery, keyed by tile so `battery_flow` visits them in `(x, y)`
    // order whatever order the world iterates in.
    let mut batteries: Vec<((i32, i32), Entity, Cell)> = Vec::new();
    // The siphons some program currently holds, collected up front so the
    // loop below asks a set rather than scanning every program per structure.
    let occupied: HashSet<Entity> = world
        .iter_entities()
        .filter_map(|e| e.get::<Siphoned>().map(|s| s.siphon))
        .collect();

    for entity_ref in world.iter_entities() {
        let Some(structure) = entity_ref.get::<Structure>() else {
            continue;
        };
        let Some(def) = db.get(&structure.kind) else {
            continue;
        };
        // A burner supplies nothing while it is dry — see `is_fuelled`, the
        // one predicate this and `systems::power_regen_system` both ask. A
        // siphon additionally supplies only while a program holds it; that
        // gate lives here, not in `is_fuelled`, because a siphon has no
        // `power_regen` for the regen system to ask about.
        let supplied = if is_fuelled(def, entity_ref.get::<PowerFuel>())
            && (!def.siphons || occupied.contains(&entity_ref.id()))
        {
            def.power_supply
        } else {
            0
        };
        supply += supplied;
        let placed = crate::structures::Placed::of(
            def,
            entity_ref.get::<crate::components::MachineRecipe>(),
        );
        // A machine with nothing set runs no job, so it draws nothing and
        // takes no part in the cut: the grid is not asked to light it.
        let runs = def.runs_a_job() && !placed.is_unconfigured();
        let drawn = if runs { def.power_draw } else { 0 };
        if def.power_supply > 0 || drawn > 0 {
            entries.push(PowerEntry {
                entity: entity_ref.id(),
                supply: supplied,
                draw: drawn,
            });
        }
        if let Some(battery) = def.battery {
            let pos = entity_ref
                .get::<Position>()
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            batteries.push((
                (pos.x, pos.y),
                entity_ref.id(),
                Cell {
                    charge: entity_ref.get::<BatteryCharge>().map_or(0, |c| c.0),
                    capacity: battery.capacity,
                    rate: battery.rate,
                },
            ));
        }
        if runs {
            let pos = entity_ref
                .get::<Position>()
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            machines.push((
                entity_ref.id(),
                // `MAX` for no rung: behind every machine that has one.
                grid_rung(placed, &chain).unwrap_or(u32::MAX),
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

    // The cut runs on everything the Batteries could give, so a Battery
    // covers a shortfall rather than watching the cut happen; but the cut
    // lights whole machines only, so they then give just what the lit ones
    // used past supply — the rest of an offer that fits no machine stays
    // stored rather than being spent on nothing.
    batteries.sort_by_key(|(tile, e, _)| (*tile, *e));
    let cells: Vec<Cell> = batteries.iter().map(|(_, _, c)| *c).collect();
    let offered = discharge(&battery_flow(supply, draw, &cells));

    let mut dark = HashSet::new();
    let mut budget = supply + offered;
    for (entity, _, _, machine_draw) in machines {
        if budget >= machine_draw {
            budget -= machine_draw;
        } else {
            dark.insert(entity);
        }
    }
    // Lowering the budget by what was left over lights the same machines:
    // each lit one still had at least its own draw plus that leftover ahead
    // of it, and each dark one had less than its draw already.
    let used = (supply + offered) - budget;
    let flow = if draw > supply {
        battery_flow(supply, used.max(supply), &cells)
    } else {
        battery_flow(supply, draw, &cells)
    };
    let supply = supply + discharge(&flow);
    let battery_flow: Vec<(Entity, i32)> = batteries.iter().map(|(_, e, _)| *e).zip(flow).collect();

    PowerLedger {
        supply,
        draw,
        dark,
        entries,
        battery_flow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(charge: u32) -> Cell {
        Cell {
            charge,
            capacity: 10,
            rate: 4,
        }
    }

    #[test]
    fn no_surplus_and_no_shortfall_moves_nothing() {
        assert_eq!(battery_flow(5, 5, &[cell(3)]), vec![0]);
    }

    #[test]
    fn a_full_battery_takes_nothing() {
        assert_eq!(battery_flow(9, 1, &[cell(10)]), vec![0]);
    }

    #[test]
    fn a_battery_takes_no_more_than_its_headroom() {
        assert_eq!(battery_flow(9, 1, &[cell(8)]), vec![2]);
    }

    #[test]
    fn an_empty_battery_gives_nothing() {
        assert_eq!(battery_flow(1, 5, &[cell(0)]), vec![0]);
    }

    #[test]
    fn a_shortfall_bigger_than_the_total_rate_gives_every_rate() {
        assert_eq!(battery_flow(0, 20, &[cell(10), cell(10)]), vec![-4, -4]);
    }

    #[test]
    fn a_shortfall_is_never_over_covered() {
        assert_eq!(battery_flow(3, 5, &[cell(10), cell(10)]), vec![-2, 0]);
    }

    #[test]
    fn the_first_battery_fills_before_the_second() {
        assert_eq!(battery_flow(5, 1, &[cell(8), cell(0)]), vec![2, 2]);
        assert_eq!(battery_flow(3, 1, &[cell(0), cell(0)]), vec![2, 0]);
    }
}
