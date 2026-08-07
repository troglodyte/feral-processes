# Battle arena implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A scenario-driven harness that runs real battles offline — pick the
opponents and (on a fresh player) the items, run N seeded reps, keep the
round-by-round transcript — so difficulty can be tuned by measurement rather
than by playing to the fight.

**Architecture:** A `pub mod arena` inside `crates/engine`, which is what lets
it reach `pub(crate) start_battle` and the private `world` field without
adding any public `Game` method. A third bin in `crates/launcher` drives it
and resolves `dev-saves/` template names, which only the launcher can do.
Scenarios are RON files in `dev-arenas/`.

**Tech Stack:** Rust 2024, `bevy_ecs` 0.19, `ron` 0.12, `serde` 1. No new
dependency — every crate this needs is already in `crates/engine/Cargo.toml`.

**Spec:** `docs/superpowers/specs/2026-08-07-battle-arena-design.md`. Read it
before Task 1; it carries the reasoning this plan only cites.

## Global Constraints

Every task's requirements implicitly include this section.

- **No new dependency in any `Cargo.toml`.** `ron`, `serde` and `rand` are
  already engine dependencies.
- **No new `pub fn` on `Game`, and no accessor for `Game::world`.** That
  private field is the compiler barrier keeping the renderer out of the ECS
  (`CLAUDE.md`). `arena` is inside the engine crate and reaches `pub(crate)`
  items directly. If you find yourself widening something to `pub`, stop —
  the module is in the wrong place.
- **Nothing in `crates/gui` or `crates/app-core` changes.**
- **Every `Scenario` field is `#[serde(default)]`**, so a scenario written
  today keeps parsing after a field is added.
- **A malformed scenario is an `Err`, never a panic** — the pattern
  `SpeciesDb::load_dir` follows.
- **`ENGAGED_GROUPS` is 2, `MAX_ENEMY_GROUPS` is 4, `MAX_GROUP_SIZE` is 100**
  (`crates/engine/src/tuning.rs`). Read them from `tuning`; never inline the
  numbers.
- **Run `cargo fmt` and `cargo clippy --workspace` after every task** and fix
  warnings rather than silencing them.
- **`cargo test --workspace` is the final gate.** Iterate with
  `cargo test -p feral-processes-engine arena`.
- **Branch is `battle-arena`.** Commit per green task. Do not bump the
  workspace version or write a `CHANGELOG.md` section until Task 10 — the
  version bump happens once, at the merge.
- **Comments explain why, never what.** This repo's doc comments carry
  constraints and rejected alternatives; match that density, not more.

---

## File structure

| File | Responsibility |
|---|---|
| `crates/engine/src/arena/mod.rs` | Public entry `run`, module wiring, the `Fresh`-player builders shared with test fixtures |
| `crates/engine/src/arena/scenario.rs` | `Scenario` and its parts, RON loading, static validation |
| `crates/engine/src/arena/setup.rs` | Turning a `Scenario` into a `Game` plus a built `Vec<EnemyGroup>` |
| `crates/engine/src/arena/run.rs` | One rep: reseed, fight, capture the transcript |
| `crates/engine/src/arena/report.rs` | `RepRecord`, `Report`, aggregation |
| `crates/engine/src/game/combat.rs` | `start_battle` split so groups can be supplied pre-built |
| `crates/engine/src/tests/support.rs` | `set_level`/`spawn_tamed` re-exported from their new home |
| `crates/launcher/src/bin/arena.rs` | CLI, template resolution, stdout formatting, report writing |
| `dev-arenas/README.md` | Schema reference |

Split this way because the four arena submodules have genuinely different
reasons to change: the schema changes when you want a new knob, `setup`
changes when the game's construction does, `run` changes when the round loop
does, `report` changes when you want a different statistic.

---

### Task 1: A seam that accepts pre-built enemy groups

`Game::start_battle` (`crates/engine/src/game/combat.rs:148`) partitions its
pack through `group_pack`, which truncates to
`group_size_ceiling() × enemy_group_ceiling()`. The arena needs everything
that function does *except* that truncation.

