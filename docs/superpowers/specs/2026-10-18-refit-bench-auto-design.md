# The Refit Bench holds cores and mends on its own

**Date:** 2026-10-18
**Status:** draft; see `INDEX.md` for whether it shipped.

## Intent

Refitting worn gear costs too much and asks for too many kinds of thing.
A Refit Bench today charges half a craftable copy's recipe, rounded up per
ingredient, plus a Charge Coil, or a rarity-scaled pile of Core Fragments
plus a Charge Coil for a found copy. A worn Nullsteel Plate costs 7 Portal
Fragments, 2 Bytecode Blocks, 1 Logic Wafer and 2 Charge Coils. It is also
a menu action: open the item, press R, pay.

The user's words (2026-10-18): "repairing worn items at the repair bench
takes way too many resources ... pick one [secondary material] that isn't
used much and use that as the repair cost", then: "load the refit bench
with annealed cores, up to 20. and when a player stands within 2 tiles of
the bench it automatically starts repairing items on the player and
companions. starting with worn items and then in inventory. there should
be a purple + floating up when it's happening similar to healing at the
repair bay", and "the player would also have to be idle within range for
more than 2 ticks".

After this change the Refit Bench is a stocked structure like a Turret.
It holds up to 20 Annealed Cores, hauled in from the base's shelves or
loaded by hand. Every refit, automatic or manual, costs exactly one core
from that stock and nothing else. When the player has stood still within
two tiles of a stocked bench for more than two ticks, the bench starts
mending: one copy per tick, worn equipped gear on the player and party
first, then worn copies in the pack, with a purple `+` rising off the
player's tile for as long as it works.

Annealed Core was chosen because it is the least-used crafted
intermediate: five recipes name it, no structure costs it, and its own
description already reads "cycled hot and cooled slow until the lattice
holds a rewrite".

## Goals

- **One material, one unit.** `Game::refit_cost` returns one Annealed Core
  for any worn gear copy, craftable or found, whatever its rarity. The
  three `REFIT_*` constants in `tuning.rs` collapse to one.
- **The bench is stocked.** `refit_bench.ron` authors
  `supply: Some("annealed_core")` and a capacity of 20. Haulers restock it
  from Depots exactly as they restock a Turret's coils. The inspection
  screen's "takes X, holds up to N" line and `in` buffer count appear with
  no renderer change.
- **Hand loading.** Standing beside the bench, the Transfer screen (`c`)
  offers putting Annealed Cores from the pack into the bench's stock, up to
  its capacity. This is the one new Transfer capability: today the screen
  fills only Depots.
- **Automatic mending.** A per-tick `Game` pass beside `run_repair_bays`:
  if the player is in base space, not in battle, has not changed base cell
  for more than `REFIT_IDLE_TICKS` (2) consecutive ticks, and stands within
  Chebyshev distance `REFIT_REACH` (2) of a Refit Bench whose stock holds a
  core, it mends one eligible copy and spends one core. One copy per tick,
  so a long stand clears a whole party.
- **Order of service.** Equipped copies before pack copies. Among equipped
  copies: the player's slots, then each party member's slots in `Party`
  order, slots in `EquipmentSlot::ALL` order; among those, the most worn
  first (highest `wear`), ties by that walk order. Pack copies follow in
  `GearCopies` row order, most worn first.
- **What counts as worn.** A copy is eligible for automatic mending when
  it is broken or its remaining fights are at or below the existing
  `GEAR_WORN_ALERT_PERCENT` line, the same line that raises the `GearWorn`
  alert. The bench fixes what the HUD warns about and leaves a scratch
  alone. Manual refit (the existing R action) still accepts any wear > 0.
- **The mark.** While the pass is mending, the player's `EntityView`
  carries a derived `refitting` flag, the base map draws a rising `+` on
  the player's tile through the same `draw_rising_mark` path the Repair Bay
  uses, in a new purple palette role.
