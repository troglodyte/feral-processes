# What a program needs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give an owned program reserves that fall on their own, amenities
that refill them, and an off-shift errand it walks to when one runs
critical — observable on the manifest and the examine line, remembered
through `Game::remember`, and priced into `mining_success_chance`.

**Architecture:** A data catalogue (`assets/needs/`) shaped after
`assets/memories/`; a `Needs` component on the roster; a single stored
`OffShift` marker for hysteresis and everything else derived; amenities
declared as a `services` block on `StructureDef`. One gate decides whether
a need may pull a body off a post, and failing that gate *is* acting out.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone in `crates/engine`),
serde/RON assets, egui via `crates/gui`.

**Spec:** `docs/superpowers/specs/2026-08-27-program-needs-design.md` —
read it first. The plan argues from it and does not restate its reasoning.

## Global Constraints

Every task inherits all of these.

- **Branch:** `feat/program-needs`. Run `git branch --show-current` before
  **every** commit — a concurrent session has fast-forwarded and deleted a
  branch mid-task in this repo before. **Never push.**
- **Staging:** stage explicit paths. Never `git add -A` — it sweeps up
  another agent's worktree gitlink under `.claude/worktrees/`.
- **Gates:** `cargo fmt` and `cargo clippy --workspace` after every change,
  fixing warnings rather than silencing them. Per-task:
  `cargo test -p feral-processes-engine <name>`. Task boundary:
  `cargo test --workspace` (3416 tests before this plan).
- **Single-crate runs shift the RNG stream.** `-p feral-processes-engine`
  and `--workspace` are different builds; a seeded test can fail in one and
  pass in the other. Confirm any seeded failure under `--workspace` before
  treating it as real.
- **Registering a new `Resource` shifts bevy's query iteration order.** A
  failure in an untouched subsystem right after Task 1 is a latent unsorted
  query in *that* test, not a regression here. Fix it by sorting, and say so.
- **No `GameRng` draws anywhere in this feature.** Not one. It is what
  keeps every seeded test in the suite from a stream shift.
- **No `SAVE_FORMAT_VERSION` bump.** Every save field added here is
  additive and `#[serde(default)]`. A RON round-trip cannot catch a skipped
  field, so a new save field needs a real save→load test too.
- **Every new test is mutation-proved:** delete the fix, watch the test
  fail, restore. A test that passes with the fix removed is not coverage.
  Record the mutation in the commit body.
- **Docs:** update the matching `assets/*/README.md` in the same change as
  a schema change. **Do not touch `docs/manual.md` or the root
  `README.md`** — both are carved out of the documentation obligation.
- **Tuning:** magnitudes authored per-need live in the `.ron`; only the cap
  goes in `crates/engine/src/tuning.rs`. Never duplicate a `.ron` value
  into tuning.
- **Test fixtures** live in `crates/engine/src/tests/support.rs`. Look
  there before writing a new one. A fixture that hand-spawns a roster
  program must go through `Game::roster_parts`.

## File structure

**New**

| File | Responsibility |
|---|---|
| `crates/engine/src/needs.rs` | `NeedId`, `NeedDef`, `NeedDb`, the range constants, and `strain` — the catalogue and the pure fold over it. No base logic. |
| `crates/engine/src/game/base/offshift.rs` | The amenity index, the gate, the walk, servicing, and the social write. New file rather than more of `work_orders.rs`, which is already ~1700 lines. |
| `assets/needs/coherence.ron`, `slack.ron`, `README.md` | The shipped catalogue and its schema reference. |
| `assets/structures/defrag_bay.ron`, `sandbox.ron` | The two shipped amenities. |
| `assets/memories/idled_with.ron`, `frayed_here.ron` | The social memory and the acting-out grudge. |

**Modified**

