# Growing the base — slice 2 implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the base something you carve — hit rock to open it, lay
VectorStasis Tiles over what you opened, mark blocks for a crew to cut and
floor, and let entropy reclaim the frontier you dug and never floored.

**Architecture:** One new component (`DigSite`) is the single representation
of rock-in-progress: it carries the wall's remaining `Durability` and whether
the player marked it, and it is spawned lazily by a swing or a mark. Bumping a
solid cell strikes it, in the branch position `move_player` holds its nest
branch. A mark's *meaning* is derived from the cell under it — marked solid
means cut, marked `Open` means floor — so one verb runs a wall all the way to
finished floor. Crews reach dig sites through the base's existing
`hauling::post_reach`, whose `BoxedIn`/`NoRoute` split is exactly the
silent-vs-complain distinction the design asks for.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), field-named RON saves,
egui/bevy renderer behind `gui/src/paint.rs`.

**Spec:** `docs/superpowers/specs/2026-08-19-base-out-of-phase-design.md`,
section **"Slice 2 in detail: growing the base"**. Read that section before
Task 1; its nine settled decisions are the argument for everything below, and
this plan does not repeat them.

## Global Constraints

- **Deviation from the writing-plans skill, on purpose.** `CLAUDE.md` §Process
  weight: a plan hands a subagent the file list, the interface it must
  produce, the intent of each test and the gates to run — **not finished code
  it will merely re-emit**. Code blocks below appear only where the thing is
  genuinely non-obvious. A task with no code block is not underspecified.
- **TDD, every task.** Failing test first, watch it fail, minimal
  implementation, watch it pass, commit. This applies at every task size.
- **Mutation-prove every new test.** Delete or invert the fix, confirm the new
  test fails, restore. A test that passes with the fix removed is not
  coverage. This repo has shipped two of those.
- **Full suite is the gate for the branch**, not for each task:
  `cargo test --workspace`. Per task, `cargo test -p feral-processes-engine
  <name>`. Note `-p` and `--workspace` are different builds and shift the RNG
  stream — a seeded test can pass in one and fail in the other, so a failure
  seen under one must be re-checked under the other before it is believed.
- `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings
  rather than silencing them.
- **Tuning values are code, in `crates/engine/src/tuning.rs`**, as documented
  `pub const`s in a labelled section. Never inline in a formula, never
  duplicated into a `.ron`.
- **Never draw from `resources::GameRng` for anything a reload must
  reproduce.** Mining's fragment roll is a live action, not world generation,
  so `GameRng` is correct there. Nothing in this slice generates world.
- **`SAVE_FORMAT_VERSION` stays at 32.** The save is field-named RON behind a
  version line (`save::save_to_file`); an added field behind
  `#[serde(default)]` loads out of a file written before it existed. The
  bincode notes elsewhere in `save.rs` are historical, from before 0.8.0. Do
  not bump. Do not recapture `dev-saves/` templates.
- **Commit freely, on green. Never push** — the user asks for that
  separately. Check `git branch --show-current` before every commit; another
  session has fast-forwarded and deleted a branch mid-task before.
- Branch: `base-growth`, in the worktree `.claude/worktrees/base-growth`.

---

### Task 1: Rock you can hit

The hand loop's first half: a solid cell becomes a thing you swing at, and a
couple of swings bring it down.

**Files:**
- Modify: `crates/engine/src/components.rs` — add `DigSite`
- Modify: `crates/engine/src/tuning.rs` — new labelled section
- Modify: `crates/engine/src/game/base_space.rs` — `strike_rock`, the bump
  branch in `move_in_base` (currently line ~191, immediately before the
  `BaseGrid::walkable` check)
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces produced:**

```rust
// components.rs — paired with the existing `Durability` on the same entity.
pub struct DigSite { pub marked: bool, pub announced_stuck: bool }

// game/base_space.rs
impl Game {
    pub(crate) fn dig_site_at(&self, x: i32, y: i32) -> Option<Entity>;
    pub(crate) fn strike_rock(&mut self, x: i32, y: i32);
}

// tuning.rs
pub const BASE_ROCK_DURABILITY: u32 = 24;
pub const BASE_MINE_FRAGMENT_CHANCE: f32 = 0.25;
```

