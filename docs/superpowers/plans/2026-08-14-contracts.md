# Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named, finite objectives with a payout, issued by a structure you build, so a session has a shape beyond "go deeper".

**Architecture:** Contracts are authored `.ron` data loaded into a `ContractDb`, exactly as achievements are. A Contract Broker structure issues them; its offers are *derived* from a seeded epoch rather than saved, the way a Stack market's shelf is. Accepting one writes an `ActiveContract` into a saved resource. One system, `contract_system`, is the only writer of progress — it polls four state-shaped objectives and drains one new event field for the fifth.

**Tech Stack:** Rust, `bevy_ecs` (engine only), `serde`/`ron` for assets and saves, `rand::StdRng` for seeded derivation.

**Spec:** `docs/superpowers/specs/2026-08-14-contracts-design.md` — read it before Task 1. It carries the arguments this plan only cites.

## Plan conventions — read this first

This plan follows `CLAUDE.md`'s **Process weight** rule, which overrides the writing-plans skill's default: **it does not spell out implementations.** You get the file list, the exact interface each task must produce, the intent of every test, and the gates to run. Writing the code here would pay for the feature twice and leave you no room to notice the plan is wrong. If a task's design looks wrong to you when you get there, say so rather than implementing it faithfully.

Code blocks appear only for signatures other tasks depend on, and for the two or three genuinely non-obvious mechanics.

## Global Constraints

- **TDD.** Failing test first, watch it fail, minimal implementation, watch it pass, commit. Every task.
- **Full suite is the gate.** `cargo test --workspace` before any task is called done. It is ~2233 tests and the engine suite runs ~24s.
- **`cargo fmt` and `cargo clippy --workspace` after every change.** Fix warnings, don't silence them.
- **No `SAVE_FORMAT_VERSION` bump.** Every save change here is a *field added* to a named struct behind `#[serde(default)]`, which since v29 loads out of a file written before it existed. If you find yourself wanting to bump it, stop — you have made a field positional or changed a meaning, and that is a different change.
- **Named structs, never positional tuples,** for anything that reaches the save. RON parses `(` in a struct position as the start of named fields, so a `Vec<(A, B, C)>` can never be widened. This already cost this repo two legacy fields.
- **Moddability.** Contracts are data. Never hardcode a contract in Rust. New field on `ContractDef` → `#[serde(default)]` and a same-change update to `assets/contracts/README.md`.
- **A malformed `.ron` is skipped with a returned warning, never a panic.** Follow `AchievementDb::load_dir`.
- **Comment discipline.** Comments explain *why*. The seams in this feature — the derived board, the two-field drain, the whole-def save — each need one, and the spec has the argument to draw from.
- **Vocabulary.** Player-facing and code-facing: *contract*, *Contract Broker*. Never "job" (a species class's post behaviour) or "cronjob" (a posted program) or "quest"/"mission".
- **No occult naming** in any authored contract text.
- **Commit freely** as work reaches green. Do not push.

## Reference points in the existing code

Read these before the task that cites them. They are the patterns to follow, not to invent alongside.

| What | Where |
|---|---|
| A content dir, its `Db`, and load-time validation | `crates/engine/src/achievements.rs` |
| One system deciding what was earned, polling + one event field | `crates/engine/src/game/achievements.rs` |
| The per-tick event queue it drains | `resources::RunFeats` |
| A shelf derived from a seed, never saved | `crates/engine/src/game/stack_market.rs` |
| Asset loading and the `AssetDbs` struct | `crates/engine/src/game/lifecycle.rs:1189` |
| Schedule registration | `crates/engine/src/game/lifecycle.rs:205-222` |
| Where a kill is finalised and `RunFeats` is written | `game/combat_rewards.rs:374` (`award_loot`) |
| Finding a nearby structure by an `EntityView` flag | `crates/app-core/src/app/trade.rs:52` |
| A group-menu row and its `available` predicate | `crates/app-core/src/app/group_menu.rs:40` |
| Engine test fixtures | `crates/engine/src/tests/support.rs` |

---

# Phase 1 — the type and its catalogue

Engine only. No game wiring: at the end of this phase contracts parse and validate, and nothing reads them yet.

### Task 1: `contracts.rs` — types, loading, validation

**Files:**
- Create: `crates/engine/src/contracts.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod contracts;`)
- Test: `crates/engine/src/tests/contracts.rs` (new), registered in `crates/engine/src/tests/mod.rs`

