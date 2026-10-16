# Floor items and stockpile zones

**Date:** 2026-10-16 · **Branch:** `floor-items` (worktree `../feral-processes-floor-items`)

## Intent

The player can put anything they carry down on the base map and pick it up
again, and can mark stockpile zones where idle programs tidy loose items.
A tile holds **one item, never a stack**. Floor space therefore runs out
quickly, and that pressure is the reason to build depots. Stockpile filters
come later; for now a stockpile takes anything.

## Goals

1. **Drop and pick up.** On the base map the player can put down any one
   thing they hold, and pick up the thing underfoot:
   - a plain item (`Inventory`, one unit at a time),
   - a gear copy (`GearCopies`),
   - a downed program (`DownedPrograms`).
2. **One thing per tile.** A floor tile holds at most one floor item, and
   that item is one unit.
3. **Per-item symbols, as data.** `ItemDef` gains a `glyph` field (generic
   symbols such as `.` `*` `,` `"` `'` `` ` `` `~` `:`) under
   `#[serde(default)]`, documented in `assets/items/README.md`. Every
   shipped item gets a glyph chosen to suit it. A downed program on the
   floor is drawn as the gray `x` the tactical map uses for a disabled
   body. That means reusing the same glyph and colour; find where the
   tactical render draws it.
4. **Stockpile zones** are marked with the same cursor/anchor/box flow as
   the Excavation plan (`app-core/src/app/excavate.rs`,
   `Game::toggle_mark_box`): pressing the box on an already-marked anchor
   clears it. Only laid-floor cells in the box are marked. A stockpile cell
   holds one item, like any other floor tile.
5. **Cleanup.** A tamed program in base space that has no job (`Task`), is
   not `Downed`, `OffShift`, `UnderStudy` or on respite, and is not crowded
   carries one loose floor item at a time to a free stockpile cell. It is
   the lowest-priority idle behaviour: a new arm in `drift_idle_staff`
   between the respite arm and the wander fall-through
   (`game/base/work_orders.rs` ~2480). Work orders and needs always win. A
   program handed a `Task` mid-carry sets its item down (see *Setting
   down*).
6. **Dropped loads spread out.** Every current producer of a floor pile
   (`floor::drop_load` and its eight callers) now puts a load of `qty`
   down as `qty` separate floor items on the nearest free floor tiles.
7. **Saves.** Floor items and stockpile cells are saved. Older saves still
   load, and an old `floor_piles` entry is spread out one item per tile on
   load.

## Non-goals

- Stockpile filters, priorities, and per-zone settings.
- Loose floor items (not on a stockpile cell) counting as building or
  crafting stock. Only stockpiled items count (see *Stockpiles as stock*).
- Floor items anywhere but base space (zones, settlements, the Stack,
  tactical maps).
- Picking up from a tile other than the player's own.

## Anti-goals (a failure even if it works)

- **An item is never destroyed** to make room or to free a body (the
  existing rule at `seams-base.md` ~258 still holds).
- **No stacking by the back door.** No code path leaves two items, or a
  quantity above 1, on one tile.
- No content hardcoded in Rust: glyphs come from `assets/items/*.ron`.
- Floor items never block movement or building placement checks that
  don't concern them. Bodies walk over them.

## Model (decided)

- **One entity per floor item**, with `Position` plus a floor-item
  component holding one value of a new enum, called `FloorThing` here:
  `Item(ItemId)` | `Gear(GearCopy)` | `Program(DownedProgram)`. It
  replaces `FloorPile`, and `spawn_floor_pile` becomes the single writer of
  the new component list, keeping the "no `Glyph` component" choice so the
  `(&Position, &Glyph)` queries still can't see it.
- **Free floor tile**, the one predicate for "an item may be set down
  here": laid floor (`BaseGrid::is_floor`), walkable, no structure, no
  build site, no floor item. Bodies standing there don't make it unfree.
  Model it on the predicate in `Game::displace_bodies_from`
  (`building.rs` ~425) and the BFS `nearest_free_floor` (`building.rs`
  ~2006). Promote that BFS to a shared helper, not a copy.
- **Setting down** = place on the drop point's own tile if free, else
  search outward ring by ring (Chebyshev radius 1, then 2, then 3, …) and
  take the free tiles in the first ring that has any, so a dump of ten
  items lands on the floor closest to where it was dropped *(user
  decided)*. Pick within a ring in a stable `(y, x)` order so a reload
  places the same way. Distance is distance, not path length: reachability
  plays no part. **If no free floor tile exists anywhere:** a plain item or gear copy goes into the player's
  `Inventory`/`GearCopies`, and a downed program goes into
  `DownedPrograms` if under the cap, otherwise the nearest Quarantine Rack
  hopper. If none of those can take it, the drop is refused. Log a message
  in every fallback case. *(My call. The requirement is only that nothing
  is destroyed.)*
- **Cleanup carry state** is a new component on the carrying body, holding
  one `FloorThing`. It is separate from hauling's `Carrying`, which
  `seams-base.md` ~256 says stays a single `(item, qty)`. Destination and
  pickup target are derived fresh each tick, as with hauling's errands:
  - target = the nearest reachable loose item (not on a stockpile cell)
    that some reachable free stockpile cell exists for;
  - destination = the nearest reachable free stockpile cell.

  If two bodies race for the same item or cell, the second re-targets on
  arrival; no claim store. Every path that ends a body's life in base space
  while it carries (the same list `drop_load`'s callers cover, plus
  anything ending `CarryingProgram`) must set the item down, never drop it
  silently.
- **Stockpile cells**: one stored set of marked positions in base space,
  saved. Building over a cell leaves it marked but not free. Excavation
  marks and stockpile marks are independent.

## Player-facing (decided)

- Keys *(my call; confirm they are free in base-space `playing.rs` before
  use. Map-level actions are lowercase letters here, but popup rows are
  also lowercase per the row-selector convention)*:
  - `z`: stockpile zone mode (Excavate-style cursor; Space/Enter anchor
    then commit; Esc clears the anchor, then exits);
  - `g`: pick up the item underfoot;
  - `t`: put down, opening a picker over Inventory, gear copies and downed
    programs (reuse the transfer picker / `downed_program_list_rows`
    patterns).
- A drop goes onto the player's own tile when it is free; otherwise it
  overflows ring by ring to the nearest free tiles, through the same set-down path
  and fallback as every other drop *(user decided)*. Pick-up respects
  `MAX_DOWNED_PROGRAMS`.
- Map draw: an item's glyph, under bodies and structures as the `%` mark
  is today. Stockpile cells get a faint mark or tint distinct from
  excavation marks. `FLOOR_PILE_MARK` and its "one mark" doc go away.
- Examine (`x`) names the item underfoot ("a scrap on the floor", "a
  downed <program> on the floor") and says when the tile is a stockpile.

## Stockpiles as stock (user decided: stockpiled items count)

A plain item (`FloorThing::Item`) on a stockpile cell counts as base stock
everywhere shelves count today. *Ordering is my call.*

- **Builder fetch:** `construction::nearest_source` gains a stockpile-cell
  source alongside `Source::Shelf` and `Source::Pack`. It is a physical
  walk-and-take, like a shelf. It sorts after shelves and before the pack
  among equals, keeping the comment's rule that the pack is the fallback.
  Each fetch lifts one unit, because a cell holds one.
- **Abstract spending:** `stock::spend_from_base`,
  `stock::spend_bill_from_base` and `Game::stores_hold` /
  `spend_from_stores` (`base_space.rs` ~1032) count stockpiled items and
  spend them after shelves, in a stable `(x, y)` order so a reload spends
  the same way.
  - Read the doc comments at those sites first. They carry the
    determinism rule.
  - Find every caller of `stores_hold`-style counts, such as the build
    menu's affordability and the HUD stock block, so what the player is
    shown matches what can be spent. One shared "stockpiled count" read
    feeds all of them, not one copy per site.
- Gear copies and downed programs on a stockpile are not materials; they
  just sit there.

## Interaction with existing hauling (my call — flagged)

Posted haulers keep `Errand::Pickup` for plain floor items a depot
accepts. That includes items sitting in a stockpile, so depots drain
stockpiles, which is the "build depots to save space" loop. A pickup now
lifts one unit per trip, because a tile holds one unit.

## Constraints

- Three crates (engine, app-core, gui) plus an `ItemDef` schema field and
  a save-format addition → the spec-and-plan pipeline.
- `#[serde(default)]` on the new `ItemDef` field and on both new
  `SaveData` fields; a save→load test for each (memory
  `ron-round-trip-cannot-catch-a-skipped-field`).
- A new Resource or entity kind can shift query iteration order and RNG
  streams (memories `new-resource-shifts-query-iteration-order`,
  `rng-stream-shift-exposes-seed-luck-tests`). Expect seed-luck tests to
  move and read each failure's code path rather than re-run.
- Read `.claude/rules/seams-base.md` and `seams-items.md` and the `seams`
  skill entries for hauling, `drift_idle_staff`, `Carrying` and
  `DownedPrograms` before changing them. A new seam (the floor-item
  invariant) gets the three writes the `seams` skill documents.
- `balance_sim` is unaffected; run it anyway as part of the full gate.
- Release: one version bump and a CHANGELOG section when it lands on
  `main`. The `CHANGELOG.md` preamble decides which digit. Saves keep
  loading, so this is not breaking.

## Settled after the builder check (my calls unless noted)

- **Spreading a load:** `drop_load` sets each unit down through the same
  set-down path and fallback as a player drop: a free tile, otherwise the
  player's stores. It never returns a remainder.
  - Loads are small: `Carrying` is capped at `tuning::HAUL_CARRY_CAPACITY`,
    so a load covers a handful of tiles, not hundreds.
  - The old-save migration uses the same path, so it cannot lose items.
  - Run one BFS per load and take the first `qty` free tiles it reaches.
    Don't run one BFS per unit.
- **Set-down search** is the ring search in *Setting down*, for every
  drop: the player's, a program's load, cleanup, and the old-save
  migration. The player's stores are the fallback only when the whole base
  has no free tile. One helper takes the drop point, a count and the "free"
  predicate, and returns up to `count` tiles. `nearest_free_floor`'s BFS
  over crossable tiles is not that helper, because it walks paths and
  rings do not.
- **Downed programs and gear copies on the floor** are plain data:
  `DownedProgram`'s condition is rolled once and never ticks, and
  `GearCopy` is a value. No system scans or ages them. Both are already
  `Serialize`, so `FloorThing` serialises directly.
- **"Crowded"** is the existing `drift_idle_staff` crowded set
  (`seams-base.md` ~225). Reuse it; don't define a new one.
- **Hauler pickup:** it re-derives its target every trip and lifts one
  unit, with no sweep. It may take an item from a stockpile while a
  cleanup body carries a different item there. The flow only runs one way
  (loose → stockpile → depot), so items can't bounce back and forth.
- **Keys:**
  - `g` on a downed program at `MAX_DOWNED_PROGRAMS` is refused with a
    message.
  - `t` with nothing to put down shows a message.
  - If `z`, `g` or `t` is already taken, the builder picks free
    replacements and reports them.
- **Save:**
  - The old `floor_piles` field stays deserialize-only, because older saves
    must load, and it is migrated after structures and the grid are
    restored.
  - The new fields use `#[serde(default)]`, so `SAVE_FORMAT_VERSION` does
    not move.
  - Dev-saves don't need re-capturing.

## Left to the builder

- Names of the component, enum and helpers.
- Which glyph each shipped item gets, and the default glyph for an item
  whose file omits one.
- Stockpile storage shape (Resource vs per-cell entity), as long as it is
  saved and order-stable.
- Exact render tint and examine wording.
- Where the cleanup arm's reach field comes from (reuse `crew_reach`).
