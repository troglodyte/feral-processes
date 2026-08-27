# What a program needs

**Status:** approved, not implemented (2026-08-27)

An owned program carries `Needs` — a small map of reserves that fall on
their own and are refilled by standing at a structure that services them.
When one crosses its critical threshold the program leaves its post, walks
to the amenity, and stands there until the reserve is full again. What it
did, where, and who else was there is written through `Game::remember`.

The catalogue is `assets/needs/`. The amenities are `assets/structures/`.
A third need and the building that answers it are **two `.ron` files and no
Rust**.

## Why

The base already has bodies that move on their own, remember what happened
to them, and act on it. `drift_idle_staff` walks unposted programs one
neighbour per beat; eight `assets/memories/` defs decay on a half-life;
`opinion_of(BaseTile)` already steers the drift and `morale` is already an
addend in `mining_success_chance`.

What is missing is that **the programs are alone in there and have nothing
they want.** An unposted program random-walks until the scheduler picks it
up again. `bonded_in_battle` is the only `Program`-subject memory in the
game and it only fires in a fight — two programs on adjacent base tiles are
as unaware of each other as of the rock.

The living-base feel in Dwarf Fortress and RimWorld is not the number of
bars on the screen. It is that **an off-shift body has somewhere it wants
to be and something it wants to do, you can watch it go do it, and it
remembers how that went.** Needs are the smallest thing that supplies the
"wants".

There is a second reason to build it on this shape: the need machinery
already exists and has exactly one subject. `systems::needs_tick_system`
and `systems::power_regen_system` are both `With<Player>`. Staff carry a
`PowerReserve` — `roster_parts` grants one, it is how a companion pays for
its own Special — that has never drained a point. The idea is proven in the
codebase; only its audience is wrong.

## Decisions

Settled in the brainstorm, recorded so they are not relitigated:

- **A need may pull a program off a working post, but only at critical.**
  Above the threshold it is serviced with idle time. This is where the
  base gets shifts. It is also the one decision that can make throughput
  lumpy, and `working_multiplier` is the knob.
- **Two needs in v1**, Coherence and Slack. Iterate later. The catalogue is
  built so a third costs no Rust.
- **Power is not a need and `PowerReserve` is not touched.** Its float is
  private with seven operations matching its call sites exactly; `Needs` is
  a separate component rather than an eighth operation on it. Power is a
  *combat* resource — routines are priced in it — and the standing note that
  Power is not a limiting resource stands. Widening its drain to staff would
  turn the Recharger Node from an amenity into a tax.
- **Teeth feed an existing formula only.** The finished term is an addend
  in `mining_success_chance`, beside the one memories already contribute.
  Nothing here is a new damage source, a new loss, or a new pressure
  competing with raids and Trace.
- **The player's answer is a building.** No new intervention verbs, no
  mediation UI, no per-program orders. You read what the base is telling you
  and you deploy the amenity.
- **Needs are a base-labour concept in v1.** Only `ProgramRole::Staff`
  drains. A party program and the wielded program are not on shift; whether
  they should drain is deferred below.

## The catalogue

`crates/engine/src/needs.rs`, shaped after `memories.rs` — the same
half-data seam, and the same guarantees.

```ron
// assets/needs/coherence.ron
(
    id: "coherence",
    name: "Coherence",
    blurb: "A process that never yields comes apart at the edges.",
    servicing: "Defragmenting",
    drain_per_tick: 0.02,
    working_multiplier: 2.0,
    critical: 20.0,
    content: 60.0,
    morale_weight: -4.0,
)
```

`NeedId` is a `#[serde(transparent)]` string newtype, `MemoryId`'s shape and
for `MemoryId`'s reason: a mod's need cannot be an enum variant. `NeedDb` is
a `Resource` with a `load_dir` that skips a malformed file with a logged
warning, per the standing rule.

All nine fields are **required** in this initial schema, exactly as
`MemoryDef`'s seven are — a def missing any of them cannot be drained,
serviced or drawn. Any field added *later* is `#[serde(default)]`.

