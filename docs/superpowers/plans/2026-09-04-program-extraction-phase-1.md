# Program Extraction — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A defeated wild program is left as a carried, instanced downed
program; a carried tool extracts materials from it. Material drops move
behind that door.

**Architecture:** A third player store (`DownedPrograms`) beside `Inventory`
and `GearCopies`, a new moddable `assets/tools/` catalogue, and one
extraction door. Phase 1 ships no research, no structures, no routine
category and no `Mode::Tools` screen — those are phases 2 and 3.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-09-04-program-extraction-design.md` —
read it first; this plan argues from its ten numbered decisions and does not
restate them.

## Global Constraints

- **No `save::SAVE_FORMAT_VERSION` bump.** Every new save field is additive
  behind `#[serde(default)]`. If you believe a bump is needed, stop and say
  so rather than bumping.
- **No content in Rust.** Tools are `assets/tools/*.ron`. A malformed file is
  skipped with a logged warning; an absent directory loads silently empty.
  `MemoryDb`/`AbilityDb`'s `load_dir` is the pattern.
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub
  const`, never inline in a formula.
- **`Inventory` must not gain an instance rule.** It stays
  `Vec<(ItemId, u32)>` and stays the plain-copy store.
- **Every refusal lands before anything is spent**, asserted per refusal.
- Follow the repo's comment discipline: comments say *why*, never *what*.
- Gates for every task: `cargo fmt`, `cargo clippy --workspace` (no new
  warnings), `cargo test -p feral-processes-engine <name>` while iterating,
  and `cargo test --workspace` before the task's commit.
- The full plan is TDD: the failing test is written and *seen to fail*
  before the implementation.

---

### Task 1: The object and its store

**Files:**
- Modify: `crates/engine/src/items.rs` — `DownedProgram` beside `GearCopy`
- Modify: `crates/engine/src/components.rs` — `DownedPrograms(Vec<DownedProgram>)`, player-only, documented as the third store and why (`GearCopies`' doc comment at `:471` is the model)
- Modify: `crates/engine/src/tuning.rs` — `MAX_DOWNED_PROGRAMS`, `CONDITION_BASE`, `CONDITION_PER_RARITY_STEP`, `CONDITION_BOSS_BONUS`, `FIGHT_CONDITION_WEIGHT` (ships `0.0`)
- Modify: `crates/engine/src/save.rs` — `PlayerSave::downed_programs`, `#[serde(default)]`, written and drained in both directions
- Test: `crates/engine/src/tests/extraction.rs` (new), registered in `tests/mod.rs`

**Interfaces produced:**
- `items::DownedProgram { species: SpeciesId, level: u32, rarity: Rarity, boss: bool, condition: u8 }`, `Clone + Debug + PartialEq`, serde
- `DownedProgram::grade(&self) -> f32` — the one fold of condition, rarity and level. Every yield formula calls it; nothing re-folds the axes.
- `components::DownedPrograms(pub Vec<DownedProgram>)`

**Steps:**

- [ ] **Test first.** Two tests: `grade` rises monotonically with each of the
  three axes held against the other two; a save→load round trip preserves a
  store of three distinct programs. The round trip must be save→load, not a
  RON round trip — a RON round trip cannot catch a `#[serde(skip)]`.
- [ ] Run them; both fail to compile.
- [ ] Implement. `rarity`'s rung index comes from its position in
  `Rarity::ALL` — do not add a second ladder.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `feat(extraction): DownedProgram, its store and its save field`

---

### Task 2: The tool catalogue