**Files:**
- Modify: `crates/engine/src/game/combat.rs:148-186`
- Test: `crates/engine/src/tests/` — add to whichever battle test module
  already covers `start_battle`; find it with
  `rg -l 'start_battle' crates/engine/src/tests/`

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn begin_battle(&mut self, groups: Vec<EnemyGroup>)
  pub(crate) fn start_battle(&mut self, pack: Vec<Entity>)  // unchanged signature
  ```

- [ ] **Step 1: Write the failing test.** Assert that `begin_battle`, handed
  two hand-built `EnemyGroup`s whose combined size exceeds
  `group_size_ceiling()` at the test's zone, opens a `BattleState` holding
  exactly those groups at exactly those sizes. Also assert
  `has_active_battle()` is true and `battle_log()` is non-empty, so the
  intercept line still fires. Use `spawn_wild_on_player_tile` from
  `tests/support.rs` for the members.

- [ ] **Step 2: Run it and watch it fail** with "no method named
  `begin_battle`".

  `cargo test -p feral-processes-engine begin_battle`

- [ ] **Step 3: Extract.** Move everything in `start_battle` *after* the
  `group_pack` call into `begin_battle(groups)`. `start_battle` becomes the
  one-line `self.begin_battle(self.group_pack(pack))` — borrow-check that as
  two statements if the inline form fights you. Carry the existing doc
  comment about `CombatBuff`/`FieldBuff` deliberately not being cleared onto
  `begin_battle`, since that is where the code it describes now lives. Add a
  line to `begin_battle`'s doc naming its two callers and stating that
  `start_battle` is the *only* one that caps a pack — a third caller that
  wants capping calls `group_pack` itself.

- [ ] **Step 4: Run the new test and the whole engine battle suite.** This is
  a pure refactor; anything else going red means the extraction moved
  behaviour.

  `cargo test -p feral-processes-engine` — expect all green.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `refactor(battle): split begin_battle out of start_battle`

---

### Task 2: The scenario schema

**Files:**
- Create: `crates/engine/src/arena/scenario.rs`
- Create: `crates/engine/src/arena/mod.rs` (declaring `mod scenario;` and
  re-exporting its public types — the rest is filled in by later tasks)
- Modify: `crates/engine/src/lib.rs` — add `pub mod arena;` beside
  `pub mod balance_sim;`

**Interfaces:**
- Produces:
  ```rust
  pub struct Scenario {
      pub player: PlayerSource,
      pub equip: Vec<EquipSpec>,
      pub inventory: Vec<InventorySpec>,
      pub party: Vec<CompanionSpec>,
      pub opponents: Vec<OpponentSpec>,
      pub reps: u32,
      pub seed: u64,
  }
  pub enum PlayerSource {
      Fresh { level: u32, zone: u32 },
      Save(std::path::PathBuf),
      Template(String),
  }
  pub struct EquipSpec { pub item: ItemId, pub tier: u32 }
  pub struct InventorySpec { pub item: ItemId, pub qty: u32 }
  pub struct CompanionSpec { pub species: SpeciesId, pub level: u32 }
  pub struct OpponentSpec { pub species: SpeciesId, pub count: u32 }

  impl Scenario {
      pub fn load(path: &Path) -> Result<Self, String>;
      pub fn from_ron(text: &str) -> Result<Self, String>;
      fn validate(&self) -> Result<(), String>;
  }
  ```
  `Default` for `PlayerSource` is `Fresh { level: 1, zone: 1 }`; `reps`
  defaults to 1 and `seed` to 0. Derive `Serialize` as well as `Deserialize`
  — Task 7's report embeds the scenario it ran.

- [ ] **Step 1: Write the failing tests.** Seven, each named for the rule it
  holds:
  - a minimal scenario (only `opponents`) parses, and `reps`/`seed`/`player`
    come out at their documented defaults
  - a full `Fresh` scenario parses with every field populated, including
    `tier` on an `EquipSpec`
  - a `Save` scenario parses and holds the path
  - syntactically broken RON is an `Err` whose message is not empty
  - `equip` on a `Save` scenario is an `Err` naming the field
  - likewise `inventory` and `party` on a `Save` scenario (one test, three
    assertions)
  - `opponents: []` is an `Err` — a scenario with nobody to fight is a typo,
    not a zero-length fight

  Write them against `from_ron` so they need no filesystem. Use the schema
  blocks in the spec verbatim as the fixture text; if RON rejects them, the
  spec's examples are wrong and must be corrected in the same commit.

- [ ] **Step 2: Run and watch them fail.**

  `cargo test -p feral-processes-engine arena::scenario`

- [ ] **Step 3: Implement.** `#[serde(default)]` on every field of
  `Scenario`. `load` reads the file and delegates to `from_ron`, mapping both
  the IO error and the parse error into a `String` that names the path —
  a bare "expected struct" with no filename is what makes a typo expensive.
  `validate` runs at the end of `from_ron` so no caller can skip it. It does
  *not* check species or item ids: those need a loaded `Game` and belong to
  Tasks 4 and 5.

