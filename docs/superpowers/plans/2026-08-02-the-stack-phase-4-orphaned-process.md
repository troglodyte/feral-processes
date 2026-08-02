# The Stack phase 4 — the orphaned process: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` or `superpowers:executing-plans`
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A second route into the roster — one program per frame, sitting in
a dead end, that joins for an `ice_breaker` instead of for a won capture roll.

**Architecture:** A new `CellKind::Orphan` placed by `stack::generate`, a
spent-ness record in `FrameMemory`, and one engine action `Game::adopt_orphan`
bound to a key. The creature does not exist as an entity until it is adopted;
before that it is a cell and a seed.

**Spec:** `docs/superpowers/specs/2026-07-31-the-stack-design.md`, section
"Phase 4 — inhabitants → Orphaned process". Read it first; it records *why*
each of the decisions below went the way it did, which this plan does not
repeat.

## Global constraints

- **Never draw the orphan's species from `resources::GameRng`.** It is
  world-generation: the party must see what the program is before paying, so
  the answer has to survive a save/load. Salt off `FrameSpec::rng_seed`,
  which CLAUDE.md names as the one scheme to salt off.
- **The orphan's *stats* are not deterministic and must not be made so.**
  `spawn_wild_creature_scaled` rolls potential and wild routines off
  `GameRng` at spawn time, exactly as every other spawn does. Only the
  species is pinned.
- **`ItemId` is a string newtype**, not an enum. Reach `ice_breaker` through
  the `ids` module in `crates/engine/src/items.rs`.
- **`floor_mark` in `crates/gui/src/render/stack.rs` is exhaustive on
  purpose.** It will not compile until the new variant has a glyph. Do not
  add a wildcard arm.
- New tuning values go in `crates/engine/src/tuning.rs` as documented `pub
  const`s, never inline in a formula.
