# Achievements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A cross-run achievement profile: depth milestones and boss kills earned in one run stamp `profile.ron`, which pays out stats, Perk Points and a starting program at the start of the next run.

**Architecture:** Achievements are `.ron` data. One `achievement_system` in the tick decides everything that has been earned — three triggers are high-water marks it polls (zone, Stack depth, cycles), the fourth is a boss kill that `award_loot` merely *records* into a per-tick `RunFeats` queue. Earned achievements are written to `profile.ron` immediately; rewards apply only in `Game::new`, never on load. The profile lives outside `SaveData`, so there is no save-format change.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), `ron`, `serde`, `rand::StdRng`.

Spec: `docs/superpowers/specs/2026-08-04-achievements-design.md`. Read it before Task 1 — it carries the reasoning this plan only carries the shape of.

## Global Constraints

- **`SAVE_FORMAT_VERSION` must stay at 21.** If a task finds itself adding a field to `SaveData`, the design has been misread — stop and re-read the spec's Storage section.
- **New `.ron` schema fields are `#[serde(default)]`**, always. A malformed file is skipped with a logged warning, never a panic — follow `PerkDb::load_dir` (`crates/engine/src/perks.rs:161`).
- **Never draw from `resources::GameRng`** for anything an achievement decides. The rolled stat must survive a save/load and must not shift the run's roll stream. Local `StdRng` only.
- **`crates/gui` never touches the ECS `World`.** Screen data reaches the renderer as a view type off `Game`, through app-core.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- Iterate with `cargo test -p feral-processes-engine <name>` (~3s). Reserve `cargo test --workspace` for the final task.
- Commit at every green step. The branch is `achievements`; it already exists and holds the spec.

---

### Task 1: The achievement data layer and the ladder

**Files:**
- Create: `crates/engine/src/achievements.rs`
- Create: `assets/achievements/*.ron` (13 files, listed below)
- Create: `assets/achievements/README.md`
- Modify: `crates/engine/src/lib.rs` (add `pub mod achievements;`)
- Test: in-file `#[cfg(test)] mod tests` in `achievements.rs`

**Interfaces — Produces:**

```rust
pub struct AchievementId(String);           // string newtype, like items::ItemId
pub enum Trigger { ZoneReached(u32), StackDepthReached(u32), CyclesSurvived(u64), BossDefeated(Option<String>) }
pub enum Reward  { RandomMainStat(u32), PerkPoints(u32), StartingProgram(String) }
pub enum MainStat { Atk, Def, Integrity, Decompiler }
pub struct AchievementDef { pub id: AchievementId, pub name: String, pub description: String, pub trigger: Trigger, pub reward: Reward }
pub struct AchievementDb { /* BTreeMap<AchievementId, AchievementDef> */ }
impl AchievementDb {
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>;
    pub fn get(&self, id: &AchievementId) -> Option<&AchievementDef>;
    pub fn iter(&self) -> impl Iterator<Item = &AchievementDef>;
}
pub fn roll_main_stat(id: &AchievementId) -> MainStat;
```

`BossDefeated(None)` means "any boss" — that is the `boss_first` rung. `BossDefeated(Some(species_id))` names one. Use `BTreeMap` for the db so `iter()` is a stable order: the achievements screen lists it and a `HashMap` would reshuffle the screen between runs.

**Notes for the implementer:**

- `AchievementDb` is a bevy `Resource`, like `PerkDb`. Derive what `PerkDb` derives.
- `roll_main_stat` is the one genuinely non-obvious piece. It must be a pure function of the id, seeded locally, for the reason `Game::orphan_species` gives:

```rust
// The seed is the id's bytes, not GameRng: this answer is written into
// profile.ron and must be identical on every machine and after every
// reload, and a GameRng draw would also shift every later roll in the run.
let mut hasher = std::collections::hash_map::DefaultHasher::new();
```
  — `DefaultHasher` is **not** stable across Rust releases. Use an explicit fold over `id.as_str().as_bytes()` into a `u64` (FNV-style) instead, so the roll is reproducible across toolchain upgrades. The test below is what catches getting this wrong.
- `load_dir` validation, warn-and-skip each: unknown/duplicate id; a `RandomMainStat(0)`/`PerkPoints(0)` reward (a rung that pays nothing); a `StartingProgram` naming a species id — cross-species validation belongs in `Game::new` where `SpeciesDb` is in hand, not here, so `load_dir` only checks the string is non-empty.