A reserve runs **0.0 to 100.0** and is seeded full, `PowerReserve`'s range
and for its readability reason: `critical`, `content` and every authored
threshold are then plainly percentages of a bar the player can be shown. A
save written before this feature loads with no entry for a need, which
seeds full on the first drain tick — a program is not punished for a
reload.

`servicing` is the player's verb for the errand and is what the manifest row
and the examine line read. `morale_weight` is the contribution at empty,
scaled linearly to zero at `content`, so a satisfied need is worth nothing
rather than worth a little.

**An absent or empty `assets/needs/` is valid and inert.** No drain, no
errand, no morale term — the pre-needs game exactly, the property
`assets/memories/`, `assets/policies/` and `assets/environment/` all hold.
Never gate a trigger, a system or a screen on the database being non-empty:
that makes the property hold by accident at one site and lapse at another.

## The component, and the one thing that is stored

`Needs(BTreeMap<NeedId, f32>)` — `Stock`'s idiom, ordered so the save
encoding cannot differ between runs. Granted by **`roster_parts`**, the one
barrier into the roster, riding beside `Memories` through all four doors.

`hauling::Errand` is derived per tick and never stored; `Carrying` is the
only thing hauling keeps. The same split holds here, and there is exactly
one thing that cannot be derived: **hysteresis.** A rule read off the
current value alone flickers every tick at the boundary — pulled off at 20,
returned at 20.1, drained to 20 again.

So **`OffShift(NeedId)` is the one component this feature stores.**
Inserted when a need falls below `critical`, removed when that need reaches
`content`. Everything else is derived:

- **Which amenity** is "nearest standing structure servicing this need",
  a pure function of position, the structure set and the need. Stable
  because the program converges on it. Ties break on a **total**
  `(distance, x, y)` order — `min_by_key` returns the first of several
  equal minima, which is where bevy's unstable iteration order leaks in.
- **Whether the program is being serviced** is `at_station` against that
  amenity, `hauling::at_station`, the same reach rule a posting uses.
- **What it is doing** is the def's `servicing` string. There is no errand
  enum and no second `Errand` type.

## Amenities are structure data

`StructureDef` gains one `#[serde(default)]` block:

```ron
services: [ (need: "coherence", per_tick: 0.8, radius: 0) ],
```

Absent or empty means the structure services nothing, which is every
shipped structure but the two below. `radius: 0` is "stand on an adjacent
tile", `hauling::at_station`'s reach; a larger radius is the Chebyshev box
`power_regen` already uses, so a mod can author a field-effect amenity.

`per_tick` is mod-supplied and so is clamped at both ends rather than
trusted, `power_regen_system`'s rule: a non-finite value is skipped
entirely and a negative one is floored at zero, or a field named for
refilling drains the reserve instead.

**The Recharger's `power_regen` block is deliberately not folded into
`services`.** Power is not a need here and its system is `With<Player>`;
merging them would put a combat resource on the needs axis and re-open a
question this design closed.

Two new structure files, no Rust:

- **Defrag Bay** — services Coherence. A program parks and goes offline for
  a stretch. This is the amenity that gives the base shifts.
- **Sandbox** — services Slack. A scratch partition where a program runs
  junk for the pleasure of it. This is where programs meet.

Both need a `build_cost`, a `max_deployed` and an upgrade path authored the
way every other structure does, and `assets/structures/README.md` documents
`services` in the same change.

## The cycle

1. **Drain.** `needs_drain_system` walks every `Staff` program and lowers
   each need by `drain_per_tick`, multiplied by `working_multiplier` when
   the program holds a `Task`. Arithmetic only, no allocation, no RNG.
2. **Critical.** A need below its threshold inserts `OffShift(need)`.
3. **Standdown.** `schedule_base_labour`'s staff list excludes `OffShift`
   holders — *except* one still holding a `Carrying`, which the existing
   never-free-a-`Carrying`-holder rule keeps posted until it delivers. That
   rule is reused rather than restated; freeing a loaded body destroys the
   goods, and `DigErrand::Return` is the precedent for walking a load home.
4. **Walk.** `drift_idle_staff` resolves the amenity and takes one step
   toward it with `hauling::step_to_post` — the walk the dig crew already
   rides. A body with no `OffShift` falls through to today's
   `wander_step` unchanged. So does one whose amenity was destroyed under
   it mid-errand — a raid on the Sandbox — which drops `OffShift` on the
   next beat through the same gate that refused to insert it.
