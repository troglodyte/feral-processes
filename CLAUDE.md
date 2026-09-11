# feral-processes

A headless Rust ECS game sim with a graphical renderer. 4-crate Cargo
workspace.

**This file is loaded into context on every turn, so it holds rules and not
arguments.** The reasoning behind each load-bearing seam — the measurement,
the history, what was tried and rejected — lives in the **memory graph** as
`seam:<slug>`. Read the matching entry there before changing a seam, and write
any new reasoning there rather than here; the `seams` skill has the two calls.

The crates:

```
crates/engine    (feral-processes-engine)   headless sim, standalone bevy_ecs
crates/app-core  (feral-processes-app-core) App/Mode input-and-flow state machine
crates/gui       (bevy + bevy_egui)         the renderer
crates/launcher  (feral-processes)          the binary
```

**Architectural rule:** the engine's `Game` struct (`crates/engine/src/lib.rs`)
is the entire public API surface the renderer talks to via app-core. The
renderer never touches the ECS `World` directly. Keep it that way. A graphical
display is required; there is no text mode and no headless play.

What enforces the rule is `Game`'s `world` field being private with no
accessor — a compiler barrier from outside the crate. **What convention alone
has to hold is the accessor never being added**: `crates/gui` now has
`bevy_ecs 0.19` in its graph via `bevy`, the same version the engine uses, so
a `pub fn world_mut()` added in a weak moment would be immediately usable by
the renderer with no new dependency and no version mismatch to give anyone
pause. The second consumer that used to hold the rule (`crates/tui`) is gone.
Relatedly, the frontend is one big system rather than idiomatic Bevy
components — `crates/gui/src/lib.rs` documents why, and the reason is this
rule.

**The drawing seam:** `crates/gui/src/paint.rs` is the only file that names a
graphics library. The ~3,000 lines in `crates/gui/src/render/` draw through
`Painter` — fifteen operations, plus local `Color`/`Rect`/`TextDims`/`TextRun`
— and know nothing about the backend. That is what made the macroquad→Bevy swap
touch five files and no drawing code. Don't reintroduce direct backend calls
in `render/`. **The panes take their origin from the caller** — a `Rect`,
not a width and a height — because the stock strip claims a row off the top
of the window. That is affordable only because each view states the origin
once: `stack::slice` for the whole corridor projection, `tile_origin_px` for
the surface map, `inset_rect` for the frame inset. A literal `0.0` in either
file draws under the strip and no test sees it. The fourteenth is `clipped`,
the only one about *not* drawing: the Stack corridor's lateral columns
overhang their pane by construction, and hand-clipping a trapezoid changes
the perspective it was drawn with. **The fifteenth is `sprite`**, the only
one that names a texture: a one-cell sprite **substitutes** for an entity's
glyph and never draws beside it, and a name the table has nothing under
returns `false` so the caller draws that glyph instead. Three things hold it
up. `assets/sprites/` is optional by construction — a missing directory, file
or name all end at the glyph, so never gate the draw or the loader on it
being non-empty. `color` is a **multiplying tint**, so art is authored
near-white and inherits `difficulty_color`, `biome_tint` and the damage
dimming for free. And `sprite` takes a **top-left** and fills its square
while `map` takes a *baseline* centred on measured ink — reading the two as
one convention is a half-cell offset that reads as a camera fault. **The trap
is overdraw**: painting the sprite over a glyph that is still there looks
perfect against opaque art and breaks the moment one has any transparency, so
the test asserts the mesh *and* the absent `@`. Sprites are 16x16 because
`map_cell`'s ladder is integer multiples of unscii's cell, and
`ImageSampler::nearest()` at load is what makes that worth anything —
bevy_egui binds the image's own sampler and Bevy's default is linear.
**The player's drawn icon is the one sprite drawn untinted, and the
player tile's fallback is three-step**: the runtime-only `"@drawn"` key,
then `player.png` under `"player"`, then the `@` glyph. It is the
exception to "art is authored near-white" because it is the one tile
that inherits none of the hues that rule protects — no species colour,
no `biome_tint`, no damage dimming — so putting the hue back reads as a
bug fix and is not one. **The player edits `ICON_GRID` (8) and the
sprite stays `ICON_SIZE` (16)**, `ICON_CELL_PIXELS` the one expression
of the ratio: each drawn cell fills a 2x2 block of the upload, and the
save string is `"v2:"` plus 64 hex digits with `v1` folded, never
dropped.

## Load-bearing seams

Facts that cost tool calls to rediscover every session. **Each is one
sentence: the rule alone.**

The trap each rule exists to close is in the **`seams` skill**
(`.claude/skills/seams/`), one reference file per subsystem — invoke it
before changing code in one of these areas. The argument behind a seam —
the measurement, the history, what was tried and rejected — is in the memory
graph as `seam:<slug>`, reached with `memory_search(…, subsystem: "seams")`
then `memory_get_entity`; read it before changing a seam itself.

**One sentence is a budget, not a style.** This file is loaded on every
turn, and it reached 151 KB by letting each seam's trap creep back in
beside its rule. A new seam is three writes — the argument to the graph, the
trap to the skill, the rule here — and the skill documents the order.

Each was verified against the source, not remembered. Verify again before
relying on one, and correct all three places if it has moved.

### The Stack

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

### The ground

- **`Game::terrain_at` is the one door onto what terrain does to you, and
  `Game::environment_biome_at` is the single definition of its gates (zone
  1, `Biome::Platform`).** Two copies of that check is how neutral zone 1
  lapses.
- **`EnvironmentEffect`'s fold is additive on every term except the ambush
  multiplier, which multiplies.** A reader that takes only the attrition
  terms compiles clean and silently drops drag and the multiplier.
- **Which `StaticEvent` is live is derived from `(seed, zone, biome,
  epoch)` and never stored.** No save field, no `SAVE_FORMAT_VERSION`
  bump, and no `resources::GameRng` draw.
- **A breach raises a tier now, and does not rebuild the world.**
  `Game::enter_next_zone` mints no new map, moves nobody and spends no
  currency — it raises `ZoneLevel`, clears `StackMemory` and
  `PopulatedChunks`, and re-stocks ground already walked at the new tier.
- **A breach's re-tier is two calls and either alone is inert** —
  `enter_next_zone` clears `PopulatedChunks` *and* calls
  `Game::clear_local_wild`, which keeps a `NestGuardian` and a `Nemesis`.
- **Where a settlement stands is derived off `(world seed, region)`, never
  stored.** `settlements::placement::settlement_at` is `rock::RockDb::
  kind_at`'s rule for the zone surface; `resources::Settlements` records
  the resolved tile and def once a town is found, never the candidate cell
  the derivation answered with.
- **A settlement is entered by walking into it, and the tile admits
  nobody.** The bump is the fourth arm of `move_player`'s ladder; it queues
  `resources::PendingVisit` and leaves the player's `Position` unchanged,
  and `Game::take_settlement_visit` is a drain, not a getter, or the screen
  it opens would reopen on the next keypress.
- **A settlement's buyback keys through a minted `"settlement/<id>"`
  string, not a widened `ShelfKey`.** `StructureId` is a bare `String`, so
  the mint plus its tile fits with no type change and no save bump.
- **A settlement's `Temperament` prices both directions off a neutral
  middle, and Mercantile is not their average.** It competes on the buy
  side and takes its margin on the sell side instead.
- **A town's opinion is written through one door, `Game::adjust_standing`,
  and the band under it is derived on every read.** `resources::Standings`
  holds the signed number; `relations::band` says what it means, and the
  door holds the clamp and the only-on-a-crossing announcement.
- **A consequence of standing is a named query on the band, never a table
  of effects** — `Standing::refuses_service`, exhaustive, `perks.rs`'s seam.
- **A town refusing service answers with a *closed* view, never `None`**,
  and the gate is applied again at `Game::commit_settlement_basket` because
  only a commit can spend.
- **`Relation::trade_credits` is a remainder, not a total**, or trade
  volume becomes a per-basket rounding rule.
- **A contract is delivered where it was signed, and
  `ActiveContract::issuer` is the whole vocabulary** — `None` is the run's
  own Broker, `Some(key)` the town that posted it.
- **A town's garrison is one term in `Game::total_raid_defense`, and the
  clamp is on the settlement half alone** — `SETTLEMENT_GARRISON_MAX`, held
  strictly below `RAID_DAMAGE`.
- **An aid radius is a fraction of `placement::REGION_TILES`, never a flat
  number** — both flat ones were dead, and a dead radius reads at the
  keyboard exactly like a weak one.