- [ ] **Step 4: Run and watch them pass.** Also run the whole engine suite —
  adding `pub mod arena` to `lib.rs` is a public-surface change.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): scenario schema and RON loading`

---

### Task 3: One home for the level and companion fixtures

`set_level` and `spawn_tamed` in `crates/engine/src/tests/support.rs` are
`#[cfg(test)] pub(super)`. Task 4 needs both from non-test code. Two copies
would put two versions of "what a level-N companion is" in the tree, and
`install_innate_routines` is already on record as the step a duplicate
dropped once.

**Files:**
- Modify: `crates/engine/src/arena/mod.rs` — new home
- Modify: `crates/engine/src/tests/support.rs:695-723` — delete the bodies,
  re-export

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn set_level(game: &mut Game, entity: Entity, level: u32)
  pub(crate) fn spawn_companion(game: &mut Game, species: &str, level: u32) -> Option<Entity>
  ```
  `spawn_companion` is a widening of `spawn_tamed`, not a rename: the fixture
  took `(hp, atk)` and hardcoded `generic_species`, while a scenario names a
  species and a level. It spawns with `Creature`, `Position` (the player's
  tile), `Tamed { owner: player }`, `Experience::default()` and
  `Stats` from the species' `base_*`, calls `install_innate_routines`, then
  applies `set_level`. Returns `None` for an unknown species id.
- Consumes: `Game::install_innate_routines`, `Game::install_unlocked_routines`
  (both `pub(crate)`, `game/combat.rs:481` and `:555`).

- [ ] **Step 1: Write the failing test** in `arena/mod.rs`'s test module: a
  companion spawned at level 5 from a named species has that species'
  `Creature`, a `Tamed` owner of the player, `Experience.level == 5`, stats
  above the level-1 baseline, and non-empty `Routines`. Plus: an unknown
  species id returns `None`.

- [ ] **Step 2: Run and watch it fail.**

- [ ] **Step 3: Implement,** then rewrite `support.rs`'s `spawn_tamed` as a
  thin wrapper that calls `spawn_companion(game, &generic_species(game).id, 1)`
  and then overwrites `Stats.hp`/`max_hp`/`atk` with the caller's numbers —
  existing tests depend on those exact values. Delete `support.rs`'s
  `set_level` body and re-export the new one with
  `pub(super) use crate::arena::set_level;`.

- [ ] **Step 4: Run the full engine suite.** Many tests call both fixtures;
  this task is green only when all of them still pass.

  `cargo test -p feral-processes-engine`

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `refactor(arena): share the level and companion fixtures with tests`

---

### Task 4: Building the player

**Files:**
- Create: `crates/engine/src/arena/setup.rs`
- Modify: `crates/engine/src/arena/mod.rs` — `mod setup;`

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn build_player(scenario: &Scenario, assets_dir: &Path) -> Result<Game, String>;
  ```
- Consumes: `Game::new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> io::Result<Game>`
  (`game/lifecycle.rs:32`), `Game::load(path: &Path, assets_dir: &Path) -> io::Result<Game>`
  (`:173`), `Game::equip(&mut self, item: &ItemId, tier: u32) -> Result<(), String>`
  (`game/crafting.rs:271`), `Game::add_copies(&mut self, item: &ItemId, tier: u32, qty: u32)`
  (`:199`), `arena::spawn_companion`, `arena::set_level`.

Behaviour by variant:

- `Fresh { level, zone }` — `Game::new(0, DifficultyMode::Forgiving, assets_dir)`,
  then set `ZoneLevel(zone)`, `set_level` the player, grant each
  `inventory` row via `add_copies`, grant each `equip` row via `add_copies`
  *and then* `equip` it, and `spawn_companion` each `party` row pushing it
  onto `Party`. **`Forgiving` deliberately**: a permadeath loss inside a
  measurement run is a `GameOver` the next rep would inherit.