- **Manual refit pays from the bench.** `refit_copy` spends its one core
  from a standing bench's stock, not from the base stores or pack. With no
  stocked bench it refuses: "Load the Refit Bench with Annealed Cores
  first." The refit screen shows the bench's current stock beside the bill.
- **Gate.** The Refit Bench is named in Program Refactoring's
  `unlocks_structures`. Program Refactoring's `min_zone` moves from 2 to 1.
  Its prerequisite Automation has no zone gate, so the ladder rule holds.
- **Copy.** Refit Bench description, Annealed Core description, the Refit
  Bench section and `supply` field docs in `assets/structures/README.md`,
  the research README sentence naming which zone-1 nodes need a subject,
  and a CHANGELOG entry.

## Non-goals

- Changing how gear wears (`GEAR_DURABILITY`, `wear_gear`).
- Mending gear on base staff, posted programs or anything outside the
  player and `resources::Party`. "Companions" is the party.
- Any effect on companions' tiles. The `+` rises on the player only.
- A save-format bump. The bench's stock rides the existing
  `StructureSave.stock_input`; the idle counter is transient.
- Touching `assets/effects/*.ron`; those are battle effects.
- A new alert kind for an empty bench.

## Anti-goals

- **A second copy of the mend.** `refit_copy` and the automatic pass must
  share one function that takes a located copy, rewrites it at wear 0
  through `swap_worn_copy` or the pack helpers, and spends the core. Two
  formulas for "what a refit does" is the bug this repo's "mirrors" rule
  exists to stop.
- **Calling `refit_copy` from inside `tick_inner`.** It calls `tick()`;
  recursion.
- **A stored "mending" marker.** The Repair Bay seam recorded why: a
  parallel component means four edit sites and a save field for a state
  the view can derive. `refitting` is computed per view, never written.
- **Painting the mark in a reserved role.** `ATTENTION`, `THREAT`,
  `FORECAST` each already mean something. The new role must pass the
  palette separability tests; if no purple fits, the fallback is the
  content hue `GlyphColor::Magenta`, not a reserved role.
- **A silent drain.** Mending a copy at 14/15 for a whole core would empty
  the bench after one fight for a party of four. The alert-line threshold
  is what prevents that; do not lower it to "any wear" without the user.

## Constraints

- The engine's `Game` is the whole API; the renderer reads
  `EntityView::refitting` and `StructureReport.supply`, nothing else new.
- `crates/gui/src/paint.rs` stays the only file naming a graphics library.
- `supply`-only structures are sourced from `stores` only and get no
  "beside" credit; do not widen `draws_from_neighbours`.
- Benches are enumerated in `(x, y)` order; bevy iteration order is not
  stable and `min_by_key` takes the first equal minimum.
- The player's `Position` is pinned to the anchor in base space; distance
  is measured from `Game::base_pos()`.
- A copy with `wear > 0` is non-plain and lives in `GearCopies`; a mended
  copy may become plain and move to `Inventory`. Use the existing pack
  helpers for that move.
- `has_machine_status()` becomes true for the bench through
  `spends_supply()`; the status writer skips non-defence supply spenders,
  so no Idle or stall alert appears. Verify, do not assume.
- The new `StructureDef` field is `#[serde(default)]` and documented in
  `assets/structures/README.md` in the same change.
- No `SAVE_FORMAT_VERSION` bump.

## Decisions