- **A gifted program's species is derived from `(world seed, region, gifts
  taken)`; choosing it spends no `GameRng` draw and adopting it spends what
  every adoption does.**
- **A relay landing starts at band 1, filters the way `move_player`'s ladder
  does, and queues the visit cue only if it lands in reach.**
- **A relay trip's tick loop breaks on a fight, and both travel keys owe
  `after_world_action`** — the charge is at most the quote, never equal to it
  on an interrupted trip.
- **The town page's aid rows are engine sentences, `AID_LINES` the census** —
  the width gate lives in gui and cannot build a `Game` to ask.
- **Every aid row is a *call* to the door that honours it, reach included** —
  `Game::town_garrisons` is the garrison's shared half, and the two verbs are
  gated on `settlement_reach` exactly as their doors are.
- **A town's board is `Game::board_defs` with four things changed** — the
  reach that gates it, the seed it draws with, the slot count
  (`Standing::job_slots`) and which tier goes first (`Specialty`, a ranking
  and never a filter).
- **A Hostile town fields a patrol, `components::TownPatrol` is the second
  tether, and `pursuit_tick` sizes its one shared field off the *maximum* of
  the two leashes.**
- **A patrol is provoked by proximity and stood down by the band re-read
  every tick, and only the provocation half carries the surface guard** —
  `Game::patrol_aggro_tick`.
- **Fielding one is a roll with a mean rather than a countdown, and its
  range is measured to the *party* where `raiding_towns` measures to the
  anchor.**
- **Killing a patrol member charges that town alone by key, never
  `credit_nearby_settlements`, and `SETTLEMENT_PATROL_KILL_STANDING` is
  bounded by what clearing one nest pays.**
- **A patrol's mark is the tile's bottom-right corner and spends none of the
  identity, danger or rarity channels**, `EntityView::patrol` carrying the
  town's name rather than a flag.
- **A patrol's tether saves by the town's tile and is resolved *after*
  `restore_settlements`**, `pending_cronjobs`' deferral.

### The base

- **A deploy is a *request*, and the Home is the only build the player's own
  hands finish.**
- **The Home is free, and the anchor lands on the tile the party founded
  from while the Home itself still stands on `BASE_EXIT_CELL`** —
  `Game::move_anchor_to` is the one writer, and founding is its only caller
  now that a breach no longer relocates the anchor.
- **A zone-portal line is ramped from the zone it was introduced in, and
  `build_cost` is `min_zone: 1`.**
- **Upgrading is a build request too, and `BuildSite::goal` is the whole of
  the difference.**
- **`Game::spawn_structure` is the one place a structure's component list is
  written**, `roster_parts`' argument on the other roster: two callers with
  nothing in common, and nothing fails to compile when a hand-written copy
  drifts — a crew-built machine missing its `MachineStatus` reads as the
  base being broken.
- **Materials are not spent until the structure is raised.**
- **Build wants are *prepended* in `schedule_base_labour`, the mirror of dig
  wants being appended** — the priority is the position in that list, since
  `truncate(staff.len())` cuts from the end.
- **An unreachable request is dropped *above* the cut in
  `schedule_base_labour`, build and dig alike, through one
  `hauling::crew_reach` field per body rather than a walk per want.**
- **A dry request is not a want, and `build_is_workable` is the one place in
  the scheduler a want is allowed to be a stock count.**
- **A builder *walks* to its materials, and that is what the dig crew does
  not do.**
- **`views::BuildOrderRow` is the one derivation of what a request looks
  like**, read by the map, the examine line and `build_order_report` alike,
  and every figure in it is a *call* — `BuildSite::required_ticks` is
  derived from the stored cost and never stored beside it.
- **A slab wide enough eats the Stack on-ramp's draw box, and the failure is
  the whole zone rather than one link.** `spawn_surface_links` shares an
  attempt budget across all three links, so an unplaceable on-ramp yields
  *zero*.
- **The map draws one space, and `Game::stands_in_base_space` is which.** A
  `Structure` or a `Tamed` program stands in base space; every other glyph —
  wild program, nest, Stack entrance, the anchor — is a zone-map fixture.
- **The "someone is on this job" mark goes on whichever end of a posting has
  a glyph to wear it**, and `wears_job_mark` / `structure_attended` are the
  two halves — exactly one per posted program at every instant.
- **A supplier that declares `power_upkeep` (an `Option<ItemId>` naming its
  fuel) does nothing at all while it is dry — grid supply and its own
  `power_regen` trickle alike — and the Home never declares it.**
- **`collect::plan_adjacent_take` is the one machine-to-machine reach**, the
  assembler's pull and a supplier's fuel walking the same four tiles.
- **A raid's flash is base-space too, and `render/base.rs` gates both draw
  sites on `base_pos`.**
- **What the base is holding is one row across the top of every screen that
  draws the world behind it.**
- **Base space carries its own seed, and it is not `WorldMap::seed()`.**
  `BaseGrid::seed` is minted once at `Game::new`; the two stay separate
  because base space and the zone surface are different subsystems, not
  because the world seed still moves — a breach stopped reseeding it.
- **A base-space cell's kind is derived, never stored**, `rock::RockDb::
  kind_at` — an FNV-1a fold of that seed and the **block** the coordinate
  falls in, reduced through `derive::index`.
- **A swing is capped at `durability / min_swings`, per kind.** The fix for
  a developed player demolishing their own base by clipping a corner —
  `swing_damage` grows all run and the rock does not.
- **A rock kind authors a brightness, never a hue or a colour.** Hue is one
  fixed signal for passability across the whole run — `biome_tint` no
  longer rotates per sector — and a rock kind may not spend it.
- **Only an *exposed* face shows its kind, and that is a display rule
  only.** `BaseGrid::is_exposed` — solid, with an **orthogonal** walkable
  neighbour — derived per lookup because cutting, and entropy re-knitting,
  both move it.
- **`resources::MiningMode` is the player's own bump and nothing else**, off
  by default and off for any save that never said otherwise.
- **`Tile::open_to_hostiles` is unreachable now, and is kept rather than
  deleted.**
- **Wild population is a property of place, and the density target is what
  "populated" means.** `Game::ensure_local_population` stocks any world
  chunk within `POPULATION_CHUNK_MARGIN` of the player's that
  `PopulatedChunks` has not marked; `maybe_spawn_wild_creature` regrows the
  local box.
- **The opening ring needs an explicit radius**, `OPENING_RING_TILES`.
- **`Stock`'s `output` is public and its `input` is private**, and that
  asymmetry is the whole of a chain's directionality.
- **A Depot's filter is the *denied* set and `Game::depot_accepts` is the one
  door**, read by the player's own put and the crew's haul alike, an absent
  `components::DepotFilter` meaning the shelf takes anything.
- **Taking and putting are one screen, one basket and one commit** —
  `Mode::Transfer`, opened with `c`.
- **The put side is one budget and the take side is per row, and the screen
  must tell "no Depot" from "a full Depot".** `App::put_available` subtracts
  the *other* rows from `basket_room` so the highlighted row can still be
  lowered and raised; `App::take_available` is that row's shelf alone.
- **On the picker screen an arrow moves stock toward the column it points
  at** — the table reads `item | you | container`, so Left takes out
  of the container and Right puts into it.
- **The picker's two figures are where the basket would leave the units**,
  `carried + amount` and `on_shelves - amount`, and they are the screen's only
  feedback on it.
- **`TransferRow::carried` is a holding and `can_put` is a permission**, and
  only `can_put` opens a row from the pack side.
- **The trade currency is offered on neither side**, its own filter rather
  than `ItemDef::banked`.
- **A modifier is four `GameKey` variants, and `App::handle_key`'s fold is
  the list of screens allowed to see one.** `ShiftLeft`/`ShiftRight` is a
  **target** (an end of the row, idempotent under key repeat);
  `CtrlLeft`/`CtrlRight` is a **step** that halves the gap to that end,
  `div_ceil` so a gap of one closes rather than stranding.
- **A machine-to-machine reach is two halves: `collect::plan_adjacent_take`
  is the walk and `collect::feeders_by_tile` is which cells it may reach.**
- **`assembler_system` sorts machines by `(x, y)` before pulling**, because
  bevy's query iteration order is not stable and two machines competing for
  one feeder would resolve differently between runs.
- **Planning is per machine, not per base.** Planning the whole base at once
  compiles just as well and lets two machines take the same units, silently
  undoing the sort.
- **`Stock` keys by `ItemId` in a `BTreeMap`, and `ItemId` derives `Ord`
  only for that** — iteration order feeds the pull phase, and a `HashMap`
  would make the save encoding differ run to run.
- **A machine's recipe is the assembled item's own `craftable.cost`**, via
  `systems::assembly_recipe`.
- **Every shipped `assembles` recipe is one ingredient, and that is a
  property of the *items*.** A second ingredient on any of the four
  intermediates silently turns its bench back into a corner puzzle.
- **A work order stores what was asked for, never how it will be done.** An
  item, a quantity and a `standing` flag — labels on the request, no plan.
- **Every unsatisfied order is worked at once, and `settle_orders` is where
  priority lives.** It accumulates the wants of every non-stalled order in
  **queue order** and dedupes by machine keeping the **first** occurrence;
  `schedule_base_labour`'s `truncate(staff.len())` does the rest, so there
  is no sort and no score.
- **A satisfied standing order is skipped, not removed** — `index += 1`, the
  branch a stalled order already takes.
- **A work order's band is an insert position, not a second sort.**
  `queue_work_order` inserts after the last order of equal-or-higher
  `OrderPriority` and nothing reads the field again.
- **`schedule_base_labour` decides the whole assignment by priority and then
  diffs it.** Filling greedily around existing postings leaves a body on a
  standing job while an order goes unworked.
- **How short of bodies the base is, is a cached figure taken *before* the
  cut.** `resources::LabourDemand` is written once a tick by
  `schedule_base_labour` — the wants it accumulated against `staff.len()` —
  and read back by `Game::labour_demand` for the work order screen's header.
- **A program's role is derived, and there is no "owned but idle" state.**
  `Game::program_role` over `ProgramRole` — disjoint and exhaustive, so a
  program you own that is not fighting beside you, not held as your weapon
  and not away on a sortie **is** base staff.
- **`accepts_a_program` is the one predicate for "a program can be posted
  here"**, and `hauling::post_reach` is the other half — the walk to it.
- **A posted program sets off from its own tile, and the player's tile is
  read nowhere in the scheduler.** `post_worker` writes no `Position` at all
  — the same omission `post_guard` makes — and `can_walk_to_post` is asked
  from the body being considered.
- **An idle program wanders the base, and laid floor is the leash.**
  `wander_step` offers one of the eight neighbours of the tile the body is
  *standing on*, or a hold, every `IDLE_STAFF_STEP_TICKS` — relative, where
  the ring it replaced was absolute.
- **`task_progress_system` and `assembler_system` both write
  `Task::progress` and are `.chain()`ed** — bevy can see the conflict but
  not the disjointness.
- **A test fixture that hand-spawns a work node needs `work_node_parts()`**,
  and one that posts a program needs `park_at_post()`.
- **`MachineStatus::Stranded` is `Unstaffed` plus the knowledge that waiting
  will not fix it.** The two systems split by writer: `haul_step_system`
  marks the *worker*, `task_progress_system` stays the only writer of a
  machine's status.
- **`set_machine_status` is the one place a stall is announced, and it logs
  only on transition.** Three callers, so "entering a state is news, staying
  in it is not" cannot lapse in one of them.
- **"Nobody is posted here" is one pass over every machine**,
  `idle_machine_system`.
- **A banked resource can never clog, so a Research Node has no "full"
  state, and a node with no project selected reads `Idle` rather than
  anything new.**
- **Research is one project at a time, and `Game::select_research` is the one
  door — every refusal before anything is filed.**
- **A research project's materials are ordinary work orders, and
  `WorkOrder::for_research` is provenance rather than a plan.**
- **Departure lives in `haul_step_system`, not the clogged branch**, because
  it has to know whether a depot exists — a base with no depot must behave
  exactly as it did before depots shipped.
- **An attached building is one the base has a reason to run**, and the
  recipe alone was never enough: an unstaffed assembler pulls nothing, so a
  Lathe nothing has been ordered from reserved a Mining Node's whole buffer
  for a machine that would never take a unit.
- **`Carrying` is the only thing hauling stores**, and the carry cap is what
  lets it be one `(item, qty)` pair.
- **Destroying a structure has two paths** — `damage_structure` and
  `remove_structure`.
- **A trader's buyback shelf is keyed by `(kind, tile)`, not by `Entity`**,
  so it outlives the building.
- **A downed program lives in three places now, and the rack is the only one
  that is not a queue.**
- **A rig holds its own tool and the player holds theirs** —
  `Game::install_rig_tool` is the one writer of `Hopper::standing_tool`, the
  strip resolves against `ToolDb`, and a rig with nothing fitted runs
  nothing.
- **The rig's tool screen is `[F]`**, because `c` already opens the transfer
  picker at a rig and `T` is one of `EASTER_EGGS.md`'s hidden keys.
- **A carrier in transit is carried, so the never-free rule and both
  destruction paths name `CarryingProgram` beside `Carrying`.**
- **Three of the five classes do something at a post, each in a different
  system.** The Leech bonus rides the **scaled** branch only, which is why
  `CycleModifiers` carries the *class* rather than a finished bonus.
- **A structure's upgrade tier is bounded twice**, `min(def.max_tier,
  zone)`, checked in that order and both before the materials check.
- **`DigSite` is the second non-`Structure` entity carrying a base-space
  `Position`**, after a posted program — so `Structure` being the space tag
  no longer answers "which space is this?" on its own.
- **A mark is one verb and the cell under it decides what it means** —
  marked solid means cut, marked `Open` means floor, and the mark outlives
  the cut.
- **A dig site's two unreachable states are not symmetrical.** `BoxedIn` is
  silent (it is the normal interior of any marked block and resolves
  itself); `NoRoute` complains **once**, latched on
  `DigSite::announced_stuck`, per `set_machine_status`'s only-on-transition
  rule.
- **Dig wants are appended last in `schedule_base_labour`, and the priority
  *is* the position in that list** — `truncate(staff.len())` cuts from the
  end, so anything inserted above them silently starves production.
- **Mining does not go through `battle::resolve_attack`**, for
  `attack_nest`'s reason: rock cannot dodge and identical swings must land
  identical damage.
- **A cost the *base* incurs is walked to and carried; a cost the *player*
  incurs is paid from their pack.** The crew's tile is fetched off a shelf
  by a body that walks there and carries it back — the same `Carrying` a
  hauler uses, over the same buffers the stock strip counts.
- **A dry floor job is not a want either — `build_wants`' deadlock rule
  crossed over**, `Game::dig_wants` asking `build_is_workable`'s question on
  the half of the one dig verb that spends anything.
- **What a program needs is a catalogue, and `assets/needs/` deleted is the
  pre-needs game.** `needs::NeedDb` is `MemoryDb`'s seam again — nine
  required fields, an absent directory loading silently empty, `iter` sorted
  by id because every caller walks it.
- **`OffShift(NeedId)` is the only thing this feature stores, and hysteresis
  is why.** In below the def's `critical`, out at its `content`, and the gap
  between them is the feature — read off the current value alone a body
  flickers on and off its post every tick at the boundary.
- **One gate decides whether a need may pull a body off a post, and failing
  it *is* acting out.** Below `critical`, something services it, not
  latched.
- **An off-shift program leaves the *posting* half of
  `schedule_base_labour`, not the drift half.** `drift_idle_staff` keeps the
  whole staff list — it is what walks the body to its amenity — while
  `record_labour_demand`, `truncate` and the standdown guard read
  `on_shift`.
- **`idled_with` is an edge, never a period** — written when a serviced need
  reaches `content`, naming everyone else in reach of that amenity.
- **`needs::strain` is a free function and `need_shift` has its own cap.**
  `party::role_of`'s reason for the first — a bevy system has no `Game`, and
  two folds would disagree about whether an unresolvable def counts, which
  is what the whole empty-catalogue property rests on.
- **The need rows share the manifest's WORK box and cost a row elsewhere.**
  A NEEDS box did not fit at 1280x720 even at two rows, so `MAX_BAND_ROWS`
  went 4 → 3 to pay for them; `MAX_NEED_ROWS` trims needs *before* the box's
  own cap, or a modded catalogue pushes the post row off the end.
- **Neither shipped amenity has an upgrade path, deliberately.** A
  `StructureTier` buys an amenity nothing — `per_tick` is not scaled by it —
  so a priced upgrade row would change no number the player could find.
- **`fray`'s two branches write different memories, and neither writes below
  `Game::base_is_established`** — `frayed_here` where the base had an answer,
  `ran_down` where it had none, and the lines stay unconditional.
- **A bad mood is an errand, and `unwound_at` is what it buys** — morale has
  no reserve to refill, so `Game::note_respites` writes a `Structure`
  fondness on a period while a disgruntled body stands in reach of an
  amenity.
- **The errand is gated on there being one, and that is what keeps
  `Game::refuses_post` reachable** — a base with no amenity keeps its
  *sulking* programs in the posting pool, while `has_downed_tools` leaves it
  unconditionally.
- **No grudge is written when a respite is stranded**, `Game::fray`'s one
  asymmetry, and `Disgruntled::stranded` latches instead.
- **`Game::is_on_shift` is the one predicate for "may be handed a job"**,
  read by `schedule_base_labour`'s filter, its free loop and its "nothing to
  do" early return alike.
- **A tantrum's non-lethal clamp is applied before `Game::apply_damage`,
  never inside it**, and it is the whole of "a tantrum never kills."
- **`Game::run_tantrums` sits between `update_disgruntled` and
  `admit_the_badly_hurt`**, and that ordering is what gets the Repair Bay
  with no new code.
- **`Grievance` is appended to, never inserted into** — `Ord` is the ladder,
  the variant name is the save, and `has_downed_tools` reads `>=`.
- **`EffectKind::Brawl` draws identically to `Hit` and exists only to carry
  sound**, at most one cue a frame.
- **A Forgiving death benches a program and `Game::bench_or_dissolve` is the
  one door**, `dissolve_tamed_program`'s own argument one level up: the
  `DifficultyMode` branch is written once, not at each of the two death
  sites (`end_battle`'s dead-party loop and the raid defender).
- **A Bay's field is `recovery:`, because `structures::RepairDef` was
  already taken** — that one restores structure `Durability` per tier.
- **`components::Downed` has two writers and they mean the same thing by
  it** — `bench_or_dissolve` for a Forgiving death and
  `admit_the_badly_hurt` below `BAY_ADMISSION_HP_FRACTION`; released at full
  Integrity, and the occupancy mark reads the same marker so it cannot
  disagree with the heal.
- **`Game::run_repair_bays` is a `Game` method, `run_dig_crew`'s reason**:
  the line names the program through `creature_label` and heals through
  `restore_hp`, and a bevy system would be a second copy of the first.
- **A downed program walks itself, and the `Downed` arm of
  `drift_idle_staff` sits above the `OffShift` arm** — recovery outranks an
  amenity — **gated on laid floor**, which is what keeps `entry_tile` the
  one arrival path for a program downed in the Stack.
- **`Downed` joins the `on_shift` filter without the `Carrying` escape**,
  and is freed in the diff **unconditionally, ahead of every keep rule**.
- **A structure remembers how well it was built, and absent means neutral.**
  `components::BuildQuality`, written only by `Game::spawn_structure` and
  `raise_one_tick`'s upgrade arm, restored from the save and never
  re-derived from the def.
- **The build term goes inside `work_ticks_at_speed`, not at its callers** —
  `class_scale`'s own argument — and the picker's preview is a *call* into
  that function through `Game::build_candidates` rather than a percentage.
- **`Potential`'s two build rolls are deliberately independent of the four
  combat rolls, and `quality_percent` still folds only the four.**

### Instrumentation

- **Every production seam reports through one door, `base_ledger::emit`, and
  the counter is a reader of the event rather than a sibling of it.**
- **The fold is unconditional and the record is built inside a closure, and
  nothing in the compiler holds that at a bevy seam.**
- **A source is recorded and never folded; a sink is both** — `Acquire`
  against `Consume`, and `grant_loot`'s eighteen callers are what make
  provenance one parameter.
- **A stall hangs on `set_machine_status`, which already speaks only on
  transition**, and `StallSite` is plain borrows because `power_grid_system`
  is exclusive.
- **`Record::BaseSnapshot` is the denominator**, once per
  `base_ledger::BUCKET_TICKS`, with its counts built inside the closure.
- **A haul is keyed to the post, not the worker, and a `Tend` writes
  nothing.**
- **The base output page is one derivation, `Game::base_output_report`, and
  its MINED/COMPILED split follows recorded provenance rather than the
  structure defs.**
- **`BASE_OUTPUT_MAX_ROWS` is a layout constraint** — the page has no scroll.

### Sorties

- **`ProgramRole` has a fourth variant, and a sortie's five consequences are
  omissions rather than checks.** `Sortie` sits **between `InParty` and
  `Staff`**, keeping `Staff` as what is left over.
- **A sortie battle is spawn, fight and despawn inside a single call, and
  that is the whole feature's load-bearing decision.**
- **The fights are real by construction, and the trained policy is
  deliberately not used.** `resolve_and_apply_attack` and `use_ability` are
  both `BattleState`-free, so the ladder, the bands, mitigation, affinity,
  Power and cooldowns are the ones a fight in front of the player uses.
- **A Relay is identified by `StructureDef::dispatches_sorties`, not by its
  id.** The research gate really is pure data, but `has_relay` is not —
  naming `"relay"` in Rust puts content in the engine and makes a mod's
  second dispatch structure impossible.
- **The board is derived and the whole record travels.**
- **`sortie_duration` reads the risk *offset* and has no term for the
  squad.** Against the absolute band every trip in a deep sector grows for
  no reason the player could name; with a strength term the feature becomes
  a throughput multiplier that scales with itself.
- **Membership rides `CreatureSave::sortie_index`; `SortieSave` carries no
  member list.** `party_slot`'s precedent — entity ids are not stable across
  a save.
- **Every dispatch refusal lands before anything is spent**,
  `commit_caravan_basket`'s rule, asserted **per refusal** — a single test
  over one of nine passes against eight paths that never spend anyway.
- **A squad's departure is a cue the engine queues and forgets, and it is
  base space's.** `dispatch_sortie` and `return_sortie` each queue one
  `resources::TransitCue` per body — a glyph and the cells it walks —
  drained by `Game::take_transits`, `take_effects`' counterpart.
- **`award_companion_xp` and `roll_work_resource_drop` are extractions, not
  copies.** The first holds the growth roll, the cap, the XP buff, the tally
  and the routine unlocks; the second holds a `Perk::Teardown` term added to
  the roll rather than drawn for.
- **A caravan route is one record with a `standing` flag, and a one-off is
  the flag turned off** — `routes::Route`, `WorkOrder`'s shape.
- **`Game::sever_route` clears `standing` and nothing else**, so the trip in
  flight arrives, sells and pays, and there is no refund path.
- **A route and a squad leave through the same door**, `Game::dispatch_reach`
  → `DispatchReach`, both gated on `StructureDef::dispatches_sorties`.
- **`Game::route_quote` is the one derivation the cargo picker's preview and
  the sale at the far end share**, reached from the screen through
  `route_manifest_quote`.
- **Route predation is a named query plus pure geometry** —
  `Standing::preys_on_routes` and `routes::settlements_near_route`, which
  measures to the **segment** anchor→destination and not to either end.
- **Predation is the only thing in `Game::run_routes` that may draw
  `GameRng`**, and a test asserts the tick draws nothing when nothing preys.

### Combat, progression and balance

- **`Game::apply_damage` (`game/combat_damage.rs`) is the only code path
  that *damages* a creature**, every rung of the fumble ladder included.
- **`PowerReserve`'s float is private, and the clamp is the type's.** Seven
  operations, matching the call sites exactly — an eighth is a signal to
  re-read the call site, not to widen the type.
- **`ability_unavailable` is the one gate, `spend_power` the one charge**,
  both priced through `abilities::routine_power_cost` so a refusal and a
  charge cannot quote different numbers.
- **Every routine that can be *run* is priced in Power; only a passive is
  exempt**, because a passive is never invoked.
- **A field buff's lifetime is decided by its kind *and* its source**, and
  `ActiveFieldBuff::runs_until_rest` is the one predicate.
- **`Trickle` is the one restore kind that does not scale with its
  invoker.** `Regen`'s ceiling is `max_hp` and grows with level; Power's is
  a fixed `POWER_MAX` forever, so a scaled `power: 1` is 7 a turn at the
  level cap and the authored number stops meaning anything.
- **`balance_sim` gates none of the Power economy** — it models no
  abilities, so the 66 costs, the multiplier and `trickle_charge`'s retune
  are all ungated.
- **A player's class grants affinities and nothing else, and
  `ability_affinity`'s player arm is where it lands.**
- **Every difficulty curve in the game is linear.** A geometric enemy curve
  racing a linear player curve outruns it wherever you put the coefficients.
- **One draw, four bands: `battle::resolve_attack` is how every
  creature-versus-creature attack resolves.**
- **Flat Accuracy has one door per axis, and the two axes are not the same
  one.**
- **Mitigation is percentage points, and `Game::effective_mitigation` is the
  one door.** It caps at `MAX_MITIGATION_PERCENT` itself, so no reader has
  to.
- **A kill's XP is priced by challenge, sharing its thresholds with the con
  colour.** `progression::kill_xp`, clamped to `XP_CHALLENGE_FLOOR`..`CEIL`.
- **Levels come at half the count and twice the size, and that is
  power-neutral by construction.** Every per-level constant carries `K = 2`,
  every levels-per constant its reciprocal, and `XP_PER_LEVEL_STEP` carries
  `K^2`.
- **The ring buys room; the fights buy the points.** A Privilege Ring (a
  lair guardian's drop, and nothing else's) opens a Kernel Ring on one
  companion, and `open_kernel_ring` grants no stats, level or XP.
- **A Kernel Ring buys talent tiers, not levels.** `talent_points`' `earned`
  is `min(level - TALENT_START_LEVEL, rings * LEVELS_PER_RING)` — both gates
  live, `saturating_sub` because a companion below the start level is the
  common case.
- **Talent points are derived, never stored.** Level minus
  `TALENT_START_LEVEL`, minus the length of `components::Talents`; no count
  on the component, none in the save.
- **A `Stat` talent bakes into `Stats` at purchase and load must not
  re-apply it** — `CreatureSave` already writes the raised numbers, so
  `Talents` is a receipt exactly as `Refactors` is.
- **A stat a purchase baked in needs a receipt, and `components::BoughtStats`
  is it** — `Perk::Buffer` and `TalentNode::Stat` read the value at purchase
  and floor at a whole point, so a respec cannot invert them, and
  `ever_bought` is the half the wipe must not reset.
- **Fusion keeps the dominant parent's ring and talents**, and
  `fuse_companions` is the door that silently drops a new component: it
  hand-writes its own list, so nothing fails to compile and the symptom
  reads as fusion being bad.
- **`Experience::xp_to_next` is derived on load and never read back from the
  save**; both load paths call `xp_for_level`.
- **Distance from home is a difficulty axis again, capped at one zone
  step.** `Game::field_stat_mult` ramps from the opening ring's edge to
  exactly the next zone's doorstep, computed by the caller and never inside
  the spawner.
- **A basic attack is an `AbilityDef`, and combat names `MoveDef` nowhere.**
  `species::basic_attack_ability` is the one conversion; `moves:` stays the
  authored shape so no species file or mod needed editing, and
  `SpeciesDef::basic_attacks()` is what the readers take.
- **`field_only` means never-in-battle and `field_runnable` means offered on
  the map, and a priced ally-facing `Heal` is the one effect that is both.**
- **Every routine that moves Integrity rolls a band, and the census is what
  keeps it that way.** `spread` on `Damage`/`Drain`/`Heal`, rolled through
  `battle::DamageRange` — one draw whatever the width, so authoring a spread
  cannot shift a seeded stream.
- **`Game::choose_wild_action` is the one place a wild program's swing is
  decided** — move and target as a single joint choice.
- **Three policy features are pinned to zero in the shipped weights**, and
  that is a design boundary.
- **`is_boss` marks an *apex* species — always a boss, never engine-scaled —
  while any species can be *rolled* into one** and takes `BOSS_STAT_MULT`
  instead.
- **A species' danger band is derived and gates where it may spawn.**
- **Which side of the ground a boss dies on decides what it pays**, and
  underground is the game's **only** source of Portal Fragments.
- **Trace's group-size lever is a `spawn_pack` parameter, never a resource
  read inside it** — surface spawns keep rolling while the party is
  underground.
- **`Game::adopt_program` is the one way a program joins the roster without
  being beaten in a fight.** Two callers with opposite premises agree on
  what *becoming* a companion means.
- **There are four doors into the roster and `Game::roster_parts()` is the
  only barrier** — `grant_starting_program`, a capture, `adopt_program`, and
  `fuse_companions`, which assembles its own component list.
- **Destroying a tamed program has two paths.** `dissolve_tamed_program`
  handles four cases; `fuse_companions` does its own `retain`/`despawn` and
  skips the detachment logging.
- **No stats operation may run while a gear bonus is sitting in `Stats`.**
  Three operations would scale or bank it, welding the difference
  permanently into base stats.
- **The wielded program's bonus is computed live**, so destroying the
  program ends the wield by omission — the regression to head off is a later
  "fix" adding an explicit clear to both destruction paths.
- **The wielded program's proc runs as the *program*, not the player**, so
  which program you wield is what the feature is worth.
- **A fight is bounded by bodies, not just by groups.** `MAX_PACK_BODIES` is
  the ceiling on the whole pack, trimmed off the largest group each pass in
  `group_pack` — the two ceilings before it bounded a fight per group and
  per group count and never their **product**.
- **`start_battle` is the only path that caps a pack; `begin_battle` opens
  one.** The split exists for `arena`, which authors its own composition.
- **There are two battle rosters, and which one a caller wants depends on
  whether it *draws* or *acts*.** `battle_view` is live truth;
  `battle_view_at(revealed)` replays `BattleTimeline`.
- **A won fight says so, and it is the only ending that needed telling.**
  `settle_rewards` heads the results with "You won!", read off
  `BattleState::groups` being empty — telemetry's own definition.
- **A finished fight keeps the battle screen; it does not hand off to a
  summary page.** It reads: the final round's blows, the outcome, the
  salvage, the XP.
- **A battle does not end when the player's HP hits zero**, and three things
  heal them before anyone outside can look.
- **A fight's rewards are granted per kill and announced once.** Moving the
  *award* to the flush is the change to refuse — a level-up full-heals
  inside `add_xp`.
- **`retain_outcomes_since_battle` runs when the player *leaves* the results
  screen, not when the fight ends.** `Game::prune_battle_narration` is the
  door and `App::leave_battle_result` the one caller; run inside
  `end_battle` it deleted the decisive round before anything could reveal
  it.
- **There is one way into a staged arena fight, `arena::stage`**, and one
  reader of what one cost, `arena::Watch`.
- **An arena session touches no disk, and all three of those are omissions**
  — save, profile, run history — each with its own test asserting on the
  *file*.
- **Battle telemetry is the fourth thing an arena session touches, and it is
  allowed to write.** `flush_battle_telemetry` sits **above** `after_tick`'s
  `in_arena()` early return.
- **`nest_aggro_tick` is the first code to call `start_battle` from inside
  `tick_inner`**, which is why `rest`'s tick loop needed a battle check.
- **`nest_aggro_tick` is a reader of the player's `Position` and needs the
  underground guard** even though it never went through `require_surface`.
- **Resting is priced by locale, never gated by it.** Free inside base
  space, one unit of an `ItemDef::enables_rest` item anywhere else — the
  open grid and the Stack alike — and **no rest advances the clock**, which
  is what makes the free half safe.
- **A charged rest rolls `REST_AMBUSH_CHANCE` for an interrupt, and the roll
  rides the branch that takes the charge** — so base space is safe by
  placement, not by a locale check, and there is no refund.
- **A rest repairs the programs standing with the player and nobody else** —
  `InParty` and `Wielded` yes, `Sortie` and `Staff` no, exhaustively matched
  on `ProgramRole` so a fifth role fails to compile rather than defaulting.
- **`power_regen_system` needs that same guard**, and is the third in the
  family.
- **`Pursuing` must only ever be inserted alongside `NestGuardian`** — an
  untethered `Pursuing` has no leash and is never cleared.
- **`walkable()` alone does not decide where a `Pursuing` guardian may
  step** — `pursuit_field` excludes `Biome::Platform` separately.
- **There is one Dijkstra walk on the surface, and the step rule is a
  *cost function*, not a predicate** — `walk_field`, with `pursuit_field` a
  one-line wrapper and every surface and base-space caller answering
  `.then_some(1)`.
- **A `NestGuardian`'s tether refuses a step only when it both leaves
  `NEST_TETHER_RADIUS` and fails to close on the nest.** The simpler check
  froze a displaced guardian for the rest of the run.
- **`BattleState::planned` indexes `Party` positionally.** Nothing may leave
  `Party` mid-battle; deferred removal is why `end_battle` exists.
- **An initiative order names the party by slot and the wild side by
  identity**, and that asymmetry is the point.
- **A profile pays at `Game::new` and never at `Game::load`, and the
  enforcement is an omission.** `install_profile` says what has been earned
  and both paths call it; `grant_profile_rewards` pays, and only the
  new-game path calls it.
- **`resources::RunFeats` is a per-tick drain queue and is not saved**, with
  two fields and two drainers, one each.
- **`ActiveContract` stores the whole resolved `ContractDef`, not an id**,
  so a contract file edited or deleted mid-run cannot strand or rewrite one
  already accepted.
- **Contracts deliberately amend "progression is earned by fighting."** XP
  is a legal reward on *any* objective; anyone "restoring" the old invariant
  by gating XP behind combat is undoing the feature.
- **`Game::level_cap` is the only ceiling in the game and it takes no
  entity** — player and every companion stop at the same zone-derived
  number, `max(ZONE_LEVEL_CAP_FLOOR, 1 + ZONE_LEVEL_CAP_STEP * (zone - 1))`.
- **The cap's constants are fitted against `balance_sim` and the lower bound
  is a correctness bound.** The cap must sit at or above the *geared* clear
  requirement — below it a fully-equipped party cannot clear the zone at any
  level it may reach, which is a dead run, not difficulty.
- **Two renames are load-bearing and neither would fail to compile.**
- **XP at the cap is banked, not discarded, and banking and taxing share the
  one accumulator.** `add_xp` accumulates into `Experience::xp` and reports
  `LevelGain::overflow`, staying pure — it reports, the caller spends.

### Tactical battles

- **A battle map's coordinates live in `TacticalBattle`; `Position` is never
  written** — the third space after the Stack's `Locale` and base space's
  own, and `tactical/` not importing `Position` is the whole enforcement.
- **A body is a wall in `reach::movement_field`, and the allowance is both
  the budget and `walk_field`'s search box** — safe only because no step
  costs less than one, which is what makes `TACTICAL_MOVE_MIN`/`MAX`
  correctness bounds.
- **A fight ends through `Game::finish_fight`, and a `FightVerdict` is what
  each model answers it with** — `won` is the roster *emptied*, never
  "nothing is alive".
- **A fight's payout is reached through `Game::fight_rewards_mut`, and what
  a hostile's death pays is `Game::finish_hostile`** — both model-blind, and
  `finish_member` keeps only what is group-shaped.
- **A tactical fight's initiative is rolled once and kept in step by
  deletion, and the cursor names a body rather than a position.**
- **A step off the board edge is a departure and not a refusal, and the
  player's own is the jack-out.**
- **A routine's `shape:` and `range:` are read in tactical fights alone, and
  `AbilityDef::tactical_shape`/`tactical_range` is the one place an authored
  figure and the one derived from `AbilityTarget` are reconciled.**
- **`use_ability` is the door the two combat models share; each converts its
  own aim, and full friendly fire is `reach::recipients` never reading
  `Hostile`.**
- **`Game::decompile_body` is the capture, and taking the captured body out
  of the fight is each model's own half.**
- **A routine's effect is shared; its refusals are not** —
  `Game::run_tactical_routine`, taking a def rather than an index, and
  `cooldown_floor` the whole of the difference between the two doors.
- **A hostile decides what it will do before it decides where to stand, and
  the closing term is a shortfall to the *band*, never a distance to the
  target.**
- **One draw an AI turn, spent on the cell, and none at temperature zero** —
  the aim and the swing target are argmaxes, and the candidates are sorted
  before they are scored.
- **A hostile's walk is a run of real `Game::tactical_step`s spent one cell a
  beat, and `tactical_ai_turn` is that beat loop rather than a second
  spelling of a turn.**
- **`TacticalBattle::walk` is an `Option` because `None` is "has not chosen
  yet" and `Some(vec![])` is "has arrived"** — read as one, a spent walk is
  re-planned every beat and the draw above becomes one a cell.
- **The pacing carry is in seconds because a battle map has two rates**, a
  cell of a walk against `TACTICAL_STEPS_PER_SECOND` and everything else
  against `TACTICAL_TURNS_PER_SECOND`, derived off `Game::tactical_walking`
  rather than held beside the carry.
- **`Game::start_battle` is where the model is chosen, by inspecting the
  pack** — the pursuit path cannot know whether it is a guardian or a
  patrol, and the arena never passes through it.
- **The arena is the second chooser, and `arena::stage` takes the model its
  *caller* can drive** rather than the one the file asks for.
- **A headless tactical rep drives both sides through `tactical_drive_turn`,
  and a party body swings without invoking** — `PartyPlan::AllAttack`'s
  parity, not a policy invented for the tester.
- **An AI turn hands the turn on once, because the action already did it.**
- **A tactical fight is drawn in the map pane, and its turn strip takes the
  compass block's slot** — a block inside the pane, never a border strip.
- **The tactical modes are deliberately not `is_battle`**, which gates the
  reveal and routes `Fx`; `App::advance_tactical` paces the wild side
  against `dt` instead.
- **A turn ends in one place, `Game::hand_on_turn`, and it hands on only if
  the body that acted is still the one acting** — a body killed by its own
  fumble or its own blast has already left the order, and `remove` handed
  the turn on as it went.
- **A round on a battle map spends the upkeep an abstract round spends, in
  that order** — `tick_combatant_upkeep`, then the reap, then the tick,
  because the upkeep can kill and `death_handling_system` rides the tick.
- **The order wraps in two places**, `end_turn` and `TacticalBattle::
  remove`, so `hand_on_turn` compares against the round its caller read
  before it acted, and a fight that ends mid-round is `settle_tactical`'s
  tick.
- **The results page has two producers, `Game::closing_rows`, and one row
  builder per half** — `planned` is the only field of fourteen the two
  models disagree about.
- **A capture is aimed at something hostile, refused at the player's door**,
  where a swing at your own is friendly fire and stays legal.
- **A cloak is a targeting rule: the filter sits at the five doors that
  *name* a body and at none of the doors that *resolve* against one**, and
  `Game::break_cloak` is the one door an action removes it through.
- **A brace on a board is `Game::begin_defend` with only the mitigation
  crossing over, and it is worth what the turn order says it is worth** —
  `DEFEND_AGGRO_WEIGHT` weights a slot a battle map has none of, and the
  round-cadence buff leaves a body on the last rung bracing against nobody.

### Items, gear and economy

- **An item's price is bounded twice, and the second bound is the
  non-obvious one.** A craftable worth more than its ingredients is an
  infinite Credit loop; a `work.produces` structure makes its item out of
  *nothing* on a timer, so that item's value is really a Credit-per-tick
  rate the recipe ceiling cannot see.
- **A zone's material is a content decision, and two censuses are what hold
  it.** Nothing in `ItemDef` says Cache Grain is what zone 2 pays you, so
  `ZONE_MATERIALS` in `tests/assets.rs` plus
  `every_zone_gated_gear_recipe_asks_for_a_zone_material` and
  `every_upgrade_path_asks_for_a_zone_material` are the whole rule.
- **A research node's material bill may only name what that node's own
  prerequisites can make**, `min_zone` granting nothing and `assembles`
  being no source.
- **A research bill is paid from the pack, topped up off the adjacent
  shelves**, and the whole bill is refused before a unit moves.
- **A weapon's reach lives on `ItemDef` and is authored as an enemy-facing
  plural `AbilityTarget`**, so it is off `copy_bonus`'s four scaling axes by
  construction and each combat model converts it with the converter it
  already has.
- **A carried copy of gear is one value, `items::GearCopy`**, and
  `Inventory` is by definition the *plain-copy* store.
- **`Game::copy_bonus` is the one expression for what gear is worth, and the
  order of its axes is load-bearing**: `scaled_for_level`, quality,
  `fused_for_tier`, `for_rarity`, over a base the affix has already been
  added to.
- **A copy's quality is a fourth axis and an integer.** `GearCopy` is the
  `GearCopies` ledger's key and `EquippedItem` holds the same key, so an
  `f32` takes `Eq` with it.
- **`Game::roll_quality` is the one formula and the one clamp**, and it sits
  beside `roll_gear_rarity` because two files roll the same axis: a drop
  passes the flat `QUALITY_DROP_BASE`, crafting a floor it builds.
- **`CraftOrder` is a struct at one implementor and the second is named**, a
  base-roster program compiling at a bench.
- **The careful surcharge is applied in `craft_cost`, and all three price
  questions take the flag.** Discount **then** surcharge, rounded up, or a
  fully perked recipe with every line floored at 1 is careful for free.
- **A compile rolls per unit, and a copy at exactly spec still stacks.** A
  batch is a spread to compare, not N of one thing; a copy that rolls
  `QUALITY_DEFAULT` is plain and lands in `Inventory`, so a test counting a
  batch must read **both** stores.
- **The swap row's stat column is a tag, not part of its head.**
  `wrapped_row_lines` never breaks the head, and the quality figure's seven
  cells put the joined form 35.6px past a 1243.2px popup body, lost in
  silence.
- **The category tag is a column on the row, not a substring of it.**
  `Row::Item::tag` carries the `WEP`/`ARM`/`MOD` token *and the lead in
  front of it*, so `draw_row` lays a row out as three `ui_runs` pieces and
  no row moves.
- **`Game::copy_power` is the one door to a gear rating, and every term in
  it is a *call*.** `Stats::power` for attack and mitigation,
  `battle::hit_chance` for accuracy and evasion — a probability is not a
  quantity and is priced as the fraction it moves the throughput it acts on,
  never summed into the total.
- **`PowerCell` has three cells and three meanings.** `Rated(n)` is a
  rating, `Unrated` is an em dash (*no answer*, not a bad answer), `Blank`
  is a row that is not an item.
- **The caravan is one basket and `Game::commit_caravan_basket` is the one
  commit door.** Every refusal lands before anything is spent, and **sells
  land before buys** so a basket can be funded by its own sales.
- **`Game::settle_basket` is the commit core both the caravan and a
  settlement call, and the currency charge deliberately stays out of
  it** — each vendor's own closure charges its own currency, held to the
  quoted cost by a test rather than the compiler.
- **The wagon's grouping lives in `caravan_view`, never in
  `caravan_shelf`.** The shelf is a round-robin whose leading slot rotates
  per visit; sorting it would make that unobservable and open every wagon
  with a weapon.
- **A worn item and a candidate are scaled at two different levels, and that
  is the point.** Gear locks in `EquippedItem::level`; collapsing the two
  hides the case the screen exists for.
- **A copy's name is built in exactly one place, `Game::copy_name`.**
  Building a name in a renderer is what lets a drop line and the next screen
  disagree about what you picked up.
- **An item's extra effects are three lengths of one derivation.**
  `item_blurb` is the crafting menu's two-word gloss, `Game::item_effects`
  the listing screens' one line per effect, `item_grant` the describe page's
  full prose — and the middle one *calls* the last rather than re-reading
  `grants`.
- **The gear inspect page is one derivation, `Game::gear_detail`, opened
  with `[I]` from every list that names gear.**
- **An affix is data and its absence is supported.** `Game::roll_affix`
  spends **no** RNG draw on an empty pool.
- **Rarity is one ladder for programs and gear**,
  `spawning::rarity_for_roll`.
- **Gear fusion has two records of the same tier, and only one is clamped.**
  `GearCopies` is the ledger and is clamped on load through
  `GearCopies::add`; `EquippedItem::fusion_tier` is the *receipt* for a
  bonus already spent, so lowering it makes an unequip subtract less than
  the equip added.
- **A trader's shelf row is `(GearCopy, qty)`, and the key is not
  decoration** — keyed on the item alone it hands back an ordinary copy for
  a rare one.
- **`render/mod.rs::fusion_color` and `popup.rs::fusion_row` are the one
  colour rule for anything fused.** Two screens deliberately opt out,
  because a second meaning on the same axis makes both unreadable.
- **Hand-compiling is priced at `Game::hand_craft_ticks`, the cycle of the
  machine that exists to do the job times a constant, and `Game::craft` is
  that loop drained to completion.**
- **That constant and `COMPILE_TICKS_PER_SECOND` are read together**, because
  the tick price divided by the bar's rate is the wall-clock wait.
- **Installing from a disk spends the disk last and refunds nothing on the
  way out; creation is the second door into a slot and spends no item at
  all**, `abilities::install_starter`.
- **A hand-compile's ticks drain Power like any others, and a batch
  projected to leave less than `HAND_CRAFT_POWER_FLOOR` is refused whole
  rather than shortened** — `max_craftable` carries the same ceiling.
- **`DownedPrograms` is a third player store beside `Inventory` and
  `GearCopies`, and it is not `Inventory`.** `Inventory`'s `count`/`take`
  read the *first* matching row, which is what lets recipes, `Stock`,
  hauling and banking treat it as a plain-copy store with no instance rule;
  a downed program's species, level, rarity, boss flag and condition make
  it instanced, so it needed the third store rather than a new rule bent
  into the first one.
- **`Game::extract_program` is the one door a downed program is spent
  through, and every refusal lands before anything is spent** — asserted
  per refusal, since a single test over one path passes against the others.
- **`Game::extraction_yield` is the one derivation of what a tool draws out
  of a downed program, shared by the act and the screen's preview**, so a
  quoted figure and a granted figure cannot differ.
- **The yield is a deterministic weighted apportionment, not an RNG draw**
  — `apportion`'s largest-remainder split, never a per-unit sample — which
  is what makes the preview able to equal the grant at all, and spends no
  `GameRng`, so extraction cannot shift the seeded stream.
- **The starter tool's knowledge is derived, never stored** — `knows_tool`
  answers true for `tuning::STARTER_TOOL_ID` and `tool_rows` unions the same
  helper, which is what makes pulling it recoverable and repairs saves
  already written.
- **A synthetic item id needs a minted `ItemDef` behind it**,
  `ItemDb::synthesise_tool_carriers` beside `synthesise_etched_disks` —
  `item_name`'s raw-id fallback means a missing one is silent.
- **A tool carrier is sold everywhere and bought nowhere**, filtered through
  `ItemId::tool_id` in `caravan::stock_pool` and `ItemDb::creation_shelf`.
- **`extraction_yield` and `extraction_ticks` read the bench tier
  themselves; there is no `structure_tier` parameter.** Both have two
  callers — the screen's preview and the act — and an argument is the crack
  a quoted figure and a granted one would differ through.
- **A standing extraction bench buys time; upgrading it buys materials.**
  The yield term is `bench_tier - 1` (a never-upgraded structure reads as
  tier 1, so a `tier` term would pay a full `TOOL_TIER_SCALE_STEP` for
  merely having built a Compiler); the tick term is the full tier, as a
  divisor floored at one. Neither is a gate — extraction works in the
  field, at the base and in the Stack alike.
- **`Game::take_routine` is the one place a routine comes off a program**,
  shared by `extract_routine` (a tamed program) and a `Routines`-category
  tool through `extract_program` (a downed one). The effect is shared; the
  refusals are not — the tamed door needs a bench and an ownership check,
  the tool door needs neither. The exclusive branch popping the disk
  instead of teaching is what keeps exactly one copy in a run.
- **A downed program's routine pool is derived from species and level**, so
  it cannot offer what that individual was actually carrying, and no
  shipped species declares an exclusive routine — the Reader's exclusive
  branch is implemented and unreachable in shipped content. Not dead code.
- **A sortie banks its downed programs and delivers them in
  `return_sortie`**; an off-screen battle never writes the player's store.
  The delivery loop stops at the first refusal, because once the store is
  full it stays full and `message_history` condenses repeats — a log-entry
  count cannot tell "said once" from "said eight times".

### What a program remembers

- **`Game::remember` is the one door a memory is written through**, and the
  four triggers are callers of it, not writers beside it.
- **Intensity is derived from `GameClock` on every read, never stored or
  ticked.** The decay is a magnitude scale and **never a sign flip**, or a
  grudge would decay into a fondness.
- **An empty catalogue is a supported install, and the property is held at
  both ends.** `remember` resolves the def *before* touching the store, and
  every reader skips what it cannot resolve — so deleting `assets/memories/`
  restores the pre-memory game rather than breaking one.
- **`MemorySubject::BaseTile` names the space, and that is why it is not
  called `Place`.** `note_strandings` writes the worker's **own `Position`**
  — base space for a posted program — and not its post, because a memory
  keyed to the machine's tile could never be read by `drift_idle_staff`.
- **The first of two hooks is `drift_idle_staff`'s last rejection, and it is
  not a score.** `opinion_of(worker, BaseTile) <
  MEMORY_AVOIDANCE_THRESHOLD`, beside the four tiles it already declines — a
  rejected candidate leaves the body standing where it was, so this opens no
  failure mode and needs no fallback.
- **The second hook is morale, and it is one addend in one formula.**
  `CycleModifiers::morale` into `systems::mining_success_chance`, priced and
  capped by `morale_shift`.
- **`memories::sum_intensity` is the fold, and `Game::morale` is a caller.**
  `party::role_of`'s reason: `task_progress_system` has no `Game` to ask,
  and two folds would eventually disagree about whether an unresolvable def
  counts — which is the property the whole empty-catalogue guarantee rests
  on.
- **A work memory is either an edge or a stretch, and `Game::note_postings`
  is the stretch half.**
- **A `Structure` memory names the kind, not the entity**, which is what
  lets it be written on the branch about to despawn the machine and what
  makes a rebuilt Lathe the same Lathe.
- **`MEMORY_TRIGGERS` in `tests/assets.rs` is the pairing census**, and a
  def shipped without a row in it fails the build.
- **The memories page is one derivation and has no scroll.** `R` from the
  roster — not `M`, which has been the manifest since long before this — and
  every figure comes from `Game::memory_report` / `Game::morale`.

### Notifications

- **A notification is a table plus a queue plus one equality, and it is not
  content.** `crates/engine/src/notifications.rs` is the whole feature;
  there is no `assets/notifications/`.
- **`def` and both censuses are exhaustive matches**, `cell_mark`'s rule —
  a lookup with a fallback ships a new variant blank.
- **`NotificationKind::latch_key`'s strings are `profile.ron`'s format and
  the variant names are not**, which is why `seen_notifications` is a
  `Vec<String>`: a retired key must be inert, not a parse error that costs
  the player their achievements.
- **Two repeat policies, and the third was rejected on its name**: once per
  *run* would latch on the session-only queue and so mean once per
  *session*.
- **Achievements are a second *source*, not a second door** — built from the
  achievement def's own `name`/`description`, so a per-achievement copy of
  that prose is the copy that drifts.
- **The timing rule is one equality**, `show_next_notification` returning
  unless `mode == Mode::Playing` — every picker, fight and text entry falls
  out of it, where a list of safe modes is `is_battle`'s history.
- **The low-Power notice is a state read once a tick, at
  `LOW_POWER_ATTACK_THRESHOLD` rather than a fraction of its own**, because
  the drain that carries most runs across is a bevy system with no `Game` to
  notify from.
- **`DownedProgram` is gated on `StructureDef::recovery` standing, not on
  the Bay's id**, and fires from `bench_or_dissolve`'s Forgiving arm.
- **The screen has no scroll and draws no popup**, so height is a layout
  constraint (`the_tallest_shipped_notification_fits_its_screen`, verified
  by mutation) and it belongs in `needs_status_banner` and `ALL_MODES`.

### Species and data

- **A species' class is derived and has exactly one derivation**,
  `SpeciesDef::affinity_class` over `AffinityClass::of_axis`.
- **A species' *stat block* is derived too**, and its one definition is
  `species::stat_shape_faults`.
- **Two censuses are reported to the tuner rather than enforced on it**, and
  which is which is a cost question.

### The HUD

- **`Game::attention` is the one derivation of what needs the player, and
  three surfaces read the same call.**
- **`1`/`2`/`3` are bound once, in `handle_playing_key`'s top match**, which
  runs before the hand-off to `handle_stack_key` — the same place `f` sits.
- **`draw_info_column` owns the column's chrome and returns the open pane's
  body rect.** The column **does not scroll**, so that rect's height is a
  layout constraint.
- **A pane body is rows, and `hud::panes::fitting_rows` is the one place
  what does not fit is counted** — `strip::fitting`'s rule turned ninety
  degrees, written once rather than once per pane.
- **`PaneData` takes `roster`/`carrying` and not a `&PlayerStatus`**: those
  are every field of it the panes read, and `PlayerStatus` derives nothing,
  so a wider dependency is a census nobody writes.
- **The buff-tag ceiling moved with the buffs.** CREW calls `buff_entries`
  rather than restating a buff row, `TagStyle::OwnLine` because the column
  cannot widen.
- **A collapsed bar carries `panes::summary` when calm, and a condition
  outranks it.** Built from the same `PaneData` as the open pane's rows, so
  a bar and the pane it stands for cannot disagree.
- **A content hue is authored, and `hud::palette::glyph` is the one table it
  is drawn from** — `render/mod.rs::glyph_color` is a call to it, shared
  with the six popups that draw a program's glyph, since a program reading
  as one colour on the grid and another on its own sheet is the failure to
  avoid.
- **A tile's con read takes the glyph's own hue whenever it can, and
  `ConRead` is the one place that is decided** — the top-left earmark is
  the fallback for the two tiles whose ink is spoken for, a boss's magenta
  and a drawn sprite, gated on the sprite call's own answer and never on
  the sprite's name.
- **The player's `@` is a role and is read off `is_player`** — `PLAYER`, br
  cyan, which nothing else may take, and never off the `GlyphColor::Cyan`
  the player happens to spawn with.
- **`ICON_PALETTE` is exactly fifteen entries because it is the player
  icon's save format, and `SPRITE_PALETTE` is a separate constant for that
  reason.**
- **A machine's stall asks for attention and never reads as a threat.**
  `Clogged`/`Stranded`/`Unpowered` take `ATTENTION` — waiting fixes none of
  them, and it is the colour `Game::attention` already spends on them —
  while `Starved`/`Unstaffed` keep the dimmer `WARN`.
- **The map's overlays take palette roles too, and `fx.rs` is why the
  palette is `pub(crate)`** — a raid's flash is painted by the effects
  layer, and a structure taking a hit is what the `THREAT` reservation
  exists for.
- **The expanded log pane is an overlay drawn last, and `map_pane` is
  derived from the *collapsed* log at every window size.**
- **One strip to a border, and the vitals get the contested one** —
  they ride `log_pane`'s top border, the filter header is the log body's
  first row again, and `map_pane`'s bottom border carries nothing.
- **A pane whose border carries a strip starts its body at
  `hud::layout::strip_inset` and buys the height for it**, because that
  strip's quad reaches as far *into* the pane as it does out of it.
- **The compass is a block *inside* the map pane, not a strip on its
  border**, so a selection costs no layout — and it starts at
  `strip_inset` because THREAT's quad hangs into the pane above it.

### Saves, logs and screens

- **The save is field-named RON, and that is what retired save migrations.**
  An additive change behind `#[serde(default)]` costs **no version bump**.