**Interfaces — Produces:**

```rust
pub struct ContractId(pub String);   // #[serde(transparent)], Ord, Display, From<&str>

pub enum Objective {
    Kill { species: Option<String>, count: u32 },
    Deliver { item: ItemId, count: u32 },
    Descend { depth: u32 },
    Breach { zone: u32 },
    Build { structure: StructureId },
}

impl Objective {
    /// Units of progress that complete this objective. `count` for the two
    /// counting variants, 1 for the three state-shaped ones — so every
    /// contract displays and completes through one `progress >= target()`
    /// rule and no caller branches on the variant to ask "am I done".
    pub fn target(&self) -> u32;
}

pub enum Reward { Credits(u32), Item(ItemId, u32), Xp(u32) }

pub struct ContractDef {
    pub id: ContractId,
    pub name: String,
    pub description: String,
    pub objective: Objective,
    pub reward: Vec<Reward>,
    #[serde(default)] pub min_zone: u32,
    #[serde(default)] pub repeatable: bool,
}

pub struct ContractDb { /* BTreeMap<ContractId, ContractDef> */ }

impl ContractDb {
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>;
    pub fn get(&self, id: &ContractId) -> Option<&ContractDef>;
    pub fn iter(&self) -> impl Iterator<Item = &ContractDef>;
}
```

`ContractDb` derives `Resource` and `Default`. `BTreeMap` not `HashMap`, for `AchievementDb`'s stated reason: the screen lists that iteration order and a `HashMap` would reshuffle it between runs.

**`Reward::PortalFragments` must not exist.** Absent, not unused — see the spec. A mod file cannot reach a variant that isn't there.

- [ ] **Step 1: Write the failing tests.** In `tests/contracts.rs`, write a temp-dir loader test per behaviour. Each writes `.ron` files into a `tempfile` dir and asserts on the returned `(db, warnings)`:
  - a well-formed file of each of the five `Objective` variants parses, and `target()` returns `count` for `Kill`/`Deliver` and 1 for the other three
  - `min_zone` and `repeatable` absent parse as 0 and false
  - a file that `ron` refuses is skipped, the db still loads its siblings, and a warning names the path
  - an empty `id` is rejected with a warning
  - a duplicate `id` is rejected with a warning
  - an empty `reward` list is rejected with a warning
  - `Credits(0)`, `Item(_, 0)` and `Xp(0)` are each rejected with a warning — a contract paying nothing is a mistake that reads as a working file, the same rule `Reward::PerkPoints(0)` already has

  Do **not** validate that item/species/structure ids exist here. `ContractDb::load_dir` has no other db in hand, exactly as `AchievementDb::load_dir` defers its `StartingProgram` check. That check is Task 2's census.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine contracts` — expected: does not compile, `contracts` module unresolved.

- [ ] **Step 3: Implement `contracts.rs`.** Follow `achievements.rs` structurally: the id newtype and its impls, the two enums, the def, the db, and `load_dir` accumulating warnings rather than returning early.

- [ ] **Step 4: Run them and watch them pass.** Then `cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`.

- [ ] **Step 5: Commit.** `feat(contracts): the contract type and its catalogue`

### Task 2: the shipped contracts and their census

**Files:**
- Create: `assets/contracts/README.md`
- Create: `assets/contracts/*.ron` — eight contracts
- Test: `crates/engine/src/tests/contracts.rs` (extend)

**Interfaces — Consumes:** `ContractDb::load_dir`, `ContractDef`, `Objective`, `Reward` from Task 1.