| File | Change |
|---|---|
| `crates/engine/src/lib.rs` | Load and insert `NeedDb`; register `needs_drain_system`. |
| `crates/engine/src/components.rs` | `Needs`, `OffShift`. |
| `crates/engine/src/structures.rs` | `ServiceDef`, `StructureDef::services`. |
| `crates/engine/src/game/spawning.rs` | The `roster_parts` tuple. |
| `crates/engine/src/systems.rs` | `needs_drain_system`, `need_shift`, `CycleModifiers::need_strain`, `mining_success_chance`. |
| `crates/engine/src/game/base/work_orders.rs` | `drift_idle_staff` delegates to `offshift`; the on-shift filter in `schedule_base_labour`. |
| `crates/engine/src/game/base/mod.rs` | `mod offshift;` |
| `crates/engine/src/game/memories.rs` | The `idled_with` and `frayed_here` triggers. |
| `crates/engine/src/save.rs` | Two additive `CreatureSave` fields. |
| `crates/engine/src/tuning.rs` | `NEED_STRAIN_MAX_SHIFT`. |
| `crates/engine/src/views.rs` | `NeedRow` and the examine errand label. |
| `crates/engine/src/tests/assets.rs` | `MEMORY_TRIGGERS` rows and the new censuses. |
| `crates/gui/src/render/manifest.rs` | The Needs section in `program_sections`. |
| `crates/gui/src/render/manifest_layout.rs` | `worst_case_program`. |
| `crates/app-core/src/app/inspection.rs` | The examine line's errand text. |

---

## Task 1: The catalogue

**Files:**
- Create: `crates/engine/src/needs.rs`, `assets/needs/coherence.ron`,
  `assets/needs/slack.ron`, `assets/needs/README.md`
- Modify: `crates/engine/src/lib.rs` (module, load, `insert_resource`)
- Test: unit tests in `crates/engine/src/needs.rs`

**Interfaces — produces:**

```rust
pub const NEED_MIN: f32 = 0.0;
pub const NEED_MAX: f32 = 100.0;

#[serde(transparent)] pub struct NeedId(String);
impl NeedId { pub fn as_str(&self) -> &str }
impl From<&str> for NeedId
impl std::fmt::Display for NeedId

pub struct NeedDef {
    pub id: NeedId,
    pub name: String,
    pub blurb: String,
    pub servicing: String,
    pub drain_per_tick: f32,
    pub working_multiplier: f32,
    pub critical: f32,
    pub content: f32,
    pub morale_weight: f32,
}

#[derive(Resource, Default)] pub struct NeedDb { /* private */ }
impl NeedDb {
    pub fn load_dir(dir: &Path) -> Self;
    pub fn get(&self, id: &NeedId) -> Option<&NeedDef>;
    /// **Sorted by id.** Every caller iterates this; an unsorted walk is
    /// where a nondeterministic tie-break gets in.
    pub fn iter(&self) -> impl Iterator<Item = &NeedDef>;
}
```

`MemoryId`/`MemoryDb` in `crates/engine/src/memories.rs` is the model for
every line of this — copy its shape, its doc-comment conventions and its
`load_dir` error handling exactly. All nine `NeedDef` fields are required;
**do not** mark them `#[serde(default)]`.

- [ ] **Step 1: Write the failing tests.** Four, against a temp asset dir:
  both shipped defs load and `get` finds them by id; a malformed `.ron` is
  **skipped with a logged warning, never a panic**; a missing directory
  yields an empty db rather than an error; `iter` comes back in id order
  from a directory whose files were written in the opposite order.
- [ ] **Step 2: Run them and confirm they fail** (module does not exist).
- [ ] **Step 3: Implement `needs.rs`.** Mirror `memories.rs`.
- [ ] **Step 4: Author the two shipped defs.** `coherence` takes the
  spec's catalogue values verbatim. `slack` takes
  `drain_per_tick: 0.012`, `working_multiplier: 1.4`, `critical: 25.0`,
  `content: 70.0`, `morale_weight: -3.0` — a slower drain and a lower
  multiplier than coherence, because monotony is not exertion, and a wider
  critical-to-content gap so a Sandbox visit is a longer stretch than a
  Defrag Bay one. These are opening values, not measured ones; see the
  spec's Deferred section.
- [ ] **Step 5: Write `assets/needs/README.md`**, following the shape of
  `assets/memories/README.md`: one section per field, the 0..100 range, and
  an explicit statement that an absent or empty directory is valid.