- **A run that ends is written into its save, and `Game::load` is what
  refuses it.** `SaveData::game_over`, written unconditionally by
  `Game::save`, sealed to disk by app-core's `App::seal_run`.
- **A log line carries two independent axes, `MessageKind` and
  `MessageSource`.** Kind has three consumers that each mean something
  different by it, which is why "this came from the base" is a second field.
- **A swing's outcome is a third axis on `LogLine`,
  `resources::SwingOutcome`, and it is keyed to the *raw* line the reveal
  releases, never to a condensed row.**
- **A refusal is one sentence on two surfaces, and `App::refuse` is the one
  door** — `App::status_line` for the popup the player typed into, and
  `Game::note_refusal` for the log they scroll back through.
- **`MessageSource` has two readers, and the battle pane's is not the
  filter.** `battle_rows` drops base news unconditionally, because
  `since_round` slices by *position*.
- **The reveal is gated on `Mode::is_battle`, and that gate keeps it off the
  map.** `MessageLog::round_start` is deliberately never closed.
- **The map log pane is filtered; the history screen (`L`) is not.**
  `pane_rows`' stage order is load-bearing: unrevealed tail, then filter,
  then fold, then capacity.
- **All three log surfaces fold repeats, and `resources::condense` is the
  one fold.** It sits in `pane_rows` and `battle_rows` — on the rows about
  to be drawn, *after* the truncation — because `revealed`,
  `hidden_log_lines` and `battle_view_at` all count **raw** lines.
