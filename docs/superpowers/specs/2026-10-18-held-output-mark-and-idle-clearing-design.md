# Held output: a dot on the structure, and idle staff who clear it

**Status:** open. Written 2026-10-18 on branch `held-output-mark`.

## Intent

The player looked at a full Depot and an empty stockpile and read it as
production stalled. The routine disks they had ordered were in the machine
that made them. Nothing on the base map said so, and nobody was coming for
them.

Two changes, independent, shipped in this order:

1. **The dot.** A structure whose finished-output buffer holds anything wears
   a small dot in its top-left corner. At a glance: *the items are in here,
   not in storage.*
2. **Idle clearing.** An idle program carries finished output out of a
   machine to a Depot with room, or failing that onto free stockpile tiles,
   so product stops piling up in the machine while storage has space.

The dot ships first because it is what was asked for, and because it makes
the second change visible when it lands.

## Why the disks sat there (what the code does today)

Verified against source on 2026-10-18; file references are to that tree.

- Finished product lands in `Stock::output` on the machine
  (`components.rs`, `assembler_system` in `systems.rs`). Capacity is
  `DEFAULT_OUTPUT_CAPACITY` (20); at zero room the machine reads `Clogged`.
- The only thing that moves output out is the **posted worker's own**
  `Errand::Tend` in `haul_step_system` (`game/base/hauling.rs`), and only
  when the machine is clogged or stranded, or has no attached consumer or
  accepting Depot beside it, **and** some Depot has room. Depots with no room
  are not candidates at all; a carried load with nowhere to go bounces back
  into the machine and posts `DepotsFull`.
- Stockpiles are never a destination for machine output. They are zoned
  floor cells; only the idle **tidy** errand (`game/base/tidy.rs`) uses
  them, carrying loose floor items onto free cells, one unit per tile.
- Once a work order is satisfied the machine leaves `wanted`, its worker is
  released unless the output is completely full, and nobody returns for a
  partial buffer.

So: every Depot was full, the stockpile was not a candidate, and the order
finishing took away the one body that could have hauled.

Precedent for the fallback this design adopts: `Game::set_machine_recipe`
already empties a machine output-to-Depots-first and the rest to the floor
(`machine_recipe.rs::empty_machine`, through `stock::return_to_depots` and
`Game::set_down`).

## Goals

- A player can tell, from the base map alone, that a structure is holding
  finished items.
- Finished output leaves a machine whenever a Depot or the stockpile has
  room for it, without a work order and without a posted worker.
- A busy base pays nothing: no body is taken off scheduled work.

## Non-goals

- Staged ingredients (`Stock::input`) are not marked and not cleared.
- Queued or racked programs (Teardown Rig hopper, Quarantine Rack) are not
  "items" for this feature.
- No new alert. The dot, plus the existing `DepotsFull` alert, is the
  signal when nothing can move.
- No change to the posted worker's `Tend`, to `depot_intake_system`, to
  Depot filters, or to the work-order priority list.
- Depots do not wear the dot; the fill bar already says how full one is.

## Anti-goals (what would make this a failure)

- A body pulled off a dig, farm, build or order to clear a machine.
- A load destroyed, or a body left standing forever holding one.
- Output set down on ordinary floor when neither a Depot nor the stockpile
  can take it. It stays in the machine.
- A base with no Depot and no stockpile behaving any differently from
  today.
- A second mark painted through the staffed mark, the progress bar, the
  depot fill bar or the rarity bar.

## Decisions (the player's)

| Question | Decision |
|---|---|
| Where the dot goes | Top-left corner, padded in from the edges. |
| What counts as holding | Finished output only (`Stock::output` non-empty). Inputs never. |
| Depots | Exempt. |
| Who clears | Idle programs, as an errand on the same fall-through as tidying, ranked above it. |
| When clearing starts | Any output at all. One unit is enough. |
| Where it goes | A Depot with room that accepts the item first; else free stockpile tiles. |
| Nowhere to go | Nothing is picked up. It stays in the machine. |

## Part 1: the dot

### View

One new field on `EntityView` (`crates/engine/src/views.rs`), beside
`output_stranded`:

```rust
/// A structure other than a Depot whose finished-output buffer holds
/// anything. The map's "the items are in here, not in storage" dot.
pub holds_output: bool,
```

Filled in `game/inspection.rs` next to `output_stranded`, from the same
`Stock` read: `is_structure`, the structure's def does **not** `stores`,
and `Stock::output_used() > 0`. Every other `EntityView` literal (the two
test fixtures in `render/base.rs` and `render/building.rs`) gets
`holds_output: false`.

A banked output (`Game::buffered_output`) never reaches `Stock::output`, so
it never lights the dot. That is correct: it is not in the machine.

### Mark

In `crates/gui/src/render/marks.rs`, a geometry function in the family of
`staffed_mark_rect` / `nemesis_mark_rect`:

```rust
pub(super) fn held_output_rect(px: f32, py: f32, tile_px: f32) -> Rect
```

- Top-left corner: `x = px + IDENTITY_MARK_INSET`,
  `y = py + RARITY_BAR_PX + IDENTITY_MARK_INSET`. The rarity bar owns the
  full top edge and is painted first (seams-hud: "the top-corner marks
  drop below the rarity bar"), and the left inset is `nemesis_mark_rect`'s
  reason: flush to the edge it reads as a line `outline_open` left off.
- Size: a new named constant `HELD_OUTPUT_MARK`, **smaller than
  `IDENTITY_MARK` (0.22)**, so it reads as a dot and not as a fourth
  corner badge. Builder's call on the exact value; 0.14 to 0.16 of the
  tile is the intended range. Painted with `painter.rect` (a square dot at
  these sizes; `poly` only if the square reads badly at the smallest
  `map_cell` rung).
- Hue: the hue the cell already wears, as every other structure mark does.
  Alpha 1.0, no bob, no blink: it is a fact about contents, not an event.
- Drawn in `draw_surface_map` (`render/base.rs`) with the other
  structure-keyed marks, after `outline_open` and after `draw_depot_fill`,
  gated on `structure.is_some_and(|ev| ev.holds_output)`.

Creatures never wear it, and the creature-only con wedge lives in the same
corner, so there is no collision on one tile; a structure's tile never
carries a con read. Add the new rect to the existing mark census test in
`render/base.rs` (the one that already measures `patrol_mark_rect`,
`nemesis_mark_rect`, `staffed_mark_rect` and `difficulty_mark_points`
against each other) so a future move cannot overlap it with the staffed
mark or the depot fill bar.

### Tests

Engine (`crates/engine`, inspection):

- A machine with one unit in `Stock::output` → `holds_output == true`.
- The same machine with the unit in `Stock::input` only → `false`.
- A Depot holding goods → `false`.
- A machine with an empty buffer → `false`.

GUI (headless `with_painter`, as the depot fill tests do):

- `holds_output: true` paints exactly one rect inside the top-left quadrant,
  below `RARITY_BAR_PX`, left of the tile's horizontal midpoint.
- `holds_output: false` paints nothing there.
- Census: `held_output_rect` is disjoint from `staffed_mark_rect` (at every
  bob lift), `depot_fill_rect`, `progress_bar_rect` and the rarity bar.

## Part 2: idle clearing

### The errand

A new idle errand, **clearing**, on `drift_idle_staff`'s fall-through
(`game/base/work_orders.rs`): below every errand that has a claim on the
body (recovery, amenity service, the heap-splitting wander), **above tidy**,
above the wander. Tidy's gates apply unchanged: `may_tidy`'s tests (on laid
floor, no `Task`, no `UnderStudy`, not already carrying, on shift) are the
same tests for clearing. Extract or share them rather than copying.

A base with nothing to clear to, no Depot and no stockpile cell, builds no
board and pays nothing. That is tidy's own rule and the one the base seam
already states for Depots.

### The board

Built once per beat alongside `TidyBoard`, shared by every clearer in the
pass:

- **Sources:** every structure with a `Stock` whose def does not `stores`
  and whose `output_used() > 0`, with the item (or items) held.
  **One exclusion:** a machine whose produced item has an accepting Depot
  touching it with room is skipped, because `depot_intake_system` is
  already pulling from it and an idle body walking across the base for one
  unit the intake takes next beat is a wasted trip. Use
  `hauling::depot_beside_accepting`; do not re-derive adjacency. (Builder's
  call, mine, flagged below.)
- **Depot targets:** `stores` structures with `output_room() > 0`,
  filtered per item through `Game::depot_accepts`, the one door.
- **Stockpile targets:** the free stockpile cells `tidy_board` already
  computes. One unit per tile, `floor.free` the one predicate.

### One trip

Nearest source first (Chebyshev, ties by `(x, y)`, reachable by
`crew_reach`/`reaches`, the tidy idiom). At the machine:

1. Pick the destination **before** picking up: the nearest reachable Depot
   with room that accepts the item (`nearest_depot`, which already encodes
   the reachability and tie rules); else the stockpile if it has at least
   one free reachable cell; else **leave the machine alone** and try the
   next source.
2. Take `min(output of that item, HAUL_CARRY_CAPACITY, room)` where room is
   the Depot's `output_room()` or the count of free stockpile cells. Use
   `take_haul_load` so the carry cap and the single `(item, qty)` pair stay
   the seam's.
3. The load is **`Carrying`**, the one load state hauling has, so every
   existing drop path (`floor::drop_load` on post, dispatch, off-shift,
   permadeath, fusion and the rest) applies without a new arm. A marker on
   the body says this hold belongs to the clearing errand and where it is
   going, so `may_tidy`'s "a body holding a hauler's load is mid-trip for
   somebody else" test still means what it says. The marker's exact shape
   is the builder's; see "Left to the builder".
4. Deliver: into the Depot through the existing deposit door; onto the
   stockpile through `floor::spawn_floor_item` on free cells nearest the
   body, one unit each, re-checking `free` on arrival as tidy does.
5. If the destination has gone (Depot filled or demolished, cells taken)
   mid-walk, fall back once to the other kind of target; if that fails too,
   `drop_load` where the body stands. A load is never destroyed and a body
   is never left holding one.

A body the scheduler posts mid-errand sets its load down first, as
`set_down_posted_tidiers` does for tidiers; extend that pass rather than
adding a second one.

### What this does not change

- The posted worker's `Tend` and its "clogged or unattached, and a Depot
  has room" rule. Two bodies pulling from one machine is harmless:
  `take_haul_load` is the one writer and whoever arrives second takes what
  is left.
- `depot_intake_system`, filters, `DepotsFull`, the priority list,
  `HAUL_CARRY_CAPACITY`, `HAUL_WALK_MAX_TILES`. No new tuning constant
  unless the builder finds a magic number with nowhere to live.

### Save

If the marker is a new component on a body, it follows `Tidying`'s save
handling in `save.rs` (saved for `carrying_program`'s reason), behind
`#[serde(default)]` so no version bump; and it gets a save-then-load test,
because a RON round-trip cannot catch a skipped field.

### Tests (engine)

The reproducer first, failing before the change:

- **The player's case.** A machine with finished output, every Depot full,
  a stockpile with free cells, one idle program: after enough beats the
  output is on stockpile tiles and the machine's buffer is empty.
- A Depot with room that accepts the item is chosen over the stockpile.
- Neither has room: nothing is picked up; `Stock::output` is unchanged
  after many beats.
- A machine beside an accepting Depot with room is not a source.
- Clearing outranks tidy: with a loose floor item and a machine both
  waiting, the idle body goes to the machine.
- Posted mid-trip: the load is on the floor, unit for unit; nothing is
  destroyed.
- No Depot and no stockpile: no board, no `Carrying` ever appears on an
  idle body, the machine's buffer is untouched.
- Save round-trip with a clearer mid-trip.

Each with a mutation check (remove the fix, the test fails). Seeded RNG,
no wall clock, and remember a `-p` run is single-threaded where the
workspace run is not.

## Docs and seams

- `CHANGELOG.md`: one entry per part when each lands.
- `assets/help/60-your-base.md` line 35 describes stockpiles; add one plain
  sentence that idle staff move finished product out of a machine into a
  Depot or onto the stockpile when there is room. Plain language, no
  jargon.
- `.claude/rules/seams-base.md` gains one line for the new errand (its rank
  and the "destination before pickup" rule); the `seams` skill gets the
  trap; the memory graph gets `seam:<slug>` with the argument. Three
  writes, in the order the skill documents.
- `docs/superpowers/INDEX.md` gets its row at landing, not before.

## Left to the builder

- `HELD_OUTPUT_MARK`'s exact value and whether `rect` or `poly` reads
  better at the smallest cell size. Check both with a real launch and a
  screenshot (`--template` plus `--screenshot`), not only the headless test.
- The marker's shape: a new component (`Clearing { to: ... }` or similar)
  versus reusing `Tidying`. `Tidying` holds one `FloorThing`, a clear holds
  up to five units of one item, so `Carrying` is the load and the marker is
  separate. Run the design-patterns dialog before choosing; the constraint
  that matters is that `may_tidy`'s "mid-trip for somebody else" reading
  survives and that no second load state appears.
- Whether to write a `Record::Haul` for a clearing trip. The instrumentation
  seam keys hauls to a post and a `Tend` writes nothing; read
  `docs/measurements/README.md` and the instrumentation rules before adding
  a record kind, and prefer none.
- Whether the board's per-beat scan of every `Stock` needs caching. It is
  the same shape as `tidy_board`'s floor-item query; no optimisation
  without evidence.

## Calls I made (check these)

- **Depot exclusion for the board source.** A machine touching an accepting
  Depot with room is skipped because intake is coming for it. If the player
  would rather the dot and the clearer ignore intake entirely, delete the
  exclusion; nothing else depends on it.
- **The dot hue is the cell's own.** Not `HEALTHY`, not `ATTENTION`: holding
  output is neither a reserve climbing nor a thing needing the player's
  hand.
- **Clearing takes one item kind per trip**, the carry seam's single pair.
  A machine holding two products gets two trips.
- **Rank above tidy.** Product in a machine blocks production; a loose
  floor item blocks nothing.

## Sizing

Two crates for the dot (engine view + gui), the base labour seam for the
clearing: a plan follows this document, dot tasks first, clearing tasks
after, each task TDD with a commit per green step and
`cargo test --workspace` as the gate.