**The ladder** — one file per row, `assets/achievements/<id>.ron`:

| id | name | trigger | reward |
|---|---|---|---|
| `breach_zone_2` | First Breach | `ZoneReached(2)` | `RandomMainStat(1)` |
| `breach_zone_4` | Deep Cut | `ZoneReached(4)` | `PerkPoints(1)` |
| `breach_zone_6` | Sector Runner | `ZoneReached(6)` | `RandomMainStat(1)` |
| `breach_zone_8` | Far Sector | `ZoneReached(8)` | `PerkPoints(1)` |
| `stack_depth_3` | Down the Stack | `StackDepthReached(3)` | `RandomMainStat(1)` |
| `stack_depth_5` | Frame Diver | `StackDepthReached(5)` | `PerkPoints(1)` |
| `stack_depth_8` | Bottom Frame | `StackDepthReached(8)` | `StartingProgram("scrapper")` |
| `uptime_500` | Uptime | `CyclesSurvived(500)` | `RandomMainStat(1)` |
| `uptime_2000` | Long Uptime | `CyclesSurvived(2000)` | `RandomMainStat(1)` |
| `uptime_5000` | Persistent Process | `CyclesSurvived(5000)` | `PerkPoints(1)` |
| `boss_first` | Root Access | `BossDefeated(None)` | `RandomMainStat(1)` |
| `boss_overseer` | Chain of Command | `BossDefeated(Some("overseer"))` | `RandomMainStat(1)` |
| `boss_wintermute` | Ghost in the Wire | `BossDefeated(Some("wintermute"))` | `PerkPoints(1)` |

Write the `description` fields in the game's register — see `assets/species/*.ron` and the existing log copy for tone. They are player-facing.

**Steps:**

- [ ] **Step 1:** Write the failing tests. Five, by intent:
  - `the_shipped_ladder_loads` — `load_dir` over `assets/achievements/` yields 13 defs and zero warnings. Use `test_assets_dir()` from `crates/engine/src/tests/support.rs`.
  - `a_malformed_achievement_file_is_skipped_not_fatal` — write a junk `.ron` to a temp dir alongside a good one; assert the good one loads, one warning is returned, no panic.
  - `a_reward_that_pays_nothing_is_refused` — `RandomMainStat(0)` is skipped with a warning. This is the `PerkDb` `cost == 0` guard's counterpart.
  - `the_full_ladder_stays_under_its_ceiling` — sum the real assets: at most 8 `RandomMainStat` points, at most 5 `PerkPoints`, at most 1 `StartingProgram`. **This is the balance gate for the whole feature** — `balance_sim` does not model the profile. Its failure message should say so.
  - `a_rolled_stat_is_a_pure_function_of_the_id` — `roll_main_stat` twice on the same id agrees, and the four ids in the ladder that use `RandomMainStat` do not all return the same variant (i.e. it is actually distributing).
- [ ] **Step 2:** Run them; expect failure to compile (`achievements` module not found).
- [ ] **Step 3:** Implement `achievements.rs` and author the 13 `.ron` files.
- [ ] **Step 4:** Write `assets/achievements/README.md` — the schema reference, matching the depth and shape of `assets/perks/README.md`. Every field, every `Trigger` and `Reward` variant, and the ceiling test named so a modder knows why their eighth stat point is rejected.
- [ ] **Step 5:** `cargo test -p feral-processes-engine achievement` — all five pass. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 6:** Commit.

---

### Task 2: The profile and its file

**Files:**
- Modify: `crates/engine/src/achievements.rs`
- Test: same in-file test module

**Interfaces — Consumes:** `AchievementId`, `MainStat` from Task 1.
**Interfaces — Produces:**

```rust
pub struct Earned { pub id: AchievementId, pub first_tick: u64, pub permadeath: bool, pub rolled_stat: Option<MainStat> }
pub struct Profile { pub earned: Vec<Earned> }     // also a bevy Resource, Default = empty
impl Profile {
    pub fn contains(&self, id: &AchievementId) -> bool;
    /// Records a first earn, or upgrades an existing entry's `permadeath`
    /// flag to true. Returns false if this was not a first earn.
    pub fn record(&mut self, entry: Earned) -> bool;
    pub fn load(path: &Path) -> Self;              // absent or malformed => empty, never Err
    pub fn save(&self, path: &Path) -> std::io::Result<()>;
}
```

