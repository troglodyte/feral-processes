# Seams: The base: production chains, machines, hauling, depots & transfer

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

- **The grid cuts its fuel makers last, nearest the fuel last of all, and
  a short burner with nothing in store staffs them.** The Home's free 4 is
  the bootstrap only if it *reaches* the bootstrapper. `ledger`'s first sort
  key is `power::grid_rung` over `power::fuel_chain` (every `grid_fuel` at
  0, what it is made of below, via `work_orders::ingredient_depths`) —
  shipped, the Power Conduit at 0 and the Mining Node at 1. **A rung, not a
  flag**: as one tier, four Mining Nodes west of every Conduit take the whole
  4 and the fuel maker is dark, and a Mining Node's fragments become a cell
  only through the player's hands. The labour half lives in `fuel_wants`:
  fuel in store is *fetched*, fuel nowhere is *made* — `wants` asked
  of a one-window `WorkOrder::batch` for it — filed after every fetch and
  never filtered for darkness itself — `base_wants` drops dark machines.
  **The dark set is asked of `ledger` fresh**, never read off `resources::PowerGrid`: `schedule_base_labour`
  runs ahead of `power_grid_system`, so the cache is last tick's and empty
  on the first tick after a load — exactly when a base arrives dark.
  **"In store" is any structure's output, not the shelves alone**, and
  `haul_step_system` widens a burner's collect list to match: shelves-only
  deadlocked a real base, a Conduit out of reach sitting on a full buffer
  beside full Depots, where the make half names nothing because a full
  Conduit cannot progress. **Charge Coil is not fuel**; the Winding Node *burns* three Power Cells
  per coil, and protecting it would deepen a blackout. And a probe that
  samples `PowerFuel::ticks_left` every 100 ticks aliases against
  `POWER_UPKEEP_TICKS` and reads a healthy burner as frozen. **`base_wants`
  drops every dark `GatherResource` want before `collapse`, not only the fuel
  ones**: `can_progress` asks about stock, not the grid, so an order named a
  dark Assembly Bay with coils beside it, `collapse` sent the line's one body
  to it as the furthest downstream, and the lit Conduit in the same line —
  the one machine that could end the blackout — never got a body
  (`a_lines_body_goes_to_its_lit_fuel_maker_not_its_dark_end_machine`). A
  fixture whose line out-draws the Home now needs `stand_ample_grid_supply`.

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
  `adjacent_stock`'s `(x, y)` sort, so no existing pull moved. **The walk is
  only half the rule**: `by_tile` is a *parameter*, so which cells are
  candidates at all is whoever builds the map's decision — and the two
  callers built it differently, one from a query carrying `&Structure` and
  one filtered on `Stock` alone. `collect::feeders_by_tile` is the other
  half and its **parameter type is the membership rule**, so a caller cannot
  feed it something it has not proved is a structure; `With<Stock>` stays a
  filter at each site because `&Stock` in the tuple collides with
  `assembler_system`'s own `Query<&mut Stock>`. Nothing constructs a `Stock`
  off a `Structure`, so no test can fail here — the signature is what holds
  it.

- **A Depot with four occupied orthogonal tiles is a Depot nothing can
  reach**, and the failure it produces is `Stranded` on some *other*
  machine's worker rather than anything naming the Depot. Found rebuilding
  the `chains` dev template's supplier bank around one: a hauler has to
  stand beside a Depot to deliver, so a supplier bank ringing one boxes it
  in. **`hauling::nearest_depot` now skips a depot the worker has no
  `post_reach` to** and takes the next in Chebyshev rank, so a boxed Depot
  only strands a worker when it is the *only* candidate — a player save had
  its one free face held by an idle body hemmed in by the very worker
  waiting on it, stranding two Mining Nodes beside a second, open Depot.
  The lone-candidate case still strands, deliberately, so keep a free tile
  on a base's only Depot.

