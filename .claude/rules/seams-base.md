---
paths:
  - "**/base*"
  - "**/base/**"
  - "**/hauling*"
  - "**/systems*"
  - "**/structures*"
  - "**/rock*"
  - "**/needs*"
  - "**/duties*"
  - "**/research*"
  - "**/power*"
  - "**/assign*"
  - "**/collect*"
  - "**/disposition*"
  - "**/party*"
  - "**/building*"
  - "**/depot*"
  - "**/transfer*"
  - "**/lifecycle*"
  - "**/memories*"
  - "assets/structures/**"
  - "assets/needs/**"
---

# Load-bearing seams: The base

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **A deploy is a *request*, and the Home is the only build the player's own
  hands finish.**
- **A build costs a tamed program exactly when the structure runs a job**, so
  a Shield, a shelf and the Zone Portal are free of it for one reason rather
  than each for its own.
- **The Home is free, and the anchor lands on the tile the party founded
  from while the Home itself still stands on `BASE_EXIT_CELL`** —
  `Game::move_anchor_to` is the one writer, and founding is its only caller
  now that a breach no longer relocates the anchor.
- **A zone-portal line is ramped from the zone it was introduced in, and
  `build_cost` is `min_zone: 1`.**
- **Upgrading is a build request too, and `BuildSite::goal` is the whole of
  the difference.**
- **A drop pod's recharge is `Upgrade`'s build request with a third goal,
  `BuildGoal::Recharge`, and `cancel_build_request` refuses it.**
- **`Game::spawn_structure` is the one place a structure's component list is
  written**, `roster_parts`' argument on the other roster: two callers with
  nothing in common, and nothing fails to compile when a hand-written copy
  drifts — a crew-built machine missing its `MachineStatus` reads as the
  base being broken.
- **Materials are not spent until the structure is raised.**
- **Priority is a want's position in `base_wants`'s list, and
  `assignment::assign_by_priority`'s bipartite matching never un-seats an
  earlier want to work a later one** — it replaced `schedule_base_labour`'s
  old `truncate(staff.len())` cut and greedy hand-out, which assumed any
  body could take any want and could strand a restricted one.
- **Build wants are *prepended* in `base_wants`, the mirror of dig wants
  being appended** — priority is the position in that list, which the
  matching fills front to back rather than a survivor list cut from the
  end.
- **An unreachable request is dropped from `wanted` before the matching
  runs, build and dig alike, through one `hauling::crew_reach` field per
  body rather than a walk per want.**
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
- **The grid cuts its fuel makers last, nearest the fuel last of all, and a
  short burner with nothing in store staffs them** — `power::grid_rung` is
  the ledger's first key and `fuel_wants`' make half never names a dark
  machine, asked of `ledger` fresh.
- **`collect::plan_adjacent_take` is the one machine-to-machine reach**, the
  assembler's pull and a supplier's fuel walking the same four tiles.
- **The raid clock is spent by a sweep, not by reaching its threshold, and
  both floors hold the pressure rather than forgiving it.**
- **An outpost's raid roll rides `raid_check`'s firing branch, after
  `run_raid`, so it inherits `RAID_MIN_ZONE` and is invisible to
  `dev_force_raid`.**
- **A raid's flash is base-space too, and `render/base.rs` gates both draw
  sites on `base_pos`.**
- **The map pane's top-left corner is one box of three sections —
  researching, base stock, downed store — drawn on the surface map and in
  base space alone, each silent when it has nothing to say and the two that
  cannot be cut claiming their height before the stock rows take the rest.**
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
  neighbour — derived per lookup because cutting a cell moves it.
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
  `base_wants` appends them in that order and `assign_by_priority`'s
  matching does the rest, so there is no sort and no score.
- **A satisfied standing order is skipped, not removed** — `index += 1`, the
  branch a stalled order already takes.
- **A work order's priority is its position in the queue, and
  `Game::move_work_order` is the one way to change it** — `queue_work_order`
  appends a player's order and files a research project's on top.
- **`schedule_base_labour` decides the whole assignment by priority and then
  diffs it.** Filling greedily around existing postings leaves a body on a
  standing job while an order goes unworked.
- **How short of bodies the base is, is a per-duty tally taken from the
  matching's own leftovers, never a truncated want list.**
  `LabourDemand::unworked` counts a want left unmatched by
  `assign_by_priority`, tallied by `unworked_by_duty` and written once a
  tick by `record_labour_demand`, read back by `Game::labour_demand` for
  the work order screen's header.
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
- **A body may cross a structure's own floor but never stop on it**, the
  wander declining `Game::structure_tiles` where `blocked_tiles` still emits
  anchors alone.