- [ ] **Step 1: Write the failing census test.** Loads the *real* `assets/contracts/` via `test_assets_dir()` (see `tests/support.rs`) alongside the real `ItemDb`, `SpeciesDb` and `StructureDb`, and asserts:
  - the directory loads with **zero** warnings
  - every `Objective::Deliver` and `Reward::Item` names an item that exists
  - every `Objective::Kill` with `Some(species)` names a species that exists
  - every `Objective::Build` names a structure that exists
  - every contract has a non-empty `description` — the only place a player is told what to do
  - **no shipped contract's reward mentions `portal_fragment`** by `ItemId`. `Reward::PortalFragments` doesn't exist, but `Reward::Item("portal_fragment", n)` would be the same thing through the back door, and the spec closes that route deliberately. This test is the door.
  - all five `Objective` variants appear at least once across the set, so every code path added in Phase 2 has shipped content exercising it

- [ ] **Step 2: Run it and watch it fail.** `cargo test -p feral-processes-engine contracts` — expected: the assets directory does not exist.

- [ ] **Step 3: Author the eight contracts and the README.** Spread across the five variants and across `min_zone` 0–3. Keep rewards modest — Credits in the tens, XP in the low hundreds; these are opening guesses and the spec says so. The README is the schema reference, on the model of `assets/achievements/README.md`: what a contract is, the full field table, both enum vocabularies, what happens to a malformed file, and what deleting a file does. Say explicitly that Portal Fragments are not a reward and why.

- [ ] **Step 4: Run it and watch it pass.** Then `cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`.

- [ ] **Step 5: Commit.** `feat(contracts): the shipped contract set and its census`

---

# Phase 2 — progress and completion

Engine only. At the end of this phase a contract can be given to a `Game` directly, advance, complete and pay — with no board and no screen.

### Task 3: run state, asset registration, and the save round trip

**Files:**
- Modify: `crates/engine/src/resources.rs`
- Modify: `crates/engine/src/save.rs`
- Modify: `crates/engine/src/game/lifecycle.rs` (`AssetDbs`, the asset loader, both `Game::new` and `Game::load` resource insertion)
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:**

```rust
// resources.rs — saved run state.
pub struct ActiveContract {
    /// The whole resolved def, not an id. A contract file edited or deleted
    /// mid-run must not strand or silently rewrite a contract already
    /// accepted — the argument `EquippedItem` stores a whole `GearCopy`.
    pub def: crate::contracts::ContractDef,
    pub progress: u32,
    pub accepted_tick: u64,
}

#[derive(Resource, Default)]
pub struct ActiveContracts {
    pub active: Vec<ActiveContract>,
    pub done: Vec<crate::contracts::ContractId>,
}
```

```rust
// save.rs — two additive fields on SaveData.
#[serde(default)] pub contracts: Vec<crate::resources::ActiveContract>,
#[serde(default)] pub contracts_done: Vec<crate::contracts::ContractId>,
```

`ContractDb` joins `AssetDbs` as `contracts`, loaded from `assets_dir.join("contracts")` beside the `AchievementDb` line at `lifecycle.rs:1257`, and inserted as a resource on both the `Game::new` and `Game::load` paths.

- [ ] **Step 1: Write the failing tests.**
  - a `Game` with two active contracts at different progress and one completed id, saved and loaded, comes back with all three intact — progress, `accepted_tick`, and the resolved `def`
  - a save file written *without* the two fields (hand-write the RON, omitting them) loads with empty vectors and **no** version bump — this is the compatibility claim, and it is the one that must not be assumed
  - `Game::new` has an empty `ActiveContracts`
  - the real `assets/contracts/` is reachable through the loaded `Game` (assert the db is non-empty)

- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-engine contracts`

- [ ] **Step 3: Implement.** Note `SAVE_FORMAT_VERSION` is **not** touched. If a test tells you otherwise, re-read the constraint above.

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): saved run state and asset registration`