- **The map log pane wraps, and the cut comes off the *oldest* end** —
  `pane_rows` hands its rows over oldest first and `draw_playing_base`
  counts the capacity it asks for in *entries*, so a `break` at the floor
  drops the newest news the moment one entry is several rows.
- **`retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp`,
  `Raid` and `Complete`.** A plain `log()` is `Info` and is pruned.
- **The map's status column cannot grow, and `draw_row` clips vertically
  only.** It holds 38.5 monospace cells; the widest shipped buff row already
  spends all but 3.8 of them, so a companion's `(holder)` tag drew 360px off
  the panel in silence.
- **A read-only screen's row count is owned by app-core and drawn by gui, so
  any per-row transform must live in the engine.** Both sides call
  `Game::message_history`; folding in the renderer opens the screen on a row
  that isn't drawn.
- **A group menu's rows are hidden dynamically**, so `base_menu_rows` /
  `party_menu_rows` must be the *only* source of them.
- **A Broker's board is derived, never stored** — seeded off `(world seed,
  zone, epoch)`.
- **Reading a Broker's board and signing it are two questions, and
  `Game::broker_reach` is the one call that answers both.** `NoBroker` /
  `OffBase` / `AtBroker`; `board_defs` refuses on `NoBroker` alone, the two
  verbs require `AtBroker`, and the base menu's row test and the screen's
  header read the same value.
- **A `starter` contract jumps the board queue, and only in sector 1.**
  `board_defs` fills its three slots from unfinished starters first — three
  uniform draws out of fourteen made a new run's first job a coin flip, and
  `min_zone: 0` says a contract *may* be offered, not that it is offered
  first.
- **You *run* (or *invoke*) a routine; the noun is an *invocation*.** "Cast"
  and "spell" are the fantasy words this setting does not use, and unlike
  Raid the rename went all the way through the identifiers —
  `Game::run_field_routine`, `Mode::FieldRoutine*`, `FieldRoutineTarget`,
  `FieldBuffKind::scales_with_invoker`.
- **"Raid" is the code's word and "GC Entropy Sweep" is the player's.** The
  `.ron` fields are mod schema and deliberately kept their names.
- **A fight's vocabulary is security, not swordplay** — an attempt lands,
  lands unchecked, fumbles or is refused, Bleed is a leak and Stun a stall,
  and the unit is Integrity rather than damage.
- **`world.get::<Stats>(e).is_none()` is the idiom for "this entity is
  gone"** — don't reach for `World::get_entity`.
- **There is one place a runtime path is decided,
  `crates/launcher/src/paths.rs`**, and `main` reads nothing else: the loose
  asset tree, the player-data directory, and whether this build has a repo
  behind it.
- **A path that spends ticks owes `after_tick()`**, and there are three:
  `handle_key`'s tail, `update_realtime` and `App::advance_compile`.
- **A screen that spends the engine's ticks paces them against `dt`, never
  one per rendered frame** — `COMPILE_TICKS_PER_SECOND`.
- **And it spends them at the world's own rate**, `COMPILE_TICKS_PER_SECOND`
  derived from `WORLD_SPEED_MULTIPLIER` rather than restated.
- **Engine test fixtures live in `crates/engine/src/tests/support.rs`.**
  Look there before writing a new one.

### Help and documentation

- **The manual's index is a menu and a page is a document**, and that is why
  `Mode::HelpPage` exists rather than the index taking a second job.
- **`[label](topic-id)` is why there is no `see_also:` field.** One gesture
  writes the sentence and the further-reading row.
- **The wrap is `text::wrap` in the engine**, and
  `render/popup.rs::wrap_text` is a call to it.

### The research tree

- **The research tree's flow-chart layout is derived once, in
  `Game::research_graph`, and keyed by `ResearchId`** — `views::
  ResearchGraph::step` is the one rule for what an arrow key does, and
  app-core computes no neighbours.

## Build & test

```sh
cargo test --workspace     # 5394 tests
cargo run                  # the game; `default-run` in crates/launcher
cargo clippy --workspace
cargo fmt