`announced_stuck` is unused until Task 5 and carries `#[allow(dead_code)]`
until then, the same way `BaseGrid::open` did through slice 1.

**Consumes:** `BaseGrid::{cell, walkable, open}`, `Game::attack_range`,
`Game::effective_atk`, `Game::grant_loot`, `resources::GameClock::tick`,
`resources::GameRng`, `tuning::PLAYER_UNARMED_DAMAGE`.

- [ ] **Step 1: Write the failing tests.** In `tests/base_space.rs`, using
      `support::stand_in_base`:
  - `a_wall_opens_after_the_swings_its_durability_implies` — stand beside
    solid rock, step into it repeatedly, assert the cell is still solid until
    the swing on which accumulated damage reaches `BASE_ROCK_DURABILITY`, and
    `BaseGrid::cell` is `Open` after it. Compute the expected swing count from
    the constants, never hardcode 3 — the point is that retuning durability
    retunes the test with it.
  - `a_swing_at_rock_does_not_move_the_party` — the locale's coordinates are
    unchanged by a swing that does not break through.
  - `identical_swings_at_rock_do_identical_damage` — two fresh walls, same
    player, take the same damage. This is `attack_nest`'s determinism rule and
    the reason mining does not go through `battle::resolve_attack`.
  - `an_opened_cell_records_the_tick_it_was_opened` — `Open { mined_at }`
    equals `GameClock::tick`, which Task 3 depends on entirely.
  - `a_swing_costs_a_turn` — `GameClock::tick` advances by one per swing.
  - `mining_a_wall_never_pays_more_than_flooring_it_costs` — a census, not a
    sampling test: assert `BASE_MINE_FRAGMENT_CHANCE` is strictly below the
    Core Fragment cost of one `blank_substrate` read from the real assets
    (`ItemDb`, `craftable.cost`). This is settled decision 5 held as an
    assertion so a retune cannot quietly turn the wall into a fragment tap.
- [ ] **Step 2: Run them; confirm each fails for the right reason** —
      `cargo test -p feral-processes-engine base_space`. A test failing to
      compile is not yet a red test; get it to a real assertion failure.
- [ ] **Step 3: Implement.** `DigSite` + `Durability` spawned lazily at the
      struck cell; damage exactly as `game/zone.rs::attack_nest:53-54`
      computes it (weapon band mean via `attack_range` plus `effective_atk`,
      floored at 1); on reaching zero, `BaseGrid::open(x, y, tick)`, despawn
      the site unless `marked` (Task 4 relies on that clause existing), roll
      the fragment, log through `log_kind(MessageKind::Loot, ..)` when it
      lands. The `move_in_base` branch mirrors `move_player`'s nest branch:
      strike, `tick()`, return — **before** the walkable check.
- [ ] **Step 4: Run them; confirm they pass.** Then mutation-prove: drop the
      `.max(1)` damage floor and confirm nothing fails (it should not — that
      floor is only reachable at zero ATK), then invert the `== 0` break
      condition and confirm the first test fails. Restore.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 2: Laying a VectorStasis Tile

**Files:**
- Modify: `crates/engine/src/game/base/building.rs`
- Modify: `crates/app-core/src/app/playing.rs` — the `T` key, base-only
- Test: `crates/engine/src/tests/base_space.rs`,
  `crates/app-core/src/tests/building.rs`

**Interfaces produced:**

```rust
impl Game {
    /// Floors the `Open` cell the party is standing on, spending one
    /// `blank_substrate` from the player's `Inventory`.
    pub fn lay_tile(&mut self) -> Result<(), String>;
}
```

**Consumes:** Task 1's `BaseGrid` states, `items::ids::BLANK_SUBSTRATE`,
`Game::base_pos`, `Game::require_base`, `components::Inventory::{count, take}`.

Pays from the player's `Inventory` because that is where `place_structure`
pays every build cost from (`building.rs:127-132`) — one store, not two.