### Task 4: the kill counter

**Files:**
- Modify: `crates/engine/src/resources.rs` (`RunFeats`)
- Modify: `crates/engine/src/game/combat_rewards.rs` (`award_loot`, around line 374)
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:** `RunFeats` gains `pub kills: Vec<String>`, holding the species id of every creature killed this tick.

The record goes in **`award_loot`**, beside the existing `is_boss` push into `RunFeats::bosses_defeated`. That is the one door every kill in the game passes through, and putting the two records side by side is what keeps them from drifting. (The spec says "beside `award_loot` in `finish_member`" — `award_loot` is called *from* `finish_member`, and `award_loot` is the precise site.)

**`kills` is a separate field from `bosses_defeated`, drained by a different system.** Do not merge them and do not have `achievement_system` drain `kills`. Each field having exactly one drainer is what removes any ordering dependency between the two systems — `achievement_system` is deliberately unchained in the schedule, and a shared queue would silently make it order-sensitive.

- [ ] **Step 1: Write the failing test.** Kill a wild creature of a known species through the real combat path and assert its species id lands in `RunFeats::kills`. Use `spawn_wild_on_player_tile` and `insert_battle` from `tests/support.rs`. Assert a boss kill lands in **both** fields, so nobody later "tidies" one into the other.

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.** One push in `award_loot`, one field on `RunFeats`. `RunFeats` stays unsaved — it is a per-tick drain queue; what accumulates is the saved `ActiveContract::progress`.

- [ ] **Step 4: Run and watch pass**, then the full gates. Watch for unrelated failures here: adding a field to a resource is one of this repo's known ways to shift bevy's query iteration order under a latent unsorted-query test. If something fails in an untouched subsystem, read it before assuming you broke it.

- [ ] **Step 5: Commit.** `feat(contracts): record every kill's species for the tick`

### Task 5: `contract_system` — the one writer of progress

**Files:**
- Create: `crates/engine/src/game/contracts.rs`
- Modify: `crates/engine/src/game/mod.rs`
- Modify: `crates/engine/src/game/lifecycle.rs` (schedule, beside `achievement_system` at line 221)
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:** `pub fn contract_system(...)`, a bevy system.

It polls the four state-shaped objectives and drains `RunFeats::kills` for the fifth:

| Objective | Read from |
|---|---|
| `Descend { depth }` | `resources::Locale` — **never `Position`**, which is pinned to the surface entrance tile underground. Same trap `achievement_system` documents. |
| `Breach { zone }` | `resources::ZoneLevel` |
| `Build { structure }` | a query for a deployed `Structure` of that kind |
| `Kill { species, count }` | drained `RunFeats::kills`; `None` counts any, `Some(id)` counts matches |
| `Deliver { .. }` | **not here.** Progress comes from the player handing items over at the Broker in Task 9. |

It writes `ActiveContract::progress` and nothing else. Completion is Task 6's; this system stops at raising the number.

Register it **unchained**, beside `achievement_system`, for the same stated reason: it shares no mutable state with the chained block above it, and what it reads are counters those have already finished writing this tick.