- Gates after every task: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test -p feral-processes-engine`. Full `cargo test --workspace`
  before the branch is called done.

---

### Task 1: The cell kind, end to end, drawing as itself

**Files:**
- Modify: `crates/engine/src/stack.rs` — `CellKind`, `walkable`,
  `blocks_sight`, and the inline test module
- Modify: `crates/engine/src/views.rs` — `StackCellView`
- Modify: `crates/engine/src/game/stack_view.rs` — wherever `CellKind` is
  mapped to `StackCellView`
- Modify: `crates/gui/src/render/stack.rs` — `floor_mark`
- Modify: `crates/gui/src/render/frame_map.rs` — the map's glyph table

**Interfaces produced:**
- `CellKind::Orphan` — `walkable() == true`, `blocks_sight() == false`
- `StackCellView::Orphan`
- Glyph `'o'` in green in both views

**Intent:** get the variant to exist and be visible before anything can be
done with it, so a later task cannot ship it invisible.

- [ ] **Step 1** — Extend the existing pinning test
  `the_new_cell_kinds_are_walkable_and_see_through` (inline in
  `crates/engine/src/stack.rs`) to cover `Orphan`. This is the trap that
  entry in CLAUDE.md exists for: a cell kind that is walkable *and*
  sight-blocking breaks both `view_cone` consumers. Run it; expect a
  compile failure, not an assertion failure.
- [ ] **Step 2** — Add `CellKind::Orphan` with a doc comment saying what it
  is and that whether it has been taken lives in `FrameMemory`, not in the
  frame — match the wording pattern `Cache` and `Lair` already use. Add it
  to `walkable()`; leave `blocks_sight()` alone.
- [ ] **Step 3** — Add `StackCellView::Orphan`, with a doc comment saying an
  adopted one comes through as `Floor`, mirroring `Cache` and `Breakpoint`.
  Compile; the mapping in `stack_view.rs` and both gui glyph tables will
  fail exhaustiveness. Fix each. Adopted-ness has no record yet, so map it
  unconditionally for now — Task 3 adds the gate.
- [ ] **Step 4** — `cargo test -p feral-processes-engine` and
  `cargo clippy --workspace`. Both green.
- [ ] **Step 5** — Commit: `feat(stack): a fourth new cell kind, and the
  views that draw it`

---

### Task 2: Placement

**Files:**
- Modify: `crates/engine/src/stack.rs` — new `place_orphan`, called from
  `generate`
- Modify: `crates/engine/src/tuning.rs` — `STACK_ORPHANS_PER_FRAME`

**Interfaces produced:**
- `fn place_orphan(level: &mut Frame, rng: &mut StdRng)` — private, same
  shape as `place_caches` and `place_breakpoint`
- `pub const STACK_ORPHANS_PER_FRAME: usize = 1;`

**Where it goes in `generate`:** immediately **after** `place_caches`, which
is currently last. A cache is no longer `CellKind::Floor`, so re-scanning for
`Floor && is_dead_end` excludes cache cells for free — the two passes stay
uncoupled and neither needs to know the other's count.

**Intent of each test:**

- [ ] **Step 1** — Write three failing tests in the `stack.rs` test module:
  1. **Every frame places exactly `STACK_ORPHANS_PER_FRAME` orphans** across
     a spread of seeds and depths — the count test.
  2. **An orphan never lands on a cache, a link, a door or the lair.** The
     naive count test passes while this fails, which is why phase 3 wrote
     the equivalent; assert on the *cell kind under* every orphan being a
     former plain floor dead end.
  3. **The cache count is unchanged by the orphan pass.** This is the claim
     "dead ends stay whole" makes, and it is the one that breaks if someone
     later moves `place_orphan` above `place_caches`. Model it on
     `the_new_passes_leave_the_cache_count_alone`.
- [ ] **Step 2** — Run them; expect failures naming `place_orphan`.
- [ ] **Step 3** — Implement `place_orphan`, copying the shape of
  `place_caches`: collect `Floor && is_dead_end` cells, Fisher-Yates over the
  row-major list so the pick is a pure function of the seed rather than of
  iteration order, `.take(STACK_ORPHANS_PER_FRAME)`. A frame short of dead
  ends places fewer, exactly as `place_caches` already degrades — do not add
  a panic or a fallback site.
- [ ] **Step 4** — Add `STACK_ORPHANS_PER_FRAME` to the Stack section of
  `tuning.rs`, documented with what it costs the player (one `ice_breaker`)
  and what actually limits it (`BASE_PET_CAPACITY`, which is 3).
- [ ] **Step 5** — Tests green, `cargo clippy --workspace` clean.
- [ ] **Step 6** — Commit: `feat(stack): one orphaned process per frame, in a
  dead end the caches left`

---

### Task 3: The spent-ness record, and the save bump

**Files:**
- Modify: `crates/engine/src/resources.rs` — `FrameMemory`
- Modify: `crates/engine/src/save.rs` — `SAVE_FORMAT_VERSION` 17 → 18
- Modify: `crates/engine/src/game/stack_features.rs` — the query
- Modify: `crates/engine/src/game/stack_view.rs` — gate the view mapping

**Interfaces produced:**
- `FrameMemory::adopted: BTreeSet<(i32, i32)>`, `#[serde(default)]`
- `pub(crate) fn orphan_present(&self, pos: StackPos, cell: (i32, i32)) -> bool`
  — reads `StackMemory`, exactly as `breakpoint_spent` and `cache_unopened` do

**Intent:** the rule "a Stack cell that can be used up needs both halves".
Without the record the orphan refills every time the party steps off and back
on.

- [ ] **Step 1** — Write a failing test: an orphan cell recorded in
  `adopted` comes through both views as `Floor`. Use `StackMemory` directly
  to set up the record — `adopt_orphan` does not exist yet.
- [ ] **Step 2** — Run it; expect a failure showing `Orphan` still returned.
- [ ] **Step 3** — Add the field with `#[serde(default)]`. Copy the reason
  from `jacked`'s doc comment: the field-named RON that `dev-saves/`
  templates are written in must keep parsing without re-capture.
- [ ] **Step 4** — Add `orphan_present` beside `breakpoint_spent` in
  `stack_features.rs` and gate the view mapping on it.
- [ ] **Step 5** — Bump `SAVE_FORMAT_VERSION` to 18 in `save.rs`.
- [ ] **Step 6** — `cargo test -p feral-processes` (the launcher's three
  tests, including `every_checked_in_template_still_loads`). Both
  `dev-saves/` templates must still load. If one does not, hand-edit the
  `.ron` — that is what the editable format is for — rather than
  re-capturing.
- [ ] **Step 7** — Commit: `feat(save): FrameMemory::adopted, and the version
  bump it forces`

---