- [ ] **Step 1: Write the failing tests.**
  - `laying_a_tile_spends_exactly_one_substrate` — and leaves the rest.
  - `a_tile_turns_the_cell_you_stand_on_into_floor`.
  - `laying_a_tile_without_substrate_refuses_and_spends_nothing`.
  - `laying_a_tile_on_floor_refuses_in_different_words_from_having_no_substrate`
    — assert the two refusal strings share no wording. Two different errands
    for the player; CLAUDE.md's `NoPost::BoxedIn`/`NoRoute` reasoning applied
    one level down.
  - `laying_a_tile_on_the_surface_refuses` — `require_base`, not
    `require_surface`. The test for whether a reader needs a guard is whether
    it *claims something about where the party is*, and this one does.
  - `the_laid_tile_is_named_a_vectorstasis_tile` — the success log names it,
    and the substrate is still called Blank Substrate in the inventory. This
    is settled decision 8, and it is the only thing pinning the player's word
    for the laid form: "VectorStasis Tile" is what you lay, `BaseCell::Floor`
    stays the code's name for it, exactly as "GC Entropy Sweep" is the
    player's word for a raid. Nothing in `assets/` changes — no new item.
  - app-core: `t_lays_a_tile_in_base_space_and_does_nothing_on_the_surface`
    — the key reaches `Game::lay_tile` and nothing else.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement**, refusals first (fail fast), then the spend, then
      `BaseGrid::lay_floor`.
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove the spend by
      removing the `Inventory::take` call and confirming the first test fails.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 3: Entropy on the frontier

**Files:**
- Create: `crates/engine/src/game/base/entropy.rs`
- Modify: `crates/engine/src/game/base/mod.rs`, `crates/engine/src/tuning.rs`,
  `crates/engine/src/game/lifecycle.rs:246-275` (`build_schedule`)
- Modify: `crates/engine/src/base_grid.rs` — whatever `revert` needs
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces produced:**

```rust
// game/base/entropy.rs
pub(crate) fn base_entropy_system(
    grid: ResMut<BaseGrid>,
    clock: Res<GameClock>,
    locale: Res<Locale>,
    occupants: Query<&Position, With<Task>>,
);

// tuning.rs
pub const BASE_ENTROPY_REFILL_TICKS: u64 = 300;
```

Registered **unchained** in `build_schedule` — it is the only writer of
`BaseGrid` in the schedule, so it shares no mutable state with anything there.
State that reason in the registration comment the way the existing entries do;
`RunFeats`' two systems are the precedent for an unchained pair and for why
the reason has to be written down.

- [ ] **Step 1: Write the failing tests.**
  - `an_unfloored_cell_reverts_after_the_entropy_window` — and is **absent**
    from `BaseGrid` afterwards, not chipped rock. The wall re-knits whole.
  - `a_cell_the_party_is_standing_on_never_reverts` — hold the party there
    past the window. This is what keeps "party inside rock" unreachable *by
    construction*, which is the whole reason the occupancy clause exists.
  - `a_cell_a_posted_program_is_standing_on_never_reverts`.
  - `a_floored_cell_never_reverts`, held well past the window.
  - `a_cell_reverts_only_after_the_window_not_on_the_tick_it_hits_it` — pins
    the comparison's direction.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement.** Collect the doomed coordinates first, then write
      — do not remove from the map while iterating it.
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove by deleting the
      occupancy clause and confirming the two occupancy tests fail.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 4: Marks

The verb the build mode drives, engine-side and headlessly testable before any
UI exists.

**Files:**
- Modify: `crates/engine/src/game/base_space.rs`
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces produced:**

```rust
impl Game {
    /// Marks — or clears — every cell in the inclusive box spanned by `a`
    /// and `b`. Which of the two it does is decided by the cell at `a`: an
    /// anchor already marked clears the box, an unmarked one marks it.
    pub fn toggle_mark_box(&mut self, a: (i32, i32), b: (i32, i32));
    /// Every marked cell, for the renderer. Sorted, for a stable draw order.
    pub fn marked_cells(&self) -> Vec<(i32, i32)>;
}
```

**Consumes:** Task 1's `DigSite`, `dig_site_at`.