- [ ] **Step 1: Write the failing tests.** One per objective variant, driving a `Game` directly:
  - a `Kill` contract for a named species advances only on that species, and a `Kill` with `species: None` advances on any
  - progress does not exceed `target()`
  - a `Descend` contract advances when `Locale` is a Stack frame at or past its depth, and — the regression that matters — **does not** advance from a surface `Position` that happens to be far from origin
  - `Breach` advances on `ZoneLevel`
  - `Build` advances when a matching structure is deployed and not before
  - a `Deliver` contract's progress is untouched by this system

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): the one system that advances contract progress`

### Task 6: completion and payout

**Files:**
- Modify: `crates/engine/src/game/contracts.rs`
- Modify: `crates/engine/src/resources.rs` (a `MessageKind` for the announcement, if none fits — check `Outcome` first)
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:**

```rust
impl Game {
    /// The single door out of an active contract: announces, moves the id
    /// into `ActiveContracts::done`, drops the `ActiveContract`, and grants
    /// every `Reward`.
    pub(crate) fn complete_contract(&mut self, idx: usize);
}
```

Called by `contract_system` when `progress >= target()`.

Reward grants:
- `Credits(n)` → `n` of `ids::CREDITS` into the player's inventory
- `Xp(n)` → through `Game::award_player_xp`, so a level-up full-heals exactly as it does from a kill
- `Item(id, n)` → `n` plain copies. **Not through `Game::grant_gear_drop`.** That is the only door a copy above `Ordinary` enters the game by, and crafting/buying/buyback are already deliberately not callers: found gear is categorically better than made gear. A contract payout is closer to made than found.

**The announcement must survive the battle-log prune** if a contract completes during a fight. `MessageLog::retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp` and `Raid` — a plain `log()` is `Info` and is deleted. Use one of the four.

- [ ] **Step 1: Write the failing tests.**
  - a contract reaching its target pays each reward exactly once, is gone from `active`, and its id is in `done`
  - it does not pay twice if the system runs again on the same tick
  - **a gear reward is always `Ordinary`** — the sibling of `crafted_gear_is_never_rare`, and it needs a test because an omission is invisible
  - a completion announced mid-battle survives `retain_outcomes_since_battle`
  - a `repeatable: true` contract can be accepted again after completing; a `repeatable: false` one cannot

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates. Also run `cargo test -p feral-processes-engine balance_sim` — XP now enters the game from a second source, and if those curves move you need to know now rather than in Phase 4.

- [ ] **Step 5: Commit.** `feat(contracts): completion and payout`

---

# Phase 3 — the Contract Broker

Engine only. At the end of this phase the board exists and can be accepted from, with no screen.

### Task 7: the structure, its research node, and the entity flag

**Files:**
- Create: `assets/structures/contract_broker.ron`
- Create: `assets/research/contract_brokerage.ron` — the node that unlocks it
- Modify: `crates/engine/src/structures.rs` (`StructureDef` gains `#[serde(default)] pub issues_contracts: bool`)
- Modify: `crates/engine/src/views.rs` (`EntityView` gains `pub issues_contracts: bool`)
- Modify: `crates/engine/src/game/inspection.rs` (both `EntityView` construction sites — one near line 405/458, one near 858)
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/contracts.rs`

Glyph `!` — verified unused across the shipped structures. Build cost in Core Fragments, in the band of the other mid-game structures.

`issues_contracts` is a `bool` on the def rather than a `Some(ContractDef)`-style block, because a Broker has no per-structure configuration: what it offers is derived, not authored on the building.

- [ ] **Step 1: Write the failing tests.** The shipped Broker loads; a deployed Broker's `EntityView` reports `issues_contracts: true` and every other structure reports false; the research node that unlocks it is reachable (follow the pattern in `tests/research.rs`).

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.** `#[serde(default)]` on the new `StructureDef` field so every existing structure file *and any mod* keeps parsing. Update `assets/structures/README.md` in this same change — that is a standing rule, not a nicety.

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): the Contract Broker structure`

### Task 8: the derived board

**Files:**
- Modify: `crates/engine/src/game/contracts.rs`
- Modify: `crates/engine/src/views.rs`
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:**

```rust
// views.rs
pub struct ContractRow {
    pub id: ContractId,
    pub name: String,
    pub description: String,
    /// What the objective asks, already worded — the engine composes this,
    /// not the renderer, so two screens cannot word one contract differently.
    pub objective_line: String,
    pub reward_line: String,
    pub progress: u32,
    pub target: u32,
}

// game/contracts.rs
impl Game {
    /// What a Broker within `CONTRACT_BOARD_RANGE_TILES` is offering, or
    /// `None` if there is no Broker in range. One call answers both "is there
    /// a board" and "what is on it", so no screen asks those separately and
    /// then disagrees — `Game::stack_market`'s contract.
    pub fn contract_board(&mut self) -> Option<Vec<ContractRow>>;