# Edit a save for testing: dump to RON, edit, pack back. `warp` runs the
# real breach rather than editing the zone number.
cargo run --bin savetool -- dump saves/save.bin s.ron
cargo run --bin savetool -- pack s.ron saves/save.bin
cargo run --bin savetool -- warp saves/save.bin 6

# Start from a known world instead of playing up to one. `dev-saves/README.md`
# lists what each template sets up.
cargo run -- --template extraction
cargo run --bin savetool -- template                        # list
cargo run --bin savetool -- capture saves/save.bin <name>   # record a new one

# Run a battle offline instead of playing to it. `dev-arenas/README.md` is
# the schema; the shipped scenarios are worth re-running after a retune.
cargo run --bin arena -- dev-arenas/opening-fight.ron
cargo run --bin arena -- dev-arenas/full-group.ron --out report.ron

# ...or play the same scenario in the real battle UI, which is the only way
# a companion Special ever fires in an authored fight. Main menu, [R] Arena.
FERAL_DEV_ARENA=1 cargo run

# Draw the whole Stack frame on both maps instead of what has been walked,
# so testing a cell kind doesn't start by walking a maze to find one. Map
# only — see `dev-saves/README.md`.
FERAL_DEV_REVEAL=1 cargo run -- --template stack
```

**Cutting a Windows or macOS release** is manual by choice — no CI, no
`cargo-dist`, and cross-compiling from Linux is deliberately unsupported.
The per-platform checklists, the toolchain each needs, and the argument for
shipping a plain binary rather than a `.app` bundle are in
[`docs/releasing.md`](docs/releasing.md). The deliverable is always an
executable **plus a loose `assets/` tree**, never a single file — fonts and
sound cues are `include_bytes!`d but game content must stay droppable, which
is the moddability rule.

**`docs/measurements/` is what the instruments have already said.** One file
per question answered, each carrying the commands that produced it, the
numbers, and what the run was blind to. Read it before running a sweep or an
arena batch to answer something — the data behind these is hundreds of
megabytes and gitignored, so a number not written down there costs
CPU-hours and an afternoon to get back. Its `README.md` is the convention
for adding one, and the bar: something was run, the data is gone, and a
decision depends on it. Balance curves are excluded on purpose — those are
`balance_sim.rs`'s job, and a copy here would drift.

**Don't reach for a fresh `Game::new` when a `dev-saves/` template would
do.** Testing anything mid-run by hand — extraction, trade, a full party,
a deep zone — otherwise starts with an hour of play, and that cost is what
makes features ship unplaytested. `capture` any state worth returning to.

`savetool` and `arena` are the launcher crate's other two bins. They sit
there rather than in the engine so `default-run` can keep a bare `cargo run`
unambiguous — `default-members` in the workspace root would have done it
too, but would also have narrowed a bare `cargo test` to the launcher.
`arena` has a second, harder reason it cannot live in the engine: it
resolves `dev-saves/` template names, and `dev_template` is the launcher's.

The launcher's `[lib]` target exists solely so its three bins can share
`dev_template` and so that module can be unit-tested once. No game logic
lives there, and none should — the crate is still the binary.

**A debug build is a playable build, and `[profile.dev]` in the root
`Cargo.toml` is what makes that true.** Dependencies build at `opt-level =
3` and the four workspace crates at `1`. Without it the renderer's own
shape-building pass ran **51.4 ms a frame** against release's 2.0 ms at an
identical shape count — under 20 fps before bevy, wgpu or egui's
tessellator had done anything. Deleting the section is a 22x frame-cost
regression that no test catches and that reads as an animation bug. Numbers
and blind spots are in
`docs/measurements/2026-08-19-debug-build-frame-cost.md`.

**Warm builds are not the bottleneck.** `cargo check --workspace` with
nothing changed is ~1.8s; touch one file and the affected crate's test
binary rebuilds in ~2.5s. The engine suite runs in ~6.7s. Save/load was
never the play cost either — on the real 190 KB save a full round trip is
1.46 ms in release. The engine depends on `bevy_ecs` alone; the
557-dependency Bevy graph is `crates/gui`'s, and only a cold build or a
dependency change costs minutes — budget ~3.5 minutes for one and nothing
for anything else. Iterate with `cargo test -p feral-processes-engine
<name>`; there is no tooling problem here to solve with sccache or nextest.

If many tests fail at once with `NotFound` on an assets path, it's stale build
artifacts, not 150 bugs — this repo was formerly at `/home/trog/code/petmud`,
and test helpers bake the asset path in via `env!("CARGO_MANIFEST_DIR")`. Cargo
doesn't invalidate on a directory rename. Fix with
`cargo clean -p feral-processes-engine -p feral-processes-app-core` rather than
a full `cargo clean`, which costs a cold rebuild of the Bevy graph.

**Disk hygiene: remove a worktree when its branch lands, and sweep
`incremental/` rather than the whole tree.** Every agent worktree builds its
own complete `target/`, and an abandoned one keeps it; that plus months of
`incremental/` took `target/` to 474 GB on 2026-09-01 (521 GiB reclaimed — the
"~4 GB" this file used to claim had been wrong for a long time). So `git
worktree remove` once a branch is merged, and `rm -rf target/debug/incremental`
for the periodic sweep: it gives back the bulk and costs only a slower next
compile. A full `cargo clean` costs the ~3.5-minute cold build every time and
is not a per-effort ritual.

## Moddability

This game must always stay moddable. Never hardcode new game content in
Rust when it can be expressed as data instead — species, structures, items,
and abilities should stay extensible by dropping in a file, not by editing
engine code.

- **New species** → add a `.ron` file to `assets/species/`. Schema is
  documented in `assets/species/README.md`.
- **New structures** → add a `.ron` file to `assets/structures/`. Schema is
  documented in `assets/structures/README.md`.
- **New items** → add a `.ron` file to `assets/items/`. Schema is
  documented in `assets/items/README.md`. `ItemId`
  (`crates/engine/src/items.rs`) is a string newtype, not an enum; shipped
  items are still reachable from Rust via the `ids` module in that same
  file (for test setup and data-defined recipes) but adding a new item
  never requires touching Rust.
- **New abilities** → add a `.ron` file to `assets/abilities/`. Schema is
  documented in `assets/abilities/README.md`. A species grants abilities by
  naming their ids with a level to unlock each at; `priority_boost` must
  exist, as it is the fallback for a companion whose species grants nothing.
- **New achievements** → add a `.ron` file to `assets/achievements/`. Schema
  is documented in `assets/achievements/README.md`. Unlike perks this is a
  real content directory — the four `Trigger` and three `Reward` shapes are
  the whole vocabulary and every combination already works. The *ceiling* on
  what the ladder may pay is not data: `tuning::MAX_PROFILE_*`, asserted over
  the real assets by `the_full_ladder_stays_under_its_ceiling`, because
  `balance_sim` models one run's curve and cannot see a cross-run profile.
- **New talent trees** → add a `.ron` file to `assets/talents/`. Schema is
  documented in `assets/talents/README.md`. A real content directory: the five
  `TalentNode` kinds are the whole vocabulary and a sixth class's tree is a
  file, not a Rust change. `Accuracy` is the one read on demand alongside
  `Affinity` — it has no `Stats` field to bake into — and the tier count is
  fixed, so a new node kind costs every shipped tree an existing choice.
  Exactly `KERNEL_RING_MAX * LEVELS_PER_RING` tiers of two choices each, or
  the tree is skipped with a warning; six censuses in `tests/assets.rs` hold
  the *shipped* trees to the design.
- **New help pages** → drop a `.md` file in `assets/help/`. Schema is
  documented in `assets/help/README.md`. Five block rules and no more; the
  filename is the ordering *and* the id a link points at, so there is no
  front matter and no second parser. A page is prose, which is why this one
  directory is markdown rather than RON.
- **Perks are half data, and the seam is deliberate.** `assets/perks/*.ron`
  is a *catalogue*: one file per `Perk` variant carrying its name,
  description and Perk Point cost, loaded by `PerkDb` and reached from the
  renderer through `Game::perk_defs`. The variants themselves stay in
  `crates/engine/src/perks.rs`, because a perk's effect is a hook into a
  particular formula with no shared shape to express as data. So a
  nineteenth perk is a new `Perk` variant plus a **named query in
  `perks.rs`**, called from the site that applies it; `PerkDef` deliberately
  has no `effect` field. **That module is the census** — one query per perk,
  and `every_perk_has_a_query_that_answers_what_it_is_worth` is exhaustive on
  `Perk` (`cell_mark`'s rule), so a variant with no query fails to compile.
  **The two signature families are not interchangeable**: most take the
  player's `Option<&Perks>` and name their own variant, so no call site says
  `Perk::` at all; `mining_roll_bonus` and `quality_floor_bonus` take a bare
  **level**, because `CycleModifiers` and `CraftOrder` carry it to a formula
  whose subject may be a program rather than the player — a `Perks` argument
  there quietly hands a program the player's investment. The three `StatGain`
  perks are in the module too: `purchase_stat_gain` says what buying one
  grants and `unlock_perk` stays the one writer of `Stats`. Per-level
  *magnitudes* stay in `tuning.rs` (below); only cost crossed over. **A
  perk's hook belongs where its sources meet**, not at each of them:
  `Obfuscation` is read inside `raise_trace` rather than at the six things
  that raise Trace. A hook that has to be repeated is the signal the perk is
  aimed at the wrong seam. **`Perk`'s variant order is save format** —
  bincode encodes enums positionally, so `PlayerSave::unlocked_perks` holds
  indices; append, don't reorder, or bump `SAVE_FORMAT_VERSION`.
- **Difficulty tuning** (`crates/engine/src/tuning.rs`) is deliberately code,
  not data. Content is moddable; how hard the game is, is not. Every knob the
  engine hardcodes — zone and Stack-depth scaling, XP curves and level caps, the
  damage and capture formulas, spawn and drop rates, raid pressure, need
  decay, perk magnitudes — is a documented `pub const` there, grouped into
  labelled sections. Put new tuning values in that file rather than inline in
  a formula, and don't duplicate `.ron` values into it.

Rules to follow whenever the schema changes:

- Adding a field to `SpeciesDef`, `StructureDef`, `ItemDef`, or `AbilityDef`?
  Mark it `#[serde(default)]` so existing `.ron` files — including anyone's
  custom mods — keep parsing without being touched.
- A malformed `.ron` file must be skipped with a logged warning, never a
  panic that crashes startup. Follow the existing pattern in
  `SpeciesDb::load_dir` / `StructureDb::load_dir` / `ItemDb::load_dir` /
  `AbilityDb::load_dir` / `PerkDb::load_dir`.
- Update the matching `assets/*/README.md` in the same change whenever a field
  is added, removed, or changes meaning — those docs are the schema reference
  for anyone modding the game.

## Code principles

- **DRY, but not prematurely.** Three similar lines beat a speculative
  abstraction.
- **KISS / YAGNI.** No half-finished implementations, no unused feature flags,
  no building for hypothetical requirements.
- **Fail fast.** Validate early, handle errors explicitly. Don't write error
  handling for scenarios that can't actually happen.
- **Consistency.** Follow existing naming and structure rather than
  introducing new conventions.
- **No backwards-compat cruft.** Don't rename unused variables to dodge a
  lint, leave `// removed` comments, or add shims for code you're free to just
  change. If something's unused, delete it.
