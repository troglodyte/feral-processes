# Seams: The base: needs, morale, tantrums, repair bays, pins & research

- **A banked resource can never clog, so a Research Node has no "full"
  state.** Its four reachable statuses are `Idle`, `Unstaffed`, `Stranded`,
  `Running`. **A node with no research project selected reads `Idle` and
  nothing new** — `Game::research_wants` simply raises no want, the scheduler
  posts nobody, and `idle_machine_system` answers. A fifth status for "there
  is nothing to work towards" would be a second writer of the same cell and a
  new variant every census has to learn.

- **Research is one project at a time, and `Game::select_research` is the one
  door — every refusal before anything is filed.** Six of them, asserted **per
  refusal** because a single test over one of six passes against the five that
  never write anyway. Two are not obvious. "No Research Node deployed" cannot
  be `work_orders::chain_break`: that function refuses every banked item by
  construction and names the research currency in its own doc as the example,
  so it would refuse the currency whatever plant was standing — it is
  `producers_of(self, &research_currency()).is_empty()` instead. And each
  material line *is* `chain_break`, quoted **verbatim**: the same sentence the
  work-order screen shows, and two spellings of one refusal is the drift this
  repo keeps recording, so the test asserts equality against a live
  `chain_break` call rather than hardcoded prose. Both together are
  `Game::research_block`, which also fills `ResearchStatus::blocked_by` —
  `Game::orderable_items`' rule, so the row the screen marks blocked and the
  refusal the player is handed cannot disagree. The refusal needs a
  **reachability** test of its own or it can ship permanent with every refusal
  test still green. Completion is `Game::settle_research`, and its two gates
  are one `&&`: the short-circuit is what stops a half-researched project
  spending its materials, and the unlock side effects are **extracted** into
  `grant_research_knowledge` rather than copied into the second path.

