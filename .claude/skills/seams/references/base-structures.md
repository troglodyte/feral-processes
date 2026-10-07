# Seams: The base: structures & building

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
  the party founded from through `Game::move_anchor_to` — its only caller
  now that the world is persistent and a breach no longer moves the
  anchor along with a reseeded zone. **The one refusal it needed is a Stack
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
  as a bare slab. And the bill is quoted through `Game::upgrade_cost` and
  `build_cost_display`, the pack no longer being the store the verb reads —
  on the program picker since `Mode::UpgradeDirection` replaced the upgrade
  list, because a prompt aimed at a *tile* does not know the machine yet.

- **A drop pod's recharge is `Upgrade`'s build request with a third goal,
  and it cannot be cancelled.** `Game::spend_pod` files `BuildGoal::Recharge`
  on the terminal's own tile and `raise_one_tick`'s `Recharge` arm is the
  only writer of `DropPod::charged = true`, so every one of `Upgrade`'s five
  traps applies. The trap is `cancel_build_request`: it was the one door that
  left a standing terminal spent with nothing filed, and no screen re-files
  one — a pod that silently never fires again. `views::PodState` has no
  "spent, nothing filed" state for that reason, and its percent is a *call*
  to `BuildOrderRow::percent`.

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

- **A build costs a tamed program exactly when the structure runs a job**,
  and both exemptions fall out of that one rule. `StructureDef::needs_program`
  is `runs_a_job()` — `work`, `assembles` or `strips` — so a body is spent on
  the thing a body will afterwards be posted to, and a Shield, a Patch Node, a
  Relay, a Data Cache, a shelf and the Zone Portal are all free for the same
  reason rather than each for its own. The Home is exempt a second time by
  `category()`, so an edited `home.ron` declaring `work:` can never leave a
  fresh run — zero programs owned — unable to found a base.
  `Game::structure_needs_program` is the id-shaped door onto it and app-core's
  build flow routes the picker off that exact call — a frontend deriving its
  own version (it was `category() == Home` once, which was *true* until the
  second exemption shipped) strands a free structure on a picker with nothing
  to confirm. **Two traps were closed in turn and the second is the one to
  keep in view.** First `stores` meant two things — a hauler may empty into
  it, *and* it costs no body — which made a modded haul target free whether
  its author meant it or not; the fix was an authored `costs_no_program`
  flag. That flag then made the exemption *content*: a seventh depot file
  that forgot the line silently cost a program, and every ornament had to
  remember to say it. Deriving off the job closes both, at the price a mod
  can no longer build a bench that costs nobody — deliberate, because the
  cost is a rule about the economy and not a property of a structure. A
  Portal was the case that forced the flag and is still free: it is despawned
  by `enter_next_zone` the moment it is walked onto, so the committed body
  would die with the doorway. An exempt build is still a *request*:
  materials, crew, ticks, all unchanged, and it is the first `BuildSite`
  reachable with `program: None`, which is why `return_build_holdings` and
  both `build_quality_of` call sites already had to handle the `Option`.
  `the_shipped_program_cost_follows_the_job` in `tests/assets.rs` is the
  census, written over the fields a structure authors for some *other*
  purpose so it stays a statement about content rather than a re-derivation.

- **A build order commits one tamed program at filing, and that is the one
  exception to "nothing is charged at filing."** `commit_program`
  (`game/party.rs`) retires the program the moment the picker confirms —
  `Party::retain`, then `world.despawn` — and `BuildSite::program` carries
  the `CreatureSave` snapshot rather than an `Entity`, `HopperEntry`'s
  reason for carrying a `DownedProgram` the same way. Materials survive a
  cancel because they are still standing on the cell; the program does not
  exist any more, so getting it back is `refund_program` respawning it off
  the snapshot — a resurrection, not a return — with its original
  `ProgramId` intact, or every memory naming it orphans. **The trap: both
  destruction doors must call `return_build_holdings`.**
  `cancel_build_request` (the player calling it off) and
  `clear_pending_build_at` (`damage_structure`'s destroyed branch and
  `remove_structure`, Home cascade included — the cell going out from under
  the request) both have to route through it, or the door left out silently
  despawns a committed program with no line anywhere accounting for it, and
  nothing fails to compile. The last-program guard checks the **whole
  roster**, not `programs_for_build(tier)` — a base can pass it holding one
  staff program and one partied one, spend the staff program, and be left
  with nobody to post to the site, so zero owned is the line rather than
  zero eligible.

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
  derived from the stored cost and never stored beside it. If the program
  committed to a request is ever shown on a screen, it goes through the same
  derivation — `BuildSite::program` is data for the refund, not a display
  record, and a copy of its name stored on the row is the same trap a stored
  `required_ticks` would be. **Two things a widening must not break**: "one
  builder at a time" is the scheduler naming a site once and not a count on
  the component, so a second builder costs no save bump; and
  `TaskKind::Construct` is the first kind whose holder may be *carrying*, so
  `schedule_base_labour`'s never-free-a-`Carrying`-holder rule is
  load-bearing for it.

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