- `Save(path)` — `Game::load`. Nothing else; the run state is what it is.
- `Template(name)` — `Err` explaining that templates are resolved by the
  `arena` bin before `run` is called. The engine cannot see `dev_template`.

Errors name the offending id: an unknown item, an unknown companion species,
or an `equip` the game refuses (wrong slot, unknown item) all stop the run.

- [ ] **Step 1: Write the failing tests.** Six:
  - `Fresh` at level 20 zone 3 produces a player at `Experience.level == 20`
    and a `ZoneLevel` of 3
  - an `equip` row lands in the player's `Equipment` **at the requested
    tier** — assert `EquippedItem::fusion_tier`, since per-copy fusion is
    exactly what a tier-blind implementation would drop
  - an `inventory` row is countable via `count_copies(item, 0)`
  - a `party` row becomes a `Party` member at the requested level
  - an unknown item id in `equip` is an `Err` naming that id
  - `Template(..)` is an `Err` mentioning the bin

  Use `test_assets_dir()` from `tests/support.rs` for `assets_dir`.

- [ ] **Step 2: Run and watch them fail.**

  `cargo test -p feral-processes-engine arena::setup`

- [ ] **Step 3: Implement.** Order matters in the `Fresh` path: set the zone
  *before* equipping, because `Game::equip` captures `EquippedItem::level`
  from the current `ZoneLevel` and gear doubles per level
  (`GEAR_LEVEL_GROWTH`). Equipping first would silently under-scale every
  weapon in every scenario.

- [ ] **Step 4: Run and watch them pass.**

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): build a player from a scenario`

---

### Task 5: Spawning the opponents and building the groups

**Files:**
- Modify: `crates/engine/src/arena/setup.rs`
- Modify: `crates/engine/src/game/combat.rs:34` and `:61` — widen
  `group_size_ceiling` and `enemy_group_ceiling` from private `fn` to
  `pub(crate) fn`. They are currently visible only inside `game::combat` and
  its descendants; `arena` is a sibling module and cannot call them. Widen to
  `pub(crate)` and no further — the warning is the only reason they leave
  that module, and a note on each saying so keeps them from drifting public.

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn build_opponents(
      game: &mut Game,
      opponents: &[OpponentSpec],
  ) -> Result<(Vec<EnemyGroup>, Vec<String>), String>;
  ```
  Returns the groups in scenario order, plus any warnings for the caller to
  print. Order is preserved because it is the formation — `ENGAGED_GROUPS` is
  2, so entries 3 and 4 are out of melee reach.
- Consumes: `Game::spawn_wild_creature_scaled(species_id, x, y, depth_mult) -> Option<Entity>`
  (`game/spawning.rs:164`), `Game::group_size_ceiling()`,
  `Game::enemy_group_ceiling()` (`game/combat.rs`).

Rules:

- Every member spawns via `spawn_wild_creature_scaled(species, px, py, 1.0)`
  on the player's own tile, so zone multiplier, potential roll, wild routines
  and the `Hostile`/`WanderAi`/`StatusEffects` bundle all apply exactly as a
  map spawn would. `depth_mult` is `1.0`: Stack depth is not a scenario knob.
- One `EnemyGroup` per `OpponentSpec`, **not** merged by species. Two entries
  naming the same species are two groups, which is how you place the same
  program both in reach and out of it.
- **Hard errors:** an unknown species id; more than `MAX_ENEMY_GROUPS`
  entries; any `count` of 0 or above `MAX_GROUP_SIZE`. Past those the fight
  is not one the game can represent.
- **Warnings** (returned, not printed here): a `count` above
  `group_size_ceiling()`, or an entry count above `enemy_group_ceiling()`.
  Each warning names the ask, the ceiling and the zone. This is the "no
  silent caps" rule — the pack is still built at the size asked for.

- [ ] **Step 1: Write the failing tests.** Six:
  - nine opponents at zone 1 produce a group of nine — the truncation
    `group_pack` would have applied does not happen
  - that same case returns exactly one warning, whose text contains the ask,
    the ceiling and the zone
  - two entries naming the same species stay two groups
  - an unknown species id is an `Err` naming it
  - five entries is an `Err` mentioning `MAX_ENEMY_GROUPS`
  - `count: 0` is an `Err`
  - a composition inside the zone's ceilings returns no warnings

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch them pass.**

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): spawn opponents and build groups from a scenario`

---

### Task 6: One rep

The heart of it, and the task with the regression that matters.

**Files:**
- Create: `crates/engine/src/arena/run.rs`
- Modify: `crates/engine/src/arena/mod.rs` — `mod run;`

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn run_rep(game: &mut Game, groups: Vec<EnemyGroup>, seed: u64) -> RepRecord;
  ```
  `RepRecord` is defined in Task 7 — write its struct in `report.rs` first if
  you are taking these tasks out of order.
