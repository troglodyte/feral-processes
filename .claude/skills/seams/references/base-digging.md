# Seams: The base: digging, rock & the base grid

- **Base space carries its own seed, and it is not `WorldMap::seed()`.**
  `BaseGrid::seed` is minted at `Game::new` and saved with the grid. The
  world is persistent now — one `WorldMap`, never reseeded — so the two
  seeds can no longer diverge in practice, but the field stays separate on
  purpose: `BaseGrid` is out of phase from the zone surface and has no
  business reading a resource that belongs to a different subsystem.
  Before the world went persistent this mattered for a sharper reason —
  `enter_next_zone` re-minted every zone's map, so a base salted off the
  world seed would have every rock seam reshuffle on a breach, a half-cut
  wall reloading as a different kind under an already-spent `Durability`.
  `base_spaces_seed_and_its_seams_survive_a_breach` still asserts both
  halves — the world seed and the base's — but neither moves any more, so
  it also asserts the zone level advanced first, or it would pass against
  a call that never actually breached.

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
  passability for the whole map, and a rock kind may not spend that signal.
  Sectors are retired and `biome_tint` no longer rotates anything — one
  fixed reference table for the whole run — so the sharper danger this rule
  used to close (an authored hue fighting that rotation, a seam changing
  appearance on a breach) is gone with them; the passability rule alone is
  still sufficient reason on its own. `SHADE_BAND`'s floor of 1.0 is
  load-bearing: a face darker than the wall around it is harder to see than
  anonymous rock.

- **Only an *exposed* face shows its kind, and that is a display rule
  only.** `BaseGrid::is_exposed` — solid, with an **orthogonal** walkable
  neighbour — derived per lookup because cutting a cell moves it.
  `strike_rock` resolves the true kind regardless, so the
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

- **`DigSite` is the second non-`Structure` entity carrying a base-space
  `Position`**, after a posted program — so `Structure` being the space tag
  no longer answers "which space is this?" on its own. Its cycle is
  `Game::run_dig_crew` and not a bevy system, because `haul_step_system`
  needs a `Structure` and `task_progress_system` a `ResourceNode` and
  `Stock`, and because `strike_rock`/`floor_cell` are the one door each to
  damaging rock and laying floor. The walk is shared —
  `hauling::step_to_post`.

- **A mark is one verb and the cell under it decides what it means** —
  marked solid means cut, marked `Open` means floor, and the mark outlives
  the cut. `toggle_mark_box` reads the **anchor cell** to decide mark versus
  clear, which is why there is no erase verb and no `Mode` field on
  `DigSite`. `Floor` takes no mark.

- **A dig site's two unreachable states are not symmetrical.** `BoxedIn` is
  silent (it is the normal interior of any marked block and resolves itself);
  `NoRoute` complains **once**, latched on `DigSite::announced_stuck` by
  `announce_dig_cut_off`, per `set_machine_status`'s only-on-transition rule.
  The latch is not saved: a reload should say it again. **Both are answered
  before the matching runs** — see the unreachable-drop entry above — so
  the announcer is only ever reached with a face to stand at, and the
  assignment loop's own skip is silent for either.

- **Dig wants are appended last in `base_wants`, and priority is still the
  position in that list** — `assign_by_priority` fills wants front to back
  and never un-seats an earlier one, so anything inserted above them
  silently starves production. `dig_wants` is structural like
  `feeders_for`, never a stock count, and sorted by tile. **The trap is any
  reachability question left for the matching to discover on its own**:
  unworkable cells sort first, `continue` costs no body when their turn
  comes, and the rim the crew could have been sent to would otherwise fall
  silently out of the matching — a plan with a crew standing idle in front
  of it and nothing said. It bit twice. `hauling::has_station` drops the
  boxed-in interior in `dig_wants` (the half of `NoPost::BoxedIn` that does
  not depend on who is asking, four grid lookups, no walk); `NoRoute` was
  left to announce itself on the argument that it would, and a sealed
  pocket or a plan past `haul_walk_radius` starved the same way. Both now
  drop in the block, shared with build, that runs **before the matching** —
  see the unreachable-drop entry above — and `can_walk_to_dig` is gone —
  with the announcement moved out it was `can_walk_to_post` renamed. **The
  trap in the fix is cost**: one
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

