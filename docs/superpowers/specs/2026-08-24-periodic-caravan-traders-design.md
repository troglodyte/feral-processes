# Periodic caravan traders

**Status:** approved, not implemented
**Date:** 2026-08-24
**Todo:** #48, "periodic traders + trading screen"

A trader walks in from the sector, phases into base space through the
anchor, stands beside the iso Market for a while selling a rolled shelf of
goods, and walks back out. It visits on a regular interval with a randomness
window, and what it brings changes every visit.

## Why this, and what it displaces

The game has three trade surfaces already and none of them is an *event*:

- **iso Market** (`assets/structures/black_market.ron`) — a built structure,
  always open, two fixed items on the buy list, buys anything at 1 Credit a
  unit, keeps a buyback shelf keyed by `(kind, tile)`.
- **Stack market** (`game/stack_market.rs`) — a shelf derived per
  `CellKind::Market` cell underground: routine disks and programs, no
  buyback, its own screen.
- **Broker board** (`game/contracts.rs`) — offers derived from
  `(world seed, sector, epoch)`, rotating on `CONTRACT_REFRESH_CYCLES` with
  no save field at all.

The caravan is the RimWorld/Dwarf Fortress shape: a counterparty that is
sometimes there and sometimes not, carrying a wide and varying stock. The
standing intent is that **the always-open iso Market counter is later
removed**, leaving the caravan as the main way to trade. This design does
not remove it — it builds the thing that will replace it, and gates itself
behind it in the meantime.

## Decisions taken

| Question | Decision |
|---|---|
| Where it docks | Beside the iso Market, in base space |
| Gate | A caravan only visits a base with an iso Market standing |
| Arrival | Regular interval plus a randomness window, both tunable |
| Visit length | Long enough to walk home from a field trip; tunable |
| Stock | Rolled gear, routine disks, programs, materials — varying per visit |
| Buy side | Same rate as the iso Market |
| Buyback | **None.** A caravan keeps no memory of what it bought |
| Glyph | `Ω`, in a colour no base fixture uses |
| Reach | Being in base space, as `broker_reach` measures it |
| Surface presence | Examinable, but nothing can fight it |

## 1. The entity and its journey

A `Caravan` entity carries `Position`, `Glyph`, and a component
`Caravan { stage, visit, arrival_tile }`.

Deliberately **not** a `Structure` — structures do not walk, and one would
enter the raid targeting query, both destruction paths, `accepts_a_program`,
`feeders_for`, the stock strip and the `(kind, tile)` buyback ledger, each
needing its own exclusion. Deliberately **not** a `Creature` — nothing
should be able to fight it. `DigSite` is the existing precedent for a
non-`Structure` entity carrying a base-space `Position`; this is the third.

Five stages, one tile a tick:

| Stage | Space | Behaviour |
|---|---|---|
| `Approaching` | zone surface | Spawns `CARAVAN_SPAWN_DISTANCE_TILES` from the anchor on a derived bearing, descends a `pursuit::walk_field` gradient rooted at the anchor tile |
| `Docking` | surface → base | Standing on the anchor, phases in and lands on the cell the anchor's door opens onto (`game/base_space.rs`) |
| `Crossing` | base space | Walks to a free floor cell orthogonally adjacent to the iso Market via `hauling::step_to_post` |
| `Docked` | base space | Stands there for `CARAVAN_STAY_TICKS`. **The only stage that trades.** |
| `Leaving` | base → surface | Reverses the route, despawns on reaching `arrival_tile` |

Both walks are the walks that already exist. `walk_field` is the one
Dijkstra walk on the surface and takes its step rule as a parameter;
`step_to_post` is the shared base-space answer to "which way", already
shared between `haul_step_system` and `Game::run_dig_crew`. Neither gets a
second copy.

**Which map it appears on.** `Game::stands_in_base_space` is currently
`Structure || Tamed`. It gains a third arm that reads the caravan's stage,
so `view_entities` shows the caravan on the surface map while it approaches
and on the base map once it is through the anchor — never both, never
neither. This is the same split the player already lives under, whose
`Position` stays pinned to the anchor while out of phase.

**Failure modes**, each announced once via the `DigSite::announced_stuck`
latch idiom rather than every tick:

- No surface route to the anchor: the caravan gives up and despawns. The
  visit is a miss.
- No base-space route to the Market: it waits at the landing cell and leaves
  at the end of its stay. A badly-laid-out base costs you a visit, which is
  a consequence rather than a bug.
- Market destroyed mid-visit: the caravan leaves early.