    /// Every contract the run currently holds. Always available, board or
    /// not: you can read what you have taken anywhere, including four frames
    /// down.
    pub fn active_contracts(&self) -> Vec<ContractRow>;
}
```

New `tuning.rs` constants, in the labelled section with the other knobs: `MAX_ACTIVE_CONTRACTS` (3), `CONTRACT_REFRESH_CYCLES` (400), `CONTRACT_BOARD_SLOTS` (3), `CONTRACT_BOARD_RANGE_TILES`.

**That last one is an engine constant, and reaching for app-core's `MENU_SCAN_RADIUS` instead is a mistake this repo has already made once.** `tuning.rs:167` records it: `MENU_SCAN_RADIUS` is a *menu window* — how much world a picker lists — which is genuinely frontend policy, and at 40 tiles it is more than twice the map pane in either axis. `EXAMINE_RANGE_TILES` exists because borrowing it for an engine question gave the wrong answer. `Game::contract_board` is an engine question, so it gets its own constant. Note this differs from how `app/trade.rs` finds a trading post — there the *frontend* passes its own window into `view_entities`, which is the frontend asking a frontend question.

**The derivation is the non-obvious part.** A local `StdRng`, never `GameRng`:

```rust
let epoch = clock.tick / CONTRACT_REFRESH_CYCLES;
let seed = /* FNV-fold of */ (world_map.seed(), zone.0, epoch, CONTRACT_BOARD_SALT);
let mut rng = StdRng::seed_from_u64(seed);
```

Four properties follow, and all four are the reason for it: the board survives a save/load with no save field; opening the screen spends no `GameRng` draw and so shifts nobody's stream; it cannot be save-scummed; and it rotates on its own. `CONTRACT_BOARD_SALT` is its own named constant so this does not collide with `FrameSpec`'s scheme — one scheme, not a second seed source.

Candidates are filtered to `min_zone <= zone`, minus anything currently active, minus anything in `done` that is not `repeatable`. Then `CONTRACT_BOARD_SLOTS` are drawn.

- [ ] **Step 1: Write the failing tests.**
  - **the same board comes back after a save and load** — the property the whole derivation exists for
  - reading the board leaves `GameRng`'s stream position untouched: draw from `GameRng`, read the board, draw again, and compare against the same sequence with no board read in between
  - advancing the clock past `CONTRACT_REFRESH_CYCLES` changes the offers
  - an active contract is not offered; a completed non-repeatable one is not offered; a completed repeatable one is
  - `min_zone` above the current zone is not offered
  - `contract_board()` is `None` with no Broker deployed and `Some` with one in range

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): the derived contract board`

### Task 9: accept, abandon, deliver

**Files:**
- Modify: `crates/engine/src/game/contracts.rs`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:**

```rust
pub enum ContractRefusal {
    TooMany,          // MAX_ACTIVE_CONTRACTS already held
    AlreadyActive,
    AlreadyDone,      // and not repeatable
    NotOffered,       // no Broker in range, or not on this board
    NothingToDeliver, // no matching items in cargo
}

impl Game {
    pub fn accept_contract(&mut self, id: &ContractId) -> Result<(), ContractRefusal>;
    /// Returns whether anything was abandoned. Progress is lost, not banked.
    pub fn abandon_contract(&mut self, id: &ContractId) -> bool;
    /// Moves as many matching items from cargo onto the contract as it still
    /// needs. Returns how many were taken.
    pub fn deliver_to_contract(&mut self, id: &ContractId) -> Result<u32, ContractRefusal>;
}
```

`deliver_to_contract` is the one place a `Deliver` objective's progress moves, and it takes items **only up to what the contract still needs** — never more, or the player loses cargo to a contract that was already satisfied. It must complete the contract when that fills it, through `complete_contract`, so delivery and the polled objectives share one completion path.