Theirs (the user's):

- Annealed Core is the material. One per refit, flat.
- Capacity 20. Reach 2 tiles. Idle more than 2 ticks.
- Equipped gear first, pack second. Player and companions.
- A purple `+` rising like the Repair Bay's.
- The Refit Bench is gated behind Program Refactoring, which becomes
  researchable in zone 1.

Mine (flagged so they can be overturned):

- **Capacity is a new optional `StructureDef` field**, `supply_capacity:
  Option<u32>`, read by `hauling::input_capacity` for supply structures in
  place of `supply_batch * INPUT_STOCK_BATCHES`. The haul trigger
  (`fuel_wants`: file a want when held < batch and a Depot holds at least
  a batch) keeps `SUPPLY_BATCH` (3), so haulers top the bench up to 20 in
  5-unit trips once it drops below 3. Changing the batch instead would
  have required a Depot to hold 10 cores before anyone moved.
- **Automatic mending only serves the worn-alert line.** Reasoning in
  Anti-goals.
- **Most worn first within a tier.** A broken sword before a scratched
  module.
- **Companions are `Party`**, not every owned program.
- **One copy per tick.** Matches the Repair Bay's cadence and lets the
  `+` mean "working now".
- **The idle counter is a transient `Resource`** `RefitStand { cell: (i32,
  i32), ticks: u32 }`, initialised in both `Game::new` and the save loader
  like the cue queues, never saved. Every tick compares `base_pos()` to
  `cell`; a changed cell resets `ticks` to 0, an unchanged one increments.
  Leaving base space resets it. A new `Resource` can shift query iteration
  order in seeded tests; if a seed-luck test moves, fix the test's
  expectation only after reading why it depended on order.
- **Manual refit pays from the bench stock**, and refuses with a stocked
  bench absent. One source of cores, one sentence to learn.
- **The purple role is new**, named `MENDING`, value picked by the builder
  to pass `the_reserved_roles_are_separable` and
  `no_content_hue_takes_a_reserved_role_by_accident`.
- **Hand loading goes through the Transfer screen**, the existing adjacent
  put/take picker, by letting `give_to_adjacent` fill a `supply`
  structure's input with its supply item up to capacity. No new key.
- **Several benches:** both the automatic pass and manual refit take the
  first stocked bench in `(x, y)` order. A bench standing before the gate
  landed keeps working; only new placement is gated.
- **Log lines.** Each automatic mend logs "The Refit Bench mended
  {copy_name}" (player) or "... {creature_label}'s {copy_name}" (party);
  `message_history` condenses repeats. The first tick a stand finds work
  and an empty bench logs "The Refit Bench is out of Annealed Cores" once
  per stand.

## Left to the builder

- Exact `MENDING` colour value.
- Whether `RefitStand` lives on the player entity or as a `Resource`, if
  the `Resource` route proves to shift too many seeded tests. Either is
  transient.
- The shape of the shared mend function and whether `WornAt` (a located
  copy: slot on a wearer, or pack) is an enum or two functions.
- How the Transfer screen labels the bench's input row.

## Tests the plan must carry

- `refit_cost` returns exactly one Annealed Core for a craftable, a
  drop-only, and a rare copy.
- Manual refit spends from the bench stock; refused when no bench holds a
  core; refused when no bench stands.
- Automatic: no mend on ticks 1 and 2 of standing; a mend on tick 3; a
  step resets the count; standing 3 tiles away never mends; a bench with
  no stock never mends and logs once.
- Order: a worn player weapon is mended before a more-worn party armour;
  a party copy before a pack copy; most worn first within equipped.
- Threshold: a copy above the alert line is not mended automatically but
  can be refitted manually.
- A mended pack copy that becomes plain moves to `Inventory`.
- `EntityView::refitting` is true on the mending tick and false otherwise;
  a gui test asserts the rising mark uses `MENDING`.
- Hauling: a bench below `SUPPLY_BATCH` with a Depot holding cores raises a
  want and is filled to 20, never beyond.
- Transfer: a player beside the bench can put cores in up to 20; the 21st
  is refused.
- `structure_unlocked("refit_bench")` is false before Program Refactoring
  and true after; `place_structure` refuses with the existing sentence.
- Asset censuses: `a_structure_named_by_no_research_file_is_buildable_from_the_start`
  drops the bench from its list; the discoverable and subject censuses
  still pass with `min_zone: 1`.
- Save round-trip: a bench with 7 cores loads with 7.
- `balance_sim` unchanged (no species, item stat or tuning curve moved).
