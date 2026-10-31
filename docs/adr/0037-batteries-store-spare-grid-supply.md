# 0037 A Battery stores the Grid's spare supply and spends it when supply falls short

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-10-31
- Pattern: —
- Depends on: [0019](0019-content-defaults-to-rust.md)

## Context

The Recharger Node can be built on turn one, and `standing_current` is a starter contract that asks for one.
It burns a Power Cell every `POWER_UPKEEP_TICKS` (`power_upkeep`), but setting an Extractor to make Power Cells was gated behind the `power_grid` research.
`keep_the_lights_on` (zone 1) tells the player to set that recipe.
The user decided Power Cells come off an Extractor with no research.
That leaves `power_grid` unlocking nothing, which `no_research_node_is_left_unlocking_nothing` refuses, and `siphoning` requires it.
The user chose to keep the research and have it unlock a Battery: a storage structure.

The grid today has no stored energy.
`power::ledger` sums `power_supply` against `power_draw` each tick, and when draw exceeds supply it cuts machines (fuel makers last, `grid_rung`).
A burner that runs dry supplies zero until its next cell arrives.

## Decision

**Content.** A new structure `assets/structures/battery.ron`, unlocked by `power_grid`'s `unlocks_structures`.
It runs no job: no tamed program cost, no staff, no draw, and it is never cut.
The values below are Claude's calls: build cost 12 Core Fragments and 2 Power Cells, `room_tags: ["workshop"]`.
`power_grid`'s description is rewritten to say it lets you build Batteries.
The art is a new 16x16 RGBA `assets/sprites/battery.png`, keyed by the id under the existing sprite rules.

**Schema.** A new optional `StructureDef` field, `battery: Option<BatteryDef>`, `#[serde(default)]`.
`BatteryDef { capacity: u32, rate: u32 }`.
`capacity` is measured in supply-ticks, meaning one unit of supply held for one tick.
`rate` is the most a battery takes in or gives out per tick.
`battery.ron` sets `capacity: 400, rate: 4`, which is one Recharger Node's supply (4) for one fuel window (100 ticks).

**The rule.** Each tick, after fuel is burned and before the cut:

- If supply exceeds draw, the surplus charges Batteries, each up to its `rate`, and stops at `capacity`.
- If draw exceeds supply, Batteries discharge, each up to its `rate` and its stored charge, and that output counts as supply before any machine is cut.

Batteries are visited in `(x, y)` order, so the result never depends on query order.
The flow is decided by a pure function over plain inputs: supply, draw, and each battery's charge, capacity and rate.
`power_grid_system` writes the result back to a `components::BatteryCharge(u32)`.
This follows the functional-core rule in `CLAUDE.md`.

**Save.** `StructureSave` gains `battery_charge: u32` under `#[serde(default)]`.
An absent field loads as an empty battery, so there is no `SAVE_FORMAT_VERSION` bump: the same reasoning as `power_fuel`, except that zero is the correct default here.

**Display.** The Grid header shows the total stored charge next to draw and supply when the base has a battery.
A Battery's structure sheet shows its charge as a fraction of capacity.

## Alternatives considered

- A Battery that holds Power Cells and feeds them to adjacent burners. This only duplicates a Depot standing next to the burner, and stores nothing the grid did not already have.
- Write the Battery in Rust with no new field. A def flag still needs its numbers somewhere, and every other grid number (`power_supply`, `power_draw`, `power_upkeep`) is already on `StructureDef`.
- Delete `power_grid`. The user chose to keep it.

## Consequences

A base with spare supply can ride out a gap in cell deliveries, so a Recharger Node running dry no longer cuts machines at once.
That softens the pressure `grid_rung` and `fuel_wants` exist to handle, but it never removes it: a drained Battery puts the grid back where it is today.
Power Cells are available from turn one, so the early grid is easier.
Check `balance_sim` and the research ladder tests.
`assets/structures/README.md` documents `battery`, and `CHANGELOG.md` covers both changes.

## Evidence

`systems::power_grid_system`, `systems::burn_grid_upkeep`, `game::base::power::{ledger, grid_rung, is_fuelled}`, `components::PowerFuel`, `save::StructureSave::power_fuel`, `tuning::POWER_UPKEEP_TICKS`, `assets/research/power_grid.ron`, `assets/research/siphoning.ron`, `assets/structures/recharger_node.ron`, `tests::research::zone_gates::no_research_node_is_left_unlocking_nothing`.