- **What the base is holding is a list over the map pane's top-left corner,
  drawn on the surface map and in base space alone.** `Game::base_stock`
  reads the same buffers `base_holding` sums, through
  `stock::output_buffers` — a second walk makes the list an opinion about
  the base rather than a readout of it — **plus every `ItemDef::banked`
  pool**, which is the one thing those buffers can never hold. Folded in by
  the flag and never by name, and `output_buffers` is **not** widened — an
  order for a banked item is refused on the grounds that no shelf holds it.
  **A row exists if the base holds any of it *or* is set up to make it** —
  `stock::producible` seeds a 0 for every deployed structure's
  `work.produces` **and** its `assembles.item`, because an assembler
  declares no `work` block at all. Deliberately not "any structure" (a Depot
  makes nothing and would seed a row for every item in the game) and not the
  researched recipe list (a bench recipe compiles into the *player's* pack,
  so its row could never move off zero). A banked pool the player has none
  of is not seeded. Ordered by item id, never by quantity; a claim about the
  base and not about where the party stands, so it needs no
  `require_surface`. The draw is `hud::stock_block`, `compass_block`'s
  sibling and built its way: a block inside the pane, from
  `layout::strip_inset` because THREAT's quad hangs into it. **Both budgets
  are measured** — a name past `NAME_COLUMN_CHARS` is cut with an ellipsis
  and rows past `HEIGHT_FRACTION` of the pane are counted as `+N more` —
  because `Painter` never clips horizontally and an over-wide name is drawn
  across the map in silence. **The gate is placement, not a check**: the
  call sits in `draw_playing_base`'s surface branch, because the Stack's
  frame-map inset owns that corner and a tactical board is the fight; the
  two negative tests fail with the call hoisted out of the branch, so keep
  them non-vacuous by stocking the base first. **The trade was a popup**:
  it rode the status bar for one reason — a centred menu buries the map and
  left the bar standing — and a menu now hides it; that was accepted for
  full names over a glossary of tags. `ItemDef::tag`/`abbrev` survive for
  the base pane's PRODUCTION rows, held unique by their census.
  **It is three sections now, not two** — `RESEARCHING`, `BASE STOCK`, and
  the player's own `DOWNED PROGRAMS` off `Game::downed_store`, which is a
  `(held, cap)` pair and deliberately not `downed_program_rows().len()`: the
  corner asks every frame and the row builder resolves a species display
  name per held program to answer a headcount. **The trap is the order the
  budget is spent in.** The stock section spends whatever it is given — it
  is the one that counts its overflow as `+N more` — so a fixed-height
  section appended *after* it is a section a well-stocked base silently
  deletes, and nothing fails to compile. Both uncuttable sections claim
  their lines off `fits` **before** the stock rows are allotted `room`;
  `a_crowded_base_does_not_push_the_store_out_of_the_block` is the gate and
  fails with the reservation removed (verified by mutation). **Every section
  says nothing when it has nothing to say**, which is what keeps the box
  honest about map it is covering: no stock, no `BASE STOCK` header; nothing
  held, no downed section; all three empty is the `None` that draws no box.
  A store reading `0/10` is not news, so the section arrives with the first
  body — which is also when the cap starts to matter. `Line::Pair` carries
  its figure's ink for one state: a **full** store takes `ATTENTION`, the
  machine-stall rule, because a full store refuses the next body the world
  hands over and `10/10` in the ordinary ink is the readout hiding the only
  number worth reading. The end-to-end gate is
  `the_base_stock_is_listed_over_the_map_in_base_space` — `draw_stock_block`
  is handed a count, so only a draw through the real `render::draw` proves
  the call site asks `Game::downed_store` at all.
  **The running research heads the same box**, from
  `Game::research_readout` — `Idle` needs a Research Node standing, a
  running project is read out whether or not one does, and `Stalled` is a
  *call* to `research_material_shortfall` rather than a second test for it.
  One box and one budget, not two stacked blocks, and the research section
  is never cut short — only stock rows are counted. The note line gets its
  own `NOTE_COLUMN_CHARS`: "needs " spends six cells of a name column, so
  cut to `NAME_COLUMN_CHARS` it ellipsised an ordinary material name.

- **`Stock`'s `output` is public and its `input` is private**, and that
  asymmetry is the whole of a chain's directionality. `Errand::Load` is not an
  exception: it is the machine's own worker loading its own hopper.
  `game/base/collect.rs::ORTHOGONAL` is the one reach rule both the player and the
  pull phase read.