5. **Service.** Standing within reach, the need rises by the structure's
   `per_tick`.
6. **Return.** At `content`, `OffShift` is removed and the program is back
   in the pool on the next schedule.

Step 4's fall-through is what makes the empty catalogue inert without a
branch: no defs means no `OffShift` means the existing drift, untouched.

## The gate, and what acting out is

**`OffShift` is inserted only if an amenity for that need exists and routes
from where the program is standing.** Both halves, one gate. A program that
fails it keeps working, keeps draining, and pays the morale term — it is
never pulled off a post it cannot leave usefully.

That single rule is what makes the feature incapable of stalling a base.
Without it a program whose amenity is unreachable never reaches `content`,
so it never returns to the pool: a raid that buries the Sandbox would take
the crew with it, permanently, and the symptom would read as the scheduler
being broken.

The route is tested by **attempting the step** — `hauling::step_to_post`,
whose `Err(NoPost::NoRoute)` is the answer — rather than by a separate
`post_reach` call, so the gate and the walk share one field per candidate
per beat rather than searching twice. `hauling::has_station` is the cheap
half and runs first.

**Acting out is failing that gate**, and it is expressed entirely through
machinery that exists:

- It **announces once**, latched on the program, per `set_machine_status`'s
  only-on-transition rule. The latch is not saved: a reload should say it
  again, `DigSite::announced_stuck`'s rule.
- It **writes a `BaseTile` grudge** where the program is standing, which
  `drift_idle_staff`'s existing avoidance hook then reads — so a program
  left frayed in a corner starts refusing that corner.
- It keeps paying the morale term, which is the whole mechanical cost.

The two failing halves say **different things**, because they leave the
player different errands — nothing services this need anywhere, versus the
Sandbox is walled off from where this program stands. That distinction is
`NoPost::BoxedIn`-versus-`NoRoute`'s rule applied one level up.
`NoPost::BoxedIn` itself is silent, as it is for a dig site: it is the
normal interior of a base under construction and it resolves itself.

There is no new damage, no new loss, and nothing competing with raids or
Trace for the player's attention. The state is legible and the answer is a
building.

## Where programs notice each other

Two programs both `OffShift` at the same structure, both within
`at_station` reach of it, each write a `Program`-subject memory about the
other through `Game::remember` — the one door, which no-ops on a `who` with
no `Memories` and so needs no guard for the player or a hostile.

One new def, `assets/memories/idled_with.ron`, positive valence, plus its
mandatory row in `MEMORY_TRIGGERS` in `crates/engine/src/tests/assets.rs` —
the pairing census, which fails the build for a def shipped without one.

**Written once, on the edge where servicing completes** — when the need
reaches `content` — naming whoever else was in reach at that moment. Never
per tick. `note_postings`' doc comment states the cost of getting this
wrong and it applies unchanged: a per-tick writer saturates `strike_cap` in
three ticks and makes `strikes` mean nothing, and because `remember` evicts
at the tail of every write it also makes eviction eager for exactly the
programs that are living the most.

This is the first `Program`-subject memory outside combat, and the first
thing in the game that makes *who else is on the roster* matter to a
program that is not fighting.

## Teeth

`needs::strain(needs, db)` is a **free function** returning a signed `f32`
— `party::role_of`'s reason: `task_progress_system` has no `Game` to ask,
and two folds would eventually disagree about whether an unresolvable def
counts, which is the property the empty-catalogue guarantee rests on.
`Game::need_strain(who)` is a caller of it, for the screens.

It is **signed around a baseline of zero**, `base_int`'s idiom and
`morale`'s: a program with full needs, the player working a node himself,
and a deleted `assets/needs/` all contribute exactly nothing, without a
branch here or at the call site.

`systems::need_shift(strain)` prices and caps it, split out beside
`morale_shift` for `morale_shift`'s own stated reason — the property that
matters is that it saturates, and asserting that through
`mining_success_chance` cannot tell a working cap from the outer
`clamp(0.0, 1.0)` swallowing the overshoot.