- **Comment discipline.** Comments explain *why* — a non-obvious constraint, a
  workaround, a subtle invariant — never *what*. Well-named code already says
  what; a comment restating it is noise.
- **Don't assume.** If you think code works a certain way from memory, open it
  and check.
- **A doc comment claiming to mirror other code must be a call, not a copy.**
  If you write "mirrors", "shared with", "matches", or "same as" about another
  module's formula, extract that formula into a pure function both sides call.
  A comment cannot hold two copies in sync, and the copy that drifts is
  usually the one nobody runs. This has bitten this repo four times, all in
  `balance_sim.rs`. `battle::attackers_in_group`, `battle::slot_aggro_weight`,
  `battle::expected_damage` and `systems::node_payout` are the pattern to
  follow — `expected_damage` is the RNG-free mean of exactly what
  `resolve_attack` rolls, and the sim calls it rather than keeping a copy.
- Composition over inheritance. Avoid global mutable state and god objects.
  Named constants over magic numbers (see `crates/engine/src/tuning.rs`). No
  optimization ahead of evidence it's needed.

## Rust idioms

- Run `cargo fmt` and `cargo clippy` after every change; fix warnings and
  deprecations rather than silencing them.
- Prefer `Result`/`?` propagation over panics in engine code. `unwrap()` /
  `expect()` are for tests, truly-infallible invariants, or startup config
  that should abort anyway.