- **A Depot's filter is the *denied* set and `Game::depot_accepts` is the one
  door** — `components::DepotFilter`, edited from `Mode::DepotFilter` (`[F]`
  out of the transfer picker). Denied and not allowed so the empty case is
  the default the whole feature is inert under: every Depot in every existing
  save takes anything, and `denied_items` is additive behind
  `#[serde(default)]` with **no `SAVE_FORMAT_VERSION` bump**. The component
  is inserted on the first denial and **removed again** when the last is
  lifted, so "takes anything" has exactly one representation — `StandingJob`'s
  absence rule, one arm up in the same restore loop — and on the *entity*
  rather than a tile-keyed resource, the deliberate opposite of
  `BuybackLedger`: a Depot rebuilt on a demolished one's footprint must not
  inherit a rule nobody set. **A refusal reads exactly as a full shelf**,
  which is why the feature is two `filter` calls: `give_to_adjacent` gains a
  `continue` beside its `room == 0` one, and `haul_step_system` narrows its
  `depots` list at the two points that know the item. **Three traps.**
  `take_haul_load` takes the first key *some Depot will accept*, not the head
  entry — guarding the call instead deadlocks a two-product buffer behind an
  item nobody wants. `TransferRow::can_put` is now a quantity as well as a
  permission, and **only the permission (`may_put`) creates the row**, or a
  full or filtered Depot silently deletes the line naming what you carry. And
  `c` opens the picker on an **empty** offer when a Depot stands there, since
  `[F]` is only reachable from inside it and a just-built Depot is exactly
  the one worth setting up. Nothing on the take side asks — a filter says
  what may come **in**, and `DepotFilterRow::held` is drawn beside the denied
  mark so the two do not read as a contradiction. **`return_to_depots` is
  the third door in and is exempt on purpose**: a refund is goods the base
  already owned, and `return_material`'s ladder ends in units left in the
  dust when the player is not in base space — a filter honoured there would
  let a closed shelf destroy materials while the player was away.
  `a_refund_ignores_the_filter` pins it, because it reads like an oversight.

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
  `base_wants` appends them in that order and `assign_by_priority`'s
  matching does the rest, so there is no sort and no score. The trap is
  that dropping the dedupe does *not* show up as two bodies on one machine
  — `post_worker` calls `displace_task_holder`, so the second posting
  evicts the first and the cost lands somewhere else entirely: an idle
  program and a lower-priority want left unworked that the matching would
  otherwise have filled.

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

- **A work order's priority is its position in the queue, and
  `Game::move_work_order` is the one way to change it.** There is no band
  and no sort at scheduling time — the `OrderPriority` bands that preceded
  this were retired for exactly that reason: a label decided once at filing
  went stale the moment anything else moved, and a sort would make Vec order
  and effective order diverge while `cancel_work_order`, `move_work_order`
  and the screen all take a **raw Vec index** into `work_order_report`.
  `queue_work_order` appends a player's order and inserts a research
  project's **after the leading run of `for_research` orders**, not at 0, or
  a bill files upside down; that run is the one project's alone because
  `select_research` refuses while a project is active and abandoning
  withdraws its lines. A move is a neighbour swap that returns the new
  index so the screen's highlight follows (the party line's `<`/`>` rule),
  and it touches nothing else — the stall latch rides inside the order. An
  old save's `priority:` field is ignored, not migrated: nothing carries
  `deny_unknown_fields`.

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

- **A carrier `Stranded` past `STRANDED_SET_DOWN_TICKS` drops its load on
  its own tile (`floor::drop_load`), whatever the depots hold.** A carrier
  is never freed while it holds a load (freeing destroys the goods), so a
  stranding that waiting will not fix held a body on shift for good — a
  downed-tools hauler read on shift for a whole bench run. No store is
  consulted: a stranded carrier is one no walk reaches anything from, and
  the pile waits for an `Errand::Pickup` once a route to an accepting depot
  exists again. (Before floor items, v0.15.10, it set the load into the
  Chebyshev-nearest accepting store — a teleport.) `CarryingProgram` is out of scope by construction: a
  rack's carrier never walks, so it cannot be stranded by a route. The
  timeout is long enough for a wanderer on a Depot's only free face to step
  off first. What strands a carrier now is structures across the route, a
  body on a face of the destination, a body on a non-destination face the
  only route runs through (a face is never crossed, since a walk would stop
  there), or the walk-radius cap. **A load is lifted only for a store the walk
  reaches** (`Errand::Tend` asks `post_reach` of the accepting depots), so
  set-down is the recovery from a route lost *mid-carry* and not a conveyor:
  an unreachable-only base lifts nothing. The gate reads the output first and
  asks `post_reach` lazily, nearest depot first, because Tend runs every tick
  for every at-post worker on a clogged or unattached machine. **Output that a
  depot would take but none can be reached reads `Stranded`, not `Clogged`**
  ("collect it with c" names the wrong cause): Tend re-inserts the worker's
  `Stranded` marker (keeping `since`) and `task_progress_system`'s clog branch
  maps the marker to the status, so the alert is edge-latched by
  `set_machine_status` and `note_strandings` forms one `stranded_at` memory per
  episode. Tend's `clogged` gate counts `Stranded` too, or an attached machine
  flaps back to `Clogged` and the marker is cleared every other tick.

- **`set_machine_status` is the one place a stall is announced, and it logs
  only on transition.** Three callers, so "entering a state is news, staying
  in it is not" cannot lapse in one of them.

- **"Nobody is posted here" is one pass over every machine**,
  `idle_machine_system`. It was the assembler's branch, which visits only
  structures declaring `assembles` — so an unstaffed extractor drew green.
  `task_progress_system` announces `Running` while a cycle is still ramping.

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