It gets **its own cap**, `tuning::NEED_STRAIN_MAX_SHIFT`. Sharing
`MEMORY_MORALE_MAX_SHIFT` would let a program with excellent memories pay
for its own neglect, and vice versa. Per-need magnitudes stay in the
`.ron` — only the cap is tuning, per the don't-duplicate rule.

The wiring is a fifth parameter on `mining_success_chance` and a
`need_strain` field on `CycleModifiers`, beside `morale` and for the same
reason: it belongs to the body doing the work rather than to the player,
and it decides whether the cycle lands rather than what a landed cycle is
worth.

It reaches **extraction only**. `assembler_system` and `run_dig_crew` are
untouched, matching where `morale` reaches today.

## What the player sees

- **The manifest.** A Needs section, pushed in `program_sections` in
  `crates/gui/src/render/manifest.rs`. Every `sections.push` there needs a
  matching entry in `manifest_layout::tests::worst_case_program`, and the
  packer is order-sensitive, so this is a fixture change and not a free
  append. Banded in words the way the memories page bands age — there is no
  player-facing tick vocabulary and there should be no player-facing float
  either — with the def's `servicing` verb on the row while off-shift.
- **`x`.** The inspector aims at a tile and opens the manifest, so examine
  inherits the whole section. The examine line itself names the errand:
  `Sprocket — defragmenting`.
- **The log.** Transitions only — going off shift, coming back, failing to
  reach an amenity — tagged `MessageSource::Base` so the battle pane's
  unconditional drop keeps working.
- **The map.** Nothing, in v1. See Deferred.

## Save format

Two additive `#[serde(default)]` fields on `CreatureSave`: the `Needs` map
and `Option<NeedId>` for `OffShift`. The save is field-named RON, so this
costs **no `SAVE_FORMAT_VERSION` bump** — an existing save loads with no
entry for either need, and the first drain tick seeds each from the
catalogue at full.

A RON round-trip test cannot catch a skipped field, so this needs a real
save→load test as well, not only the round trip.

The announce latch is **not** saved: a reload should say it again.

## Efficiency

The cost is not the arithmetic, and the design is shaped around where it
actually lands.

- **Drain** is one query over `Staff` programs, floats, no allocation —
  roughly thirty programs times two needs per tick.
- **The amenity index is built once per beat and shared**, never once per
  program. `drift_idle_staff` already builds `structures_by_tile` this way
  and the new index sits beside it.
- **Pathfinding is the only real cost, and it is the existing profile.**
  `hauling::step_to_post`, one field per off-shift body per
  `IDLE_STAFF_STEP_TICKS` beat — the cadence the dig crew already runs at,
  not per tick. Because the target is a stable pure function, nothing
  re-searches on a beat where nothing moved.
- **Zero new RNG draws.** Nothing here touches `GameRng`, which is what
  keeps every seeded test in the suite from a stream shift.
- **Zero per-tick memory writes**, which keeps `strike_cap` meaningful and
  eviction lazy.
- `BTreeMap` over a sorted `Vec` is a deliberate consistency call, not a
  measurement: at two entries per program the difference is unmeasurable
  and `Stock`'s idiom is what the save-determinism argument already rests
  on.

## Testing

TDD, failing test first, `cargo test --workspace` as the final gate.

- **An empty `assets/needs/` is the pre-needs game exactly** — no drain, no
  `OffShift`, no morale term, `drift_idle_staff` unchanged.
- **The gate holds on both halves.** A base with no Defrag Bay keeps its
  whole crew at critical; so does a base whose Defrag Bay is walled off
  from where the crew stands. Both pay only the morale term, both announce
  once, and each says its own sentence. Testing the second half needs a
  base built as **two islands** — one cell with no standing room and two
  cells with standing room and no route are different faults, and a fix
  for one is not a fix for the other.
- **Hysteresis.** A program pulled off at `critical` does not return until
  `content`; the tick after it crosses back over `critical` it is still
  off shift. Delete the hysteresis and this must fail.
- **`working_multiplier` bites.** A posted program reaches critical
  strictly sooner than an idle one from the same start.