A marked **solid** cell needs a `DigSite` with full `Durability`; a marked
**`Open`** cell needs one whose durability is already spent. A `Floor` cell
takes no mark at all — there is nothing left to do to it.

- [ ] **Step 1: Write the failing tests.**
  - `marking_a_box_marks_every_solid_cell_in_it`, including the far corner
    when `b` is up-left of `a` (the box is normalised, not assumed ordered).
  - `an_anchor_on_a_marked_cell_clears_the_box_instead_of_marking_it`.
  - `marking_a_floor_cell_does_nothing` — no site spawned, nothing to draw.
  - `a_mark_survives_the_cut_and_clears_when_the_cell_is_floored` — the whole
    of settled decision 4 in one test: mark a wall, cut it by hand, assert
    the cell is `Open` **and still marked**, floor it, assert the mark is
    gone and no `DigSite` entity is left behind.
  - `an_unmarked_wall_leaves_no_entity_behind_when_it_is_cut` — the leak
    check on Task 1's despawn clause.
  - `marked_cells_is_sorted` — the renderer draws in a stable order run to
    run, for the reason `Stock` keys by `BTreeMap`.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove the anchor rule
      by making the box always mark, and confirm the clear test fails.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 5: The crew

**Files:**
- Modify: `crates/engine/src/components.rs` — `TaskKind::Excavate`
- Modify: `crates/engine/src/game/base/work_orders.rs` —
  `schedule_base_labour` (from line ~596)
- Modify: `crates/engine/src/game/base/building.rs` — `post_digger`
- Modify: `crates/engine/src/systems.rs` — `task_progress_system`'s new arm
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/base_space.rs` (new dig-crew section)

**Interfaces produced:**

```rust
pub enum TaskKind { GatherResource, Guard, Excavate }

impl Game {
    pub(crate) fn post_digger(&mut self, worker: Entity, site: Entity);
    /// Every dig site a body could usefully be sent to this tick, lowest
    /// priority, appended after work-order and standing wants.
    pub(crate) fn dig_wants(&mut self) -> Vec<(Entity, TaskKind)>;
}

// tuning.rs — a crew cycle is one swing; the worker's own ATK is the damage,
// so a stronger program digs faster exactly as a stronger player does.
pub const BASE_DIG_TICKS_PER_SWING: u32 = 12;
```

**Consumes:** `hauling::post_reach(grid, from, target, blocked,
pocket_radius) -> Result<(), NoPost>`, `hauling::NoPost::{BoxedIn, NoRoute}`,
`Game::effective_atk`, `Game::base_staff`, `Game::lay_tile`'s substrate spend,
Task 4's marks.

**The two reach outcomes are not symmetrical, and this is the task's whole
subtlety:**

- `NoPost::BoxedIn` — nothing can stand beside the cell. For a dig site that
  is the *normal* interior of any block the player marked, and it resolves
  itself as the shell comes down. **Skip it silently.** A complaint here fires
  every tick for every interior cell of every marked block.
- `NoPost::NoRoute` — something can stand beside it, and no body can get
  there. Only the player can fix that. **Complain, once**, then set
  `DigSite::announced_stuck` and stay quiet; clear the flag when a route
  appears again. This is `systems::set_machine_status`' rule — one writer,
  logging only on transition, because entering a state is news and staying in
  it is not.

- [ ] **Step 1: Write the failing tests.**
  - `a_crew_cuts_a_marked_wall_without_the_player` — post a body, run ticks,
    assert the cell opens.
  - `a_crew_floors_a_marked_cell_after_cutting_it` — and spends exactly one
    substrate doing it.
  - `a_dig_job_never_takes_a_body_off_a_work_order` — a base with one body and
    both an order and a mark works the order. Settled decision 7, and the
    reason digging cannot starve production.
  - `a_dig_job_is_taken_when_there_is_a_spare_body`.
  - `an_unreachable_dig_site_complains_exactly_once` — count matching log
    lines across many ticks; assert 1, not "at least 1".
  - `a_dig_site_with_no_exposed_face_never_complains` — mark a 3x3 block,
    assert the centre cell produces no log line at all while its shell stands.
  - `a_site_that_becomes_reachable_again_can_complain_again` — the flag
    clears, or the second stall is silent forever.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement.** Append dig wants **after** `standing_wants` in
      `schedule_base_labour` so `truncate(staff.len())` cuts digging first —
      the priority is expressed by position in that list, exactly as standing
      jobs already express theirs. `task_progress_system` gains an `Excavate`
      arm; it already writes `Task::progress` and is `.chain()`ed with
      `assembler_system`, so no scheduling change is needed.
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove the priority by
      appending dig wants *before* standing wants and confirming the work-order
      test fails; mutation-prove the once-only complaint by removing the flag
      write and confirming the count test fails.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 6: Saving what you dug and what you marked

**Files:**
- Modify: `crates/engine/src/save.rs` — `DigSiteSave`, `SaveData::dig_sites`,
  both the write and the restore path
- Test: `crates/engine/src/save.rs`'s own `#[cfg(test)] mod tests` (where
  `a_save_file_written_before_a_defaulted_field_existed_still_loads` already
  lives, ~line 1060) for the encoding half, and
  `crates/engine/src/tests/base_space.rs` for the game-level round trip.
  There is no `tests/save.rs`; don't create one.