**Notes for the implementer:**

- `Profile::load` returns `Self`, not `Result` — an unreadable profile must never block starting the game. Log the reason through the caller's warning channel; do not `unwrap`.
- `record` is where the difficulty rule lives: first earn wins, and a later permadeath re-earn upgrades `permadeath` to `true` but never downgrades it. Nothing else may write that field.
- Serialize with `ron::ser::to_string_pretty`, matching `save::to_ron`. The file is meant to be readable and hand-editable.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `a_profile_survives_a_round_trip_through_ron` — mirrors `a_save_survives_a_round_trip_through_ron_unchanged` in `save.rs`.
  - `an_absent_profile_is_empty_not_an_error`.
  - `a_corrupt_profile_is_empty_not_a_panic`.
  - `recording_the_same_achievement_twice_earns_it_once` — second `record` returns false and does not append.
  - `a_permadeath_re_earn_upgrades_the_flag_and_never_downgrades` — record on Forgiving then Permadeath sets true; the reverse order stays true.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p feral-processes-engine profile` — green. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 3: `RunFeats` and the boss-kill record

**Files:**
- Modify: `crates/engine/src/resources.rs`
- Modify: `crates/engine/src/game/combat_rewards.rs` (inside `award_loot`'s `if species.is_boss` branch, around line 105)
- Modify: `crates/engine/src/game/lifecycle.rs` (insert the resource in `new` **and** `load`)
- Test: `crates/engine/src/tests/` — a new `achievements.rs` test module, registered in `crates/engine/src/tests/mod.rs`

**Interfaces — Produces:**

```rust
#[derive(Resource, Default)]
pub struct RunFeats { pub bosses_defeated: Vec<String> }   // species ids, drained each tick
```

**Notes for the implementer:**

- Read the spec's "`BossDefeated(species_id)` is a feat, not a threshold" section first. The whole point of this task is that the kill site **records and nothing else** — no achievement lookup, no reward, no profile write. If you find yourself importing `AchievementDb` into `combat_rewards.rs`, stop.
- The push goes inside the existing `if species.is_boss` branch, beside the `BOSS_PORTAL_FRAGMENT_DROP` grant. Do not add a second `is_boss` check elsewhere. The three comments above it (lines 96–103) explain why this specific point is the one that knows the boss *died* rather than being fled from — your record depends on exactly that guarantee, same as `mark_lair_cleared` and `raise_trace` do.
- `RunFeats` must be inserted in **both** `Game::new` and `Game::load`, or a loaded save panics on first boss kill.
- It is not saved. That is deliberate and the spec says why; do not add it to `SaveData`.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `killing_a_boss_records_its_species` — spawn a boss (`overseer`) on the player's tile via `spawn_wild_on_player_tile` from `tests/support.rs`, resolve until dead with `resolve_round_with`, assert `RunFeats.bosses_defeated == ["overseer"]`.
  - `killing_an_ordinary_program_records_nothing`.
  - `fleeing_a_boss_records_nothing` — use `flee_until_clear` from `tests/support.rs`. This is the guarantee the placement buys; assert it rather than assume it.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p feral-processes-engine boss` — green. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 4: `achievement_system` — the one place that decides

**Files:**
- Create: `crates/engine/src/game/achievements.rs`
- Modify: `crates/engine/src/game/mod.rs` (declare the module)
- Modify: `crates/engine/src/resources.rs` (add `PendingProfileWrites`)
- Modify: `crates/engine/src/game/lifecycle.rs` (insert `AchievementDb`, `Profile`, `PendingProfileWrites` in `new` and `load`; add `achievement_system` to `build_schedule`)
- Test: `crates/engine/src/tests/achievements.rs`

**Interfaces — Consumes:** `AchievementDb`, `Profile`, `Trigger`, `Earned` (Tasks 1–2); `RunFeats` (Task 3).
**Interfaces — Produces:**

```rust
#[derive(Resource, Default)]
pub struct PendingProfileWrites(pub Vec<AchievementId>);   // drained by app-core, which owns the path

pub fn achievement_system(
    db: Res<AchievementDb>, mut profile: ResMut<Profile>, mut feats: ResMut<RunFeats>,
    clock: Res<GameClock>, zone: Res<ZoneLevel>, locale: Res<Locale>,
    difficulty: Res<DifficultyMode>, mut pending: ResMut<PendingProfileWrites>,
    mut log: ResMut<MessageLog>,
);
```