Ordering rule, the same one `use_symlink` and `install_routine` follow: **every refusal is checked before any item leaves cargo.** A refused delivery must leave the inventory exactly as it found it.

- [ ] **Step 1: Write the failing tests.**
  - accepting puts it in `active` at zero progress with the current tick
  - a fourth acceptance is **refused with `TooMany`**, not silently capped — the "no silent caps" rule
  - accepting something not on the board is `NotOffered`
  - abandoning drops it and loses progress; re-accepting starts from zero
  - delivering takes exactly what is needed and no more, and completes the contract when it fills it
  - **a refused delivery leaves cargo untouched** — assert the inventory, not just the error

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): accept, abandon and deliver`

---

# Phase 4 — the screen

Two crates. Follow an existing screen end to end before starting: `Mode::Research` is the closest shape (a list, a verb, a group-menu row).

### Task 10: app-core — mode, handler, menu row

**Files:**
- Create: `crates/app-core/src/app/contracts.rs`
- Modify: `crates/app-core/src/lib.rs` (`Mode::Contracts`)
- Modify: `crates/app-core/src/app/mod.rs`, `crates/app-core/src/app/input.rs` (dispatch)
- Modify: `crates/app-core/src/app/group_menu.rs` (`BASE_ROWS`)
- Test: `crates/app-core/src/tests/contracts.rs` (new), registered in `crates/app-core/src/tests/mod.rs`

The screen has two stacked sections — **active** first, then **available** — and resolves a row number against them the way `trade_row` does in `app/trade.rs:23`. Pull that offset arithmetic into a testable free function for the same stated reason: a screen with more than one section is where row indexing goes wrong, and it is the part testable without a Broker to stand in front of.

The `BASE_ROWS` entry:
- `label: "Contracts"`
- `surface_only: false` — the screen reaches no zone-map state through `Position`, and reading your active contracts four frames down is exactly when you want to. (Note this is *not* an exemption from the rule: the row's `available` predicate must not depend on a `nearby_*` scan for the underground case, or it would report on a base four frames overhead.)
- `available`: a Broker is in range **or** any contract is active — so the row can never open a screen with nothing on it, which is what `group_rows` requires.

- [ ] **Step 1: Write the failing tests.**
  - the row is hidden with no Broker and no active contracts, and shown with either
  - the row is shown underground when a contract is active
  - row-index resolution: a number in the active section, one in the offers section, and one past the end
  - accepting from the screen puts the contract in `active`; abandoning removes it
  - Esc returns to the base menu (`close_screen`'s contract)

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): the contracts screen in app-core`

### Task 11: gui — the renderer

**Files:**
- Create: `crates/gui/src/render/contracts.rs`
- Modify: `crates/gui/src/render/mod.rs` (dispatch on `Mode::Contracts`)
- Test: `crates/gui/src/` — follow the existing popup-width tests (`popup.rs`, and see `no_shipped_inventory_row_overflows_its_popup`)

Draw through `Painter` only. **No file in `render/` may name a graphics library** — `paint.rs` is the only one that does, and that seam is what made the macroquad→Bevy swap touch five files and no drawing code.

Row count is owned by app-core and drawn by gui. Any per-row transform lives in the engine, or the two sides disagree about which row is under the highlight.

