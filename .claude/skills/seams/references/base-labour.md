# Seams: The base: labour scheduling & postings

- **Priority is a want's position in `base_wants`'s list, and the matching
  (`assignment::assign_by_priority`) fills it front to back and never
  un-seats an earlier want for a later one** — replacing the old
  `truncate(staff.len())` cut and greedy hand-out, which assumed any body
  could take any want and could strand one a `Duties` column restricts
  (the module doc's case: A takes anything, B builds only, wants `[Build,
  Dig]` — greedy gives A the build and stalls the dig; the matching gives
  B→Build, A→Dig). **Build wants are *prepended*, the mirror of dig wants
  being appended.** The trap is the empty-queue standdown guard: read off
  `WorkOrders` alone it reasons from "nobody has told this base anything"
  while somebody has, so it is gated on build wants too.

- **An unreachable request is dropped from `wanted` before the matching
  runs, build and dig alike, and that placement is the fix.** There is no
  truncation any more for a want's position in the list to hide behind, so
  a site nobody can walk to is announced rather than silently taking a
  slot and leaving the base with an idle body and an unworked order every
  tick. The check asks the **staff**, short-circuiting on the first body
  that routes, and announces once via `announced_stuck` with **no silent
  arm**. There is deliberately **no `has_station` pre-filter** in
  `build_wants` — the route check subsumes it. Testing it needs **two**
  islands: one cell with no standing room, two cells with standing room
  and no route.

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
  quiet. Both latches clear in `build_wants` and nowhere else. **A third
  trap is that this question has a second asker**: `builder_errand` decides
  what the body standing there fetches next, and reading the bill its own
  way it drifted from this one by a whole line — this side said *any*
  outstanding line had a source, that side fetched the *first* and answered
  `Errand::Dry` if it could not get that one. Every shipped bill of one item
  makes them identical; a Zone Portal's does not, and no base can ever make
  a Portal Fragment. So the site stayed workable on the strength of its
  Routine Disks, held a body forever, delivered nothing and said nothing —
  `Errand::Dry` is silent because the scheduler reports it, and the
  scheduler had just decided the site was fine. `Game::next_fetch` is the
  one definition now and both call it; `from` chooses *which* source, never
  *whether* there is one.

- **The "someone is on this job" mark goes on whichever end of a posting
  has a glyph to wear it**, and `wears_job_mark` / `structure_attended` are
  the two halves — exactly one per posted program at every instant. A
  machine wears it while its worker stands there and the worker takes it
  along to deliver; a guard is never drawn, so its structure keeps it; a
  **digger wears it for the whole job**, because a `DigSite` has no glyph at
  the other end. Exhaustive on `TaskKind`, `cell_mark`'s rule. Distinct from
  `position_is_honest`, which is whether the program may be drawn *at all*.

- **`assign_base_labour` decides the whole assignment by priority — the
  bipartite matching in `assignment::assign_by_priority` — and then diffs
  it against who is already posted.** Filling greedily around existing
  postings leaves a body on a standing job while an order goes unworked,
  which is the matching's own reason for existing; the diff on top is a
  second, narrower anti-thrash rule, for *movement* rather than for which
  wants get worked — a holder the matching kept on its own want is left
  untouched even though the whole assignment was just recomputed.
  **A body holding a `Carrying` is never freed** — freeing one destroys the
  goods — and neither is one standing on a machine with **no output room
  while a Depot stands**: a clogged machine drops out of `wanted`, and that
  body is the only thing that can carry the clog away and let it run again.
  **It stands nobody down on a base with an *empty queue*** — a run-dry base
  or a save written before work orders would otherwise be swept on the first
  tick — but that guard must be **qualified**, or it also fires on a base
  whose orders are all *satisfied* and the line runs on for the rest of the
  run. It draws no RNG at all.

- **How short of bodies the base is, is a per-duty tally taken from the
  matching's own leftovers — `LabourDemand::unworked`, never a
  wanted-count minus `staff.len()`.** `unworked_by_duty` walks the wants
  the matching left with no assigned body and resolves each to the one
  `Duty` that `admits` its `TaskKind`; `record_labour_demand` writes that
  tally once a tick alongside `wanted.len()` and `on_shift.len()`, read
  back by `Game::labour_demand` for the work order screen's header. Cached
  rather than derived because the derivation is `&mut self` and logs, so a
  screen cannot call it. The `staff.is_empty()` early return writes it too
  — the header says **nothing** at zero. A duty toggle writes it between
  ticks through `reassign_base_labour` — the posting half alone, because
  the beat's other half rolls tantrums and a paused keypress must not
  re-roll one.

- **A program's role is derived, and there is no "owned but idle" state.**
  `Game::program_role` over `ProgramRole` — disjoint and exhaustive, so a
  program you own that is not fighting beside you, not held as your weapon
  and not away on a sortie **is** base staff. There is no marker to assign
  and no verb to assign it. **The rule is `party::role_of`, a free
  function**, for `stack::surfaced`'s reason: `haul_step_system` has no
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

- **A body is squeezed past and never stopped on, and `Occupancy` is what
  holds it.** It began as one body to a cell with bodies as walls, and that
  closed corridors: a `chains` base is a ring of machines with a one-cell
  corridor round it, whoever stood in it stranded every carrier behind them,
  and since a carrier is never freed while it holds a load, a hauler stood
  2,291 ticks holding one. Bodies now cost a posted walk
  `SQUEEZE_EXTRA_TICKS` more per cell (`Occupancy::step_cost`, paid in time
  through `hauling::stride` and the `Squeezing` marker), and the heap the
  old rule broke up — 71 downed programs on one cell outside a Bay — is held
  by the half that is about **arrival**: a cell a body holds is never a
  station, and the walk refuses a body's cell wherever arriving there would
  end the walk (the target's own faces). **The trap is the walkers whose
  arrival is a radius** — a patient for its Bay, an off-shift body for an
  amenity, a respite, a subject for its pen: `in_reach` admits cells no
  station list names, and `drift_idle_staff` hands a body sharing a cell to
  the wander, so a drift walker squeezing through would be knocked off its
  route at every pass. Those and the caravan take `Occupancy::rigid`, the
  old bodies-as-walls walk. A passer that briefly shares a cell with an idle
  body makes *that* body take the wander, which is the step aside the
  squeeze wants. `hauling::blocked_tiles` takes the structures and the bodies
  as **two iterators**, `collect::feeders_by_tile`'s argument: `post_field` and
  `crew_reach` have to answer the same question about which cells are
  crossable, and a caller that could pass the structures alone would silently
  be asking a different one. Two construction sites and no more —
  `Game::blocked_tiles` and `haul_step_system`, which has no `Game` — and the
  system's copy is **grown as bodies move and never shrunk**, `held`'s rule,
  or two haulers heading for the same free cell are both told it is free.
  **Which bodies count is `party::walks_the_base`**, shared with
  `watch_position`: `Staff` less the guards, because a party companion, a
  wielded program, a sortie squad and a posted guard all keep a `Position`
  written once and never again, and base space aliases onto the zone surface
  freely enough (both origins usually `(0, 0)`) that a stale tile cannot be
  told from a live one by looking at it. **Three traps.** `has_station` keeps
  the **structures-only** set (`Game::structure_tiles`, the narrower of the
  two, and they are not interchangeable): a body on the only face of a marked
  cell is *proof* something can stand there, and counting it deadlocks the dig
  crew — the want is dropped because its own digger is standing at it, the
  digger is freed and wanders off, the want returns, and the base never cuts
  again. `post_reach`'s and `reaches`' `at_station` short-circuits became
  load-bearing, since a body standing at its own post is standing on one of
  that post's station tiles; seven tests across `tests::building`,
  `tests::chains` and `tests::power` fail without them, and an exemption
  written into `station_candidates` for the asking body was removed again —
  instrumented, it never fired across the whole suite or 1,200 ticks of a real
  106-body save, because every caller answers `at_station` first. And
  blocking alone leaves an existing heap a heap, which is the next entry.

- **Sharing a cell outranks every errand in `drift_idle_staff`.** A body that
  has *arrived* never consults a walk again — a patient in reach of its Bay
  holds there deliberately, and so does a program at its amenity — so a heap
  already in a save, or left by a structure raised on top of one, would stay a
  heap forever with every future step correctly refused. A body sharing its
  cell therefore takes the wander instead of its errand, which is the one arm
  that already knows how to decline a tile somebody else has. The tally is
  built off `Game::base_bodies` and not off `staff`, because a posted worker
  standing there is as much in the way as an idle one and is not in that list.

- **An idle program wanders the base, and laid floor is the leash.**
  `wander_step` offers one of the eight neighbours of the tile the body is
  *standing on*, or a hold, every `IDLE_STAFF_STEP_TICKS` — relative, where
  the ring it replaced was absolute. Pure, RNG-free and folded **a byte at
  a time**, `descriptions::Slot::tags`'s idiom: `derive::index` reads bit 63
  and a step counter folded whole never reaches it. **`is_floor`, never
  `walkable`** — open ground is walkable but is not the base's footprint,
  only laid floor is, so the paving is the roam limit and there is no radius
  to tune. A test
  fixture needs an unfloored cell beside the pocket or the two predicates
  agree and it proves nothing. `park_tile` survives as **`entry_tile`**,
  asked only of a body not on floor: a tamed program's `Position` is the
  surface tile it was beaten on. Three rejections: never onto a tile another
  idle body holds, never onto the party's cell, and never onto a cell any
  structure's **footprint** covers. That last one read `structures_by_tile` —
  keyed on a structure's own `Position`, the **anchor alone** — until the
  first time anyone studied a program at the keyboard and an idler parked in
  the Research Station's pen: `pin_subject` refused with "Something is
  already standing in the pen" while the Station's examine line read `Idle`,
  nobody posted, two true sentences neither of which was about the other. It
  is `Game::structure_tiles` now, so the wander is a second *reader* of
  `has_station`'s decision rather than a second opinion about it. Declining
  the cell is only half: a body already parked there is not *arrived* in
  `crowded`'s sense, so it steps off on its own next beat — pinned by its own
  test, since that is the half a save written before the fix depends on.

- **Three of the five classes do something at a post, each in a different
  system.** The Leech bonus rides the **scaled** branch only, which is why
  `CycleModifiers` carries the *class* rather than a finished bonus. The
  Bastion job multiplies mitigation that already existed. The Medic job counts
  `TaskKind::Guard` **only**.

- **`components::Duties` is a denied-set, `DepotFilter`'s
  absent-means-everything precedent, and `Duty::admits` is checked against
  a `PostDesc` rather than a bare `TaskKind`.** No component, or an empty
  one, admits every duty; `duty_admits`'s "some checked duty admits it"
  rule (not "the one partitioning duty does") is what lets a later
  `Duty::Structure(id)` column overlap `Operate` with no special case at
  the call site. The trap is `Duty::admits`'s match being exhaustive on
  **both** enums, `cell_mark`'s rule — a fifth `Duty` variant or a fifth
  `TaskKind` fails to compile there rather than a wildcard arm silently
  answering `false` for it.

- **The Base staff table's row order is `StaffRank`, and `move_staff_row`
  renumbers the whole order densely by position rather than swapping rank
  *values*.** Two programs can share a rank (a build-site refund mints one
  off the live count, which no longer includes the program it replaced) or
  carry none at all (a pre-feature save's `staff_rank: None`); swapping
  values a tie shares leaves both unchanged, and swapping into a `None`
  strands a row with nothing to compare next time. `display_order` — Staff
  section first, then Away, rank order held within each — is the order the
  table is actually built and moved in, and it is **not** the same as
  `roster_order`: an away program's rank can fall between two staff ranks,
  so `<`/`>` moving within `roster_order` would sometimes swap a visible
  staff row with an invisible away one, with nothing moving on screen.

- **A duty toggle (`set_duty`/`set_duty_column`) reassigns through
  `Game::reassign_base_labour` and never runs the beat's clock-driven
  stages — no needs, no morale rung, no tantrum, no bay admission, no
  drift.** Those advance with the clock, and `run_tantrums` draws
  `GameRng`, so running them per keypress would let a player re-roll the
  base by toggling a checkbox with the game paused. What `reassign_base_labour`
  does run — `base_wants`, the unreachable and dry drops, the matching, the
  diff and `LabourDemand` — draws nothing and reads the markers the last
  real beat wrote, so a second pass in the same tick settles on the same
  answer. Without the call at all, the table's `N!` counts and header are
  the *previous* pass's figures until the next real tick — `Mode::BaseStaff`
  spends none of the clock itself, unlike every screen that pages ticks
  through `after_tick`.

- **The Drop Trooper column is `WorkColumnKey::DropTrooper`, never a fifth
  `Duty`.** `Duties` is a denied-set, so a fifth `Duty` makes every program —
  and every old save — a trooper by default where the feature is opt-in; the
  marker is `components::DropTrooper` and `duty_admits` never reads it. The
  column is appended by `Game::work_table` only while a `DropPod` stands, and
  every reader acting on a column matches `WorkColumn::key` — an index past
  `columns.len()` meaning "trooper" is the version that drifts. Its label is
  three letters because the widest shipped row leaves exactly that
  (`the_widest_base_staff_row_stays_inside_the_popup`).

- **A production line is derived on every labour pass, never stored, and staffed
  by one worker who leaves a machine only at a cycle boundary.** `lines::lines_in`
  is computed once in `schedule_base_labour`/`reassign_base_labour` and passed
  down; a cached line goes stale on the first place or demolish and posts a body
  to a line that is gone. `lines::collapse` keeps one want per line at the active
  machine, and `assign_base_labour` counts a body on *any* member as the line's
  holder, so a move re-posts the same body; matching the exact target instead
  swaps bodies on every move. Rule 1 (a holder mid-cycle stays put) is what keeps
  half-made batches from being abandoned whenever a later machine can progress.
  An edge is a call to `systems::feeds` over the pull's own `ORTHOGONAL` reach;
  re-deriving it makes lines that move nothing. Teardown Rigs are not members
  until a rig gate exists: the scheduler never staffs one, and a rig strips with
  each hopper entry's own tool, not `standing_tool`. The holder is read from staff only, and a
  non-staff body (the player's own `work_structure`) removes only its own member
  from the line's candidates in `base_wants`, before `collapse` picks the active
  machine, so the want falls to the next wanted member. Dropping the whole line
  for any outsider stalled every other machine in it; counting the player as
  holder aimed the want at the player's machine, the outsider filter removed it, and the staff worker
  flickered on and off.
