# The base

- **A deploy is a *request*, and the Home is the only build the player's own
  hands finish.** `place_structure` answers every refusal it always did and
  then spawns a `BuildSite`; a body posted by `schedule_base_labour` fetches
  the bill of materials by hand, sets it down on the cell, and raises the
  thing at `BUILD_TICKS_PER_MATERIAL` per unit. Founding is exempt because
  base space does not exist before a Home stands. **Nothing is charged at
  filing**: a shortfall is a *report* from the builder at the site, latched
  on `announced_dry`, and that latch **clears when a source appears**, in
  `build_wants`. A dig site's `announced_dry` clears in `Game::dig_wants`
  for the same reason — a base running dry more than once over a run is the
  common case, not an edge one.
- **The Home is free and the anchor lands where it was founded, while the
  Home itself still stands on `BASE_EXIT_CELL`.** The two halves live in
  different spaces: base space has one origin — the pocket is laid around it
  and `leave_base` refuses to let the party out anywhere else — while the
  anchor is a zone-surface fixture, so `place_structure` puts it on the tile
  the party founded from through `Game::move_anchor_to`, the one writer,
  shared with `enter_next_zone`. **The one refusal it needed is a Stack
  link**: a link tile is walked onto to *descend*, so an anchor sharing one
  could never be stepped on to be entered, and the check sits with
  `require_surface` where the door tile is resolved rather than beside the
  materials check. The cost went to nothing because the wizard's Kit step
  replaces the class kit — a run that bought gear over fragments could not
  open a base at all — and the founding path stays generic over
  `build_cost`, so a mod that prices it back up still works. **The fixture
  trap it exposed**: app-core's compiler fixture pushed its cargo onto the
  save's inventory `Vec` as a *second* row for an item the kit already
  carried, which was invisible only while founding happened to zero the kit's
  row — `Inventory::count` reads the first matching row and `Game::load`
  restores the `Vec` verbatim.

- **A zone-portal line is ramped from the zone it was introduced in, and
  `build_cost` is `min_zone: 1`.** The trap: a `zone_build_cost` line
  authored for a later sector but ramped from zone 1 — the naive
  `zone_portal_cost(base_qty, zone)` applied uniformly — arrives already
  inflated the first zone it can legally be demanded, and nothing fails to
  compile; it reads as a normal price until someone checks the number against
  the `.ron` file. `Game::structure_build_cost` counts each line's ramp from
  its own `min_zone` — `zone.saturating_sub(min_zone) + 1` — which is also
  what makes `build_cost`'s implicit `min_zone: 1` a no-op under the new
  formula rather than a second special case. **Early-return order matters
  too**: the qualifying `zone_build_cost` lines are appended to `build_cost`
  *before* the function branches on `zone_portal`, so a non-portal structure
  that authors one still gets it — unramped, since only the `zone_portal`
  branch grows anything. Branching on `zone_portal` first and returning
  `build_cost` unchanged for everything else would make the append
  unreachable for the common case the field exists to serve.