A base with more than one Market is resolved by sorting candidates by
`(x, y)` and taking the first, for `assembler_system`'s reason: bevy's query
iteration order is not stable, and a caravan that docked at a different
Market between two loads of the same save would be reporting the iteration
order rather than the base.

It walks whether or not the player is watching — ticks run regardless, and
there is no special case for a party that is underground.

## 2. Schedule — derived, tunable

```
visit_index = current_tick / CARAVAN_VISIT_INTERVAL_TICKS
seed        = fold(BaseGrid::seed(), CARAVAN_SALT, visit_index)
```

`fold` is `contracts::fold`, the FNV-1a byte-at-a-time scheme this repo
already salts with — reused rather than re-invented, per `FrameSpec::salted`'s
rule that there is one salting scheme and not a second that could collide.
From that seed come the arrival offset within `CARAVAN_ARRIVAL_JITTER_TICKS`,
the approach bearing, which trader type visits, and the shelf itself.

Seeded off `BaseGrid::seed()` rather than `WorldMap::seed()` because that
seed is minted at `Game::new` and **travels with the base** across a breach,
while the world seed is re-minted per zone. The rhythm is a property of your
base, not of the sector you happen to be standing in.

New `tuning.rs` constants, all documented `pub const` in a labelled section:

- `CARAVAN_VISIT_INTERVAL_TICKS` — the regular period.
- `CARAVAN_ARRIVAL_JITTER_TICKS` — the randomness window on arrival.
- `CARAVAN_STAY_TICKS` — how long it stands docked.
- `CARAVAN_SPAWN_DISTANCE_TILES` — how far out it appears.
- `CARAVAN_MARKUP` — the premium over an item's own value.

How many rows a trader brings is **not** here: it is `CaravanDef::rows`, and
`tuning.rs`'s rule is that it holds what the engine hardcodes and never a
copy of a `.ron` value.

**The split that keeps this honest.** *What is on offer* is derived, and
inherits the Broker board's four properties for free: it survives a reload
with no save field, reading it spends no `GameRng` draw and so shifts
nobody's stream, it cannot be rerolled by save-scumming, and it rotates on
its own. *Where the trader is standing* is state, saved like any entity's
`Position`. Two different questions, so there is no duplicate source of
truth to drift.

## 3. The shelf — data, not Rust

A new asset directory `assets/caravans/`, one `.ron` per trader type, loaded
by a `CaravanDb::load_dir` following the existing pattern — a malformed file
is skipped with a logged warning, never a panic. `CaravanDef` carries:

- `id`, `name`, `description`
- `glyph`, `color`
- `rows` — how many shelf rows this trader brings
- `weights` — relative weight across the four pools: rolled gear, routine
  disks, programs, materials
- `min_zone` / `max_zone` — the sector window this trader appears in

The visit seed picks *which trader type* visits, so "the stock changes" has
teeth beyond a reroll of the same table: a gear runner one visit, a program
broker the next. A mod adds a trader by dropping in a file, and adds none of
its content twice — the four pools resolve against the existing item,
ability and species catalogues.

Prices are `item_value × CARAVAN_MARKUP`, scaled by `ZoneLevel`, and for
anything craftable floored above what its recipe's ingredients cost.
Programs are priced by power, sharing `stack_market`'s existing helper
rather than growing a second appraisal.

Selling to it uses the iso Market's rate and **stocks no shelf** — a caravan
has no `BuybackLedger` entry and nothing sold to one can be bought back.

## 4. Within-visit depletion

Buying a row removes it for the rest of the visit. `market_spent`'s pattern:
a resource `CaravanMemory { visit: u64, bought: BTreeSet<usize> }`, the
derived shelf filtered by the bought set. Keyed by visit index, so it
self-clears the moment the index moves and needs no explicit reset.

This is distinct from a buyback shelf and does not reintroduce one: it
records which of *its own* rows the caravan has sold, not what it took off
the player.

## 5. Screen

A new `Mode::Caravan` and a `Mode::CaravanQuantity`, in their own app-core
file, sharing only the pricing calls with `Mode::Trade` — the precedent
`app/stack_market.rs` set for a second counterparty, whose header states the
reasoning. Two stacked sections resolved through a `caravan_row` helper on
`market_row`'s model: what the caravan sells, then what it will take. No
buyback section, so there is no third offset.

`Game::caravan_reach() -> CaravanReach` with three states — `NoCaravan`,
`NotDocked`, `AtCaravan` — one call answering both "is there a trader" and
"can I deal with it", which is `Game::stack_market`'s and `broker_reach`'s
shared contract. Three states rather than two booleans for `NoPost::BoxedIn`'s
reason: the three leave the player different errands.