- **A rig holds its own tool and the player holds theirs.**
  `Game::install_rig_tool` is the one writer of `Hopper::standing_tool` and
  the strip resolves against `ToolDb`, never `installed_tools()`. Read off
  the player's slots — which it was, because the tool arrived as a side
  effect of a hand-load — pulling your own tool starved every rig in the
  base that had been loaded with it, and `set_machine_status` speaks only on
  transition so a rig that had already said `Starved` said nothing at all.
  **The asymmetry with `uninstall_tool` is deliberate**: a slot holds
  knowledge the player never lost, so re-granting a carrier there would mint
  one from nothing and make pulling the never-spent starter a way to print
  them; a rig holds the object that was carried in. Both live in
  `game/tools.rs` so the reason is in front of whoever tidies them into
  agreement. **No return has a pack-room rung** — `Inventory` is an
  unbounded `Vec` and `add` saturates only against `u32`, so the refusal
  cannot fire; that is the whole difference from `return_carried_program`,
  whose third rung exists because `DownedPrograms` is capped.
  `HopperEntry::tool` is re-stamped on every fit and so can never differ
  from the rig's tool — kept rather than deleted only because it is a save
  field, and a field *removed* is the one case field-named RON does not
  excuse from a `SAVE_FORMAT_VERSION` bump.

- **The rig's screen is `[F]`, and both of the obvious keys were wrong.**
  Not `c`: a rig declares `capacity`, so `adjacent_stock` already finds it
  and `c` opens the transfer picker there — binding the holder to it forks
  on whether the output buffer happens to be empty. Not `T` either, which
  `crates/engine/EASTER_EGGS.md` reserves for the battle taunt and throw:
  a documented feature cannot own a key no help page may name, and
  `no_shipped_help_page_names_a_hidden_key` is what fails the build.
  **And `open_rig_tool` answers `bool`** — `playing.rs`'s match value is
  `acted` and `after_world_action` clears `status_line` when it is true, so
  reporting a refusal as an action erases the sentence `App::refuse` just
  wrote. `c` may return `true` on its own refusal only because
  `refuse_transfer` speaks engine-side into the log instead.

- **A carrier in transit is carried, so the never-free rule and both
  destruction paths name `CarryingProgram` beside `Carrying`.** `Carrying`
  could not be widened — it is one `(item, qty)` pair *because*
  `HAUL_CARRY_CAPACITY` bounds a trip, and a carrier has no `ItemId`. None of
  the three sites fails to compile if it is missed and the symptom is a lost
  kill with no error. `is_on_shift` returns true for a holder, as it does for
  `Carrying`; the two destruction paths **put the carrier back** rather than
  adding it to the dropped tuple — `Game::return_carried_program`, first rack
  with room in `(x, y)` order, then the player's store, and only then "lost
  with the machine". All three are verified by deletion, which is the bar a
  fourth site would owe too. The fetch itself sits in `run_teardown_rigs`
  because that is where it cannot disagree with the strip about the hopper's
  room: both routes in go through `Game::hopper_room`.

- **A trader's buyback shelf is keyed by `(kind, tile)`, not by `Entity`**, so
  it outlives the building — and now outlives a breach too. **Breaching
  does not despawn structures**, and the world is persistent now, so the
  ledger is not wiped at breach either any more: `StackMemory` and
  `PopulatedChunks` are the two base-adjacent resources still wiped by
  name in `enter_next_zone`, not `BuybackLedger`.

- **A carrier's load is never destroyed: every path that ends a hold other
  than delivery calls `floor::drop_load`.** Before floor items each of those
  paths (post demolished, post destroyed, siphon lock-in, study, reinforcement
  call-up, off-shift free of a `Downed` body, displacement by another
  posting, joining the party, a temporary structure burning out, sale,
  extraction, permadeath, fusion) removed `Carrying` or despawned the body
  and the cargo vanished, and sortie dispatch left it frozen on a body the
  labour pool no longer sees; the stranded timeout is the one that runs inside
  `haul_step_system`, so it reaches the core through `commands.queue`. The
  off-shift free fires for `Downed` only, because `is_on_shift` keeps every
  other carrier on shift. The re-match drop in `assign_base_labour` is a
  no-op kept for safety: the matching pool excludes carriers, so a carrier is
  never re-matched away. The trap is a new path written as
  `remove::<Carrying>()` or a bare despawn: nothing fails to compile and no
  test notices, the cargo is just gone. The only removals that are not drops
  are delivery (`hauling.rs`, at arrival) and the builder and besieger loads
  (`construction.rs`, `combat_rewards.rs`), which are not hauled cargo.
  Piles merge one per tile and come home through `Errand::Pickup`, which
  lifts only what an accepting depot the walk can reach will take, trying
  every pile nearest first.