- [ ] **Step 6: Wire it up in `lib.rs`** beside the `MemoryDb` load, and
  `insert_resource` it.
- [ ] **Step 7: `cargo test --workspace`.** Expect possible unrelated
  failures from the new-resource iteration-order shift — see Global
  Constraints. Fix any by sorting the offending query in *that* test.
- [ ] **Step 8: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task 2: The component and the drain

**Files:**
- Modify: `crates/engine/src/components.rs`,
  `crates/engine/src/game/spawning.rs` (`roster_parts`),
  `crates/engine/src/systems.rs`, `crates/engine/src/lib.rs` (registration),
  `crates/engine/src/save.rs`
- Test: `crates/engine/src/systems.rs` unit tests + a save test

**Interfaces — produces:**

```rust
#[derive(Component, Clone, Debug, Default)]
pub struct Needs {
    reserves: BTreeMap<NeedId, f32>,
    /// Task 7's latch. **Never saved** — a reload should say it again.
    stalled_announced: BTreeSet<NeedId>,
}
impl Needs {
    pub fn get(&self, id: &NeedId) -> Option<f32>;
    /// Clamps to `NEED_MIN..=NEED_MAX`. The clamp is the type's, exactly as
    /// `PowerReserve`'s is — no caller clamps.
    pub fn set(&mut self, id: &NeedId, value: f32);
    pub fn iter(&self) -> impl Iterator<Item = (&NeedId, f32)>;
    /// Any def in `db` with no entry here gets `NEED_MAX`.
    pub fn seed_missing(&mut self, db: &NeedDb);
}

pub fn needs_drain_system(/* Needs, Option<&Task>, Tamed/role filter, Res<NeedDb> */);
```

Seeding lives **only** in `seed_missing`, called at the top of the drain —
one code path covers a freshly spawned program, a program that predates a
new def, and a save written before this feature. Do not also seed in
`roster_parts`.

The drain applies `working_multiplier` when the entity holds a `Task`, and
runs for **`ProgramRole::Staff` only** — narrow the query with
`party::role_of`, the free function, never a second copy of the rule.

- [ ] **Step 1: Write the failing tests.** (a) A staff program's reserve
  falls by `drain_per_tick` in one tick. (b) One holding a `Task` falls
  strictly faster, by the authored multiplier. (c) A program with an empty
  `Needs` and a loaded db is seeded to `NEED_MAX` before the first drain
  subtracts. (d) With an **empty** `NeedDb` nothing is seeded and nothing
  drains. (e) `set` clamps at both ends. (f) A party member does not drain.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `Needs`, add it to the `roster_parts` tuple,
  write `needs_drain_system`, register it.**
- [ ] **Step 4: Run the tests and confirm they pass.**
- [ ] **Step 5: Add the save field.** `CreatureSave` gains
  `#[serde(default)] pub needs: BTreeMap<NeedId, f32>`, written and read on
  both save paths. `stalled_announced` is **not** written.
- [ ] **Step 6: Write the save tests** — a RON round trip **and** a real
  save→load asserting the reserves survive, plus one asserting a save
  written *without* the field loads and seeds full.
- [ ] **Step 7: `cargo test -p feral-processes-engine needs`, then
  `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task 3: Amenities are structure data

**Files:**
- Modify: `crates/engine/src/structures.rs`,
  `assets/structures/README.md`
- Create: `assets/structures/defrag_bay.ron`,
  `assets/structures/sandbox.ron`
- Test: `crates/engine/src/structures.rs` unit tests,
  `crates/engine/src/tests/assets.rs` census

**Interfaces — produces:**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServiceDef {
    pub need: NeedId,
    pub per_tick: f32,
    /// Chebyshev, `power_regen`'s form. `0` is "adjacent", the
    /// `hauling::at_station` reach.
    pub radius: i32,
}
// StructureDef gains: #[serde(default)] pub services: Vec<ServiceDef>,
```