- Consumes: `Game::begin_battle` (Task 1), `Game::has_active_battle()`
  (`game/turn.rs:90`), `Game::battle_plan_remaining(action) -> Result<(), String>`
  (`game/combat.rs:421`), `Game::battle_round_ready() -> bool` (`:334`),
  `Game::battle_resolve_round()` (`game/combat_round.rs:28`),
  `Game::battle_log() -> Vec<LogLine>` (`game/turn.rs:69`),
  `resources::GameRng(pub StdRng)`.

The loop, in order:

1. Reseed: `game.world.insert_resource(GameRng(StdRng::seed_from_u64(seed)))`.
   Per rep, so twenty reps are a sample and any one of them replays alone.
2. `game.begin_battle(groups)`.
3. While `game.has_active_battle()` and the round budget is not spent:
   `battle_plan_remaining(BattleAction::Attack { group: 0 })`, then
   `battle_resolve_round()` if `battle_round_ready()`, then **push
   `game.battle_log()` onto the transcript**.
4. Record the outcome.

Three things that will bite:

- **Capture the log after every round, never at the end.** `end_battle` calls
  `MessageLog::retain_outcomes_since_battle`, which deletes the blow-by-blow
  and keeps only `Outcome`/`Loot`/`LevelUp`/`Raid`. `MESSAGE_LOG_CAP` is a
  second reason: a long fight drops lines off the front before it finishes.
- **Guard the loop with a round budget.** Use `balance_sim`'s `TURN_CAP`
  reasoning — a fight that has not resolved in `arena::ROUND_CAP` rounds is
  recorded as a stalemate rather than hanging the tool. Define `ROUND_CAP` as
  a documented `const` in `run.rs` (2000, matching `balance_sim::TURN_CAP`)
  and say in its doc that it catches a genuine stalemate and nothing else.
- **`battle_plan_remaining` can return `Err`** if the battle has already
  ended between the check and the call. Treat that as the fight being over,
  not as a panic.

Won/lost: `game.has_active_battle()` is false and the player is alive means
won; use `game.is_game_over()` and the player's `Stats.hp` to tell a loss from
a win. Read the player HP fraction *before* the last round if it must be
non-zero on a loss — record `0.0` on a loss, matching `balance_sim`.

- [ ] **Step 1: Write the failing tests.** Five, and the third is the one
  this task exists for:
  - **determinism** — the same `Game` construction and the same seed, run
    twice, produce byte-identical `RepRecord`s including the transcript
  - **divergence** — two different seeds against a marginal pack produce
    different transcripts, so the reseed is doing something
  - **the transcript survives `end_battle`** — a *won* fight's record holds
    round narration, not only the outcome lines. Assert on a line matching
    `── round 1 ──`, which `battle_resolve_round` logs with
    `MessageKind::Round`, a kind `retain_outcomes_since_battle` drops. A
    naive implementation passes every other test here and returns an empty
    transcript.
  - **a lopsided win** — an overwhelming party beats one weak opponent, and
    `rounds` is small and non-zero
  - **a lopsided loss** — a bare level-1 player against a full group of the
    toughest ordinary species does not win. Use
    `balance_sim::toughest_ordinary_species` rather than naming a species, so
    a roster retune cannot quietly turn this into a win.