**Notes for the implementer:**

- One pass over `db.iter()`. Skip anything `profile.contains`. Evaluate:
  - `ZoneReached(n)` → `zone.0 >= n`
  - `StackDepthReached(n)` → `matches!(*locale, Locale::Stack { depth, .. } if depth >= n)`
  - `CyclesSurvived(n)` → `clock.tick >= n`
  - `BossDefeated(None)` → `!feats.bosses_defeated.is_empty()`; `BossDefeated(Some(s))` → the vec contains `s`
- **Read `Locale`, never `Position`.** `Position` is pinned to the surface entrance tile while the party is underground — see the CLAUDE.md seam entry and `nest_aggro_tick`'s guard.
- **Clear `feats.bosses_defeated` at the end of the pass, unconditionally** — including when nothing was earned, or a single kill re-earns forever.
- On an earn: `profile.record(Earned { id, first_tick: clock.tick, permadeath: matches!(*difficulty, DifficultyMode::Permadeath), rolled_stat })`, push the id to `pending`, and log. `rolled_stat` is `Some(roll_main_stat(&id))` only for a `RandomMainStat` reward, `None` otherwise.
- The log line needs a `MessageKind` that survives `MessageLog::retain_outcomes_since_battle`, which keeps only `Outcome`, `Loot`, `LevelUp` and `Raid`. A rung can be crossed mid-fight, so `Info` would silently vanish when the battle ends. Use `MessageKind::Outcome`.
- Add it to `build_schedule` **unchained**, after `death_handling_system`. It shares no mutable state with anything already there.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `reaching_zone_two_earns_the_first_breach` and `staying_in_zone_one_earns_nothing`.
  - `descending_to_depth_three_earns_the_stack_rung` — use a `dev-saves/` stack template or descend via the real path; do not hand-write `Locale`.
  - `surviving_five_hundred_cycles_earns_uptime`.
  - `killing_a_boss_earns_both_the_generic_and_the_species_rung_in_one_tick`.
  - `a_second_tick_after_a_boss_kill_earns_nothing_more` — the `RunFeats` clear. This is the one that catches the forever-re-earn bug.
  - `an_achievement_is_earned_only_once` — cross a threshold, tick again, assert `pending` got one id total.
  - `an_earned_line_survives_the_end_of_a_battle` — earn during a battle, `end_battle`, assert the line is still in the log.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p feral-processes-engine achievement` — green. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 5: Rewards pay after a new game, never after a load

**Files:**
- Modify: `crates/engine/src/game/lifecycle.rs`
- Test: `crates/engine/src/tests/achievements.rs`

**Interfaces — Produces:**

```rust
impl Game {
    /// Replaces the empty `Profile` both constructors leave in the world.
    /// Pays nothing — `achievement_system` needs to know what is already
    /// earned on a loaded save too.
    pub fn install_profile(&mut self, profile: Profile);
    /// Pays out the installed profile. Called after `new`, never after `load`.
    pub fn grant_profile_rewards(&mut self);
}
```

**Notes for the implementer:**

- **`Game::new`'s signature does not change.** It has 667 call sites, essentially all in engine tests, and a fourth parameter would mean ~30 files of mechanical churn for no gain. Both `new` and `load` insert `Profile::default()`; app-core installs the real one afterwards.
- These are two orthogonal operations, not one with a flag. Installing says *what has been earned*; granting says *pay for it*. The "never on load" rule then reduces to one fact with one enforcement point: app-core's load path calls `install_profile` and does not call `grant_profile_rewards`. A save already has its bonuses baked into `Stats` and `Perks::points`; paying again on load doubles them on every reload. This is the one real trap in the feature.
- `grant_profile_rewards` reads the installed `Profile` resource — it takes no argument, so the two calls cannot disagree about which profile is in play.
- For each earned entry, apply its def's reward:
  - `RandomMainStat(n)` → `n` points into the axis named by that entry's stored `rolled_stat` (not a fresh roll — the profile's recorded answer). `Atk`/`Def` → `Stats.atk`/`.def`; `Integrity` → **both** `max_hp` and `hp`, or the run starts damaged; `Decompiler` → the `Decompiler` component.
  - `PerkPoints(n)` → `Perks.points += n`.
  - `StartingProgram(species_id)` → follow `adopt_orphan`'s sequence (`crates/engine/src/game/stack_features.rs:376-385`): `spawn_wild_creature_scaled` at the player's start tile with multiplier 1.0, `remove::<(Hostile, WanderAi)>()`, insert `(Tamed { owner }, Experience::default())`, then `install_innate_routines`. **Do not push to `Party`** — `Party` is the deployed battle line and gaining a member is an explicit `add_to_party` capped at `MAX_PARTY_SIZE`. The program arrives owned, and the player deploys it, like every other acquisition.
  - An unknown species id logs a warning and pays nothing. This is the cross-db validation Task 1 deliberately deferred to here.
- Log a short summary line so the player can see what the profile paid.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `a_profile_stat_reward_applies_at_new_game` — build a `Profile` with a known `rolled_stat`, assert the player's stat is above `PLAYER_BASE_STATS` by exactly the right amount.
  - `an_integrity_reward_starts_the_run_at_full_hp` — `hp == max_hp`.
  - `a_perk_point_reward_applies_at_new_game`.
  - `a_starting_program_is_owned_but_not_deployed` — a `Tamed` entity exists, `Party` is empty.
  - `installing_a_profile_pays_nothing_on_its_own` — `install_profile` alone leaves stats at `PLAYER_BASE_STATS`. **The trap test**, and it is the load path in miniature.
  - `granting_twice_pays_once` — a second `grant_profile_rewards` is a no-op. Nothing should call it twice, but the doubling bug is invisible if it does.
  - `a_starting_program_naming_an_unknown_species_warns_and_pays_nothing`.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement. No existing `Game::new` call site changes.
- [ ] **Step 4:** `cargo test -p feral-processes-engine achievement` — green. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 6: app-core owns the path

**Files:**
- Modify: `crates/app-core/src/lib.rs` (the `App` struct — add `profile_path`)
- Modify: `crates/app-core/src/app/lifecycle.rs` (`App::new`, new-game and tick paths)
- Modify: `crates/launcher/src/main.rs` (supply the path, ~line 50 beside `history_path`)
- Modify: `crates/app-core/src/tests/support.rs`, `tests/saves.rs`, `tests/quitting.rs` (the `App::new` fixtures)
- Test: `crates/app-core/src/tests/` — new `achievements.rs`

**Interfaces — Produces:**

```rust
impl App {
    pub fn new(assets_dir: PathBuf, saves_dir: PathBuf, history_path: PathBuf, profile_path: PathBuf) -> Self;
}
```

**Notes for the implementer:**

- `profile.ron` sits at the repo root beside `run_history.log`, **not** in `saves/`. Follow how `history_path` is built in `crates/launcher/src/main.rs:50`.
- **New game:** construct the `Game`, then `install_profile(Profile::load(&self.profile_path))`, then `grant_profile_rewards()`.
- **Load:** construct the `Game`, then `install_profile(...)` and **stop**. Not calling `grant_profile_rewards` here is the whole of the never-on-load rule; put a comment saying so, because the omission is invisible otherwise.
- After each `tick`, drain `PendingProfileWrites` and, if non-empty, write the profile to disk. Immediately — not at run end. A permadeath run that ends badly must not lose what it proved. Expose the drain through `Game` (a small `pub fn take_pending_profile_writes(&mut self) -> Vec<AchievementId>` plus a profile accessor); the renderer and app-core still never touch the `World`.
- A failed write must not crash the game. Log it the way `game.write_history` failures are handled in `app/lifecycle.rs:267`.
- Each test needs a unique temp path — copy the `format!` + timestamp idiom already in `crates/app-core/src/tests/saves.rs:14`.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `earning_an_achievement_writes_the_profile_immediately` — tick past a threshold, assert the file exists on disk mid-run.
  - `the_written_profile_reloads_equal`.
  - `a_new_game_starts_from_the_profile_on_disk` — write a profile, start a new game through `App`, assert the reward landed.
  - `loading_a_save_does_not_re_apply_rewards` — new game with a profile, save, load through `App`, assert the stat did not move. This is the trap's end-to-end form; Task 5's engine test only covers the halves.
  - `an_unwritable_profile_path_does_not_crash_the_tick`.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p feral-processes-app-core` — green. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 7: The achievements screen