- Work with the borrow checker's grain — small, focused functions are usually
  the fix for a fighting-the-borrow-checker moment, not reflexive `.clone()`.
- Keep error types explicit; avoid `Box<dyn Error>` catch-alls where callers
  need to branch on failure mode.

## Testing

- Business/sim logic gets unit tests. For a bug fix, write the failing
  reproducer first.
- **No flaky tests.** No `sleep()`, no wall-clock dependence, no reliance on
  RNG you didn't seed — background systems (habitat spawning, nests) can and
  will interfere with a naive assertion.
- **Full suite is the final gate.** Run `cargo test --workspace` before calling
  anything done. Passing only the tests you wrote is not evidence of
  correctness.
- **`balance_sim.rs` is the balance regression gate.** Despite the name it is
  not a constants table (those are in `tuning.rs`) — it's a deterministic,
  RNG-free battle simulator whose tests assert hardcoded empirical level
  curves computed from the live constants against the real `.ron` assets. Any
  change to `tuning.rs`, a species file, or an item file should be checked
  with `cargo test -p feral-processes-engine balance_sim`. A curve that moves
  means progression changed — that's the signal, not a broken test.

## Process weight

Match the process to the blast radius, the same way **Surgical changes**
below matches the diff to the request. Measured on the permadeath feature: a
6-file, ~280-line change carried a 178-line spec and an 874-line plan — 3.7x
the deliverable, in write-once prose, and essentially none of the ~58,000
lines that accumulated under `docs/superpowers/` was ever read twice.