- [ ] **Step 2: Run and watch them fail.**

  `cargo test -p feral-processes-engine arena::run`

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch them pass.**

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): run one seeded rep and capture its transcript`

---

### Task 7: Records, report and aggregation

**Files:**
- Create: `crates/engine/src/arena/report.rs`
- Modify: `crates/engine/src/arena/mod.rs` — `mod report;`

**Interfaces:**
- Produces:
  ```rust
  pub struct RepRecord {
      pub seed: u64,
      pub won: bool,
      pub rounds: u32,
      pub player_hp_fraction: f32,
      pub companions_downed: u32,
      pub transcript: Vec<String>,
  }
  pub struct Report {
      pub scenario: Scenario,
      pub warnings: Vec<String>,
      pub reps: Vec<RepRecord>,
  }
  pub struct Summary {
      pub reps: u32,
      pub wins: u32,
      pub win_rate: f32,
      pub mean_rounds: f32,
      pub median_rounds: u32,
      pub mean_player_hp_fraction: f32,
      pub mean_companions_downed: f32,
      pub loss_seeds: Vec<u64>,
  }
  impl Report {
      pub fn summary(&self) -> Summary;
      pub fn to_ron(&self) -> Result<String, String>;
  }
  ```
  `transcript` is `Vec<String>` rather than `Vec<LogLine>`: the report is for
  reading and post-processing, and `MessageKind`/`MessageSource` would drag
  the log's internal vocabulary into a file format. Derive `Serialize` on
  `RepRecord`, `Report` and `Summary`.

- [ ] **Step 1: Write the failing tests.** Four, all over hand-built
  `RepRecord`s so no battle runs:
  - win rate and mean rounds over a mixed set of five records
  - `median_rounds` over an even-length set picks the lower of the two middle
    values, and the test says so — an unstated median convention is the kind
    of thing that quietly changes between refactors
  - `loss_seeds` holds exactly the seeds of the losing records, in rep order
  - an empty `reps` vector produces a `Summary` with `win_rate: 0.0` and no
    division by zero

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch them pass.**

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): rep records, report and summary statistics`

---

### Task 8: The public entry point

**Files:**
- Modify: `crates/engine/src/arena/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn run(scenario: &Scenario, assets_dir: &Path) -> Result<Report, String>;
  ```
  The engine's whole public arena surface, alongside the re-exported
  `Scenario`, `Report`, `RepRecord`, `Summary` and the spec types.

Per rep: `build_player`, `build_opponents`, `run_rep(game, groups, scenario.seed + rep)`.
**A fresh `Game` per rep** — a `Game` carries the last fight's dead
companions, spent items and XP, so reusing one would make rep 2 measure a
different party from rep 1. Warnings are collected from the first rep only;
they are identical every rep and printing fifty copies is noise.

- [ ] **Step 1: Write the failing tests.** Three:
  - an end-to-end `Fresh` scenario with `reps: 3` returns three records whose
    seeds are `seed`, `seed + 1`, `seed + 2`
  - running the same scenario twice returns equal reports — the top-level
    determinism guarantee the whole tool rests on
  - a scenario whose party is wiped in rep 1 still fields a full party in
    rep 2. Assert on rep 2's transcript naming the companion, which is what
    catches a shared `Game` — a `won` flag would not.

- [ ] **Step 2: Run and watch them fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch them pass,** then the full engine suite.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): public run entry point`

---

### Task 9: The driver bin

**Files:**
- Create: `crates/launcher/src/bin/arena.rs`
- Modify: `crates/launcher/Cargo.toml` — a third `[[bin]]`

**Interfaces:**
- Consumes: `feral_processes_engine::arena::{run, Scenario, PlayerSource, Report}`,
  `feral_processes::dev_template::{assets_dir, generate, working_copy, known}`.

Usage, matching `savetool`'s `USAGE` const style:

```
usage:
  arena <scenario.ron> [--out <report.ron>]   run a scenario
  arena templates                             list dev-saves/ templates
