---
paths:
  - "**/*stack*"
  - "**/*stack*/**"
  - "**/trace*"
  - "**/descriptions*"
  - "**/frame_map*"
  - "**/floors*"
---

# Load-bearing seams: The Stack

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **The player's `Position` stays on the surface while underground.** Stack
  coordinates and facing live in `resources::Locale`; `Position` is pinned
  to the entrance tile.
- **Examine names only what the surface map draws, and that rule is one
  function**: `views::drawn_on_surface_map`, read by `render/base.rs` and
  `Game::find_target_in_direction` alike.
- **`run_symlink` is the one action that leaves the Stack instead of being
  refused by it**, and it is a field routine landing on the anchor rather
  than a key of its own.
- **A Forgiving death is the second thing that leaves the Stack, and the
  only one that isn't an action.** `difficulty::death_handling_system` is a
  bevy system, not a `Game`, so the reset lives in `game::stack::surfaced`
  and both sides apply it.
- **Beating a stack's guardian is the third, and the only one that takes the
  way back out with it.** `Game::collapse_stack`.
- **World generation must not draw from `resources::GameRng`.**
  `stack::generate`, `Game::spawn_surface_links` and
  `Game::pick_lair_species` each seed a local `StdRng`.
- **A Stack frame is regenerated; what the party *saw* of it is saved.**
  `stack::generate` is pure in `FrameSpec`; `resources::StackMemory` holds
  the run's history.
- **`view_cone` is the one walk both Stack views are built from**, and
  `visible_rows` is where sight stops.
- **A Stack cell is narrated on two axes.** `announce_sighting` fires on
  discovery (first sight, once ever, ranked cells only); `announce_passage`
  fires from `Game::arrive` on arrival, has no notion of new, and describes
  what lies ahead.
- **`walkable()` and `blocks_sight()` are not complements.** A door is both,
  so "the party is inside an occluder" is reachable.
- **`render/stack.rs`'s `cell_mark` is exhaustive, and must stay so.** As a
  `_ => None` match a new `CellKind` shipped invisible.
- **The frame map is drawn twice and defined once.** `draw_frame_map` and
  `draw_map_inset` differ only in layout; `draw_grid`, `tile_color` and
  `cell_glyph` are shared.
- **A sealed door is `walkable()`** so the generator can see through it for
  connectivity.
- **A Stack cell that can be used up needs both halves** — a `CellKind`
  *and* a `FrameMemory` record, both in `game/stack_features.rs`.
- **An orphan's *species* is pinned to the frame seed; its *stats* are
  not.** What it is, is a property of the place and must survive a reload;
  what it is worth is a property of the moment you took it.
- **There is one way into a frame, `Game::enter_frame`.** The landing is a
  closure over the generated frame, because two of the three callers cannot
  name their cell until the frame exists.
- **There is one way to arrive *on a cell*, `Game::arrive`.** Corruption
  first (a property of arriving), the fault before the encounter roll.
- **`Game::run_field_routine` is Stack-only for two of the effects it runs,
  and `require_surface` is not what does it** — `Phase` and `Jump` read and
  write `Locale::Stack`'s own coordinates, so the refusal is
  `Game::stack_pos` returning `None`.
- **`Trace` is a resource because `descend_to`/`ascend_to` rebuild the
  `Locale::Stack` variant.** Both frame transitions *construct* a fresh
  variant, so a field there is zeroed on every descent, which is exactly
  when the meter should be accumulating.
- **A lethal Wild Jump never writes `Locale`.** `die_in_the_rock` damages
  and stops, which is what makes "party inside rock" unreachable rather than
  merely unlikely — so neither `view_cone` consumer needs a new exception.
- **A Stack description is derived, never stored.** `descriptions.rs`
  reduces a per-`Slot` fold of `FrameSpec::salted` via Lemire's high-bit
  reducer, **never `%`**.
- **`balance_sim` has no Stack term at all**, so the arena is the only
  instrument for a lair.