- [ ] **Step 1: Write the failing width census.** Build the widest row the shipped contracts can produce — longest name, longest reward line, a progress figure at its widest — measure it with `paint::with_painter`, and assert it fits the popup body. Two open `TODO.md` bugs are exactly this failure going unmeasured; do not assume, measure.

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement the renderer.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): the contracts renderer`

### Task 12: documentation

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `assets/achievements/README.md`
- Modify: `TODO.md`
- Modify: `CLAUDE.md` and `AGENTS.md`

- [ ] **Step 1: Update `CHANGELOG.md`.** Its preamble is the one statement of which digit moves — read it rather than guessing. Nothing here breaks a save, so this is not a breaking change.

- [ ] **Step 2: Correct `assets/achievements/README.md`.** It currently states there is deliberately no "kill N bosses in one run" trigger because *"counting within a run needs saved run state the game doesn't keep"*. That is now false — Phase 2 added exactly that state. Correct the claim and say a counting trigger is now cheap but deliberately not built.

- [ ] **Step 3: Drop TODO #21.**

- [ ] **Step 4: Add a `CLAUDE.md` load-bearing seam entry**, then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift. The entry should carry: the derived board and its four properties; the two-field `RunFeats` drain and why one queue would make `achievement_system` order-sensitive; `ActiveContract` storing the whole def; and the deliberate amendment to "progression is earned by fighting". Do **not** update `docs/manual.md` or the root `README.md` — both are explicitly carved out.

- [ ] **Step 5: Commit.** `docs: contracts`

---

# Phase 5 — rolled contracts

Deliberately last. A rolled objective naming an item this sector cannot produce is an unfinishable contract, and those validity rules are far easier to write once the authored ones have been played.

**Do not start this phase without playing Phase 4 first.** A green suite is not evidence that the loop is any good, and the whole reason this phase was deferred is to learn what a good contract feels like before generating them.

### Task 13: templates and their validity rules

**Files:**
- Modify: `crates/engine/src/contracts.rs`
- Modify: `crates/engine/src/game/contracts.rs`
- Create: `assets/contracts/templates/*.ron`
- Modify: `assets/contracts/README.md`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces — Produces:** a `ContractTemplate` that rolls into a `ContractDef`. **The same `ContractDef`** the authored files parse into — an authored contract is a template with no free variables, so there is one accept path, one progress path and one completion path. If you find yourself writing a second completion path, the design is wrong; stop and say so.

Rolled ids must be distinguishable from authored ones and stable within an epoch, since the board is derived and a rolled offer must be the same offer after a save and load.

Validity is the substance of this task: a rolled objective must name something reachable in this sector. A `Deliver` of an item nothing here produces, or a `Kill` of a species absent from the local habitat pools, is an unfinishable contract. `Game::habitat_pools` is the existing seam for the species half — widen it rather than copying the pool-building.

- [ ] **Step 1: Write the failing tests.** A rolled contract is always finishable: its `Deliver` item is craftable or drops here, its `Kill` species appears in a local habitat pool, its `Build` structure is unlocked or unlockable. Rolled offers are stable across a save/load within an epoch. A template set that can produce nothing valid yields an empty board rather than an unfinishable contract or a panic.

- [ ] **Step 2: Run and watch fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch pass**, then the full gates.

- [ ] **Step 5: Commit.** `feat(contracts): rolled contracts`

---

## What this plan deliberately leaves out

Each was considered and cut in the spec. Do not add them because they seem natural while you are in the file:

- **Deadlines and expiry.** Adds a failure state, a clock reader and an abandonment path for no proven value.
- **Reputation, tiers, contract chains.** YAGNI until the loop is played.
- **`Tame` objectives.** Two call sites rather than one, and neither as cleanly funnelled as the kill site.
- **A counting achievement trigger.** Cheap once Task 3 lands, but a separate feature.
- **`Reward::PortalFragments`.** Absent on purpose. Breaching stays earned by fighting and descending.

## Known risks while executing

- **XP magnitudes are ungated.** `balance_sim` is RNG-free and models one run's combat curve; it cannot see a contract. The authored XP numbers are guesses. If `balance_sim`'s level curves move when you touch creature XP, that movement is the signal, not a broken test.
- **Adding fields to resources shifts bevy's query iteration order.** A failure in an untouched subsystem right after Task 3 or 4 is most likely a latent unsorted-query test, not your regression. Read it before "fixing" it.
- **Do not reach for `GameRng` anywhere in this feature.** Its stream position is not persisted and drawing from it shifts every later roll in the run — which has silently rewritten a seeded combat test three files away before.