### Task 4: Which species, decided once and stably

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` — extract `habitat_pools`,
  promote `spawn_wild_creature_scaled` to `pub(crate)`
- Modify: `crates/engine/src/game/stack_features.rs` — `orphan_species`

**This is the one genuinely non-obvious piece of the phase.**
`pick_habitat_species` cannot be reused as-is: it draws from `GameRng`, which
the constraint above forbids. It also cannot be copied — an independent copy
of the habitat and opening-ring rules is precisely the duplicated-formula
trap CLAUDE.md records this repo falling into four times.

The seam is that the function is candidate-building followed by exactly two
draws, both at the very end. Split it there:

```rust
/// The ordinary and boss candidate pools for a spawn at `(x, y)`, after the
/// opening-ring gentling and before any draw. `None` for an unwalkable tile
/// or a biome with nothing eligible.
///
/// Split out so the draw itself belongs to the caller:
/// `pick_habitat_species` spends `GameRng`, and the orphan spends a
/// frame-seeded `StdRng` because its answer has to survive a save/load.
pub(crate) fn habitat_pools(&mut self, x: i32, y: i32)
    -> Option<(Vec<String>, Vec<String>)>
```

`allow_boss` is deliberately **not** a parameter: both places it is consulted
sit after the pools are built, so it stays in `pick_habitat_species`. That
also means the split changes `pick_habitat_species`'s RNG draw order not at
all, which the seeded spawn tests depend on.

**Interfaces produced:**
- `Game::habitat_pools` as above
- `pub(crate) fn orphan_species(&mut self, pos: StackPos) -> Option<String>`
- `spawn_wild_creature_scaled` widened from `fn` to `pub(crate) fn` — it is
  currently private to `game::spawning` and `game::stack_features` is a
  sibling module, so it is not visible without this

- [ ] **Step 1** — Write the failing test that matters most: **the species a
  frame offers is the same across a save/load.** Read it, round-trip the game
  through `Game::save`/`Game::load`, read it again, assert equal. A test that
  only calls `orphan_species` twice in one session would pass against a
  `GameRng` draw and prove nothing.
- [ ] **Step 2** — Write a second failing test: two frames at different
  depths of the same stack offer independently-drawn species (they share an
  entrance and therefore a biome pool, so this pins that the *seed* differs
  by depth, not the pool).
- [ ] **Step 3** — Run both; expect failures naming `orphan_species`.
- [ ] **Step 4** — Extract `habitat_pools` with no behaviour change, then run
  the existing spawning and encounter tests to prove the extraction was
  inert. Do this before writing `orphan_species` — a refactor and a feature
  in one step means a failure tells you nothing about which broke.
- [ ] **Step 5** — Implement `orphan_species`: build a local
  `StdRng::seed_from_u64` salted off the frame's `FrameSpec::rng_seed`, call
  `habitat_pools(pos.entrance.0, pos.entrance.1)`, draw the index from the
  local RNG, return the ordinary pool's pick. Bosses are not eligible —
  `maybe_stack_encounter` refuses one for a fight you did not see coming, and
  a free boss companion is a stronger version of the same objection.
- [ ] **Step 6** — Promote `spawn_wild_creature_scaled` to `pub(crate)`,
  extending its doc comment with the second caller.
- [ ] **Step 7** — Full engine suite green — this task touches a function
  every spawn path calls, so a targeted run is not enough here.
- [ ] **Step 8** — Commit: `refactor(spawning): split the habitat pools from
  the draw, for a species that must survive a save`

---

### Task 5: Adopting one

**Files:**
- Modify: `crates/engine/src/game/stack_features.rs` — `adopt_orphan`
- Modify: `crates/engine/src/lib.rs` — re-export if the action needs one
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces produced:**
- `pub fn adopt_orphan(&mut self) -> Result<(), String>`

**Consumes:** `orphan_present` (Task 3), `orphan_species` and
`spawn_wild_creature_scaled` (Task 4), and the existing `taming_catalyst`,
`pet_count`, `pet_capacity`, `install_innate_routines`,
`stack_depth_multiplier`.

**The ordering is the whole of this task.** Every refusal happens before the
`ice_breaker` is spent and before anything is spawned. `attempt_decompile`
(`game/combat_rewards.rs:243`) is the model — including its comment about why
the catalyst check cannot be trusted from a caller.

**Intent of each test:**

- [ ] **Step 1** — Write five failing tests:
  1. **Success** — the roster grows by one (`pet_count`), the entity carries
     `Tamed` with the player as owner, and the cell is in `adopted`.
  2. **Not on an orphan** — refused, with the inventory untouched.
  3. **No catalyst** — refused with "no taming catalyst", **and the roster is
     unchanged**. Assert both; a refusal that has already spawned the
     creature passes a message-only assertion.
  4. **Roster full** — `pet_count() >= pet_capacity()` refused with "roster
     is full", **and the `ice_breaker` count is unchanged**. This is the
     ordering bug this phase is most likely to ship.
  5. **Does not recur** — adopt, step off, step back on, and the cell reads
     `Floor` and refuses a second adoption.
- [ ] **Step 2** — Run them; expect failures naming `adopt_orphan`.
- [ ] **Step 3** — Implement, in this order: locate the cell and check
  `orphan_present`; `taming_catalyst()`; capacity; *then* take the catalyst,
  spawn through `spawn_wild_creature_scaled` at `pos.entrance` with
  `stack_depth_multiplier()`, insert `Tamed { owner } + Experience`, call
  `install_innate_routines`, record the cell, log an `Outcome`.
- [ ] **Step 4** — Deliberately **omit** three things, and say why in the doc
  comment so a later reader does not add them back: no `StackSpawn` tag (it
  never fights, and `end_battle` despawns whatever carries it —
  `game/combat_teardown.rs:182`), no XP award (no fight was won), no `Party`
  push (the roster is the destination; fielding is a separate choice).
- [ ] **Step 5** — Confirm no Trace is raised. The spec argues this is a
  design judgement rather than a constraint; record that in the doc comment
  so the first playtest knob is findable.
- [ ] **Step 6** — Engine suite green.
- [ ] **Step 7** — Commit: `feat(stack): adopt an orphaned process for an ICE
  Breaker`

---

### Task 6: The key, and the documentation obligation

**Files:**
- Modify: `crates/app-core/src/app/playing.rs:~216` — the underground key
  dispatch, beside `'>'` and `'<'`
- Modify: `README.md`, `CHANGELOG.md`, the in-game manual, `CLAUDE.md`
  (and `cp` it to `AGENTS.md` — they are gitignored twins with no tracking
  to catch drift)
- Modify: `docs/superpowers/specs/2026-07-31-the-stack-design.md` — the
  status table row

- [ ] **Step 1** — Bind `GameKey::Char('t')` to `game.adopt_orphan()` in the
  underground arm. Two lines; no new `Mode`. Check `t` is not already bound
  in that arm before using it, and pick another letter if it is.
- [ ] **Step 2** — Add an app-core test that the key reaches the engine, in
  the style of the existing underground key tests.
- [ ] **Step 3** — Update the player-facing docs. Grep for claims the change
  falsifies rather than only adding new prose — the standing rule is that the
  `assets/*/README` schema docs are not the whole obligation.
- [ ] **Step 4** — Add a **Load-bearing seams** entry to `CLAUDE.md` for the
  thing a future session will otherwise rediscover: that the orphan's species
  is salted off `FrameSpec::rng_seed` while its stats are rolled from
  `GameRng` at adoption, and why the two differ.
- [ ] **Step 5** — Flip the spec's status table row for phase 4 to built.
- [ ] **Step 6** — `cargo test --workspace`, `cargo clippy --workspace`,
  `cargo fmt`. All green.
- [ ] **Step 7** — Commit: `docs: the orphaned process in the README,
  manual, CHANGELOG and seams`

---

## The gate this branch does not merge without

**Capture nothing, and play `dev-saves/stack.ron`.** The template already
drops the party on frame 3 of 6, which is where an orphan now sits.

The spec records what this plan cannot settle, and it is not a detail: one
orphan per frame across six frames is six programs, against a
`BASE_PET_CAPACITY` of **3**. The binding limit is the roster cap, not the
`ice_breaker` supply, so most descents will refuse after one or two and the
rest of the stack's orphans are scenery. `balance_sim` models no roster and
cannot gate this.

Phase 2 shipped an arithmetic fault — three caches at 10 against a first band
at 40 — through a spec, a plan and a code review, and one crawl found it in
minutes. The question to answer while playing is the same shape: **does the
maximum a player can plausibly do in one stack actually reach the limit the
design assumes it does?**