- **One body to a cell, and `hauling::blocked_tiles` takes the structures and
  the bodies as two iterators so no walk can ask the narrower question.**
- **`party::walks_the_base` is the one definition of which bodies occupy
  ground**, shared with `Game::watch_position`.
- **`has_station` keeps the structures-only set**, `Game::structure_tiles`,
  because a body on the only face of a marked cell is proof something can
  stand there.
- **Sharing a cell outranks every errand in `drift_idle_staff`**, which is
  what makes one-body-to-a-cell true of a save written before it.
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
- **A demolition hands back what the structure was *holding* — both `Stock`
  maps and a rack's shelf — and a sweep does not**, the one deliberate
  exception to the rule above.
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
- **Dig wants are appended last in `base_wants`, and priority is still the
  position in that list** — `assign_by_priority` fills wants front to back
  and never un-seats an earlier one, so anything inserted above them
  silently starves production.
- **Mining does not go through `battle::resolve_attack`**, for
  `attack_nest`'s reason: rock cannot dodge and identical swings must land
  identical damage.
- **A cost the *base* incurs is walked to and carried; a cost the *player*
  incurs is paid from their pack.** The crew's tile is fetched off a shelf
  by a body that walks there and carries it back — the same `Carrying` a
  hauler uses, over the same buffers the stock block counts.
- **A dry dig job is not a want either — `build_wants`' deadlock rule
  crossed over**, and `Game::drop_dry_dig_wants` is where it is asked.
- **A cut claims the tile that will hold it**, so the substrate is one
  budget claimed in want order — open cells before solid ones, and settled
  *after* the unreachable drop rather than inside `Game::dig_wants`.
- **The dig plan asks for its own tiles, through `Game::sync_dig_order`** —
  one standing order flagged `WorkOrder::for_dig`, sized to the budget's
  total claim, filed at the bottom and never cancellable by hand.
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
  `schedule_base_labour` (`assign_base_labour`'s `on_shift` filter), not
  the drift half.** `drift_idle_staff` keeps the whole staff list — it is
  what walks the body to its amenity — while the matching,
  `record_labour_demand` and the standdown guard all read `on_shift`.
- **`components::Duties` is a denied-set, `DepotFilter`'s
  absent-means-everything precedent** — no component, or an empty one,
  admits every duty — **and the Base staff table's row order is
  `StaffRank`**, renumbered densely from 0 on every move.
- **A duty toggle (`set_duty`/`set_duty_column`) reassigns through
  `Game::reassign_base_labour`, the posting half of `schedule_base_labour`
  alone, and never runs the beat's clock-driven stages** (needs, morale,
  tantrums, bay admission, drift) — so a keypress with the game paused
  cannot re-roll anything.
- **The Drop Trooper column is `WorkColumnKey::DropTrooper`, never a fifth
  `Duty`**, because `Duties` is a denied-set and the marker is opt-in.
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
  asymmetry, and `Disgruntled::stranded` latches instead — re-asked every
  `RESPITE_RETRY_TICKS` rather than held until morale recovers.
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
- **A Bay's capacity is the cells in reach of it that a body can stand in,
  since bodies block**, and `RecoveryDef::radius` is the whole of it.
- **A patient with no station to stand at mills instead of holding, and
  `Bays::is_empty` is what tells that from a base with no Bay at all** —
  which still leaves a benched program lying where it fell.
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
- **A finish is a third dig mark over laid floor, stored beside `BaseGrid`'s
  cells and never in them, and `view_finishes_at` is gui's only door to it.**
- **A structure's `footprint` is a claim on placement and a derivation on
  read, never stored, and the anchor is the one cell that blocks.**
- **A pinned program is a fifth `ProgramRole` whose consequences are
  omissions, and only the rest-repair one fails to compile.**
- **The pin mark means *settled*, not *selected***, `view_pinned_at` gated on
  the subject standing on its own pen so it cannot disagree with
  `pinned_subject` or the research gate.
- **`PinMark::Strained` narrows that one answer rather than opening a second
  door, and only the body's own ink rattles.**
- **A research node's subject gate is one term both the screen and the
  selection reach through**, `research_block_with`, outside the per-`ItemId`
  memo.
- **A production line is derived from the grid every labour pass and staffed by
  one worker who switches machines only at a cycle boundary; its edges call
  `systems::feeds` over the pull's reach, and Teardown Rigs are never members.**