- **Upgrading is a build request too, and `BuildSite::goal` is the whole of
  the difference.** `upgrade_structure` keeps every refusal in the same
  order, drops the pack charge and the tier write, and files a site carrying
  `BuildGoal::Upgrade { to_tier }` on the machine's own cell; exactly one
  step branches on the goal, `raise_one_tick`'s completion. The site names a
  **tile, never an `Entity`** — the machine is resolved by position at
  completion — and carries **no `Glyph`**: the machine is still standing,
  still drawing that cell and **still producing**. **Five traps.**
  `count_build_requests` must count `New` only, or a pending upgrade eats a
  `max_deployed` slot. Both destruction paths must go through
  `clear_pending_build_at`, the Home cascade included, or delivered units
  stand on a cell nothing occupies. The builder wears the job mark itself
  (`wears_job_mark`, `Excavate`'s rule). `render/base.rs` tests
  `is_structure` **before** `build.is_some()`, or an upgrading machine draws
  as a bare slab. And the upgrade menu quotes `build_cost_display` through
  `Game::upgrade_cost`, the pack no longer being the store the verb reads.
- **`Game::spawn_structure` is the one place a structure's component list is
  written**, `roster_parts`' argument on the other roster: two callers with
  nothing in common, and nothing fails to compile when a hand-written copy
  drifts — a crew-built machine missing its `MachineStatus` reads as the
  base being broken. It performs **no checks at all**, deliberately, which
  is why `max_deployed` counts pending requests alongside standing
  structures.
- **Materials are not spent until the structure is raised.** They leave
  their shelf when a builder picks them up and stand on the cell until the
  site is despawned at completion — which is what makes
  `cancel_build_request` a refund of goods that still exist, and makes
  `BuildSiteSave::delivered` the load-bearing save field.
- **Build wants are *prepended* in `schedule_base_labour`, the mirror of
  dig wants being appended** — the priority is the position in that list,
  since `truncate(staff.len())` cuts from the end. **The trap is the
  empty-queue standdown guard**: read off `WorkOrders` alone it reasons from
  "nobody has told this base anything" while somebody has, so it is gated on
  build wants too.
- **An unreachable request is dropped *above* the cut in
  `schedule_base_labour`, and that placement is the fix.** Every other kind
  tests reachability below the truncation, affordable only because dig wants
  are appended *last*; a build want is prepended and so inside the cut by
  construction, so a site nobody can walk to takes the slot and leaves the
  base with an idle body and an unworked order every tick, silently. The
  check asks the **staff**, short-circuiting on the first body that routes,
  and announces once via `announced_stuck` with **no silent arm**. There is
  deliberately **no `has_station` pre-filter** in `build_wants` — the route
  check subsumes it. Testing it needs **two** islands: one cell with no
  standing room, two cells with standing room and no route.
- **A dry request is not a want, and `build_is_workable` is the one place
  in the scheduler a want is allowed to be a stock count.** Unconditionally
  listed it **deadlocks** a one-program base: the build outranks production,
  the body stands at a site with nothing to fetch, and the node that would
  make that material is never worked again. It asks only for a **next
  unit**, never the whole bill. **Two traps fell out of it**: the dry report
  had to move to `build_wants`, since a dropped site is never posted and
  nobody is left to say it (the scheduler owns stall announcements); and it
  must count a load **already in a builder's hands**, or the base announces
  the *whole* bill the instant a builder scoops the last shelf, then latches
  quiet. Both latches clear in `build_wants` and nowhere else.
- **A builder *walks* to its materials, and that is what the dig crew does
  not do.** `stock::spend_from_base` teleports a unit off a shelf;
  `construction::Source` is a tile to walk to — any deployed structure's
  output buffer, plus the party's pack, which is a source **only while the
  party is in base space** and sorts last among equals. The put-back
  (`stock::return_to_depots`) is deliberately **narrower than the draw**:
  Depots alone, because a unit pushed into a machine's output reads as
  something that machine produced. What fits nowhere is logged, never
  dropped in silence.
- **`views::BuildOrderRow` is the one derivation of what a request looks
  like**, read by the map, the examine line and `build_order_report` alike,
  and every figure in it is a *call* — `BuildSite::required_ticks` is
  derived from the stored cost and never stored beside it. **Two things a
  widening must not break**: "one builder at a time" is the scheduler naming
  a site once and not a count on the component, so a second builder costs no
  save bump; and `TaskKind::Construct` is the first kind whose holder may be
  *carrying*, so `schedule_base_labour`'s never-free-a-`Carrying`-holder
  rule is load-bearing for it.
- **A slab wide enough eats the Stack on-ramp's draw box, and the failure is
  the whole zone rather than one link.** `spawn_surface_links` shares an
  attempt budget across all three links, so an unplaceable on-ramp yields
  *zero*. It draws from the Chebyshev ring just outside
  `MAX_BUILD_DISTANCE_FROM_HOME`, walked directly rather than
  rejection-sampled; `frames_for` subtracts that radius.
- **The map draws one space, and `Game::stands_in_base_space` is which.** A
  `Structure` or a `Tamed` program stands in base space; every other glyph
  — wild program, nest, Stack entrance, the anchor — is a zone-map fixture.
  `view_entities` and `find_target_in_direction` both select on it, because
  the map and the examine ray must stay the same set. **It is a second rule
  beside `drawn_on_surface_map`, not the same one**: that says whether a
  program's tile is live, this says which space it is live in, and reading
  the first as the second put the base's roster on the open grid. **The
  player is in both spaces and is held out**, read through `scan_center` —
  `view_entities` is the map's only source of the `@`, and its `Position`
  is pinned to the anchor tile while the party is out of phase. A fixture
  that wants to *locate* an owned program wants `owned_program_views`.
- **The "someone is on this job" mark goes on whichever end of a posting
  has a glyph to wear it**, and `wears_job_mark` / `structure_attended` are
  the two halves — exactly one per posted program at every instant. A
  machine wears it while its worker stands there and the worker takes it
  along to deliver; a guard is never drawn, so its structure keeps it; a
  **digger wears it for the whole job**, because a `DigSite` has no glyph at
  the other end. Exhaustive on `TaskKind`, `cell_mark`'s rule. Distinct from
  `position_is_honest`, which is whether the program may be drawn *at all*.
- **A supplier that declares `StructureDef::power_upkeep` supplies nothing
  while it is dry, and the Home never declares it.** The Home's free 4 is
  the bootstrap — a base holding no Power Cells could otherwise never run
  the Power Conduit that makes the first one, which is a dead run rather
  than difficulty. Three traps under it. A burner **runs no job**, so
  neither writer of a structure's component list gave it a `MachineStatus`
  before this: both now gate on `def.runs_a_job() || def.power_upkeep.is_some()`
  and both insert `components::PowerFuel` — `spawn_structure`'s rule, and the
  load path is the hand-written copy that drifts with nothing failing to
  compile. `ledger` counts a burner's `power_supply` **only through that
  component**, so absence reads as dry: loud when a writer forgets, where
  the lenient direction would be a supplier on free power forever.
  `idle_machine_system` skips a structure that runs no job, or the supplier
  flips `Starved`↔`Idle` every tick and `set_machine_status` logs both. The
  spend and the refuel live **inside `power_grid_system`, before it calls
  `ledger`** — decrement, then refuel on reaching zero, so a supplier that
  can pay never leaves the grid for a tick — and `Starved` is the existing
  variant rather than a new one, `cell_mark`'s rule.
- **`power_upkeep` is `Option<ItemId>`, not `bool`, and the *building*
  gates on it, not just the Grid.** Task D shipped the bool and flagged the
  wider type as a decision it wouldn't make alone; Task E took it, because
  content naming its own fuel is the moddability rule with nothing left to
  weigh. And Task D's original gate only stopped a dry supplier's Grid
  contribution — its personal `power_regen` trickle kept running regardless,
  which is exactly backwards for a game that treats Power as not a limiting
  resource: the trickle is the half a player feels. `game::base::power::
  is_fuelled(def, fuel)` is the one predicate now, extracted rather than
  copied a second time — `ledger` and `power_regen_system` both call it,
  and `power_regen_system` needed `Option<&PowerFuel>` added to its query to
  have anything to pass. `burn_grid_upkeep` reads each burner's own fuel id
  out of its def instead of a hardcoded `ids::POWER_CELL`, so the typo trap
  moved from "impossible to author" to "silently ships" — closed by a third
  census assertion, every declared fuel must resolve in `ItemDb`.
- **A trickle test has to spend `PowerReserve` down before timing it, or a
  saturated reserve hides the very thing being tested.** `power_regen_system`
  runs *ahead* of `needs_tick_system` in the schedule, so a reserve parked at
  `POWER_MAX` converges to the fixed steady state `POWER_MAX -
  HUNGER_DECAY_PER_TICK` whether or not the trickle actually fired — regen
  pushes it to the ceiling, decay knocks it back down by exactly its own
  rate, every tick, trickle or no trickle. A test that doesn't `spend()` some
  Power first before capturing "before" reads that steady state as "nothing
  changed" in precisely the case where something should have. And
  `tests::support::spawn_structure_at` bare-spawns a `Structure` with no
  `PowerFuel` at all — its own doc says it is for what a standing structure
  *enables*, not for the build rules — so a fixture built on it needs a
  fuelled `PowerFuel` inserted by hand before a trickle test means anything;
  three in `tests::building` and one in `tests::power` needed exactly that
  once the dry gate landed.
- **The one machine-to-machine reach is `collect::plan_adjacent_take`**, and
  `Game::take_from_adjacent` is not it — that one is the *player's* collect,
  keyed on where the party stands and needing `&mut Game`, which a bevy
  system cannot have. `assembler_system` inlined its own `by_tile` +
  `ORTHOGONAL` walk and `power_grid_system` would have been the second copy;
  both call the helper now. It **plans** rather than moves, because both
  callers read a neighbour's `output` and write their own buffer through one
  `Query<&mut Stock>`, and it keeps `ORTHOGONAL`'s array order rather than
  `adjacent_stock`'s `(x, y)` sort, so no existing pull moved.
- **A Depot with four occupied orthogonal tiles is a Depot nothing can
  reach**, and the failure it produces is `Stranded` on some *other*
  machine's worker rather than anything naming the Depot. Found rebuilding
  the `chains` dev template's supplier bank around one: a hauler has to
  stand beside a Depot to deliver, so a supplier bank ringing one boxes it
  in. Keep a free tile on every Depot.
- **A raid's flash is base-space too, and `render/base.rs` gates both draw
  sites on `base_pos`.** Every `VisualEffect` names a structure's tile, so
  the queue is base-space by construction — ungated, `tile_flash` and
  `draw_bursts` painted a raid onto whatever surface ground shared those
  numbers, usually the party's own tile. Suppressed rather than moved to the
  anchor: the log pane's flash and the `Raid` line already carry the news.
  `VisualEffect` has **no** space tag on purpose — one variant is not an axis.
- **What the base is holding is one row across the top of every screen that
  draws the world behind it.** `Game::base_stock` reads the same buffers
  `base_holding` sums, through `stock::output_buffers` — a second walk makes
  the strip an opinion about the base rather than a readout of it — **plus
  every `ItemDef::banked` pool**, which is the one thing those buffers can
  never hold. Folded in by the flag and never by name, and `output_buffers`
  is **not** widened — an order for a banked item is refused on the grounds
  that no shelf holds it. **A row exists if the base holds any of it *or* is
  set up to make it** — `stock::producible` seeds a 0 for every deployed
  structure's `work.produces` **and** its `assembles.item`, because an
  assembler declares no `work` block at all. Deliberately not "any structure"
  (a Depot makes nothing and would seed a row for every item in the game) and
  not the researched recipe list (a bench recipe compiles into the *player's*
  pack, so its row could never move off zero). A banked pool the player has
  none of is not seeded. Ordered by item id, never by quantity; a claim about
  the base and not about where the party stands, so it needs no
  `require_surface`. `ItemDef::tag` derives two letters from the name and
  `abbrev` settles the one shipped collision, held by a census. The width is
  **measured**, `stock::fits`, for the status column's reason: an over-wide
  row is drawn off the panel in silence, so what does not fit is counted.
- **Base space carries its own seed, and it is not `WorldMap::seed()`.**
  `BaseGrid::seed` is minted at `Game::new` and saved with the grid, because
  `enter_next_zone` mints each zone's map from
  `seed().wrapping_add(0x9E37_79B9)` while the base *travels*. Salted off the
  world seed, every rock seam in the base reshuffles on a breach and a
  half-cut wall reloads as a different kind under an already-spent
  `Durability`. `base_spaces_seed_and_its_seams_survive_a_breach` asserts
  **both** halves — that the world seed moved and the base's did not.
- **A base-space cell's kind is derived, never stored**, `rock::RockDb::
  kind_at` — an FNV-1a fold of that seed and the **block** the coordinate
  falls in, reduced through `derive::index`. Blocked, or kinds are pepper and
  an exposed face says nothing about what is behind it. `Game::wall_at` is
  the one door from a coordinate to it; `tuning::BASE_ROCK_DURABILITY` is now
  only the *fallback* kind's number, and reading it where the question is
  about a particular wall caps every dense wall in the base at 24.
- **A swing is capped at `durability / min_swings`, per kind.** The fix for a
  developed player demolishing their own base by clipping a corner —
  `swing_damage` grows all run and the rock does not. **Level-independent on
  purpose**: scaling durability with the player is the thing `tuning.rs`
  forbids. Player and crew meet it identically because `strike_rock` is the
  one door.
- **A rock kind authors a brightness, never a hue or a colour.** Hue answers
  passability for the whole map, and `biome_tint` rotates it per sector — an
  authored hue would fight that rotation and a seam would change appearance
  on a breach. `SHADE_BAND`'s floor of 1.0 is load-bearing: a face darker
  than the wall around it is harder to see than anonymous rock.
- **Only an *exposed* face shows its kind, and that is a display rule
  only.** `BaseGrid::is_exposed` — solid, with an **orthogonal** walkable
  neighbour — derived per lookup because cutting, and entropy re-knitting,
  both move it. `strike_rock` resolves the true kind regardless, so the
  "fix" to refuse: resolving unseen rock to the default kind so the two
  halves agree. The map and the examine ray are asserted against **each
  other**, never against a string.
- **`resources::MiningMode` is the player's own bump and nothing else**, off
  by default and off for any save that never said otherwise. `run_dig_crew`
  must never read it — a mark is an instruction the base already has, and
  gating it stalls every dig job while reading as the crew being broken. A
  fixture that digs by *walking* needs `game_at_the_frontier_cutting`;
  omitted, the wall never comes down and it reads as the dig being broken.
- **`Tile::open_to_hostiles` is unreachable now, and is kept rather than
  deleted.** Nothing writes `Biome::Platform` into a `WorldMap` — the base is
  out of phase and its floor is `BaseGrid` — so the predicate, and
  `link_site_free`'s own `Biome::Platform` check, are both dead and both
  deliberately left standing for slice 2/3. The *rule* is what has to be
  re-established there: the base is the one ground hostiles may not enter,
  and `walkable` alone was never it.
- **Wild population is a property of place, and the density target is what
  "populated" means.** `Game::ensure_local_population` stocks any world chunk
  within `POPULATION_CHUNK_MARGIN` of the player's that `PopulatedChunks` has
  not marked; `maybe_spawn_wild_creature` regrows the local box. The mark is
  written **before** the chunk is stocked, or unplaceable ground is retried
  every tick; it is zone-local and so wiped **by name** in
  `enter_next_zone`. `cull_to_cap` evicts **whole chunks**, and takes
  candidates from **where hostiles stand, not from the mark set** —
  otherwise a wandering program in unstocked ground is never evictable. The
  gate is in `maybe_spawn_wild_creature`, not `spawn_wild_nearby`, and is
  checked **after** the roll so a miss leaves the RNG stream untouched.
- **The opening ring needs an explicit radius**, `OPENING_RING_TILES`. It has
  been decoupled from a derivation twice; both times the derived form turned
  a base-geometry or curve change into a silent difficulty change. The four
  species it draws from are the only `beatable_by_a_fresh_player` clears, and
  `habitat_pools` falls back to the *unfiltered* roster when nothing
  qualifies — so raising them empties the ring while leaving it looking
  intact.
- **`Stock`'s `output` is public and its `input` is private**, and that
  asymmetry is the whole of a chain's directionality. `Errand::Load` is not an
  exception: it is the machine's own worker loading its own hopper.
  `game/base/collect.rs::ORTHOGONAL` is the one reach rule both the player and the
  pull phase read.
- **Taking and putting are one screen, one basket and one commit** —
  `Mode::Transfer`, opened with `c`. `game/base/transfer.rs` holds the union
  offer (`transfer_offer`), the room (`transfer_room`), the two refusals
  (`refuse_transfer`) and the one commit door (`transfer_items`); it
  reimplements neither half. The two movers are `take_from_adjacent` and
  `give_to_adjacent`, **guard-free, log-free and tick-free** by construction,
  so the caller owns the announcement and the turn. **`transfer_items` takes
  before it gives**: a rebalance that empties a full Depot and refills it
  from the pack lands both halves only in that order, and the other way the
  give clamps to zero *silently*. The reach is `ORTHOGONAL` through the
  private `adjacent_stock`, and `hauling::take_from` is still the one way a
  unit leaves a buffer. **The trap is that the scan is sorted by `(x, y)`**:
  a partial take across two neighbours holding the same item must drain them
  in the same order every run, and a bare `reverse()` does not prove it. An
  over-ask is clamped, never refused, at both ends.
- **The put side is one budget and the take side is per row, and the screen
  must tell "no Depot" from "a full Depot".** `App::put_available` subtracts
  the *other* rows from `basket_room` so the highlighted row can still be
  lowered and raised; `App::take_available` is that row's shelf alone. A
  pending take deliberately does **not** credit the put budget — a take may
  come off a machine that is not a Depot — and under-offering is safe
  precisely because the commit takes first. `basket_room` is `Option<u32>`
  and **`None` is "no Depot beside you" while `Some(0)` is "a Depot with
  nothing left"**: the room line is omitted entirely on `None`, since a line
  reading 0 beside a Mining Node claims the base is full when it has no shelf
  at all. `Game::transfer_room` is the one call that answers both, and
  nothing infers the `None` from a zero. Three silent traps besides: the pack
  side must filter `ItemDef::banked` (a bank is not cargo, though a banked
  item on a *shelf* is still a real take row), must close entirely without a
  `stores` neighbour, and reads `Inventory` — the plain-copy store — so a
  rare copy is never cargo.
- **`TransferRow::carried` is a holding and `can_put` is a permission**, and
  the screen draws the first while `put_available` clamps against the second.
  They part company in exactly the two cases above — no `stores` neighbour,
  and a banked item — both of which may still be *taken*, so one field doing
  both jobs drew a `you` column reading 0 while the pack held twelve. **Only
  `can_put` creates a row from the pack side**, or a pack full of cargo
  beside a Mining Node opens a screen of rows that move in neither direction.
- **The trade currency gets no row on either side, and it is its own filter
  rather than `ItemDef::banked`.** Credits are carried in the same
  `Inventory` as cargo, are spendable from the pack and survive a breach, so
  nothing already in the offer excluded them: a Credits row could be *put into
  a Depot*, and while the pack column was the shared put budget, spending it
  on another row lowered the Credits figure — putting a Power Cell away read
  as the base charging money for it. `caravan_shelf` and the stack market's
  listing already say `item != currency && !is_banked(item)`; this is the
  third.
- **On the picker screen an arrow moves stock toward the column it points
  at.** The screen is a table reading `item | you | container`, so
  Left pulls off the container toward you (a take, **positive**) and Right
  pushes from you into it (a put, **negative**). The **sign convention is
  untouched** by that — only which arrow reaches which end — so nothing below
  `handle_basket_key` knows the arrows moved, and
  `left_takes_out_and_right_puts_in` is the pin. This replaced an inversion
  that was specified and still read as a slip; the caravan's key table used to
  cite the old test by name to say it was *not* following it. `[A]` writes the
  take ceiling over **every** row, clearing a pending give: that is what "take
  everything" means on one axis.
- **The picker's two figures are the projection, not the holdings and not the
  ceilings**: `carried + amount` and `on_shelves - amount`, so a take fills
  the pack and empties the container in front of the player. That is the
  screen's **only** feedback on the basket — the `change` column it replaced
  said the movement a second time in a notation the two moving numbers do not
  need, and a row redrawn from its raw figures leaves the keys moving a number
  nothing on the page shows. `projected` clamps to `0..=u32::MAX` because
  `i64 as u32` **wraps**: `edit_row` cannot reach either end, and a column
  reading four billion units is what a slip would draw.
- **The picker's figures are padded into the row's own label, not ridden in
  the suffix column, and the header is why.** `suffix_x` places a suffix one
  `m.inset` — a *pixel* gap — past the label's advance, and a column header
  cannot be a `Row::Item` (`popup_layout` splits the body at the first item
  row, so a header built as one scrolls away). A `Row::Text` header is drawn
  flat at `x + m.pad` and no string reproduces a 7.5px offset in monospace
  cells, so every gap on this screen is a whole number of cells instead.
  **The second half is the lead**: `draw_row` opens an item label with `"  "`
  and a text row with nothing, so a header padded from the same widths sits
  two cells left of its own table and reads as the *figures* being crooked.
  `Columns::header` carries `HEADER_LEAD` itself, and
  `the_header_sits_over_the_columns_it_names` measures both sets of column
  boundaries rather than comparing strings. `no_transfer_row_overflows_its_popup`
  is still the census that stops an over-wide row being drawn off the panel in
  silence, and it measures the header too.
- **A modifier is four `GameKey` variants, and `App::handle_key`'s fold is
  the list of screens allowed to see one.** `ShiftLeft`/`ShiftRight` is a
  **target** (an end of the row, idempotent under key repeat);
  `CtrlLeft`/`CtrlRight` is a **step** that halves the gap to that end,
  `div_ceil` so a gap of one closes rather than stranding. gui's
  `with_modifiers` promotes the **horizontal** arrows alone. **The trap is
  that every other key handler ends in `_ => {}`**, so a modified arrow
  reaching them is a dead key nothing catches: `App::handle_key` folds them
  back to bare `Left`/`Right` for every mode the condition above the dispatch
  does not name — `Mode::Transfer`, `Mode::Caravan` and `Mode::CraftQuantity`
  — and never in the renderer, since what a modifier means belongs beside the
  mode that decides it. **Miss the name and the new screen's four modified
  arrows are plain steps**, which is a feel bug no test of that screen's own
  handler can see.
- **`assembler_system` sorts machines by `(x, y)` before pulling**, because
  bevy's query iteration order is not stable and two machines competing for
  one feeder would resolve differently between runs. The test spawns the
  competitors in the *opposite* order to their positions on purpose.
- **Planning is per machine, not per base.** Planning the whole base at once
  compiles just as well and lets two machines take the same units, silently
  undoing the sort.
- **`Stock` keys by `ItemId` in a `BTreeMap`, and `ItemId` derives `Ord` only
  for that** — iteration order feeds the pull phase, and a `HashMap` would
  make the save encoding differ run to run.
- **A machine's recipe is the assembled item's own `craftable.cost`**, via
  `systems::assembly_recipe`. There is deliberately no recipe on
  `AssembleDef`, so a bench recipe and a machine recipe cannot drift and
  every craftable a mod adds is automatable for free.
- **Every shipped `assembles` recipe is one ingredient, and that is a property
  of the *items*.** A second ingredient on any of the four intermediates
  silently turns its bench back into a corner puzzle. The engine's multi-input
  support is untouched and mods may ship two-ingredient assemblers.
- **A work order stores what was asked for, never how it will be done.** An
  item, a quantity and a `standing` flag — labels on the request, no plan.
  Which machines a line needs, who is on each and how far along it is are
  recomputed every tick; "percent done" is not stored,
  `Game::work_order_report` *calls* `wants`. Cancelling unwinds nothing. The
  two functions everything runs through are `can_progress` and `wants`, and
  "within reach" has three sources and one definition,
  `work_orders::batch_within_reach`. `depot_holding` is deliberately narrower
  than `base_holding`. **"Where can this ingredient come from" is a second
  one-definition rule, `work_orders::feeders_for`** — an orthogonal producer,
  or any deployed producer when a Depot is standing. `break_at` and
  `walk_feeders` both call it; a neighbours-only copy in `chain_break` hid
  the whole work-order row from a base the hauler could already have fed. It
  is structural and never a stock count, or the picker would flicker as the
  shelf drained.
- **Every unsatisfied order is worked at once, and `settle_orders` is where
  priority lives.** It accumulates the wants of every non-stalled order in
  **queue order** and dedupes by machine keeping the **first** occurrence;
  `schedule_base_labour`'s `truncate(staff.len())` does the rest, so there is
  no sort and no score. The trap is that dropping the dedupe does *not* show
  up as two bodies on one machine — `post_worker` calls
  `displace_task_holder`, so the second posting evicts the first and the cost
  lands somewhere else entirely: an idle program and the want the truncation
  cut to make room for the duplicate.
- **A satisfied standing order is skipped, not removed** — `index += 1`, the
  branch a stalled order already takes. `WorkOrder::standing` makes an order
  a level the base holds rather than a batch, because a target that deletes
  itself when reached is not a target. **Returning its empty wants would
  starve every order below it forever**, which is the one correctness point.
  No hysteresis and no `refill_at`: the drain is a burst, not a trickle. It
  logs nothing on top-up — "complete" is a lie about something that is not
  complete — so filing one says so instead. `base_holding` counts machine and
  depot buffers only. **`queue_work_order` takes the whole order**, built by
  `WorkOrder::batch` or `WorkOrder::level` — a batch and a level are
  different errands, not one errand with a flag.
- **A work order's band is an insert position, not a second sort.**
  `queue_work_order` inserts after the last order of equal-or-higher
  `OrderPriority` and nothing reads the field again. A sort at scheduling
  time makes Vec order and effective order diverge, and `cancel_work_order`
  takes a **raw Vec index** while the screen indexes straight into
  `work_order_report`. **After** the last equal order, not before the first,
  or ties stop breaking by insertion order. `OrderPriority` is not `Ord` on
  purpose (`High < Normal` reads backwards); a private `rank()` does the
  comparing. `[P]` sets it at filing and raises first; there is no reorder
  verb, and refiling restores the band.
- **`schedule_base_labour` decides the whole assignment by priority and then
  diffs it.** Filling greedily around existing postings leaves a body on a
  standing job while an order goes unworked. The diff is the anti-thrash rule.
  **A body holding a `Carrying` is never freed** — freeing one destroys the
  goods — and neither is one standing on a machine with **no output room
  while a Depot stands**: a clogged machine drops out of `wanted`, and that
  body is the only thing that can carry the clog away and let it run again.
  **It stands nobody down on a base with an *empty queue*** — a run-dry base
  or a save written before work orders would otherwise be swept on the first
  tick — but that guard must be **qualified**, or it also fires on a base
  whose orders are all *satisfied* and the line runs on for the rest of the
  run. It draws no RNG at all.
- **How short of bodies the base is, is a cached figure taken *before* the
  cut.** `resources::LabourDemand` is written once a tick by
  `schedule_base_labour` — the wants it accumulated against `staff.len()` —
  and read back by `Game::labour_demand` for the work order screen's header.
  Written after `truncate` the figure is `staff.len()` by construction and
  the shortfall is always zero, so the header never draws and every test
  that merely reads the two fields stays green. Cached rather than derived
  because the derivation is `&mut self` and logs, so a screen cannot call
  it. The `staff.is_empty()` early return writes it too, and the header says
  **nothing** at zero.
- **A program's role is derived, and there is no "owned but idle" state.**
  `Game::program_role` over `ProgramRole` — disjoint and exhaustive, so a
  program you own that is not fighting beside you, not held as your weapon
  and not away on a sortie **is** base staff. There is no marker to assign
  and no verb to assign it. **The rule is `party::role_of`, a free
  function**, for `stack::surfaced`'s reason: `base_entropy_system` has no
  `Game` to ask and must not hold a second copy — its query is deliberately
  wider than the rule and narrows through `role_of`. `CreatureSave::staff`
  is still written and read nowhere, so this cost **no
  `SAVE_FORMAT_VERSION` bump**. Two traps: `assign_cronjob` no longer pins a
  worker (the poster is in the pool, so the scheduler moves it next tick),
  and base output now scales with roster size, bounded only by
  `pet_capacity`.
- **`accepts_a_program` is the one predicate for "a program can be posted
  here"**, and `hauling::post_reach` is the other half — the walk to it. It
  reports `NoPost::BoxedIn` apart from `NoPost::NoRoute` because the two leave
  the player different errands. `schedule_base_labour` **skips** a machine
  with no route rather than filling it.
- **A posted program sets off from its own tile, and the player's tile is
  read nowhere in the scheduler.** `post_worker` writes no `Position` at all
  — the same omission `post_guard` makes — and `can_walk_to_post` is asked
  from the body being considered. `drift_idle_staff` runs *first* precisely
  so that tile is a live one. Measured from the player instead, one seam
  broke twice: a wandering body teleported across the map onto you, and
  walking out of the walk field stopped the base filling a single machine.
- **An idle program wanders the base, and laid floor is the leash.**
  `wander_step` offers one of the eight neighbours of the tile the body is
  *standing on*, or a hold, every `IDLE_STAFF_STEP_TICKS` — relative, where
  the ring it replaced was absolute. Pure, RNG-free and folded **a byte at
  a time**, `sectors::sector_seed`'s idiom: `derive::index` reads bit 63 and
  a step counter folded whole never reaches it. **`is_floor`, never
  `walkable`** — entropy reverts a mined `Open` cell nobody stands on, and a
  wanderer sealed into a fresh corridor is unpostable for the rest of the
  run, so the paving is the roam limit and there is no radius to tune. A test
  fixture needs an unfloored cell beside the pocket or the two predicates
  agree and it proves nothing. `park_tile` survives as **`entry_tile`**,
  asked only of a body not on floor: a tamed program's `Position` is the
  surface tile it was beaten on. Two rejections: never onto a tile another
  idle body holds, never onto the party's cell.
- **`task_progress_system` and `assembler_system` both write `Task::progress`
  and are `.chain()`ed** — bevy can see the conflict but not the disjointness.
  An assembler's rate comes from **`Task::required`, not `ticks_per_unit`**; a
  fixture that hand-writes a `Task` must set it to the machine's real value.
- **A test fixture that hand-spawns a work node needs `work_node_parts()`**,
  and one that posts a program needs `park_at_post()`. Both omissions read as
  a payout curve that moved rather than as a fixture short something.
- **`MachineStatus::Stranded` is `Unstaffed` plus the knowledge that waiting
  will not fix it.** The two systems split by writer: `haul_step_system` marks
  the *worker*, `task_progress_system` stays the only writer of a machine's
  status. Giving the status two writers makes them ping-pong every tick.
- **`set_machine_status` is the one place a stall is announced, and it logs
  only on transition.** Three callers, so "entering a state is news, staying
  in it is not" cannot lapse in one of them.
- **"Nobody is posted here" is one pass over every machine**,
  `idle_machine_system`. It was the assembler's branch, which visits only
  structures declaring `assembles` — so an unstaffed extractor drew green.
  `task_progress_system` announces `Running` while a cycle is still ramping.
- **A banked resource can never clog, so a Research Node has no "full"
  state.** Its four reachable statuses are `Idle`, `Unstaffed`, `Stranded`,
  `Running`.
- **Departure lives in `haul_step_system`, not the clogged branch**, because
  it has to know whether a depot exists — a base with no depot must behave
  exactly as it did before depots shipped. `hauling::consumer_beside` is the
  second trigger, asked of the **recipe** rather than of whether the neighbour
  is pulling. The cost falls on extractors alone.
- **An attached building is one the base has a reason to run**, and the recipe
  alone was never enough: an unstaffed assembler pulls nothing, so a Lathe
  nothing has been ordered from reserved a Mining Node's whole buffer for a
  machine that would never take a unit. `work_orders::queue_needs` is that
  reason — the **closure** of the **whole queue** under `ItemDef::craftable`,
  over items rather than machines so a bevy system can ask it — or a standing
  work job. **Do not "fix" this by bounding the hoard instead**: a producer
  that hauls its surplus never clogs, and the clog is what hands a lone body
  downstream.
- **`Carrying` is the only thing hauling stores**, and the carry cap is what
  lets it be one `(item, qty)` pair. What a worker is *doing* is
  `hauling::Errand`, derived per tick and never stored; every variant carries
  **owned** data. Direction needs no field. Both destruction paths must drop
  `Carrying` with the `Task` by hand.
- **Destroying a structure has two paths** — `damage_structure` and
  `remove_structure`. Anything that must happen as a structure comes down
  needs wiring into both.
- **A trader's buyback shelf is keyed by `(kind, tile)`, not by `Entity`**, so
  it outlives the building. **Breaching does not despawn structures** —
  anything zone-local has to be wiped by name in `enter_next_zone`.
- **Three of the five classes do something at a post, each in a different
  system.** The Leech bonus rides the **scaled** branch only, which is why
  `CycleModifiers` carries the *class* rather than a finished bonus. The
  Bastion job multiplies mitigation that already existed. The Medic job counts
  `TaskKind::Guard` **only**.
- **A structure's upgrade tier is bounded twice**, `min(def.max_tier, zone)`,
  checked in that order and both before the materials check. A structure at
  its zone ceiling stays *listed*, or all of zone 1 would lose the Upgrade row
  entirely.
- **`DigSite` is the second non-`Structure` entity carrying a base-space
  `Position`**, after a posted program — so `Structure` being the space tag
  no longer answers "which space is this?" on its own. Its cycle is
  `Game::run_dig_crew` and not a bevy system, because `haul_step_system`
  needs a `Structure` and `task_progress_system` a `ResourceNode` and
  `Stock`, and because `strike_rock`/`floor_cell` are the one door each to
  damaging rock and laying floor. The walk is shared —
  `hauling::step_to_post`. **The trap is `base_entropy_system`**: the party's
  cell is `Locale::Base`'s coordinates, never the player's `Position`, while
  a posted program's cell *is* its `Position`, and its query excludes
  `Player` because the player can hold a `Task` too.
- **A mark is one verb and the cell under it decides what it means** —
  marked solid means cut, marked `Open` means floor, and the mark outlives
  the cut. `toggle_mark_box` reads the **anchor cell** to decide mark versus
  clear, which is why there is no erase verb and no `Mode` field on
  `DigSite`. `Floor` takes no mark. The trap is `Durability`: entropy
  reverts to *solid*, not chipped, so `strike_rock` refills a spent meter or
  the next swing opens a whole wall for free.
- **A dig site's two unreachable states are not symmetrical.** `BoxedIn` is
  silent (it is the normal interior of any marked block and resolves itself);
  `NoRoute` complains **once**, latched on `DigSite::announced_stuck` by
  `announce_dig_cut_off`, per `set_machine_status`'s only-on-transition rule.
  The latch is not saved: a reload should say it again. **Both are answered
  above the truncation** — see the entry below — so the announcer is only
  ever reached with a face to stand at, and the assignment loop's own skip
  is silent for either.
- **Dig wants are appended last in `schedule_base_labour`, and the priority
  *is* the position in that list** — `truncate(staff.len())` cuts from the
  end, so anything inserted above them silently starves production.
  `dig_wants` is structural like `feeders_for`, never a stock count, and
  sorted by tile. **The trap is any reachability question left below the
  cut**: unworkable cells sort first, `continue` costs no body when their
  turn comes, and the rim the crew could have been sent to is cut off the
  end of the list — a plan with a crew standing idle in front of it and
  nothing said. It bit twice. `hauling::has_station` drops the boxed-in
  interior in `dig_wants` (the half of `NoPost::BoxedIn` that does not
  depend on who is asking, four grid lookups, no walk); `NoRoute` was left
  below the cut on the argument that it announces itself, and a sealed
  pocket or a plan past `haul_walk_radius` starved the same way. Both now
  drop in the block above the cut that already dropped unreachable build
  requests, and `can_walk_to_dig` is gone — with the announcement moved out
  it was `can_walk_to_post` renamed. **The trap in the fix is cost**: one
  `post_route` per want is a Dijkstra field per face times a hundred-cell
  plan, every tick, permanently. `hauling::crew_reach` builds the field from
  the *body* once and `hauling::reaches` makes each want a lookup — one walk
  for the whole scheduler in a connected base. Its box is centred on the
  body rather than the face, which is identical up to base radius
  `HAUL_WALK_MAX_TILES / 2` and only *drops* a workable want past that, so
  `post_reach` stays the authority at the posting itself.
- **Mining does not go through `battle::resolve_attack`**, for
  `attack_nest`'s reason: rock cannot dodge and identical swings must land
  identical damage. `swing_damage` is shared with the crew, so a stronger
  program digs in fewer swings rather than faster ones —
  `BASE_DIG_TICKS_PER_SWING` is the rate, not the bite.
  `BASE_ROCK_DURABILITY` is **never** scaled by zone, depth or level: what
  changes over a run is the player. The one draw is the fragment roll,
  bounded above by what flooring the cell costs.
- **A cost the *base* incurs is walked to and carried; a cost the *player*
  incurs is paid from their pack.** The crew's tile is fetched off a shelf
  by a body that walks there and carries it back — the same `Carrying` a
  hauler uses, over the same buffers the stock strip counts. Depot-only was
  rejected for the draw (it makes the strip a lie about what the base can
  afford) and kept for the **put-back**. `Game::lay_tile` is deliberately
  untouched: a player verb pays the way every player verb does, and the pack
  is **not** a fallback for the crew. **Two traps.** Silence: a crew with
  nothing to lay leaves a marked cell, a posted body and no news, so
  `DigSite::announced_dry` says it once beside `announced_stuck`. And a body
  holding a load for a cancelled job: `schedule_base_labour` may never free
  one, so `DigErrand::Return` walks it back and gives the post up there.
- **A dry floor job is not a want either — `build_wants`' deadlock rule
  crossed over**, `Game::dig_wants` asking `build_is_workable`'s question on
  the half of the one dig verb that spends anything. Gated on the *cell*
  (`grid.is_solid`), never the mark alone, or a drought stops every cut job
  in the base too. `Game::announce_dig_dry`, called from `dig_wants`, is the
  one writer of `DigSite::announced_dry`, both the set and the clear. **The
  trap the build side never needed**: `schedule_base_labour`'s "quiet base"
  guard tests `wanted.iter().all(posted.contains)`, vacuously true whatever
  `posted` holds the tick a dry site's drop empties `wanted`. `queue_is_empty`
  also reads whether any `DigSite` is `marked` at all, the same "a base with
  instructions" carve-out a `BuildSite` on order already gets.
- **What a program needs is a catalogue, and `assets/needs/` deleted is the
  pre-needs game.** `needs::NeedDb` is `MemoryDb`'s seam again — nine
  required fields, an absent directory loading silently empty, `iter` sorted
  by id because every caller walks it. Nothing is seeded, nothing drains, no
  body leaves a post and `strain` answers zero, all by arithmetic rather
  than by a branch. Never gate a system or a screen on the db being
  non-empty.
- **`OffShift(NeedId)` is the only thing this feature stores, and hysteresis
  is why.** In below the def's `critical`, out at its `content`, and the gap
  between them is the feature — read off the current value alone a body
  flickers on and off its post every tick at the boundary. Everything else
  is derived, `hauling::Errand`'s rule. Both save fields are additive behind
  `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump. **Seeding is one
  site**, `Needs::seed_missing` at the top of the drain, covering a fresh
  program, a def added between sessions and a pre-needs save at once.
- **One gate decides whether a need may pull a body off a post, and failing
  it *is* acting out.** Below `critical`, something services it, not
  latched. **Reachability is never asked as its own question** — the walk
  (`step_off_shift`, riding `hauling::step_to_post`) discovers it, and
  `NoRoute` latches the need and drops the marker. Asked up front it is
  insert → failed step → remove → insert every beat forever. The two failure
  halves share one latch and one `frayed_here` grudge but say **different
  sentences**, `BoxedIn`-versus-`NoRoute`'s rule one level up.
- **An off-shift program leaves the *posting* half of
  `schedule_base_labour`, not the drift half.** `drift_idle_staff` keeps the
  whole staff list — it is what walks the body to its amenity — while
  `record_labour_demand`, `truncate` and the standdown guard read
  `on_shift`. The one exception is a `Carrying` holder, the existing
  never-free-a-`Carrying`-holder rule and not a second one. The header's
  shortfall *grows* while bodies are off shift; that is the readout.
- **`idled_with` is an edge, never a period** — written when a serviced need
  reaches `content`, naming everyone else in reach of that amenity.
  `note_postings`' cost applies unchanged: a per-tick writer saturates
  `strike_cap` in three ticks and makes eviction eager for exactly the
  programs living the most.
- **`needs::strain` is a free function and `need_shift` has its own cap.**
  `party::role_of`'s reason for the first — a bevy system has no `Game`, and
  two folds would disagree about whether an unresolvable def counts, which
  is what the whole empty-catalogue property rests on. For the second,
  `MEMORY_MORALE_MAX_SHIFT`'s: the outer `clamp(0.0, 1.0)` exists because
  `random_bool` panics, and it would swallow an uncapped overshoot where no
  test reading the finished chance could see it. Extraction only, matching
  where `morale` reaches. Signed around **zero**, so full reserves, the
  player and a deleted catalogue all contribute nothing.
- **The need rows share the manifest's WORK box and cost a row elsewhere.**
  A NEEDS box did not fit at 1280x720 even at two rows, so `MAX_BAND_ROWS`
  went 4 → 3 to pay for them; `MAX_NEED_ROWS` trims needs *before* the box's
  own cap, or a modded catalogue pushes the post row off the end. Banded in
  **words** (`views::need_band`), sorted by need **id** and never by value.
  `program_errand_label` folds into `program_activity` above the post — a
  body off shift holds no `Task`, so without it the roster calls it "idle".
- **Neither shipped amenity has an upgrade path, deliberately.** A
  `StructureTier` buys an amenity nothing — `per_tick` is not scaled by it —
  so a priced upgrade row would change no number the player could find.

- **A Forgiving death benches a program and `Game::bench_or_dissolve` is the
  one door**, `dissolve_tamed_program`'s own argument one level up: the
  `DifficultyMode` branch is written once, not at each of the two death sites
  (`end_battle`'s dead-party loop and the raid defender). The benched program
  keeps `Tamed`, HP 1, `components::Downed` — **and its roster slot**, which is
  what makes a wipe cost something under Forgiving. Selling it or extracting a
  routine are the two things that free the slot, and `add_companion`'s refusal
  names both. `end_battle` stays the only legal removal point.
- **A Bay's field is `recovery:`, because `structures::RepairDef` was already
  taken** — that one restores structure `Durability` per tier. `RecoveryDef` is
  **`i32`, not `PowerRegenDef`'s `f32`**: `Stats::hp` is an integer, so it needs
  only half that type's clamp (negatives floored in `rate()`, no non-finite
  case). `radius` is Chebyshev and the shipped Bay authors `0` — standing on it.
- **`Game::run_repair_bays` is a `Game` method, `run_dig_crew`'s reason**: the
  line names the program through `creature_label` and heals through
  `restore_hp`, and a bevy system would be a second copy of the first. **The
  scan centres on each downed program's own `Position`** — the whole of what
  differs from `power_regen_system`, which centres on the party because it
  serves the player, so don't copy its `Locale` early return. Query and `Bays`
  are both **sorted by tile** (`min_by_key` takes the first equal minimum), and
  the marker comes off and the line is logged **only at full Integrity**.
- **`components::Downed` has two writers and they mean the same thing by it**:
  `bench_or_dissolve`'s Forgiving arm, and `Game::admit_the_badly_hurt` for a
  staff program under `BAY_ADMISSION_HP_FRACTION` (0.20). Before the second, a
  Bay served *corpses* — so a raid's surviving defender had no route back to
  full, and under Permadeath a Bay was inert. **Insertion is all the second one
  does**: the `on_shift` filter, the diff's unconditional free,
  `drift_idle_staff`'s Bay arm and `run_repair_bays` all already key on the
  marker, and the map's `+` follows through `Bays::serving`. The shape to
  refuse is a parallel `Mending` component — an edit at all four sites, a fifth
  state to disagree about, and a save field, for two states that want identical
  treatment. `Staff` alone off `program_role` (a `Sortie` is away and cannot
  walk; it is admitted the beat after it comes home), and **refused outright
  while no Bay stands**, because the marker is a one-way door without one.
  **One threshold, not two**: release is `hp == max_hp`, the exit
  `run_repair_bays` already had, so the flicker gap is the whole bar and a
  hysteretic pair would be a second way out of one state. Low on purpose — it
  pulls a working body off a machine. **A Bay has no capacity**: everyone in
  reach mends at full rate on the same tick, which matters now that a stay is
  the whole 20%-to-full climb rather than a corpse's moment.
- **A downed program walks itself, and the `Downed` arm of `drift_idle_staff`
  sits above the `OffShift` arm** — recovery outranks an amenity — **gated on
  laid floor**, which is what keeps `entry_tile` the one arrival path for a
  program downed in the Stack. `Game::step_to_repair` is `step_off_shift`'s
  shape on the same walk, and **`Err` holds rather than dropping the marker**:
  nothing re-inserts `Downed`, so there is no flicker to stop, and dropping it
  would silently heal a program that could not reach a Bay. No Bay standing is
  `NoRoute` at the first line — a benched program lies where it fell.
- **`Downed` joins the `on_shift` filter without the `Carrying` escape**, and
  is freed in the diff **unconditionally, ahead of every keep rule**. The
  `Carrying` exception exists because freeing a loaded body destroys the goods;
  a body that just died is going to the Bay regardless. `LabourDemand`'s
  shortfall grows while it is down, as it does off shift.