The 46 implementation plans were deleted on 2026-08-13 — git history is their
archive. The 59 specs moved to `docs/superpowers/archive/specs/`, because
nine of them *are* cited from source doc comments as the rationale record for
a seam. `docs/superpowers/INDEX.md` is the one-file answer to "what shipped,
and where is its argument"; every spec in it is implemented, and a spec's own
`**Status:**` header is stale for 14 of them, so never answer from it.

- **One crate, no schema or save-format change, fits in one context** →
  brainstorm to a decision, then TDD inline with a commit per green step. No
  spec file, no plan file. A plan document exists to hand context to a
  subagent that lacks it; writing one for work you are about to do yourself
  in the same session is writing the feature twice.
- **Two or more crates, a schema or save-format change, or you genuinely
  want subagent isolation** → the full spec-and-plan pipeline. That is what
  it is for, and it earns its cost there.

When a plan *is* warranted, don't write the implementation inside it. A
subagent that has the repo and this file needs the file list, the interface
it must produce, the intent of each test, and the gates to run — not
finished code it will merely re-emit. Reserve code blocks for the genuinely
non-obvious: a borrow-scoping trick, an ordering constraint, a formula.

The size rule governs the pipeline, not the discipline. TDD, the failing
test first, and the full-suite gate apply at every size.

## Running subagents

Multi-agent workflows (superpowers SDD and friends) are expensive here — a
single Phase-1 refactor cost ~1.6M subagent tokens across 16 sequential
dispatches. The isolation is worth paying for on a cross-crate refactor;
it is not worth paying for by reflex. Rules learned the hard way:

- **Sonnet is the default. Opus is for judgment, not volume.** A large
  type-flip review or a whole-branch review earns opus. A mechanical fix
  pass, a cleanup sweep, or a re-review of a fix does not — those ran 100+
  tool uses on opus and shouldn't have.
- **Don't re-run the full suite to confirm what a subagent already
  reported.** Spot-check with a targeted `cargo test -p … <name>`; save
  `cargo test --workspace` for task boundaries. The full-suite gate above
  is about not shipping untested work, not about auditing every claim.
- **Never brute-force a flake with repeated runs.** Read the code path
  first — the one intermittent failure this repo produced was diagnosed
  from source in a dispatch that was already running, after 15 rebuilds
  found nothing.
- **Per-task review gates are optional; the final whole-branch review is
  not.** Dropping the per-task gate roughly halves the dispatch count, at
  the cost of defects surfacing at the end instead of immediately. Ask
  which tradeoff the user wants before starting, not after.
- **Give the diff as a file, never pasted into the prompt.** Everything
  pasted into a dispatch stays in context for the rest of the session.

## Guardrails

- **Git:** commit freely as work reaches a green, coherent state — a passing
  test, a finished task. Branch first if on `main`. Pushing still needs an
  explicit ask, and so do force-push, `reset --hard`, and amending pushed
  commits.
- **Versioning: one release per change that lands on `main`.** A feature or
  fix merged to `main` bumps the workspace version in the root `Cargo.toml`,
  gets its own `## X.Y.Z` section in `CHANGELOG.md` and an annotated `vX.Y.Z`
  tag. Commits *on a branch* stay unversioned — the bump happens once, at the
  merge, so a rebase or squash can't invalidate a version already tagged.
  Which digit moves is decided by `CHANGELOG.md`'s preamble, the one
  statement of the policy; the short of it is that "breaking" means **a
  player's save stops loading** (`save::SAVE_FORMAT_VERSION`), not a changed
  type signature. This replaced batching into `## Unreleased`, which ran to
  2,200 lines and two save-format breaks between `v0.2.0` and `v0.3.0` — a
  version number that said nothing about what is installed. Note the tag push
  is separate from the commit push: `--follow-tags` sends annotated tags, a
  bare `git push` does not, and a tag that only exists locally is not a
  release.
- **Surgical changes:** match blast radius to the request. A bug fix doesn't
  need drive-by refactors. Don't take destructive shortcuts past an obstacle —
  find the root cause.
- **Investigate before overwriting:** before creating or replacing a file at a
  conventional path, check what's already there. Same for unfamiliar branches,
  config, or in-progress state you didn't create.
- **Secrets:** before staging, check nothing sensitive is included.

## Working with the user

- State assumptions and decisions plainly; don't hedge with caveats.
- Ask a specific blocking question over guessing at something only the user
  knows — but don't stall on decisions you can reasonably make yourself.
- **Batch independent questions into one round trip.** Ask sequentially only
  when a question genuinely depends on an earlier answer. This overrides the
  brainstorming skill's one-question-per-message rule, which spends a full
  round trip per decision — the permadeath brainstorm took four where two
  would have done.
- Report what you actually verified (commands run, output seen), not what you
  expect should work.