- **The raid clock is spent by a sweep, not by reaching its threshold, and
  both floors hold the pressure rather than forgiving it.**
  `resources::RaidPressure` accrues `RAID_PRESSURE_PER_ZONE * zone` a tick
  against an interval drawn once per sweep, replacing a `0.012` per-tick roll
  that fired every 42 seconds of play — too frequent for any one sweep to
  carry weight, and memoryless, so no stretch could read as safe or
  threatened. Three holds, and each closes a different trap. `RAID_MIN_ZONE`
  gates **accrual**: as a gate on firing, sector 1 would build pressure it
  could never spend and a player would be swept within a tick or two of
  breaching into sector 2, punished for the quiet the exemption had just
  granted. `RAID_MIN_BASE_STAFF` gates **firing** and the clock holds — under
  the roll a base below the floor dodged every draw, so benching your own
  crew was a way never to be swept; held, the sweep waits for a base that can
  absorb it, which is what the floor was always for. And `run_raid` returns
  `bool` so a tick with nothing standing to sweep holds too: resetting on a
  no-op rewinds the meter every tick a base is bare, so the first machine a
  player raises would buy them an interval they had not served. The jitter is
  on the **target**, never the accrual — summed over ~2400 ticks the law of
  large numbers flattens a per-tick jitter to within a percent of its mean
  and the clock is a metronome again. `RAID_PRESSURE_WARN_PERCENT` is read
  off the *drawn* target, latched on `warned`, or the line is an inequality
  said twice a second for four minutes. `SETTLEMENT_RAID_CHANCE_PER_TICK` is
  coupled and moved with it: its own doc calls it the rarer of the two
  events, and against a 2400-tick clock the old `0.006` was fourteen times
  *more* frequent than the sweep. **Unmeasured** — `balance_sim` has no raid
  term and `tuning.rs` says no instrument here models one, so every figure
  was reasoned from the tick rate rather than measured.

- **An outpost's raid roll rides `raid_check`'s firing branch, after
  `run_raid`, so it inherits `RAID_MIN_ZONE` and is invisible to
  `dev_force_raid`.** `Game::run_outpost_raids()` sits inside the same
  branch `raid_check` reaches only once the base's own sweep has actually
  fired — a deliberate placement (design spec §8, correction 9) over the
  design spec's first draft, which gave outposts their own independent
  roll. Riding the branch means `RAID_MIN_ZONE`'s zone-1 gate at
  `raid_check`'s own entry applies for free, so a zone-1 outpost is never
  raided — intended, and stated on the outposts help page. It also means
  `dev_force_raid`, which calls `run_raid()` directly and deliberately
  bypasses `raid_check` (its own doc comment says so, for the console), never
  reaches `run_outpost_raids()` either — an outpost reads as unraidable from
  the console, which is a real omission and not a bug. **The trap**: "fixing"
  that gap by calling `run_outpost_raids()` from inside `dev_force_raid` (or
  from `run_raid` itself, which both paths share) would restore console
  coverage at the cost of making outposts raidable in zone 1, since neither
  of those has `raid_check`'s own zone gate. The fix for missing console
  coverage is a second, explicit dev hook that mirrors the gate, never a
  shared call site.