**Files:**
- Modify: `crates/engine/src/views.rs` (the row type)
- Modify: `crates/engine/src/game/inspection.rs` (`Game::achievement_report`)
- Modify: `crates/app-core/src/lib.rs` (`Mode::Achievements`, key handling, row count)
- Modify: `crates/app-core/src/app/menus.rs` (the main-menu row)
- Modify: `crates/gui/src/render/meta.rs` (draw it)
- Test: `crates/engine/src/tests/achievements.rs`, `crates/app-core/src/tests/achievements.rs`

**Interfaces — Produces:**

```rust
pub struct AchievementRow { pub name: String, pub description: String, pub reward: String, pub earned: Option<EarnedSummary> }
pub struct EarnedSummary { pub tick: u64, pub permadeath: bool, pub rolled_stat: Option<String> }
impl Game { pub fn achievement_report(&self) -> Vec<AchievementRow>; }
```

**Notes for the implementer:**

- **The row count is owned by app-core and the rows are drawn by gui, so any per-row transform lives in the engine.** Both sides call `achievement_report()`; a renderer that rebuilt the list would scroll to a row that isn't drawn. This is the same rule `Game::message_history` and `Game::structure_report` follow — see the CLAUDE.md seam entry.
- The screen lists **every** authored achievement, earned or not, in `AchievementDb::iter()` order — the point is showing the player what is left.
- `render/meta.rs` is "the screens outside a run". Follow `draw_main_menu`'s use of `draw_popup` and the `Row` helpers.
- Reachable from the main menu only. Do not add it to the group menus.
- The profile is loaded lazily for this screen when no `Game` exists — the main menu has no run in progress. `App` can hold a `Profile` loaded at construction and reload it on write.