**Files:**
- Create: `crates/engine/src/tools.rs`, registered in `lib.rs`
- Create: `assets/tools/README.md` — the schema doc, in the voice of `assets/abilities/README.md`
- Create: `assets/tools/salvage_clamp.ron` (tier 1, `Materials`, the starter) and `assets/tools/core_tap.ron` (tier 2, `Cores`)
- Modify: `crates/engine/src/tests/assets.rs` — the censuses
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces produced:**
- `tools::ToolId` — a string newtype, `items::ItemId`'s shape
- `tools::ToolCategory { Materials, Parts, Cores, Routines }` — a Rust enum, exhaustively matched wherever it is read (`cell_mark`'s rule)
- `tools::ToolDef { id, name, description, category, yields: Vec<(ItemId, f32)>, tier: u32, ticks: u64 }`
- `tools::ToolDb` with `load_dir(&Path) -> io::Result<(Self, Vec<String>)>`, `get`, `all`, `iter` sorted by id
- Loaded into the world as a resource at `Game::new` and `Game::load`, beside `AbilityDb`

**Steps:**

- [ ] **Test first.** In `assets.rs`, five censuses, each failing the build:
  every shipped tool's `yields` resolve to real items; every non-`Routines`
  tool has a non-empty `yields`; weights are finite and positive;
  `STARTER_TOOL_ID` resolves; ids are unique. In `extraction.rs`: an absent
  directory loads empty with no error, and a malformed file is skipped with a
  warning while its well-formed neighbour still loads.
- [ ] Run; fail.
- [ ] Implement `ToolDb::load_dir` following `AbilityDb::load_dir` exactly.
  Author the two `.ron` files and the README. `salvage_clamp`'s yields are
  tuned in Task 6 — ship the pool now, the numbers there.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `feat(extraction): the assets/tools catalogue`

---

### Task 3: Slots and the starter tool

**Files:**
- Modify: `crates/engine/src/components.rs` — `Tools(pub Vec<ToolId>)`
- Modify: `crates/engine/src/tools.rs` — `player_tool_slots(level) -> usize`
- Modify: `crates/engine/src/tuning.rs` — `TOOL_SLOT_BASE`, `TOOL_SLOT_PER_LEVEL`, `TOOL_SLOT_CAP`, `STARTER_TOOL_ID`
- Modify: `crates/engine/src/game/creation.rs` — grant at `Game::new` only
- Modify: `crates/engine/src/save.rs` — `PlayerSave::tools`, `#[serde(default)]`
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces consumed:** Task 2's `ToolId`, `ToolDb`.
**Interfaces produced:** `Game::installed_tools(&self) -> Vec<ToolDef>` (slot order), `tools::player_tool_slots`.

**Steps:**

- [ ] **Test first.** Slot count matches `abilities::player_routine_slots`'
  shape — grows on the per-level step, clamped at the cap. A new game has
  the starter tool in slot 1. A `Game::load` of a save that already has tools
  does **not** re-grant it (the profile rule: pay at `new`, never at `load`).
  A save→load preserves the loadout.
- [ ] Run; fail.
- [ ] Implement. `player_tool_slots` mirrors `abilities::routine_slots`'
  private helper rather than restating the clamp.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `feat(extraction): tool slots and the starter tool`

---

### Task 4: Kills leave programs

**Files:**
- Modify: `crates/engine/src/game/combat_rewards.rs` — delete `roll_work_resource_drop`, add `leave_downed_program`; `award_loot`'s call site; the boss branch's floor
- Modify: `crates/engine/src/game/zone.rs:80` — `grant_nest_cache` leaves programs
- Modify: `crates/engine/src/game/sortie.rs` — the sortie's kill loop calls the same function rather than a copy (it called `roll_work_resource_drop` for exactly that reason); phase 1 grants to the player on return, `Sortie::programs` is phase 3
- Modify: `crates/engine/src/tuning.rs` — `NEST_CACHE_PROGRAM_COUNT`, `BOSS_CONDITION_FLOOR`, `BOSS_RARITY_FLOOR`
- Test: `crates/engine/src/tests/extraction.rs`, `crates/engine/src/tests/combat_rewards.rs`

**Interfaces produced:**
- `Game::leave_downed_program(&mut self, wild: Entity) -> bool` — `false` when the store is full. The one writer of `DownedPrograms` from a defeat.

**Steps:**

- [ ] **Test first.** A kill leaves exactly one program carrying that
  creature's species, level and rarity. A boss's program is at or above both
  floors. A nest leaves `NEST_CACHE_PROGRAM_COUNT`. A full store refuses,
  logs one line, and **destroys nothing already held**. With
  `FIGHT_CONDITION_WEIGHT` at `0.0`, condition is independent of the killing
  blow's size — assert two kills differing only in overkill produce equal
  condition.
- [ ] Run; fail.
- [ ] Implement. `Perk::Teardown`'s term moves into Task 5's yield formula —
  leave a `TODO`-free comment in `perks.rs` pointing at its new site, and
  keep `every_perk_has_a_query_that_answers_what_it_is_worth` exhaustive.
  Removing `roll_work_resource_drop` changes no `GameRng` draw count at the
  kill site unless you add one: reuse the single existing draw.
- [ ] Run; green. `cargo test --workspace` — expect seeded fixtures elsewhere
  to move if you changed the draw count. If any do, **stop and report**
  rather than re-seeding them.
- [ ] Commit: `feat(extraction): kills, nests and bosses leave downed programs`

---

### Task 5: The extraction door

**Files:**
- Create: `crates/engine/src/game/extraction.rs`, registered in `game/mod.rs`
- Modify: `crates/engine/src/game/turn.rs` — `LootSource::Extract`
- Modify: `crates/engine/src/species.rs` — `SpeciesDef::rich_in: Option<ItemId>`, `#[serde(default)]`, falling back to `work_resource`
- Modify: `assets/species/README.md` — document `rich_in` in the same change
- Modify: `crates/engine/src/tuning.rs` — `TOOL_BASE_UNITS`, `RICH_IN_UNITS`, the tier scale
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces consumed:** Tasks 1–4.
**Interfaces produced:**
- `Game::extraction_yield(&self, program: &DownedProgram, tool: &ToolDef) -> Vec<(ItemId, u32)>` — the one derivation, called by the act **and** by the screen's preview
- `Game::extract_program(&mut self, index: usize, tool: &ToolId) -> Result<(), String>` — the one door
- `Game::rich_in(&self, species: &SpeciesId) -> Option<ItemId>`

**Steps:**

- [ ] **Test first.** Extraction removes the program and grants the yield.
  The previewed figure equals the granted one — call `extraction_yield`, then
  extract, then compare against the inventory delta. `rich_in` falls back to
  `work_resource` for every shipped species. A higher-grade program yields
  more than a lower one, all else equal. A higher-tier tool yields more than
  a lower one on the same program. **One test per refusal** — game over,
  active battle, index out of range, tool not installed — each asserting the
  program still exists and the pack is unchanged; a single test over one path
  passes against the others.
- [ ] Run; fail.
- [ ] Implement. Refusals first, then removal, then `grant_loot(..,
  LootSource::Extract)`, then the log line, then `self.tick()` for
  `tool.ticks`. Draw items by weight from one `GameRng` draw per unit.
  `Perk::Teardown` is *added* to the unit count, never drawn for.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `feat(extraction): extraction_yield and the extract_program door`

---

### Task 6: Drop neutrality

**Files:**
- Modify: `assets/tools/salvage_clamp.ron` — the tuned pool and weights
- Modify: `crates/engine/src/tuning.rs` — `TOOL_BASE_UNITS` fitted here
- Test: `crates/engine/src/tests/extraction.rs`

**Steps:**

- [ ] **Test first.** The starter tool's expected units from a median program
  (ordinary rarity, `CONDITION_BASE`, level 1) equals the mean of
  `tuning::WORK_RESOURCE_DROP` within one unit. Compute the expectation
  RNG-free from the constants — do not sample. This is spec decision 8 and
  it is the phase's only economy gate; `balance_sim` models no loot and gates
  none of this.
- [ ] Run; fail.
- [ ] Fit `TOOL_BASE_UNITS` and the clamp's weights until it passes. Do not
  change `WORK_RESOURCE_DROP`.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `test(extraction): the starter tool is drop-neutral`

---

### Task 7: The screen

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode::DownedPrograms`, its entry in `ALL_MODES` and `needs_status_banner`, the key that opens it from the pack, the selection state and the row count
- Create: `crates/gui/src/render/extraction.rs`, registered in `render/mod.rs`
- Modify: `crates/engine/src/views.rs` — `DownedProgramRow`, the one derivation of what a row says
- Test: `crates/app-core/src/tests/extraction.rs` (new), `crates/engine/src/tests/extraction.rs`

**Interfaces produced:**
- `views::DownedProgramRow { name, level, rarity, condition, boss, grade }`
- `Game::downed_program_rows(&self) -> Vec<DownedProgramRow>`
- `Game::extraction_options(&self, index: usize) -> Vec<(ToolId, Vec<(ItemId, u32)>)>` — each installed tool and what it would give, built from `extraction_yield`

**Steps:**

- [ ] **Test first.** The screen has **no scroll**, so height is a layout
  constraint: a test asserting `MAX_DOWNED_PROGRAMS` rows plus the header fit
  at 1280x720, verified by mutation (raise the cap by one and watch it fail).
  Row count comes from the engine, not the renderer — assert app-core and the
  view agree. The row's quoted yield equals what extracting grants.
- [ ] Run; fail.
- [ ] Implement. The engine owns the row count and gui draws it; any per-row
  transform lives in the engine (`message_history`'s rule). Draw through
  `Painter` only — `render/` names no graphics library.
- [ ] Run; green. `cargo test --workspace`.
- [ ] Commit: `feat(extraction): the downed programs screen`

---

### Task 8: Documentation and the release

**Files:**
- Modify: `CHANGELOG.md` — a new `## X.Y.Z` section; the digit is decided by
  `CHANGELOG.md`'s own preamble. No save format break here, so this is a
  minor at most.
- Modify: `Cargo.toml` — the workspace version bump, **at the merge, not on
  the branch**
- Modify: `CLAUDE.md` and `AGENTS.md` — one sentence per new seam under
  "Load-bearing seams"; they are gitignored twins, so edit `CLAUDE.md` then
  `cp CLAUDE.md AGENTS.md`
- Modify: `docs/seams.md` — the argument behind each new seam
- Modify: `.claude/skills/seams/` — the trap behind each
- Do **not** touch `docs/manual.md` or the root `README.md`; both are carved
  out of the doc obligation.

**Steps:**

- [ ] Add the three seam sentences: the third store and why it is not
  `Inventory`; `extract_program` as the one door; `extraction_yield` as the
  one derivation the preview and the grant share.
- [ ] Write the `docs/seams.md` entries and the skill reference file. A new
  seam is three writes and the skill documents the order.
- [ ] `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
- [ ] Commit: `docs(extraction): the three new seams`

---

## Not in this phase

Named so an implementer does not build them speculatively: research unlocks,
`forge_tool`/`install_tool`, `Mode::Tools`, the `extracts_programs`
structure, the routine category, `Sortie::programs`, and the bulk
work-order path. Spec §8 has their phases.