- **A raid's flash is base-space too, and `render/base.rs` gates both draw
  sites on `base_pos`.** Every `VisualEffect` names a structure's tile, so
  the queue is base-space by construction — ungated, `tile_flash` and
  `draw_bursts` painted a raid onto whatever surface ground shared those
  numbers, usually the party's own tile. Suppressed rather than moved to the
  anchor: the log pane's flash and the `Raid` line already carry the news.
  `VisualEffect` has **no** space tag on purpose — one variant is not an axis.

- **A deploy is refused onto a cell a body is standing in**, the third refusal
  on `place_structure`'s ladder and its own for the other two's reason: this
  cell needs a moment rather than a demolition or a cancelled request. **The
  program being spent on the build is exempt**, and it is the one that has to
  be — `commit_program` retires it below, so a refusal naming it names a body
  that would not have been there, and it is the likely one, since the picker
  offers the whole roster wherever it is standing. Skipped while founding,
  `is_floor`'s reason.

- **Destroying a structure has two paths** — `damage_structure` and
  `remove_structure`. Anything that must happen as a structure comes down
  needs wiring into both.

- **A demolition hands back what the structure was *holding*, and a sweep
  does not** — the one deliberate exception to the rule above, so a reader
  who knows it does not read the omission as the bug it usually is.
  `remove_structure` drains `Stock` through `drain_stock` and a rack's shelf
  through `return_racked_programs`; `damage_structure` destroys both. The
  argument is that a GC Entropy Sweep is a *loss*, and handing the shelf back
  makes losing a building partly a payday — so salvage follows the 30% build
  refund, which is already demolish-only, rather than following
  `return_rig_tool` and `return_carried_program`, which fire on both paths.
  **Both `Stock` maps**: a machine's `input` is ingredients a hauler walked
  over and set down, the same units the player paid for as the `output`
  beside them, and draining only the visible half is the same omission one
  level down. **`LootSource::Salvage` rather than `Refund`**, because a
  refund is build cost the demolition mints while salvage is stock the base
  already acquired once through some other source — folded together,
  demolishing a full Depot reads as a materials windfall on any analysis of
  where a run's resources come from. Inserting the variant mid-enum is safe
  only because `LootSource` derives no serde and reaches the wire solely
  through `as_str()`, unlike `Perk`, whose variant order *is* save format.
  The two are merged again for the one "You recover ..." sentence: which
  half was build cost is a distinction only the ledger has a use for.
  **`return_racked_programs` deliberately skips `return_carried_program`'s
  first rung** — a Home cascade demolishes every rack, so a sideways move
  into another rack would make a program's survival depend on despawn order
  inside the target loop. It keeps the third: `DownedPrograms` is capped
  where `Inventory` is an unbounded `Vec`, so this return can genuinely
  refuse and says "is lost with the machine" rather than going quiet.
  Verified by deletion, the bar that entry sets for a fourth site.

- **A structure's upgrade tier is bounded twice**, `min(def.max_tier, zone)`,
  checked in that order and both before the materials check. A structure at
  its zone ceiling stays *listed*, or all of zone 1 would lose the Upgrade row
  entirely.

- **A structure's `footprint` is a claim on placement and a derivation on
  read**, never stored, and the anchor is the one cell that blocks. That
  asymmetry is what made "same structure id" cost nothing: a legacy 1x1
  Research Node already occupies the cell it keeps, so it stands unrefused
  with a pen it can never use, and there is no migration and no
  `SAVE_FORMAT_VERSION` bump. The trap is that the reach machinery measures
  from an *anchor*: `blocked_tiles` emits anchors alone while
  `footprint_tiles` emits every cell, and the two are deliberately not
  interchangeable, so `has_station` calls a Station's own floor taken while
  `station_tiles` would let a body stand there — and `drift_idle_staff`'s
  wander is the third reader of that split, having shipped on the wrong side
  of it. `at_station` had to grow the
  same exclusion — two adjacent footprint cells are each other's orthogonal
  neighbours, so a worker standing on a Station's own floor read as *at* it,
  and the equivalence test against `station_candidates` is what caught that.
  Every call site that threads a literal `side: 1` is asserting its
  destination is single-celled; a census holds the four amenity ones, and
  `footprint: 0` is rejected at load because an empty cell list makes every
  placement refusal pass vacuously.