- **A research project's materials are ordinary work orders, and
  `WorkOrder::for_research` is provenance rather than a plan.** The flag says
  *who asked* and nothing about which machines run it — the module header's
  rule survives. `#[serde(default)]` and **not** `#[serde(skip)]` like
  `announced_stalled` above it, because a project abandoned after a reload has
  to take its orders with it; a skipped field is invisible to a RON round trip,
  so that is asserted by a **save→load** test. `withdraw_research_orders` is a
  `retain` on the **flag and never the item** — a player's own order for the
  same material has to survive — and it logs nothing, because
  `cancel_work_order`'s line would be a lie about who cancelled it.
  `research_wants` sits **between `fuel_wants` and `settle_orders`**: the
  priority is the position in that list, and the player's own pick outranks the
  queue they filed and forgot. It returns `standing_wants`' `(Entity,
  TaskKind)` and not `settle_orders`' depth pairs, since a Research Node is the
  top of its own line. **Do not touch `queue_is_empty`** — when a project ends
  on a base with nothing else queued the body stays standing at the node, which
  is the documented run-dry behaviour and is what puts it in place for the next
  project; a test pins it so nobody "fixes" it.

- **A routine node is synthesised, never authored, and "researched" means
  `KnownRoutines` contains it.** `routine_tree::synthesise_nodes` mints one
  `ResearchDef` per eligible ability (`ItemDb::synthesise_etched_disks`'s own
  shape) with `teaches: Some(ability)`, and `Game::node_researched` is the one
  door: `is_researched`, `missing_prereqs` and `select_research`'s
  already-researched refusal all read it instead of the `Research` set for a
  `teaches` node. **`listed_research` gates the whole filter on "is the tree
  open", "already known" included** — a review (2026-09-16, todo #101) took
  the spec's "Nothing is listed until the tree is open" literally over an
  earlier pass's own reasoning, which let an already-known rung bypass a
  closed tree so an old save could still see Patch Party v1.0 with
  `routine_fabrication` unresearched. A closed tree now hides that rung too;
  `the_closed_tree_lists_nothing_even_when_a_rung_is_known` pins it. The
  "hidden parent" case `research_graph` has to treat as absent (in both its
  tier and edge computation, or the Kahn pass never settles the child and a
  listed node gets no cell) is still reachable with the tree **open**: a
  known rung is listed unconditionally past a zone gate its own unknown
  parent is still waiting on —
  `a_known_child_with_a_hidden_parent_still_gets_a_graph_cell`. **A whole
  version chain needs the same zone gate as its root, not just the rung a
  spec table happened to name** — `checksum_repair`/`cold_boot`/
  `mirror_restore`/`redundancy_sync` all chain off `hot_patch`'s
  `research_zone: 2` and default to zone 1 if left alone, which
  `no_research_node_is_gated_below_its_own_prerequisite` catches as a gate
  that can never fire.

- **A downed program lives in three places now, and the rack is the only one
  that is not a queue.** `DownedPrograms` on the player, `Racked` on a
  Quarantine Rack, `Hopper` on a rig — plus `CarryingProgram` on a body
  walking one from the second to the third. The pack and the hopper are both
  *worked through*; a rack is a shelf nothing works through, which is why
  `stores` stays false on it (that flag means "a hauler may empty items into
  it", and no item ever enters a rack) and why it needed its own census
  rather than a row in the one keyed on `def.stores`. What still holds is the
  half the old two-place claim was protecting: what leaves a **rig** is plain
  items in `Stock::output`, so hauling, depots, `collect::plan_adjacent_take`
  and work orders need no instance rule. Its ceiling is `slots * tier`
  derived per read — a new pattern, since `capacity` is copied in at build
  and never tier-scaled, and a stored ceiling goes stale the moment a tier
  lands.

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
  `schedule_base_labour` (`assign_base_labour`'s `on_shift` filter), not
  the drift half.** `drift_idle_staff` keeps the whole staff list — it is
  what walks the body to its amenity — while the matching,
  `record_labour_demand` and the standdown guard all read `on_shift`. The
  one exception is a `Carrying` holder, the existing
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

- **`fray`'s two branches write different memories, and neither writes
  below the grace gate.** `frayed_here` (`BaseTile`) on the unreachable
  branch, `ran_down` (`Nothing`) on the quiet one — the withheld thing was
  always the *blame*, not the feeling, and until 0.13.131 the common case (a
  base that never built the amenity) moved morale not at all, which made
  needs decorative on the acting-out ladder. `Game::base_is_established`
  gates both: `BASE_ESTABLISHED_STAFF` **and** `BASE_ESTABLISHED_STRUCTURES`,
  development rather than ticks, because a tick grace punishes founding late
  and forgives founding early and neglecting the place. **The lines stay
  unconditional** — the player must still be told, because the line is the
  errand. A predicate wired `||` passes any test that starves both halves at
  once, so there is one test per half plus a control.

- **A bad mood is an errand, and `unwound_at` is the mechanism rather than
  the flavour.** Morale is a derived fold with no reserve to refill, so
  "recuperate at an amenity" has to be expressible as a *memory* or not at
  all: `Game::note_respites` writes a `Structure` fondness on
  `MEMORY_POSTING_PERIOD` while a disgruntled body stands in reach, and
  `Game::morale` folds it back. `note_postings`' stretch-not-edge argument
  transfers whole, and **arriving is what counts** — `in_reach` gates the
  write, so walking toward the amenity is worth zero. Reverses the module
  doc's own former claim that morale recovers by time alone; see
  `seam:a-bad-mood-is-an-errand-now-and-the-memory-is-the-mechanism`.

- **The errand is gated on there being one, and that is what keeps the ladder
  at two rungs.** `Game::on_respite` requires `Amenities::any()`, so a base
  with no amenity keeps its *sulking* programs in the posting pool — which is
  where `refuses_post` governs them. Ungated, every disgruntled body leaves
  the pool and that rung ships green and unreachable. **The two morale
  exclusions are therefore not one question**: `has_downed_tools` is
  unconditional (a program at -50 does not work, amenity or no), `on_respite`
  is not, and writing them as one clause puts a body at -50 back on the line —
  written, and caught by the disposition suite rather than by this feature's
  own tests.

- **No grudge on the stranded branch**, the one asymmetry with `Game::fray`.
  A failed walk deepening the mood that sent the body out is a loop with no
  floor, on a meter whose ladder already ratchets. The log line stays (the
  line is the errand), `Disgruntled::stranded` latches so the Dijkstra is not
  re-run every beat (`step_off_shift`'s rule), and the body rejoins the
  posting pool rather than standing stalled — this errand's whole difference
  from that one. The latch is **carried across the ratchet** in
  `update_disgruntled`, or a severity climb silently restarts the walk.
  **The latch is re-asked, not permanent**: `update_disgruntled` clears
  `stranded` every `RESPITE_RETRY_TICKS` so a route that reopens is found
  again without waiting on recovery — a real save showed most of an
  eleven-body roster stuck like this for tens of thousands of ticks. A
  separate `Disgruntled::told` (never saved, `Needs::stalled_announced`'s
  rule) survives the retry so a body still stuck after several of them does
  not repeat the line once a period.

- **`Game::is_on_shift` is the one predicate for "may be handed a job", and
  the scheduler asks it twice for one reason.** The `on_shift` filter and the
  free loop used to state the rule separately, each spelling out the uneven
  `Carrying` escape (kept by `OffShift`, `has_downed_tools` and a respite;
  refused by `Downed`, which is going to the Bay regardless). Relatedly, the
  pass's "nothing to do" early return sits **above** the free loop, so it has
  to test whether a posted body is off the line — read off `staff`, never
  `on_shift`, since the bodies it must see are the ones that filter dropped.
  Without it, a body that went off shift while the base's only instruction was
  a *standing* job (not a `WorkOrder`, so `queue_is_empty` stays true) kept
  that posting for the rest of the run.

- **A tantrum's non-lethal clamp is applied before `apply_damage`, never
  inside it.** `apply_damage` floors HP at 0 and reaching 0 *is* a kill; the
  guarantee is one expression at the call site, `game/throw.rs`'s idiom
  `raw.min(hp - 1).max(0)` on the **input**. Moving the damage calculation
  without carrying the clamp turns a bad mood into a way to lose companions
  and changes no signature. A blow the clamp takes to zero is not thrown at
  all, which is what makes `dealt`/`taken` counts of what *landed* and what
  makes a one-sided brawl reachable.

- **The tantrum step sits between `update_disgruntled` and
  `admit_the_badly_hurt`**, and that ordering is the whole of "the loser is
  swept into a Repair Bay" — the bay's one writer already runs there and
  already knows what "hurt enough to stop" means. Inside `run_tantrums` the
  three steps are advance, close, **then** open, so a fight opened this beat
  throws its first blow next beat: the alert always precedes the damage and
  `ticks_left` is an honest count. **The roll is inside the per-candidate
  loop**, so a base with nobody on the rung draws no `GameRng` at all —
  `run_routes`' predation rule. `Brawls` is not saved, and the cooldown lives
  on it rather than being derived from the aggressor's `vented` memory, which
  would be free but would leave an empty catalogue brawling unbounded.

- **`Grievance` is appended to, never inserted into.** `Ord` derives from
  declaration order and *is* the ladder the ratchet compares;
  `SaveData::disgruntled` encodes the variant **name**, so appending costs no
  format bump; and the exit side stays one `MORALE_RECOVERED_AT` comparison,
  so the ladder keeps **one** hysteresis gap however many rungs it grows.
  **`has_downed_tools` reads `>=`, not `==`** — `LashingOut` is strictly
  worse than `DownedTools`, so equality hands a body jobs again on the way
  past the rung that took them away. `vented` is catharsis and is
  load-bearing: `Disgruntled` never eases, so without it a program past -75
  fights on every roll for the rest of the run.

- **`EffectKind::Brawl` draws identically to `Hit` and exists only to carry
  sound.** Identical in all three `fx.rs` tables; the difference is
  `SoundEvent::Hit` in `crates/gui/src/lib.rs`. Sounding every base-space
  `Hit` instead would be less code and would give raids audio they have never
  had. In gui: check **before** `Fx::begin_frame` consumes the vector, call
  `sounds.play` directly (`take_sounds` is drained earlier), and play **at
  most one cue a frame** however many blows are in it. `MessageKind::Tantrum`
  is the same argument on the log — reusing `Raid` would file a staff scuffle
  as a GC Entropy Sweep and put it in `retain_outcomes_since_battle`'s
  keep-list.

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
  pulls a working body off a machine. **A Bay's capacity is now the cells in
  reach of it that a body can stand in**, and `RecoveryDef::radius` is the
  whole of it: everyone in reach still mends at full rate on the same tick,
  but one body to a cell means "in reach" is an area rather than a heap. This
  **reverses** the rule that stood here, and the arithmetic is worth having in
  front of you before touching either half — measured on a real save with 106
  staff, 80 of them benched, two Bays built side by side: at the shipped
  `radius: 0` one to four mend at a time and the benched count sits flat; at
  1, three to four; at 2, six to eight; at 3, ten to fourteen. With bodies not
  blocking at all it was thirty-nine to forty-six, because seventy-one bodies
  were standing on one cell — the old throughput *depended* on the overlap, so
  unlimited simultaneous mending and one-body-to-a-cell cannot both be had.
  What the design takes instead is that a base wanting more patients mended
  builds more Bays, which is a decision that did not exist before.

- **The recovery `+` rides the patient, not the Bay, and it is green.**
  `EntityView::recovering` is a fact about a *program* — this body is in reach
  of a Bay right now, so its Integrity is climbing — and
  `Game::recovering_programs` keys it by `Entity` rather than by Bay tile. A
  tile identifies a structure and does not identify a patient: at the shipped
  `RecoveryDef::radius` of `0` the patient stands *on* the Bay, and past `0`
  one Bay mends several bodies and a single mark on the building says nothing
  about which. `render/base.rs` passes the tile's `actor` and not its
  `structure`, which is also the glyph the tile draws — an actor takes the
  glyph off a structure — so at radius `0` the mark lands on the same cell it
  always did, over the body. **The colour moved with it**: `palette::HEALTHY`,
  because `THREAT` is reserved for hostility and inbound harm and a red `+`
  over rising Integrity reads as the harm rather than the cure. **The trap is
  the fixtures**: `view_entities` selects on `Glyph`, `spawn_machine_at` writes
  none (which is why the Bay is *placed*) and neither does `spawn_tamed` — so
  the body must come from `spawn_tamed_on_map` or it has no view at all and the
  flag reads as lost.

- **A downed program walks itself, and the `Downed` arm of `drift_idle_staff`
  sits above the `OffShift` arm** — recovery outranks an amenity — **gated on
  laid floor**, which is what keeps `entry_tile` the one arrival path for a
  program downed in the Stack. `Game::step_to_repair` is `step_off_shift`'s
  shape on the same walk, and **`Err` never drops the marker**: nothing
  re-inserts `Downed`, so there is no flicker to stop, and dropping it would
  silently heal a program that could not reach a Bay. **What `Err` no longer
  does is hold the body still**, and the two failures are told apart by the
  caller off `Bays::is_empty` rather than in there. No Bay standing at all is
  still a benched program lying where it fell — it is on an errand it cannot
  start. A Bay that is standing but *full* is the other case:
  `step_to_repair` reports `BoxedIn` where a hauler would wait, because a
  hauler's post is its own and a Bay is shared, and the patient goes back to
  milling. Holding there is what built the heap, and it **deadlocks** —
  measured on the real save, the pile's cell had three free neighbours, one
  body stepped into each, those three then held their ground too, and 71
  bodies thinned to 68 and froze there for 1,200 ticks. With the milling rule
  they reach one body to a cell in about a thousand and hold.

- **`Downed` joins the `on_shift` filter without the `Carrying` escape**, and
  is freed in the diff **unconditionally, ahead of every keep rule**. The
  `Carrying` exception exists because freeing a loaded body destroys the goods;
  a body that just died is going to the Bay regardless. `LabourDemand`'s
  shortfall grows while it is down, as it does off shift.

- **A pinned program is a fifth `ProgramRole` whose consequences are
  omissions**, and only the rest-repair one fails to compile. The workspace
  has three exhaustive matches on `ProgramRole`; the labour scheduler, the
  wander, the party recall and fusion candidacy all compare with `==` and
  ship silently, so those four are held by one test each and by nothing
  else. The trap that actually fired: a *posted* program is `Staff`, so
  `pin_subject`'s `role != Staff` refusal never caught one, and pinning it
  inserted the marker, logged success and did nothing — `drift_idle_staff`
  skips any body holding a `Task` before it reads the role at all, so the
  subject never walked, never arrived, and `pinned_subject()` (arrival is
  derived off `Position == study_pen`, never stored) stayed `None` forever
  while every subject-gated node stayed refused. Pinning frees the `Task`,
  `Downed`'s rule one role over. **The census was short by one**: a fifth
  `==` comparison lives in `Game::position_is_honest`, the *map's* draw gate,
  so a subject was not drawn at all and the pin mark sat over bare floor
  — the player reported it as the mark replacing the glyph, there being
  no glyph to replace. Its exactness is right for `Sortie` (an away program
  must not claim a tile it is not on) and wrong here, where a subject stands
  exactly where it claims to; fixed as its own arm, since the `Staff` arm's
  `&& Task.is_none()` clause is about a body hidden under a machine's glyph.
  **And the mark meant *selected* rather than *settled***:
  `Game::view_pinned_at` marked every `UnderStudy` body wherever it stood, so
  the mark latched on at selection and rode along for the whole walk. It
  is gated on the body standing on its own pen now, making mark ⇔
  `pinned_subject` ⇔ gate-satisfied one invariant instead of three answers.
  Neither was caught because every existing test drove the mark through
  `pin_subject_at_pen`, which *teleports* the body onto the pen — so nothing
  ever observed a subject mid-walk, the one state where the two questions
  differ. **`PinMark::Strained` is the same walk's answer narrowed, not a
  second door.** A body rattles while the project spending it is earning, and
  the three terms are each a *call*: `requires_subject` off the def (a project
  that spends no body strains nobody, however many are pinned),
  `research_readout` being `Earning` (so the stall stays
  `research_material_shortfall`'s one decision rather than being tested twice),
  and `pinned_subject`'s own answer (so the mark and the gate cannot name
  different programs while a second Station holds its own settled subject). A
  `view_strained_at` beside `view_pinned_at` would have been the fourth answer
  to the one question this seam exists to keep singular. **The motion is the
  ink's alone** — the pen's four squares sit on the tile-edge ring, where
  any offset
  overhangs the neighbouring cell, and a pen that shakes with what it holds
  reads as loose apparatus rather than as a body under stress. Its amplitude
  is a share of `(tile_px - glyph_px) / 2`, the margin the ink already has,
  and not a pixel count: at zoom 1 that margin is exactly 2px, so the fixed
  `2.0` this started as put a sprite flush against its own pen at the
  tightest zoom and was invisible at the widest.

- **A research node's subject gate is one term both the screen and the
  selection reach through.** It sits outside `research_block_memo`, which
  memoises per `ItemId` where the subject test is per *node* — and that is
  exactly how it first shipped broken: the gate lived in `research_block`,
  whose only caller is `select_research`, while `Game::research_nodes` built
  `blocked_by` straight off the memo. Nineteen of twenty-seven nodes drew as
  available with no block line and were refused on selection, under a doc
  comment claiming the screen and the refusal could not disagree. The tests
  were green because they compared `select_research`'s error against
  `research_block` — the same door twice. A block reason is only tested by a
  test that drives the *screen's* producer.

## Instrumentation
