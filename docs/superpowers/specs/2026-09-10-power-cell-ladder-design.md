# Power Cell ladder — design

**Status:** approved, unimplemented
**Date:** 2026-09-10

## The problem

`power_cell` is the only Power consumable in the game. It restores 25 against
a fixed `POWER_MAX` of 100, costs 2 Core Fragments, and is the same item in
zone 1 as it is in zone 8. Two complaints follow from that:

- **Inventory churn.** A quarter-tank a click means a deep run is a stack of
  cells and a stack of keypresses.
- **No progression feel.** Nothing about the pack says you have got further.

It also has a second, quieter job: it is the fuel a `power_upkeep` supplier
burns (`recharger_node`, `line_driver`), one cell per `POWER_UPKEEP_TICKS`.

## What this is not

Making `POWER_MAX` per-creature and level-scaled was raised and **deferred to
a later session**. It reverses a documented seam (`Trickle` is the one restore
kind that does not scale with its invoker, precisely because Power's ceiling
is fixed while `Regen`'s grows), it changes the type-level clamp in
`PowerReserve`, and `balance_sim` gates none of the Power economy so it would
land with no regression curve behind it. Nothing in this spec assumes a fixed
ceiling *stays* fixed, but nothing here moves it either.

## Design

Two families of item, and the split between them is the legible part:

> **The plain ladder feeds the base. The trickle line feeds you.**

A plain cell restores a lump of Power and burns as grid fuel. A trickle cell
restores a smaller lump and then drips, and is not fuel at all.

### The content

| Item | value | Power | grid fuel | recipe (ingredient value) | gate |
|---|---|---|---|---|---|
| Power Cell | 1 | 25 | 1 window | 2 `core_fragment` (2) | none — **unchanged** |
| **Buffered Cell** | 9 | 60 | 3 windows | 3 `power_cell`, 1 `logic_wafer`, 3 `cache_grain`, 1 `bytecode_block` (13) | research, zone 2, Winding Node |
| **Sustain Cell** | 9 | 15 + drip | — | 2 `power_cell`, 1 `logic_wafer`, 2 `cache_grain`, 1 `bytecode_block` (11) | research, zone 2, Winding Node |
| **Capacitor Array** | 20 | 100 (full) | 8 windows | 2 `buffered_cell`, 2 `bytecode_block`, 4 `cache_grain`, 1 `logic_wafer` (33) | research, zone 3, Winding Node |
| **Backfeed Cell** | 15 | 25 + drip | — | 1 `buffered_cell`, 1 `bytecode_block`, 3 `cache_grain`, 1 `logic_wafer` (19) | research, zone 3, Winding Node |

The drips, priced against `HUNGER_DECAY_PER_TICK` (0.15 Power a tick — the
line a trickle has to clear to mean anything):

| Item | drip | rate | gross | against drain |
|---|---|---|---|---|
| Sustain Cell | 3 every 10 ticks for 300 | 0.30/tick | 90 | 2× |
| Backfeed Cell | 5 every 10 ticks for 600 | 0.50/tick | 300 | 3.3× |

Gross is a ceiling, not a grant: `PowerReserve::restore` clamps, so a drip
into a full reserve is wasted. That is the trade — the trickle line beats the
lump only if you are actually spending, which is what makes it a field item
rather than a bigger version of the same thing.

### Two schema fields

Both additive, both `#[serde(default)]`, **no `SAVE_FORMAT_VERSION` bump** —
neither is stored in a save, and the save is field-named RON.

1. **`ItemDef::grid_fuel: Option<u32>`** — how many windows of
   `POWER_UPKEEP_TICKS` one unit buys. `power_cell.ron` declares `Some(1)`,
   which is exactly today's behaviour.

   **Windows, not ticks.** The window length is a difficulty knob and stays
   in `tuning.rs`; content only says "this cell is worth three of the
   staple". Authoring raw ticks in a `.ron` would move balance into content,
   which the moddability rule explicitly excludes.

2. **`PrebattleBuff::interval: u32`** — turns between firings, defaulting to
   `abilities::every_turn()`. Mirrors `AbilityEffect::FieldBuff::interval`,
   which already exists for routines.

   This is what makes the trickle line authorable at all. `consume_item`
   currently hardcodes `every_turn()` when it arms an item's field buff, so
   without this field a trickle cell fires **every tick** — at any useful
   magnitude that is not a drip, it is a faucet, and Power is already
   documented as not a limiting resource.

### The burn rule

A supplier accepts **any item declaring `grid_fuel`**, and burns the
**lowest-window one it can reach**. `PowerFuel::ticks_left` is set from the
item actually burnt (`windows * POWER_UPKEEP_TICKS`) rather than from the
constant directly.

Cheapest-first is what keeps the tiers honest in both directions: plain cells
get spent at the base where they are cheap to make and a hauler is already
walking them, and the dense ones stay in your pack for the field. It is also
what satisfies the standing rule that **a new tier must never retire the one
below it** — Power Cell keeps its job as the base's fuel *and* becomes the
ingredient of everything above it.

`StructureDef::power_upkeep` keeps its type and its meaning: the named item is
what a hauler fetches by default and what the structure's description talks
about. It is deliberately not widened to a list — content naming its own fuel
is the moddability rule, and a second spelling of "this burns grid fuel" is a
second thing to get wrong.

### Sourcing and gating

- **Crafted, zone-gated, multi-material** — every recipe names `cache_grain`,
  the zone-2 material, so the ladder opens exactly when its ingredient does
  and the zone-material census is satisfied.
- **Two research nodes**, both on the `power_grid` branch:
  - `charge_density` — zone 2, requires `power_grid`, unlocks Buffered Cell
    and Sustain Cell.
  - `capacitance` — zone 3, requires `charge_density`, unlocks Capacitor
    Array and Backfeed Cell.
  Each node's `materials` bill names only what its own prerequisite closure
  can already make (`power_cell`, `cache_grain`, `bytecode_block`, and for
  the second node `buffered_cell`). `refinery` and `winding_node` are both
  ungated, so `bytecode_block` is legal in a bill.
- **Traders** stock them with no code at all: `caravan::stock_pool` takes
  anything without an `EconomyRole` that is not `banked`, and the settlement
  shelf draws from the same source. **This means Credits buy what research
  otherwise gates** — deliberate, and consistent with the tool carrier, which
  is sold everywhere and bought nowhere.
- **No `cache_drop`** on any new tier. Caches stay a Power Cell source only.

### The value bound, and the trap under it

Every recipe's result is priced below the sum of its ingredients, or the
craft-and-sell loop mints Credits. That is the well-known bound.

The second bound is the one that moved the recipes: **`ItemDb::creation_shelf`
filters by `value <= CREATION_SHELF_MAX_VALUE` (8) and truncates to
`CREATION_SHELF_ROWS`.** A cell priced at 5 would be offered to a character
that has not been created yet — research-gated content on the starting shelf
— *and* would push an existing row off the end silently, because the shelf
sorts by `(price, id)` and cuts. This is the tool-carrier trap
(`TOOL_CARRIER_VALUE` sat under every `forge_cost`) in a new place.

So each cell's recipe is rich enough to carry a value clear of 8. That is a
pricing decision holding an invariant, which is exactly the kind of thing that
rots, so it gets a census rather than a comment.

## Engine changes

All in `crates/engine`. No app-core, no gui, no save bump.

1. `items_db.rs` — `ItemDef::grid_fuel`, `PrebattleBuff::interval`.
2. `game/turn.rs::consume_item` — read `buff.interval` instead of hardcoding
   `every_turn()`.
3. `systems.rs::burn_grid_upkeep` — the fuel id it reads out of the def
   becomes a candidate set; pick the lowest-window fuel reachable (own hopper
   first, then `collect::plan_adjacent_take`, both unchanged in shape); set
   `ticks_left` from the item burnt. The ledger `Consume` event names the item
   actually burnt.
4. `systems::intake_recipe` and `Game::fuel_wants` — accept the family so a
   hauler fetching a dense cell to a dry supplier is not walking it something
   it cannot use.

`game::base::power::is_fuelled` is untouched — `ticks_left > 0` still means
what it meant.

## Tests

TDD, failing test first, per item below.

- **A trickle test must `spend()` Power down before timing it.** This is a
  documented trap: `power_regen_system` runs ahead of `needs_tick_system`, so
  a saturated reserve converges to `POWER_MAX - HUNGER_DECAY_PER_TICK` whether
  or not the trickle fired, and a naive before/after reads "nothing changed"
  in exactly the case where something should have.
- An item's `interval` is honoured — a cell authored at every-10 fires 30
  times over 300 ticks, not 300 times.
- A supplier fed only a dense cell burns it and gets `windows * POWER_UPKEEP_TICKS`.
- A supplier with both a plain and a dense cell in reach burns the **plain**
  one.
- A supplier with no `grid_fuel` item in reach goes `Dry` exactly as now.
- Arming a cell's trickle displaces a running Patch Routine buff (asserting
  the existing `arm_field_buff` consumable rule, which this feature makes
  reachable in a new way).