- **A dry dig job is not a want either — `build_wants`' deadlock rule
  crossed over**, and **a cut claims the tile that will hold it.** Cutting
  spends nothing itself, so asked per site it is always affordable — and
  0.13.239, which made a cut claim `0` when bare ground stopped reverting,
  shipped a base that cut 112 cells and floored none. Bare ground is not
  buildable, so an unfloored cut is still a plan half-done. The substrate is
  a **budget** claimed in want order — a cut claims `1`, the tile job the
  same cut turns into claims the same `1` (a site is only ever one of the
  two), an `Apply` claims `FLOOR_FINISH_COST`, and a `Strip` claims nothing.
  **Open cells sort ahead of solid ones** inside the `finish: None` block,
  so a short-handed base floors what it has already cut before it opens
  more. **Holding the cut is only half the fix**: nothing asked the Lathe
  for substrate, so a plan with a dry shelf waited forever. The same pass
  sums every claim and hands it to `Game::sync_dig_order`, which keeps one
  standing order flagged `WorkOrder::for_dig` at that level — filed at the
  bottom only where `chain_break` passes, resized silently, withdrawn at
  zero, and refused by `cancel_work_order` because a hand-cancelled one is
  filed again next tick. The flag is provenance (`for_research`'s rule),
  `#[serde(default)]` and not skipped, or a reload orphans the line and the
  plan files a second. The deadlock rule still holds in the direction that matters — a
  dropped want frees the body, which is what sends it to the Mining Node and
  the Lathe that make the substrate — and the player's own bump is
  untouched, which is the bootstrap out of a base with nothing on its
  shelves. `Game::drop_dry_dig_wants` is the one writer of
  `DigSite::announced_dry`, both the set and the clear, and it owns both
  wordings — a held cut has its own (`DigDryReason::Cut`), since told in
  the tile job's words it reads as a cut that already happened, and it
  must not borrow the *cut off* wording either (`CUT_OFF`'s rule — two
  stalls sharing a needle is two tests each satisfied by the other's bug).
  **The trap that moved it out of `dig_wants`**: a budget claimed there
  goes to sites in tile order,
  including the sealed pocket and the plan past `haul_walk_radius` that
  `schedule_base_labour` is about to drop anyway — the unroutable-mark
  starvation, one dimension over, with the one cell a body could have cut
  announced dry instead. So the claim is settled over the assembled want
  list, **after** the unreachable drop and **before the matching runs**,
  where it also keeps a job nobody can pay for out of `record_labour_demand`.
  `dig_wants` keeps `hauling::has_station`, which is four grid lookups and
  answers for a cell on its own. **The trap the build side never needed**:
  `schedule_base_labour`'s "quiet base" guard tests
  `wanted.iter().all(posted.contains)`, vacuously true whatever `posted`
  holds the tick a dry site's drop empties `wanted`. `queue_is_empty` also
  reads whether any `DigSite` is `marked` at all, the same "a base with
  instructions" carve-out a `BuildSite` on order already gets.

- **A finish is a third dig mark over laid floor, stored beside `BaseGrid`'s
  cells and never in them** (`seam:floor-finishes` has the full argument —
  why `Tile` was not widened, why `FloorShade` is an enum, the separation
  measurement). Five traps ship with it:
  - **A finish on a non-floor cell.** `BaseGrid::set_finish` refuses unless
    `is_floor`, but nothing stops a cell from *stopping* being floor under a
    site that already exists (`BaseGrid::revert`, the only path there, walks
    it back to solid). The crew's `Apply` arm re-checks `is_floor` at landing
    and despawns uncharged rather than trusting the check it passed at mark
    time; `prune_finishes` re-checks the grid's own map at every load for
    the same reason.
  - **`DigSite` is no longer `Copy`.** `FinishOrder::Apply(FloorId)` holds a
    `String`, so every call site that used to copy a `DigSite` needs a
    borrow or a `.clone()` instead — a mechanical fix, but one the compiler
    only finds one site at a time.
  - **A finish want inserted above tile wants starves the floor.**
    `dig_wants`'s sort key is `(f.is_some(), x, y)`, not a plain `(x, y)` —
    a fixture that places its finish site at a *lower* coordinate than its
    tile site and asserts the tile wins passes by coincidence of position
    rather than by the sort key actually being exercised; the ordering test
    has to place the finish site where a naive `(x, y)` sort would rank it
    *first* to mean anything.
  - **An Apply id that no longer resolves.** A save can carry a
    `FinishOrder::Apply(id)` for a finish a mod has since renamed or
    deleted. `restore_dig_sites` drops one of these at load (the same
    warning shape `BaseGrid::prune_finishes` uses), and the crew's `Apply`
    arm checks `FloorDb` again at landing and despawns uncharged rather than
    trusting a load-time check that predates the save, or a live `DigSite`
    from before this guard existed — `BaseGrid::set_finish` itself does not
    validate the id, so skipping either check paints a name nothing can
    display onto the grid and charges for it.
  - **Opaque art hides the dim fill.** `draw_finish` paints the shade fill
    at the tile's ambient `dim`, then an edge ring and a sprite on top.
    Shipped finish sprites are opaque squares, not translucent overlays —
    tinting the sprite (and the edge) at the shade's *full* strength paints
    clean over the dimmed fill beneath it, so a finished tile stops
    answering to the Power vignette and to cloud dimming the way a plain
    floor tile does. All three layers must scale by the same `dim`.
