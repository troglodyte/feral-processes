# The Stack

- **The player's `Position` stays on the surface while underground.** Stack
  coordinates and facing live in `resources::Locale`; `Position` is pinned to
  the entrance tile. `Game::require_surface` guards the eleven actions that
  reach the zone map through it. The test for whether a `Position` reader
  needs the guard is not "does it act" but **"does it claim something about
  where the party is"** — a read-only screen falls into the same hole.
- **Examine names only what the surface map draws, and that rule is one
  function**: `views::drawn_on_surface_map`, read by `render/base.rs` and
  `Game::find_target_in_direction` alike. The scan is a **ray one tile wide**
  at `tuning::EXAMINE_RANGE_TILES`, and ties are a *total*
  `(step, kind, entity)` order — `min_by_key` returns the first of several
  equal minima, which is where bevy's unstable iteration order leaks in.
  Nests, surface links and zone portals draw a glyph and carry neither
  `Creature` nor `Structure`, so the ray looks through them: a known gap, in
  `TODO.md`. It answers honesty only — which *space* a tile belongs to is
  `stands_in_base_space`, under **The base**.
- **`use_symlink` is the one action that leaves the Stack instead of being
  refused by it.** It calls `clear_stack` and *then* teleports — the locale
  drops only after every check passes. Anything else that moves the player
  out goes through `clear_stack` in that order, not around it.
- **A Forgiving death is the second thing that leaves the Stack, and the only
  one that isn't an action.** `difficulty::death_handling_system` is a bevy
  system, not a `Game`, so the reset lives in `game::stack::surfaced` and
  both sides apply it. A fourth resource added to that tuple fails to compile
  at both call sites, which is why the values are returned rather than
  written.
- **Beating a stack's guardian is the third, and the only one that takes the
  way back out with it.** `Game::collapse_stack`. The trigger is the field
  `BattleState::lair`, not a positional read at teardown; it is keyed on the
  **guardian** dying (not on winning), matched against `LairFight::guardian`
  so an escort's death does not spend the lair; taming is refused at
  `battle_set_action` and `mark_lair_cleared` is written anyway; and the
  replacement link site is found **before** the old one comes down, since a
  zone with no link is a run that can never breach. The replacement stands on
  a *new tile*, so it keys a fresh `FrameSpec` with an uncleared lair — which
  is what makes a zone's Portal Fragment supply **renewable rather than
  fixed**, and is the whole reason spending fragments at a bench cannot
  strand a run. Read that before "fixing" a softlock here.
- **World generation must not draw from `resources::GameRng`.**
  `stack::generate`, `Game::spawn_surface_links` and `Game::pick_lair_species`
  each seed a local `StdRng`. A `GameRng` draw does not survive a save/load
  and shifts every later roll in the run. `FrameSpec::rng_seed` is the one
  salting scheme — don't invent a second that could collide.
- **A Stack frame is regenerated; what the party *saw* of it is saved.**
  `stack::generate` is pure in `FrameSpec`; `resources::StackMemory` holds the
  run's history. Keyed by `(link tile, depth)` and zone-local, so like
  `BuybackLedger` it must be wiped **by name** in `enter_next_zone`.
- **`view_cone` is the one walk both Stack views are built from**, and
  `visible_rows` is where sight stops. **Never at `ahead == 0`** — a cell
  cannot hide the party from their own surroundings. Both are `fn`, not
  `pub(crate)`: `game/stack_view.rs` holds them and all their consumers, so
  "one walk" is a module boundary rather than a convention.
- **A Stack cell is narrated on two axes.** `announce_sighting` fires on
  discovery (first sight, once ever, ranked cells only); `announce_passage`
  fires from `Game::arrive` on arrival, has no notion of new, and describes
  what lies ahead. So **`notability` no longer decides whether a cell can
  speak**, only whether finding it is news. Both resolve the cell through
  `ahead_target`, and whether a cell speaks is derived off `PASSAGE_SALT`,
  never `GameRng`.
- **`walkable()` and `blocks_sight()` are not complements.** A door is both,
  so "the party is inside an occluder" is reachable. Any new cell kind that
  is walkable *and* sight-blocking inherits that trap; the four Phase-3/4
  kinds deliberately don't, pinned by
  `the_new_cell_kinds_are_walkable_and_see_through`.
- **`render/stack.rs`'s `cell_mark` is exhaustive, and must stay so.** As a
  `_ => None` match a new `CellKind` shipped invisible. Exhaustive is not by
  itself enough — doors had an arm of `None` and relied on colour, which fog
  ate. Marks fade on `MARK_FOG`, not `FOG`.
- **The frame map is drawn twice and defined once.** `draw_frame_map` and
  `draw_map_inset` differ only in layout; `draw_grid`, `tile_color` and
  `cell_glyph` are shared. A third caller widens `layout`'s `fill` parameter
  and does not get its own glyph table.
- **A sealed door is `walkable()`** so the generator can see through it for
  connectivity. Whether the party may pass is decided in `Game::step` against
  `FrameMemory::opened`, nowhere else.
- **A Stack cell that can be used up needs both halves** — a `CellKind` *and*
  a `FrameMemory` record, both in `game/stack_features.rs`. Forget the record
  and the thing refills every time the party steps off and back on. `Fault`
  and `Corruption` have none because neither is *used up*.
- **An orphan's *species* is pinned to the frame seed; its *stats* are not.**
  What it is, is a property of the place and must survive a reload; what it
  is worth is a property of the moment you took it. `Game::habitat_pools` is
  the shared seam — widen it rather than copying the biome rules.
- **There is one way into a frame, `Game::enter_frame`.** The landing is a
  closure over the generated frame, because two of the three callers cannot
  name their cell until the frame exists.
- **There is one way to arrive *on a cell*, `Game::arrive`.** Corruption
  first (a property of arriving), the fault before the encounter roll. It
  deliberately does **not** call `remember_view` — each caller does that
  first. `a_jump_fires_the_arrival_tail` asserts behaviour, not that a
  function was called.
- **`Game::run_field_routine` is Stack-only for two of three effects, and
  `require_surface` is not what does it** — `Phase` and `Jump` read and write
  `Locale::Stack`'s own coordinates, so the refusal is `Game::stack_pos`
  returning `None`. `AbilityEffect::field_only` is the one predicate, and
  `use_ability`'s `unreachable!` is only unreachable because three callers
  agree with it.
- **`Trace` is a resource because `descend_to`/`ascend_to` rebuild the
  `Locale::Stack` variant.** Both frame transitions *construct* a fresh
  variant, so a field there is zeroed on every descent, which is exactly when
  the meter should be accumulating. It resets wherever the party surfaces,
  via the one value `stack::surfaced`. `Game::raise_trace` is the only thing
  that raises it and holds the `is_underground` guard for all three sources.
- **A lethal Wild Jump never writes `Locale`.** `die_in_the_rock` damages and
  stops, which is what makes "party inside rock" unreachable rather than
  merely unlikely — so neither `view_cone` consumer needs a new exception.
- **A Stack description is derived, never stored.** `descriptions.rs` reduces
  a per-`Slot` fold of `FrameSpec::salted` via Lemire's high-bit reducer,
  **never `%`**. Three things break it: `GameRng` (won't survive a reload),
  `StdRng` (sequence not stable across a `rand` upgrade), and letting a
  caller pass its own seed (two sites then drift on how they salt). A cache
  or save field for description text means something is reading run state it
  shouldn't.
- **`balance_sim` has no Stack term at all**, so the arena is the only
  instrument for a lair. `Encounter::Lair` exists because `Encounter::Stack`
  passes `allow_boss: false`.
