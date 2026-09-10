# The Quarantine Rack — design

**Status:** approved, not implemented
**Date:** 2026-09-10

## The problem

The player's `DownedPrograms` store caps at `MAX_DOWNED_PROGRAMS` (10), and a
full store **refuses the kill**: "No room to carry another downed program —
the store is full." There is nowhere to put a carrier down. A Teardown Rig's
`Hopper` (6, from `strips: Some((hopper: 6))`) is not storage either — it is a
queue the rig is actively chewing through, and it is loaded by hand.

So a deep run with a good tool ladder throws kills away, and the base has no
part in it at all.

## What this is not

**Not an auto-feeder.** An earlier draft had an adjacent rig pull carriers off
the rack the way an assembler pulls ingredients. Dropped: base staff retrieve
a carrier when a job needs one, which is the same outcome through the seam the
base already has (`run_dig_crew`'s "a builder walks to its materials").

**No tool goes near the rack.** `extraction_yield(program, tool)` reads the
tool for both halves of its answer — `tool.yields` is the pool, `tool.tier` is
the scale — so any route that puts a carrier into a rig without the player
standing there needs a tool named from somewhere. That is a fact about the
**rig**, not the shelf. `Hopper::standing_tool` answers it once (below); the
rack stores bare `DownedProgram`s and knows nothing about tools.

**No `SAVE_FORMAT_VERSION` bump.** Every field here is additive behind
`#[serde(default)]`, which the payload's field-named RON loads for free. What
earns a bump is a field removed or one whose meaning changes under a name it
keeps, and this has neither.

## The structure

`assets/structures/quarantine_rack.ron` — a shelf for carriers.

```
(
    id: "quarantine_rack",
    name: "Quarantine Rack",
    description: "Holds downed programs. Walk up to it and press c to access.",
    glyph: 'Q',
    color: Orange,
    build_cost: [("core_fragment", 14), ("bytecode_block", 2)],
    racks: Some((slots: 8)),
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 10), ("cache_grain", 1)])),
    costs_no_program: true,
)
```

Orange and not Cyan: it belongs to the rig's family, not the Depot's, and a
second cyan `D`-alike on the base map reads as storage for items.

**`stores` stays false.** That flag means "a hauler may empty items into it",
and no item ever enters a rack. This is exactly the case the `stores` /
`costs_no_program` split was made for — the seam's own note is that closing
the flag "left a structure that wants the exemption without being a haul
target nowhere to say so". The rack is that structure.

Consequence: it is **not** covered by
`every_shipped_shelf_and_the_portal_cost_no_program`, which keys on
`def.stores`. It needs its own assertion (see *What has to be true*).

### Schema

```rust
/// A structure's carrier-storage capability — see `StructureDef::racks`.
/// Mirrors `StripDef` rung for rung.
pub struct RackDef {
    /// How many downed programs this box holds **at tier 1**.
    pub slots: u32,
}

// on StructureDef
#[serde(default)]
pub racks: Option<RackDef>,
```

`slots` is authored per machine rather than in `tuning.rs`, for
`StripDef::hopper`'s stated reason: it is how big *this* box is, and a
modder's second rack should be able to differ.

**Tier buys slots**, and the ceiling is derived per read as `slots * tier` —
never stored beside the component, `BuildSite::required_ticks`' rule. This is
a new pattern: `capacity` is copied into `Stock::new(def.capacity)` at build
and is never tier-scaled (the mk2..mk6 depots are separate files instead). A
stored ceiling would go stale the moment a tier landed.

### The store

```rust
/// A Quarantine Rack's shelf of downed programs.
#[derive(Component, Default, Clone, Debug)]
pub struct Racked(pub Vec<DownedProgram>);
```

A `Vec`, `DownedPrograms`' own reason: two equal-comparing programs are still
two separate kills.

**This makes a `DownedProgram` live in three places, not two.** The doc on
`components::Hopper` (`components.rs:600`) currently states the two-place
instance boundary as load-bearing. It is rewritten in the same change to name
the third and restate what still holds: what *leaves* a rack is still either
the player's pack or a rig's hopper, and what leaves a **rig** is still plain
items in `Stock::output`, so hauling, depots, `collect::plan_adjacent_take`
and work orders are untouched by any of this.

Saved as `StructureSave::racked: Vec<DownedProgram>` behind a default —
`PlayerSave::downed_programs`' precedent, which stores `DownedProgram`
directly rather than through a parallel `*Save` type because it has no legacy
shape to reconcile.

## The picker

The rack is reached from `Mode::Transfer` (`c`), the one screen and one basket
that already moves cargo both ways.

**A carrier is a row whose range is `[-1, +1]`.** One row per carrier, the
name column its label, the two figures 1/0 or 0/1 depending on which side it
is sitting on. Everything about the screen then works unchanged: the
`item | you | container` table, the arrow convention (an arrow moves stock
toward the column it points at), the projected figures, the Shift/Ctrl
modifiers clamping to the same ends, and the single commit door.

The screen scrolls (`popup_layout` keeps the selected row visible), so a full
rack is not a height constraint. Its **width** is: carrier labels are longer
than item names, and `no_transfer_row_overflows_its_popup` measures the
longest shipped name. That census is written **before** the rows are.

- The row list becomes a sum type — item rows and carrier rows — rather than
  `Vec<TransferRow>`. `TransferRow` itself is not widened: `carried` /
  `can_put` / `on_shelves` are quantities, and a carrier has no quantity.
- With several racks adjacent, room is the sum of their free slots, and a put
  lands in the first rack with room in `(x, y)` order — `plan_adjacent_take`'s
  existing ordering discipline, not a second rule.
- Taking is bounded by `MAX_DOWNED_PROGRAMS` on the way back into the pack,
  and a take that would exceed it is refused before anything moves —
  `commit_caravan_basket`'s rule, asserted per refusal.
- Carriers and items commit together in one action, one turn, one
  announcement.

## The rig's standing tool

```rust
// on components::Hopper
#[serde(default)]
pub standing_tool: Option<ToolId>,
```

Written by `load_teardown_rig` — the tool the player hands over is the tool
that rig is set up with — and read whenever a carrier arrives by any route
other than the player's own hands. The player's hand-load is unchanged and
stays the only way to *set* it, which is why hand-loading is kept rather than
replaced: removing it would need a new screen to name the tool.

A rig with no standing tool does not fetch, and says so through
`set_machine_status` rather than sitting silently idle. `Routines` and `Gear`
are refused at the hand-load exactly as they are today, so a standing tool can
never be one of them.

## The crew fetch

A rig already requires a posted body, so the fetch is that body leaving its
post — `run_dig_crew`'s shape. No new want, no `schedule_base_labour` change,
no `LabourDemand` term.

The carrier is **carried**, not claimed:

```rust
/// A downed program a posted body is physically carrying to a rig.
#[derive(Component, Clone, Debug)]
pub struct CarryingProgram(pub DownedProgram);
```

`Carrying` could not be widened: it is one `(item, qty)` pair *because*
`HAUL_CARRY_CAPACITY` bounds a trip, and a carrier has no `ItemId` at all.

**This is the risky part of the feature and the spec says so.** Two rules
written for `Carrying` now have a second subject, and neither fails to compile
if a site is missed — the symptom is a lost kill with no error:

1. `schedule_base_labour` never frees a body holding a `Carrying`, because
   freeing one destroys the goods. It must not free a `CarryingProgram`
   holder either.
2. Both structure-destruction paths (`damage_structure` and
   `remove_structure`) drop `Carrying` with the `Task` by hand. Both drop
   `CarryingProgram` too — and a carrier being destroyed with the rig is the
   wrong answer: it returns to a rack with room, or to the player's store,
   before it is dropped.

Each gets a test that fails with the line removed.

Where the carrier goes on arrival is the hopper, through the **same capacity
helper the hand-load calls** — the one place the two routes meet.

## What has to be true

Censuses and tests this change owes, beyond the reproducers for each
behaviour:

- **A rack costs no program.** New assertion keyed on `def.racks.is_some()`,
  beside `every_shipped_shelf_and_the_portal_cost_no_program` rather than
  inside it.
- **A rack's upgrade asks for a zone material**, satisfying the existing
  `every_upgrade_path_asks_for_a_zone_material`.
- **`no_transfer_row_overflows_its_popup` covers carrier labels**, written
  before the rows.
- **A freed body never drops a carrier** — one test per rule above, each
  verified by deleting the line and watching it fail.
- **A save round trip keeps a stocked rack**, and a save written before this
  branch loads with an empty one. The RON round trip alone cannot catch a
  skipped field, so this is a save→load test.
- **An empty `assets/structures/` still loads**, and a def with `racks: None`
  is a structure that racks nothing — the `#[serde(default)]` rule.

## Deferred

- **Auto-feed** (a rig pulling from an adjacent rack with no body involved).
  Dropped above; the standing-tool field is what a later version would need,
  and it lands here.
- **A rack filter**, `DepotFilter`'s analogue — refusing carriers below a
  condition or of a given species. No evidence anyone wants it yet.