`per_tick` is mod-supplied, so it is **clamped at both ends rather than
trusted**, exactly as `power_regen_system` clamps: a non-finite value skips
the service entirely and a negative one floors at zero. Copy that function's
doc comment reasoning — a field named for refilling must never drain.

The two new structures need a `build_cost` drawn from an existing zone
material, a `max_deployed`, and an upgrade path authored the way the
shipped benches are. **An upgrade path must ask for a zone material** or
`every_upgrade_path_asks_for_a_zone_material` in
`crates/engine/src/tests/assets.rs` fails the build. They declare **no
`work` block** and no `assembles` block — they produce nothing, so
`stock::producible` seeds no stock-strip row for them.

- [ ] **Step 1: Write the failing tests.** (a) A def with a `services`
  block parses and exposes it. (b) A def *without* one parses with an empty
  vec — assert against a **shipped** file, so a mod's untouched `.ron` is
  proved to still load. (c) A `per_tick` of `NaN`/`inf` is rejected at use.
  (d) A negative `per_tick` floors at zero rather than draining.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Add `ServiceDef` and the field.**
- [ ] **Step 4: Author `defrag_bay.ron` and `sandbox.ron`.**
- [ ] **Step 5: Add the census** to `crates/engine/src/tests/assets.rs`:
  `every_shipped_need_has_a_shipped_amenity` — for each `NeedDb` def, at
  least one `StructureDb` def services it. This is a *shipped-content*
  rule like `MEMORY_TRIGGERS`, and it does not contradict the
  empty-catalogue property: zero needs and zero amenities passes.
- [ ] **Step 6: Document `services` in `assets/structures/README.md`.**
- [ ] **Step 7: `cargo test --workspace`, `cargo fmt`, clippy, commit.**

---

## Task 4: `OffShift` and the gate

**Files:**
- Create: `crates/engine/src/game/base/offshift.rs`
- Modify: `crates/engine/src/components.rs`,
  `crates/engine/src/game/base/mod.rs`, `crates/engine/src/save.rs`
- Test: unit tests in `offshift.rs`

**Interfaces — consumes:** `NeedDb`, `Needs`, `ServiceDef`.
**Interfaces — produces:**

```rust
#[derive(Component, Clone, Debug)]
pub struct OffShift { pub need: NeedId }

/// Built once per caller pass over the structure query, never once per
/// program. Takes an iterator so a bevy system and a `&Game` can both
/// build one.
pub(crate) struct Amenities { /* private */ }
impl Amenities {
    pub(crate) fn build<'a>(
        structures: impl Iterator<Item = (&'a Structure, &'a Position)>,
        db: &StructureDb,
    ) -> Self;
    pub(crate) fn has(&self, need: &NeedId) -> bool;
    /// Ties broken on a **total** `(chebyshev distance, x, y)` order —
    /// `min_by_key` returns the first of several equal minima, which is
    /// where bevy's unstable iteration order leaks in.
    pub(crate) fn nearest(&self, need: &NeedId, from: Position)
        -> Option<(Position, f32, i32)>;
}

/// The need furthest below its own `critical`, as a fraction of `critical`
/// so two needs with different thresholds compare fairly. Ties by id.
pub(crate) fn pressing_need(needs: &Needs, db: &NeedDb) -> Option<NeedId>;

/// Inserts, keeps or removes `OffShift` for each of `staff`.
pub(crate) fn update_off_shift(game: &mut Game, staff: &[Entity], amenities: &Amenities);
```

**The gate, stated once so it cannot drift.** `OffShift(need)` is inserted
when all three hold:

1. the reserve is below the def's `critical`,
2. `amenities.has(need)` — something in the base services it at all,
3. the need is **not latched** in `Needs::stalled_announced`.

It is removed when the reserve reaches `content`, when the amenity stops
existing, or when the walk reports `Err(NoPost::NoRoute)`.

**Reachability is never asked as its own question.** It is discovered by
Task 5's walk attempt, and a `NoRoute` sets the latch — which is what stops
the obvious flicker of insert → failed step → remove → insert on every
beat. One Dijkstra per newly off-shift body, then nothing until the need
recovers. The latch clears when the reserve rises back above `critical`,
and Task 7 hangs the announcement and the grudge off the same edge that
sets it.