- **The social write is an edge.** Two programs at one Sandbox write
  `idled_with` **once** across a full servicing stretch, not per tick —
  asserted by `strikes`, not by the entry existing.
- **The `MEMORY_TRIGGERS` census row** for `idled_with`.
- **`need_shift` saturates**, asserted on `need_shift` directly rather than
  through the finished chance.
- **A `Carrying` holder is never freed** by going off shift.
- **Determinism.** Two amenities equidistant resolve the same way across
  runs; no `GameRng` draw is spent anywhere in the feature.
- **Save.** A round trip *and* a save→load, both fields.
- **Layout.** The manifest fixture updated, and the tallest program page
  still fits its popup.
- **A malformed need file is skipped with a warning**, not a panic.

Every new test is mutation-proved: delete the fix, watch it fail, restore.

## Files

**New**
- `crates/engine/src/needs.rs` — `NeedId`, `NeedDef`, `NeedDb`, `strain`
- `assets/needs/coherence.ron`, `assets/needs/slack.ron`, `README.md`
- `assets/structures/defrag_bay.ron`, `assets/structures/sandbox.ron`
- `assets/memories/idled_with.ron`

**Engine**
- `components.rs` — `Needs`, `OffShift`
- `game/spawning.rs` — the `roster_parts` tuple
- `systems.rs` — `needs_drain_system`, `need_shift`, `CycleModifiers`,
  `mining_success_chance`
- `game/base/work_orders.rs` — `drift_idle_staff`, the staff filter in
  `schedule_base_labour`
- `game/memories.rs` — the `idled_with` trigger
- `structures.rs` — the `services` block
- `save.rs` — two additive fields
- `tuning.rs` — `NEED_STRAIN_MAX_SHIFT`
- `views.rs` — the manifest rows and the examine line
- `tests/assets.rs` — the censuses

**app-core / gui**
- `crates/gui/src/render/manifest.rs` — `program_sections`
- `crates/gui/src/render/manifest_layout.rs` — `worst_case_program`
- `crates/app-core` — the examine line's errand text

**Docs**
- `assets/structures/README.md`, `assets/needs/README.md`,
  `assets/memories/README.md`, `CHANGELOG.md`, `docs/seams.md`, `CLAUDE.md`

## Deferred — return to later

Flagged during the brainstorm, deliberately not in v1:

1. **An off-shift map mark.** `wears_job_mark` is exhaustive on `TaskKind`
   and an off-shift program holds no `Task`, so today it draws as a plain
   wanderer. Left unmarked because the mark means "someone is on this job"
   and off-shift is its opposite — but it is the cheapest thing to add if
   the base reads as illegible, and the rule to respect is that exactly one
   end of a posting wears the mark at every instant.
2. **`working_multiplier` is the first thing to tune.** It is the mechanism
   that turns needs into shifts and the one number that can make base
   throughput lumpy. Nothing in `balance_sim` models base production, so it
   is ungated numerically and only a session can answer it.
3. **Whether a party program drains.** v1 drains `Staff` only, which keeps
   the feature out of the Stack entirely and sidesteps the whole
   `require_surface` family. Draining in the party is flavour-rich — a
   program comes home frayed — but a program four frames underground cannot
   walk to a Sandbox, so it needs an answer for accumulating strain before
   it can ship.
4. **A third need.** The catalogue is built so one costs two `.ron` files.
   "Eat" was considered and rejected for v1: Power already does that job
   for the player and is a combat resource, and inventing a separate
   allocation reserve fills a slot the theme did not ask for.

## Rejected

- **Widening `needs_tick_system` past `With<Player>`** so staff drain Power
  and walk to the Recharger. One query change and a genuine visible loop,
  but it is the wrong need: no shifts, no meeting, no acting out, and it
  makes the Recharger mandatory infrastructure rather than an amenity.
- **A full needs board** — four or five bars, named mood modifiers, breaks
  with recovery arcs. That is a pressure system that grows its own teeth,
  competing with raids, Trace and entropy for attention. The brief was
  teeth that feed existing formulas.
- **Storing the walk target on `OffShift`.** It is derivable and stable;
  storing it adds a save field and a second thing that can be stale.
- **A shared cap with memories.** Lets excellent memories pay for neglect.