**Steps:**

- [ ] **Step 1:** Write the failing tests:
  - `the_report_lists_every_authored_achievement` — 13 rows over the real assets.
  - `an_unearned_achievement_reports_no_summary`.
  - `an_earned_achievement_reports_its_cycle_mode_and_rolled_stat`.
  - `the_screen_opens_from_the_main_menu_and_esc_returns_to_it` (app-core).
  - `the_screens_row_count_matches_the_report` (app-core) — the drift guard.
- [ ] **Step 2:** Run; expect failure.
- [ ] **Step 3:** Implement engine → app-core → gui, in that order.
- [ ] **Step 4:** `cargo test -p feral-processes-engine achievement && cargo test -p feral-processes-app-core achievement`. `cargo fmt`, `cargo clippy --workspace`.
- [ ] **Step 5:** Commit.

---

### Task 8: Docs and the full gate

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`
- Modify: `TODO.md` (tick the achievements line)

**Notes for the implementer:**

- `CLAUDE.md` and `AGENTS.md` are gitignored twins with nothing tracking their drift — edit the first and copy it over the second.
- Two new entries belong in CLAUDE.md's **Load-bearing seams**, written in that section's voice — each stating the trap, not just the fact:
  1. Rewards apply at `Game::new` and never at `Game::load`, because a save has them baked in and a reload would double them.
  2. `RunFeats` is a per-tick drain queue and is not saved, which is only sound while every boss trigger names one species; an in-run *count* trigger needs saved state and a format bump.
- Do **not** touch `docs/manual.md` — it is deliberately carved out until the user says otherwise.
- Grep the root `README.md` for claims this change falsifies before editing (progression, what persists, save format).

**Steps:**

- [ ] **Step 1:** `cargo test --workspace` — the real gate. Everything green.
- [ ] **Step 2:** `cargo test -p feral-processes-engine balance_sim` — confirm the run curve did not move. It should not: the profile sits outside the simulated run.
- [ ] **Step 3:** `cargo clippy --workspace` clean, `cargo fmt` applied.
- [ ] **Step 4:** Write the docs.
- [ ] **Step 5:** Commit.

---

## After the plan

**The three `CyclesSurvived` thresholds (500 / 2000 / 5000) are uncalibrated guesses.** No test can tell us whether they are trivial or unreachable — only a real run can, and this repo has a standing pattern of shipping arithmetic-plausible numbers that were never played. Launch the game and check where a run actually lands before treating them as final.