So this task's `update_off_shift` tests reachability **not at all** — it is
`has()` plus the latch. Task 4's test (c) is written against a program
whose latch is already set; Task 5 is what makes a real unreachable
amenity set it.

- [ ] **Step 1: Write the failing tests.** (a) Below `critical` with a
  reachable amenity → `OffShift` present. (b) Below `critical` with **no**
  amenity built → absent. (c) Below `critical` with the need already
  latched in `stalled_announced` → absent, and present again once the
  reserve rises above `critical` and falls back. (d) **Hysteresis:** a program at `critical - 1` given
  `OffShift`, then raised to `critical + 1`, is **still** off shift; raised
  to `content`, it is not. Delete the hysteresis and (d) must fail. (e) Two
  amenities exactly equidistant resolve to the same one across repeated
  builds seeded in opposite orders. (f) The amenity despawned under an
  off-shift program drops `OffShift`.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `offshift.rs` and `OffShift`.** `has()` and the
  latch only — no `post_reach`, no walk.
- [ ] **Step 4: Run the tests and confirm they pass.**
- [ ] **Step 5: Add the save field.** `CreatureSave` gains
  `#[serde(default)] pub off_shift: Option<NeedId>`, plus a round-trip and
  a save→load test.
- [ ] **Step 6: `cargo test -p feral-processes-engine offshift`, fmt,
  clippy, commit.**

---

## Task 5: The walk and the servicing

**Files:**
- Modify: `crates/engine/src/game/base/offshift.rs`,
  `crates/engine/src/game/base/work_orders.rs` (`drift_idle_staff`),
  `crates/engine/src/systems.rs` (`needs_drain_system`)
- Test: unit tests in `offshift.rs`

**Interfaces — produces:**

```rust
/// One step toward the amenity for `worker`'s `OffShift` need, or the
/// gate's verdict when there is no route.
pub(crate) fn step_off_shift(game: &mut Game, worker: Entity, amenities: &Amenities)
    -> Result<(), NoPost>;
```

`drift_idle_staff` calls this **first** for any `OffShift` holder and falls
through to today's `wander_step` for everyone else. The existing rejections
— an occupied structure tile, a non-`is_floor` tile, the party's cell —
still apply to the off-shift step: an off-shift body is still a body.

Restore is per tick and lives in `needs_drain_system`, after the drain,
raising the reserve by the amenity's `per_tick` when the program is within
its `radius` (`hauling::at_station` for `radius: 0`). The system builds its
own `Amenities` once per tick; `drift_idle_staff` builds its own once per
beat. Two cheap builds beat one stale cached copy, and a cached one would
be a new `Resource` and another iteration-order shift.

- [ ] **Step 1: Write the failing tests.** (a) An off-shift program one
  tile from its amenity steps **onto** the reach tile rather than
  wandering. (b) Standing in reach, its reserve **rises** on the next tick.
  (c) A program with no `OffShift` still wanders exactly as before — assert
  against the pre-existing wander test's expectation, unchanged. (d) An
  off-shift program is refused a step onto the party's cell and onto a
  non-floor tile, the same as a wanderer.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `step_off_shift` on `hauling::step_to_post`** —
  the walk the dig crew already rides. Do not write a second walk.
- [ ] **Step 4: Make `Err(NoPost::NoRoute)` set the latch and drop
  `OffShift`.** This is the only place reachability is decided. Add the
  test: an off-shift program whose amenity does not route loses `OffShift`
  on the first beat, has the need latched, and is **not** re-inserted on
  the next beat. Remove the latch write and that last clause must fail.
  The fixture needs **two islands** — one cell with no standing room and
  two cells with standing room and no route are different faults, and a fix
  for one is not a fix for the other. `game_at_the_frontier_cutting` in
  `tests/support.rs` is the nearest existing shape.
- [ ] **Step 5: Add the restore to `needs_drain_system`** and confirm (b).
- [ ] **Step 6: `cargo test -p feral-processes-engine offshift`, fmt,
  clippy, commit.**