```

Responsibilities, in order:

1. Parse args by slice-matching on `args.as_slice()`, exactly as
   `savetool::main` does. Return `ExitCode::FAILURE` with the message on
   stderr for anything unrecognised.
2. `Scenario::load`.
3. **Resolve `PlayerSource::Template(name)`** into `PlayerSource::Save(path)`
   by calling `dev_template::generate(name, &working_copy(name))`. This is
   the only reason the bin is in the launcher: the engine cannot see
   `dev_template`. An unknown template name errors with
   `dev_template::known()` appended, so the message lists the real options.
4. `arena::run(&scenario, &dev_template::assets_dir())`.
5. Print `report.warnings` to **stderr**, so piping stdout to a file keeps
   the data clean.
6. Print to stdout: at `reps == 1`, the transcript line by line followed by
   the outcome; above 1, the `Summary` — win rate, mean and median rounds,
   mean player HP%, mean companions downed, and the loss seeds.
7. Write `report.to_ron()` to `--out` or `arena-report.ron`, and print the
   path written.

- [ ] **Step 1: Write the failing tests** in the bin's own `mod tests`,
  following the three `dev_template` tests as precedent. Extract arg parsing
  into a `fn parse_args(args: &[&str]) -> Result<Command, String>` so it is
  testable without running a battle, and test: a bare scenario path; a path
  with `--out`; an unknown flag is an `Err`; `templates`; no args at all is
  an `Err` carrying the usage text.

- [ ] **Step 2: Run and watch them fail.**

  `cargo test -p feral-processes --bin arena`

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run and watch them pass.**

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

  `feat(arena): the arena bin`

---

### Task 10: The scenario library, docs and release

**Files:**
- Create: `dev-arenas/README.md`
- Create: `dev-arenas/opening-fight.ron`, `dev-arenas/full-group.ron`,
  `dev-arenas/geared-vs-boss.ron`
- Modify: `CHANGELOG.md`, root `Cargo.toml`

The three scenarios, chosen to be worth keeping rather than to demonstrate
syntax:

- `opening-fight.ron` — `Fresh(level: 1, zone: 1)`, no gear, one opponent
  drawn from the opening ring. The fight the game actually opens on.
- `full-group.ron` — `Fresh(level: 20, zone: 3)` with best-in-slot gear and
  three companions against a full zone-3 group. The progression sweep's
  scenario, run for real.
- `geared-vs-boss.ron` — a `Template` player against a boss species, showing
  the template path.

`dev-arenas/README.md` documents every field, the defaults, and — from the
spec — the two non-obvious properties: **order is formation** (`ENGAGED_GROUPS`
is 2), and **there is no per-enemy level** (the zone is the strength dial,
`count` the volume dial). It also states the blind spot plainly: All-Attack
fires no companion Specials, so an arena number is a floor on the party's
output, the same gap `balance_sim` has.

- [ ] **Step 1: Write the three scenarios and run each one.** They are the
  acceptance test for the whole feature; a scenario that will not run is a
  bug in Tasks 2-9, not in the file.

  `cargo run --bin arena -- dev-arenas/opening-fight.ron`

- [ ] **Step 2: Write `dev-arenas/README.md`.**

- [ ] **Step 3: Check for docs this falsifies.** `CLAUDE.md` gains a
  load-bearing-seams entry for the `begin_battle`/`start_battle` split — that
  `start_battle` is the only path that caps a pack is exactly the kind of
  fact that costs tool calls to rediscover. `rg` for claims about
  `start_battle` being the sole entry to a battle and correct them. Then
  `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to
  catch drift. Do **not** touch `docs/manual.md` or the root `README.md`;
  both are carved out of the doc obligation.

- [ ] **Step 4: Run the full gate.**

  `cargo test --workspace` — expect all green, and note the new total.

- [ ] **Step 5: Bump and release.** Minor bump in the root `Cargo.toml`
  (a feature, no save-format change — `save::SAVE_FORMAT_VERSION` is
  untouched, so nothing breaks a player's save). Add the `## X.Y.Z` section
  to `CHANGELOG.md` per its preamble, commit, and tag `vX.Y.Z` annotated.
  Pushing needs an explicit ask, and `git push` alone does not send tags.

---

## Self-review against the spec

| Spec section | Task |
|---|---|
| Arena lives inside the engine crate | 2 (module), 8 (entry point) |
| Driver bin in the launcher beside `savetool` | 9 |
| Party plays the game's own All-Attack | 6 |
| Opponents spawned for real, grouped by hand | 5 |
| Cap bypass with a stderr warning | 5 (returns), 9 (prints) |
| One rep, one seed | 6 (reseed), 8 (`seed + rep`) |
| Transcript captured per round | 6 |
| Save import wholesale; `Fresh` picks items | 4 |
| Fixtures move out of `support.rs` | 3 |
| Scenario schema and its rules | 2 |
| Order is formation; no per-enemy level | 5 (order preserved), 10 (documented) |
| Output: transcript / summary / report file | 7 (data), 9 (formatting) |
| Testing | every task; full gate in 10 |
| Not a balance regression gate | nothing asserts a win rate — by omission, on purpose |

The seam in Task 1 is not named in the spec, which describes the bypass
without saying how. It is the mechanism, and it is where the "one way into a
battle" invariant either holds or quietly stops holding — hence its own task
and its own `CLAUDE.md` entry in Task 10.