Censuses in `tests/assets.rs`:

- Every `StructureDef::power_upkeep` id resolves to an item declaring
  `grid_fuel` — the existing "every declared fuel resolves in `ItemDb`"
  census, tightened.
- **No research-gated recipe result appears on `creation_shelf`.** New, and
  the one that holds the pricing above honest.
- `every_zone_gated_gear_recipe_asks_for_a_zone_material` — its `checked`
  count goes 6 → 10. Its name and message both say "gear" while it actually
  covers every zone-gated recipe; the message wants rewording in the same
  change.
- The existing value bound covers the five recipes with no edit.

## Documentation

- `assets/items/README.md` — `grid_fuel`, and `interval` on `prebattle_buff`.
- `assets/structures/README.md` — `power_upkeep` now names the *preferred*
  fuel, and any `grid_fuel` item is accepted.
- `docs/items.md` — regenerated.
- `CHANGELOG.md` — its own section at the merge, per the release policy.

## Known flatness

The ladder finishes at zone 3, so zones 4-8 gain nothing here. That is a
decision, not an oversight: the cells are an early-to-mid convenience that
stops mattering once the base is established. If they should keep saying
something late, the lever is the trickle line (a deep-zone cell that drips
until you rest, reusing `ActiveFieldBuff::runs_until_rest`) rather than more
rungs on the lump ladder.