**Interfaces produced:**

```rust
pub struct DigSiteSave { pub position: (i32, i32), pub durability: u32, pub marked: bool }
// SaveData gains:  #[serde(default)] pub dig_sites: Vec<DigSiteSave>,
```

`SAVE_FORMAT_VERSION` **stays 32** — see Global Constraints. Do not touch it,
and do not recapture templates.

- [ ] **Step 1: Write the failing tests.**
  - `a_half_cut_wall_survives_a_save_round_trip` — through a real
    `save_to_file`/`load_from_file` on a temp path, **not** only the RON
    round trip: a round trip cannot catch a `#[serde(skip)]`.
  - `a_mark_survives_a_save_round_trip`.
  - `a_save_written_before_dig_sites_existed_still_loads` — the
    `#[serde(default)]` guarantee, modelled on the existing
    `a_save_file_written_before_a_defaulted_field_existed_still_loads`.
  - Confirm `every_checked_in_template_still_loads` stays green untouched.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement** both directions.
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove by dropping
      `marked` from the restore path and confirming the mark test fails.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 7: Excavation plan — the build mode

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode::Excavate` plus the cursor and
  anchor state on `App`
- Modify: `crates/app-core/src/app/playing.rs` — `m` enters, base only
- Create: `crates/app-core/src/app/excavate.rs` — the mode's key handling
- Modify: `crates/gui/src/render/base.rs` — mark tint, cursor, box preview
- Modify: `crates/gui/src/render/meta.rs:209` — `HELP_ROWS`, the one help
  table, read back by the test at ~line 297
- Test: `crates/app-core/src/tests/` (new file), plus the existing gui help
  text test

**Consumes:** `Game::toggle_mark_box`, `Game::marked_cells`, `Game::base_pos`.

**Interaction, exactly:** `m` from `Mode::Playing` while in base space opens
it; cursor starts on the party's cell; `hjkl`/arrows move it; `space` drops
the anchor, and with an anchor down the cursor's movement previews the box;
`space` again commits through `toggle_mark_box`; `esc` leaves, dropping any
anchor first so one press is never two undos. **Nothing here ticks** — this
is a mode, not an action, so marking a wing of the base costs no game time.

Draw the marks through the existing `Painter` operations. `paint.rs` must not
gain a fifteenth operation for this; tint and glyph over the grid
`render/base.rs` already draws is the whole of it.

- [ ] **Step 1: Write the failing tests** (app-core, headless):
  - `m_opens_excavation_plan_in_base_space_and_does_nothing_on_the_surface`.
  - `the_cursor_starts_on_the_party_and_moves_with_the_direction_keys`.
  - `committing_a_box_reaches_toggle_mark_box` — assert on the marks the
    engine now holds, not on a call count.
  - `excavation_plan_never_ticks_the_game` — `GameClock::tick` is unchanged
    across opening, moving, committing and leaving.
  - `esc_with_an_anchor_down_drops_the_anchor_and_stays_in_the_mode`.
  - gui: the help text names the key; and the existing test holding the help
    text to never naming the `W` easter egg must stay green.
- [ ] **Step 2: Run them; confirm they fail.**
- [ ] **Step 3: Implement**, app-core first, then the renderer.
- [ ] **Step 4: Run them; confirm they pass.** Mutation-prove the no-tick rule
      by adding a `tick()` to the commit path and confirming that test fails.
- [ ] **Step 5: fmt, clippy, commit.**

---

### Task 8: The documentation the seams owe

Not optional and not a chore: two of these are rules that go silently wrong
later if they are only in a commit message.

**Files:**
- Modify: `docs/seams.md` — the argument, at length
- Modify: `CLAUDE.md` **and** `AGENTS.md` — the one-line rules. They are
  gitignored twins with no tracking to catch drift: edit `CLAUDE.md`, then
  `cp CLAUDE.md AGENTS.md`.
- Modify: `CHANGELOG.md` — a new section, at the version the merge takes

**The seams to write:**
1. **`DigSite` is a non-`Structure` entity with a base-space `Position`.**
   `Structure` was the space tag with posted programs as its only exception;
   this is the second. A `Position` read in the wrong coordinate space fails
   silently, and 0.13.0 shipped two fixes for exactly that.
2. **A mark is one verb whose meaning is derived from the cell under it**, and
   the anchor cell decides mark-versus-clear. Someone will otherwise add a
   second designation kind and an erase verb.
3. **A dig site's two unreachable states are not symmetrical** — `BoxedIn` is
   silent because it is the normal interior of a marked block, `NoRoute`
   complains once. Cross-reference `set_machine_status`.
4. **Dig wants are appended last in `schedule_base_labour`, and the priority
   *is* the position in that list.** A future want inserted above them
   silently starves production.
5. **Mining does not go through `battle::resolve_attack`**, for
   `attack_nest`'s stated reason.

- [ ] **Step 1: Write `docs/seams.md` entries**, with the measurement and what
      was rejected, matching the file's existing voice.
- [ ] **Step 2: Add the one-line rules to `CLAUDE.md`** under **The base**,
      then `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 3: Grep for claims this slice falsifies** — `rg -n "Heap
      Pillar|Heap Block|slab"` across `docs/` and `assets/*/README.md`, and
      the spec's own superseded text. Fix what is now untrue. Do **not** touch
      `docs/manual.md`, `README.md`, or `TODO.md` — all three are carved out.