---

## Task 6: Standing down

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs`
  (`schedule_base_labour`, around lines 854 and 939-940)
- Test: unit tests in `work_orders.rs`

An off-shift program leaves the **posting** half of the scheduler, not the
drift half. `drift_idle_staff(&staff)` keeps the full list — it is what
walks them to their amenity. Everything from there to `truncate` uses a new
`on_shift` list: `staff` minus `OffShift` holders, **except one still
holding a `Carrying`**, which stays on shift until it delivers.

That exception reuses the existing never-free-a-`Carrying`-holder rule
rather than restating it: freeing a loaded body destroys the goods, and
`DigErrand::Return` is the precedent for walking a load home.

`record_labour_demand` and `truncate` both take `on_shift.len()`. The
shortfall the work-order header shows therefore *grows* while bodies are
off shift, which is the intended readout and not a bug.

- [ ] **Step 1: Write the failing tests.** (a) An off-shift program is not
  posted to a machine that wants a body. (b) An off-shift program holding a
  `Carrying` **is** still posted, and its load is not dropped. (c)
  `LabourDemand::staff` falls by the number off shift. (d) A base whose
  whole crew is off shift posts nobody and does **not** panic or stand
  anybody down (the empty-queue standdown guard still holds).
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement the `on_shift` split.**
- [ ] **Step 4: Run the tests, then the whole work-orders module** —
  `cargo test -p feral-processes-engine work_orders` — this is the task
  most likely to move an existing scheduler test.
- [ ] **Step 5: fmt, clippy, commit.**

---

## Task 7: Acting out

**Files:**
- Create: `assets/memories/frayed_here.ron`
- Modify: `crates/engine/src/game/base/offshift.rs`,
  `crates/engine/src/game/memories.rs`,
  `crates/engine/src/tests/assets.rs`, `assets/memories/README.md`
- Test: unit tests in `offshift.rs`

A program below `critical` that cannot be serviced announces once and
writes a `MemorySubject::BaseTile` grudge at its own `Position`, through
`Game::remember` — the one door. Both are hung off the **edge where
`Needs::stalled_announced` gains the need**, which Task 4 sets for "nothing
services this at all" and Task 5 sets for "it does not route". One edge,
one announcement, one grudge, whichever half failed. The latch clears when
the reserve rises back above `critical` and is **never saved**.

The two failing halves say **different sentences**, because they leave the
player different errands: nothing in the base services this need, versus
the amenity is walled off from where this program stands. That is
`NoPost::BoxedIn`-versus-`NoRoute`'s rule one level up. `BoxedIn` itself is
silent, as it is for a dig site.

Log lines are `MessageSource::Base`.

`frayed_here` is a `BaseTile`-subject def with negative valence, authored
beside `stranded_at` and distinct from it — a hauler that nothing reaches
and a program worn thin in a corner are different complaints.

- [ ] **Step 1: Write the failing tests.** (a) No amenity anywhere →
  exactly one log line across many ticks, and one `frayed_here` entry.
  (b) An amenity that does not route → one log line, and a **different**
  sentence from (a). (c) The reserve rising above `critical` and falling
  again produces a **second** announcement — the latch clears. (d) A
  reload re-announces: the latch is not in the save. (e) The grudge is at
  the program's own tile, and `drift_idle_staff` subsequently declines it.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Author `frayed_here.ron` and add its `MEMORY_TRIGGERS`
  row** — a def shipped without one fails the build.
- [ ] **Step 4: Implement the announcement and the write.**
- [ ] **Step 5: Document the new def in `assets/memories/README.md`.**
- [ ] **Step 6: `cargo test --workspace`, fmt, clippy, commit.**

---

## Task 8: Where programs notice each other

**Files:**
- Create: `assets/memories/idled_with.ron`
- Modify: `crates/engine/src/game/base/offshift.rs`,
  `crates/engine/src/game/memories.rs`,
  `crates/engine/src/tests/assets.rs`, `assets/memories/README.md`
- Test: unit tests in `offshift.rs`

On the **edge** where a program's reserve reaches `content`, it writes an
`idled_with` memory naming every *other* program that was in reach of the
same amenity at that moment. `MemorySubject::Program(ProgramId)`,
positive valence.

**Once per servicing stretch, never per tick.** `note_postings`' doc
comment states the cost and it applies unchanged: a per-tick writer
saturates `strike_cap` in three ticks, makes `strikes` meaningless, and —
because `remember` evicts at the tail of every write — makes eviction eager
for exactly the programs that are living the most.

- [ ] **Step 1: Write the failing tests.** (a) Two programs servicing at
  one Sandbox each hold **one** `idled_with` about the other when both
  finish — assert `strikes == 1`, not merely that an entry exists. (b) A
  lone program finishing writes nothing. (c) A hundred ticks of shared
  servicing before either finishes still produces `strikes == 1`. (d) A
  program at a *different* amenity is not named.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Author `idled_with.ron` and add its `MEMORY_TRIGGERS`
  row.**
- [ ] **Step 4: Implement the edge write.**
- [ ] **Step 5: Document it in `assets/memories/README.md`.**
- [ ] **Step 6: `cargo test --workspace`, fmt, clippy, commit.**

---

## Task 9: Teeth

**Files:**
- Modify: `crates/engine/src/needs.rs` (`strain`),
  `crates/engine/src/systems.rs` (`need_shift`, `CycleModifiers`,
  `mining_success_chance`), `crates/engine/src/tuning.rs`
- Test: unit tests in `systems.rs`

**Interfaces — produces:**

```rust
/// Signed, baseline **zero**. Sums each def's `morale_weight` scaled
/// linearly from full at `NEED_MIN` to nothing at `content`. Unresolvable
/// entries are skipped, exactly as every `Memories` reader skips one.
pub fn strain(needs: &Needs, db: &NeedDb) -> f32;   // free fn, needs.rs