`AtCaravan` measures **base space**, exactly as `broker_reach` does — the
walk to the Market is visibility and flavour, not a gate.

The base menu row is added to `base_menu_rows`, which must stay the only
source of those rows.

## 6. Rendering and examine

`Ω` in a `GlyphColor` no base fixture uses, chosen after a census of shipped
glyphs and colours. Authored in each shipped `assets/caravans/*.ron` rather
than hardcoded — `glyph` and `color` are `CaravanDef` fields, so a modded
trader picks its own, and no Rust names a caravan id. The caravan draws through `view_entities` like any other
`(Position, Glyph)` entity, so no new draw path is needed in
`render/base.rs`.

The examine ray (`views::drawn_on_surface_map` /
`Game::find_target_in_direction`) gains a caravan arm, so an inbound caravan
can be named. It carries neither `Creature` nor `Structure`, which is
precisely the known gap that makes nests, surface links and zone portals
invisible to the ray today — so this arm closes part of that gap rather than
widening it.

Arrival and departure each log one line. Arrival is `MessageKind::Info`;
neither is `Raid`.

## 7. Save

Two additive fields behind `#[serde(default)]`, so **no
`SAVE_FORMAT_VERSION` bump** — the save is field-named RON and an additive
change costs no version.

- The caravan's journey: stage, position, visit index, arrival tile. A
  **named struct, never a positional tuple** — the one shape field-named RON
  does not save you from.
- `CaravanMemory`.

Both are wiped **by name** in `enter_next_zone`, beside `BuybackLedger`,
`StackMemory` and `PopulatedChunks`: the caravan is mid-walk on a zone
surface that is about to stop existing, and its visit does not survive the
breach.

## Fences

The design spine is that progression is earned by fighting. What holds here:

- Prices scale with `ZoneLevel`, so Credits banked in zone 1 do not buy
  zone-6 power.
- `CARAVAN_MARKUP` floors any craftable above its recipe cost — you pay for
  not waiting.
- Buys at the iso Market's rate with no buyback, so there is no round trip
  to arbitrage.
- **Portal Fragments can never appear on a caravan shelf.** Held by a census
  over the real assets, the same shape as the contracts census that keeps
  `Reward::PortalFragments` absent rather than merely unused.
- A caravan's shelf is finite per visit and does not restock.

## Testing intent

- The schedule: a visit opens on the interval, the jitter moves arrival
  within the window and not outside it, and two consecutive visits differ.
- The derivation survives a save/load unchanged, and reading it draws no
  `GameRng` — asserted by comparing the stream position across a read.
- The journey: each stage transition, on a fixture base with a Market. The
  two stuck cases announce once and not per tick.
- `stands_in_base_space` puts the caravan on exactly one map per stage —
  asserted through `view_entities` from both locales, not by calling the
  predicate.
- No buyback: selling to a caravan leaves `BuybackLedger` untouched.
- Depletion: a bought row is gone this visit and back next visit.
- The refusal census (`every_screen_draws_a_refusal_exactly_once`) covers
  both new `Mode`s.
- Popup width and height censuses for the new screen, per
  `the_tallest_gear_page_fits_its_popup` — a text-row page has no scroll.
- The Portal Fragment census above.
- Full `cargo test --workspace` is the gate, and
  `cargo test -p feral-processes-engine balance_sim` because prices move.

## Out of scope

- Removing the iso Market's always-open counter. That is the stated
  direction, not this change.
- Caravans being attacked, escorted, or raided on approach.
- Trading with a caravan out on the surface before it docks.
- Multiple caravans at once.
- Caravan-specific contracts or reputation.

## Risk: the seam docs are stale here

`CLAUDE.md`'s **The base** section — and the matching entries in
`docs/seams.md` — describe code that has been deleted. `resources::Platform`,
`Game::build_radius`, `Platform::covers` and `build_radius_bonus` are gone;
the base is no longer a slab stamped on the zone surface but a phased-out
interior pocket in `base_grid::BaseGrid`, entered through an anchor, with
floor laid by mining rather than derived from a radius. `broker_reach` now
reads `base_pos().is_some()`. `hauling::walk_leg` and its `Leg` enum, named
in the seam list, do not exist; `hauling::step_to_post` is the shared walk.

Every claim in this document was checked against the source rather than
against those docs. Implementation should do the same, and a separate pass
to correct both files is worth scheduling — it is unrelated to this feature
and should not be folded into it.