- **A structure remembers how well it was built, and absent means neutral.**
  `components::BuildQuality`, written only by `Game::spawn_structure`'s
  fourth argument and by `raise_one_tick`'s upgrade arm (which **overwrites**
  — not the average, not the better), read only through
  `Game::cycle_ticks_for`. **The trap is the absence**: the Home costs no
  program, every hand-spawned fixture carries none, and a pre-feature save
  has no key — so the read is `map_or(1.0, …)` and a reader that treats
  absence as an error breaks the whole existing suite. **The second trap is
  the load**: `Game::load` rebuilds several structure components off the def
  rather than the file (`Stock::capacity`, and `ResourceNode::level` already
  needed carving out), but the program that raised this machine is gone —
  re-deriving means silently resetting every machine to neutral on reload.
  Caught by a **file** round trip, never a RON one.

- **The build term goes inside `work_ticks_at_speed`, not at its callers.**
  `class_scale`'s own argument: the fourth parameter is the raw quality and
  the scale is computed in the function, because there are two callers — the
  live rate and the picker's preview. `speed` is the *posted worker's* and
  `build_quality` is the *builder's*, and they **multiply**, which is why
  `BUILD_QUALITY_TICK_WEIGHT` is half of what `WORK_TICKS_PER_SPEED` is worth
  over the same range. **The trap is quoting a percentage**: the function
  rounds to whole ticks and floors at one, so on a short cycle a percentage
  promises a change that does not happen — `views::BuildEffect` carries two
  whole tick figures out of the same call instead, and `NoCycle` is a
  separate variant because `Cycle { shipped: n, built: n }` is the *right*
  answer for a two-tick machine and the wrong one for a Depot.

- **`Potential`'s two build rolls are deliberately independent of the four
  combat rolls, and `quality_percent` still folds only the four.** An
  Excellent fighter that builds badly is the tension the feature exists for.
  `Potential::roll_label` is the one five-rung ladder both they and
  `quality_label` speak. **The trap is the RNG stream**: `roll_potential`
  going from four draws to six moved every seeded spawn, and ten tests were
  re-baselined by seed or fixture, never by assertion.

- **A held program is `ProgramRole::Siphoned` and grid supply is counted off the
  `Siphoned` marker, so removing the marker is the whole release; every despawn
  path for a siphon must release first.** `release_siphon_at` is the one
  release a despawn calls: `remove_structure` and `damage_structure` (a raid
  destroying the siphon) both despawn the structure, and a path that skips it
  leaves the program holding a dangling `Entity` forever, supplying nothing and
  never freed. The plan found the second path (raid destruction) that the spec
  named only as deconstruct.

- **Rooms are derived per call and never stored, and a door is the only structure anchor the crew can walk through.**
  `rooms::of_world` flood-fills the live floor each time it is asked, so a
  build, a demolish, a raid or a dig changes the answer with nothing to
  invalidate; a `Rooms` Resource would go stale and feed amenity refill,
  workshop time and the room thoughts a wrong band without a sign. Callers
  compute once per pass and hand the result down (`Surroundings`,
  `SiteScales`). `StructureDef::door` marks the one non-barrier anchor
  `blocked_tiles` skips; every other anchor still blocks, and a door cell
  bounds a room like a wall does. Nothing is saved: `door` and `room_tags` are
  asset-side, so `SAVE_FORMAT_VERSION` did not move.

- **`[c]` is the interact dispatcher: `Game::adjacent_interactions` lists Transfer and each awaiting rebuild site beside the party, and the key handler dispatches off that list, never its own adjacency check.** A second adjacency test in app-core would drift from what the engine offers; several results ask for a direction.
- **Destruction records a ruin in `damage_structure` (not `remove_structure`, which is the player's demolish), and `file_ruins` files it as a rebuild site only when no siege runs.** Filing mid-siege would raise a build site on a board still being fought over; a raid files on the next tick.
- **Build sites are raised lowest production level first (`StructureDb::level`, derived at load from what a structure makes and needs, never authored in assets).** A stored level would let a mod disagree with its own recipes.