pub(crate) fn need_shift(strain: f32) -> f64;       // systems.rs
pub(crate) fn mining_success_chance(
    level: u32, keen_scavenger_level: u32, base_int: i32,
    morale: f32, strain: f32,
) -> f64;
// CycleModifiers gains: pub need_strain: f32,
```

`strain` is a **free function** for `party::role_of`'s reason: a bevy
system has no `Game` to ask, and two folds would eventually disagree about
whether an unresolvable def counts — the property the whole empty-catalogue
guarantee rests on. `Game::need_strain(who)` is a caller of it, for the
screens.

`need_shift` is split out beside `morale_shift` for `morale_shift`'s own
stated reason, and it gets **its own cap**,
`tuning::NEED_STRAIN_MAX_SHIFT`. The outer `clamp(0.0, 1.0)` is not a cap;
a test reading the finished chance cannot tell a working cap from that
clamp swallowing the overshoot.

Reaches **extraction only**. Leave `assembler_system` and `run_dig_crew`
untouched, matching where `morale` reaches today.

- [ ] **Step 1: Write the failing tests.** (a) A program with every reserve
  at `NEED_MAX` has `strain == 0.0` exactly. (b) An **empty** `NeedDb`
  gives `strain == 0.0` — no branch, arithmetic. (c) `need_shift`
  saturates at `±NEED_STRAIN_MAX_SHIFT`, asserted on `need_shift`
  **directly**. (d) An entry naming a def no file defines is skipped, not
  counted as zero-weighted noise. (e) A drained program's extraction
  success is strictly lower than a full one's, and a full one's is
  **unchanged from today's shipped value**.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `strain`, `need_shift`, the constant, the
  `CycleModifiers` field and the fifth parameter.**
- [ ] **Step 4: Run the tests, then
  `cargo test -p feral-processes-engine balance_sim`** — no curve should
  move, because nothing in `balance_sim` models base production. A moved
  curve here means the term leaked somewhere it should not have.
- [ ] **Step 5: `cargo test --workspace`, fmt, clippy, commit.**

---

## Task 10: The readout

**Files:**
- Modify: `crates/engine/src/views.rs`,
  `crates/gui/src/render/manifest.rs` (`program_sections`),
  `crates/gui/src/render/manifest_layout.rs` (`worst_case_program`),
  `crates/app-core/src/app/inspection.rs`
- Test: unit tests in `views.rs`, `manifest_layout.rs`

**Interfaces — produces:**

```rust
pub struct NeedRow {
    pub name: String,
    /// A word, never a number and never a tick count.
    pub band: &'static str,
    /// The def's `servicing` verb while off shift, `None` otherwise.
    pub servicing: Option<String>,
}
impl Game {
    pub fn need_rows(&self, who: Entity) -> Vec<NeedRow>;   // sorted by need id
    /// The examine line's tail: the `servicing` verb, or `None`.
    pub fn program_errand_label(&self, who: Entity) -> Option<String>;
}
pub fn need_band(fraction: f32) -> &'static str;  // four bands, in views.rs
```

Banded in words, the way the memories page bands age: there is no
player-facing tick vocabulary in this game and there should be no
player-facing float either. Rows sort by **need id**, never by value, or
the labels move under the eye reading them.

Every `sections.push` in `program_sections` needs a matching entry in
`manifest_layout::tests::worst_case_program` — **the packer is
order-sensitive**, so the fixture must match `sections_for`'s emission
*order*, not just its row count. A drifted fixture has hidden a live
overflow behind a green suite in this repo before.

- [ ] **Step 1: Write the failing tests.** (a) `need_rows` returns one row
  per loaded def, in id order, banded. (b) With an **empty** `NeedDb` the
  section is absent entirely, not present-and-empty. (c) The `servicing`
  verb appears only while `OffShift`. (d) `program_errand_label` is `None`
  for an on-shift program. (e) The layout fixture: the tallest program page
  with the Needs section still fits the tightest window.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement `need_band`, `need_rows`,
  `program_errand_label`.**
- [ ] **Step 4: Push the section in `program_sections` and update
  `worst_case_program` in the same edit**, matching emission order.
- [ ] **Step 5: Add the errand tail to the examine line.**
- [ ] **Step 6: `cargo test --workspace`, fmt, clippy, commit.**

---

## Task 11: Documentation

**Files:**
- Modify: `CHANGELOG.md`, `docs/seams.md`, `CLAUDE.md`, `AGENTS.md`

No code. **Do not touch `docs/manual.md` or the root `README.md`.** The
version bump and the tag happen at the merge, not here — commits on a
branch stay unversioned.

- [ ] **Step 1: Write the `docs/seams.md` entry** under "The base": the
  reasoning, the measurement, and what was tried and rejected. This is
  where the argument lives.
- [ ] **Step 2: Write the matching `CLAUDE.md` bullets** — the rule and the
  trap it exists to close, one or two lines each, under the same title.
  Candidates, all load-bearing: the one gate and why failing it is acting
  out rather than a stall; `OffShift` being the only stored state and why
  hysteresis is the reason; the empty-catalogue property; the social write
  being an edge and never per tick; `need_shift`'s own cap; the drift
  fall-through.
- [ ] **Step 3: `cp CLAUDE.md AGENTS.md`** — they are gitignored twins with
  no tracking to catch drift.
- [ ] **Step 4: Write the `CHANGELOG.md` section**, following the
  preamble's policy on which digit moves. A save that still loads is not a
  break.
- [ ] **Step 5: `rg` for claims this change falsifies** in `docs/` and the
  `assets/*/README.md` files, and correct them.
- [ ] **Step 6: `cargo test --workspace` one final time, commit.**

---

## Verification before calling it done

- [ ] `cargo test --workspace` green, and the count has **risen** from
  3416 by the number of tests added.
- [ ] `cargo clippy --workspace` clean.
- [ ] `git diff --quiet assets/` after any experiment that toggled an
  asset — a timed-out loop has left `grants:` commented out in a shipped
  item in this repo before.
- [ ] Deleting `assets/needs/` and running the suite still passes: the
  pre-needs game, exactly.
- [ ] `git log --oneline` shows one commit per green task, and every
  commit body records the mutation that proved its tests.