- [ ] **Step 4: Verify the test count** in CLAUDE.md's build section against a
      real `cargo test --workspace` run and update it.
- [ ] **Step 5: Commit.**

---

## Final gates, in order

- [ ] `cargo fmt --check` and `cargo clippy --workspace` clean.
- [ ] `cargo test --workspace` green. If many tests fail at once with
      `NotFound` on an assets path, that is stale build artifacts from the
      petmud rename, not 150 bugs: `cargo clean -p feral-processes-engine -p
      feral-processes-app-core`, never a full `cargo clean`.
- [ ] `cargo test -p feral-processes-engine balance_sim` — expected to be
      untouched. `balance_sim` has no base term and gates none of this; a
      moved curve here means something unrelated changed and wants
      explaining, not re-baselining.
- [ ] **Play it.** A green suite is not evidence of play, and all three tuning
      values are unmeasured guesses. `cargo run -- --template chains` opens on
      a running base. What to answer: does a wall take a satisfying number of
      swings early and does that decay well; is 300 ticks of entropy generous
      enough that a normal dig-then-floor cycle never loses ground, and short
      enough that over-digging is felt at all; does marking a block and
      walking away actually produce floor.
- [ ] Record what play said under `docs/measurements/`, per that directory's
      `README.md` — the bar is that something was run, the data is gone, and
      a decision depends on it. All three knobs clear it.
- [ ] Whole-branch code review before landing. Per CLAUDE.md this one is not
      optional even when per-task gates are skipped, and it earns opus.
