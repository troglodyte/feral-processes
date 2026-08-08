# Interactive arena — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a dev build a battle scenario in the game's own UI, play it
with the full battle interface, and round-trip it with the headless `arena`
bin.

**Architecture:** `arena::run`'s setup is split out as `arena::stage` so the
played fight and the measured one share one code path; `arena::Watch` is the
one reader of what a fight cost. app-core holds an `ArenaSession` whose
presence makes the run inert on disk, and edits a real `arena::Scenario`
rather than a parallel builder type. gui draws it through `Painter`.

**Tech stack:** Rust, `bevy_ecs` 0.19 (engine only), `bevy` + `bevy_egui`
(gui only), `ron` + `serde`.

**Spec:** `docs/superpowers/specs/2026-08-08-interactive-arena-design.md` —
read it before starting. It carries the *why* for every non-obvious choice
below, and several of those choices look arbitrary without it.

## Global constraints

Read `CLAUDE.md` first; these are the ones this plan trips over most.

- **No new public `Game` method.** `begin_battle`, `spawn_wild_creature_scaled`
  and the `world` field must stay unreachable from outside the engine crate.
  The arena reaches them because it *is* the engine.
- **A doc comment claiming to mirror other code must be a call, not a copy.**
  This plan exists largely because the outcome-reading logic would otherwise
  be copied into app-core.
- **A screen's rows come from one function**, which is both the count the
  handler dispatches against and the labels gui draws.
- **Only `crates/gui/src/paint.rs` names a graphics library.** Nothing in
  `render/` may.
- **Tuning values go in `crates/engine/src/tuning.rs`**, not inline.
- **Comments explain why, never what.**
- **TDD:** failing test first, every task. `cargo fmt` and
  `cargo clippy --workspace` after every task; fix warnings rather than
  silencing them.
- **`cargo test --workspace` is the final gate** — currently 1545 tests.
  Per-task gates are the targeted `-p` runs named in each task.
- **No save-format change.** `SAVE_FORMAT_VERSION` must not move.
- **Docs:** `README.md` and `docs/manual.md` are carved out. `CHANGELOG.md`
  and `dev-arenas/README.md` are not.

If a warm build takes minutes, something is wrong — `cargo check --workspace`
is ~1s and the engine suite ~3s. Mass `NotFound` failures on an assets path
mean stale artifacts, not bugs: `cargo clean -p feral-processes-engine -p
feral-processes-app-core` (never a full `cargo clean`, `target/` is ~4 GB).

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `crates/engine/src/arena/watch.rs` | **new** — `Watch`: what a fight cost, sampled per round | 1 |
| `crates/engine/src/arena/mod.rs` | `stage`, `Staged`, re-exports | 1 |
| `crates/engine/src/arena/run.rs` | `run_rep` reduced to the auto-play loop | 1 |
| `crates/launcher/src/dev_template.rs` | gains `resolve`, shared by the bin and the game | 2 |
| `crates/launcher/src/bin/arena.rs` | drops its private `resolve_template` | 2 |
| `crates/launcher/src/main.rs` | installs `DevTemplates` on `App` | 3 |
| `crates/app-core/src/app/arena.rs` | **new** — the whole arena screen family: session, rows, handlers | 3–8 |
| `crates/app-core/src/lib.rs` | `Mode` variants, `App` fields, `DevTemplates` | 3 |
| `crates/app-core/src/app/mod.rs` | declares `arena` | 3 |
| `crates/app-core/src/app/input.rs` | dispatches the five modes | 3 |
| `crates/app-core/src/app/menus.rs` | the gated main-menu row | 3 |
| `crates/app-core/src/app/lifecycle.rs` | the three omissions | 4 |
| `crates/app-core/src/app/battle.rs` | the `settle_after_round` hook | 4 |
| `crates/app-core/src/tests/arena.rs` | **new** — app-core's arena tests | 3–8 |
| `crates/gui/src/render/arena.rs` | **new** — builder, picker, result | 9 |
| `crates/gui/src/render/mod.rs` | dispatches the five modes | 9 |

`app/arena.rs` is one file rather than five because every screen in the
family mutates one `ArenaSession` and they are meaningless apart — this
follows `app/trade.rs` and `app/group_menu.rs`. If it passes ~600 lines,
split the *pickers* out as `app/arena_pick.rs`; do not split by mode.

---

## Task 1: `arena::stage` and `arena::Watch`

Engine only. Nothing outside the engine changes, and the headless bin must
behave identically when this lands.

**Files:**
- Create: `crates/engine/src/arena/watch.rs`
- Modify: `crates/engine/src/arena/mod.rs`, `crates/engine/src/arena/run.rs`

**Interfaces produced:**

```rust
// arena/mod.rs
pub struct Staged { pub game: Game, pub watch: Watch, pub warnings: Vec<String> }
pub fn stage(scenario: &Scenario, assets_dir: &Path, seed: u64) -> Result<Staged, String>;

// arena/watch.rs
pub struct Watch { /* private */ }
impl Watch {
    pub(crate) fn new(game: &Game, seed: u64, opponents: Vec<Entity>) -> Self;
    pub fn observe(&mut self, game: &Game);
    pub fn finish(&self, game: &Game) -> RepRecord;
    pub fn rounds(&self) -> u32;
}
```

`Watch::new` is `pub(crate)` — only `stage` may build one, so a `Watch` can
never be out of step with the fight it is watching. `observe` and `finish`
are `pub` because app-core calls them.

**What moves where.** `stage` takes over `build_player`, `build_opponents`,
the `GameRng` insert, the `keep_battle_narration` set and `begin_battle` —
all currently split between `run::run_rep` and `mod::run`. `run_rep`'s new
signature is `run_rep(game: &mut Game, watch: &mut Watch) -> RepRecord` and
its body is only the loop: the break conditions, `battle_plan_remaining` /
`battle_round_ready` / `battle_resolve_round`, and `watch.observe` after each
resolve. Every per-round concern it currently owns — the round count, the
HP sample, the level check, the transcript append — moves into `observe`.
`run` becomes `stage` then `run_rep`, per rep, at `scenario.seed + rep`.

**The one non-obvious thing:** `observe` must sample HP *only* when the
player's level is unchanged since the last call, and must append
`game.battle_log()` on *every* call. Both reasons are in `run.rs`'s existing
comments — carry them across; do not restate them in the plan's words.

- [ ] **Step 1: Write the failing tests** in `arena/mod.rs`'s test module
      and a new one in `watch.rs`.
  - `staging_leaves_the_fight_open_with_nobody_having_acted` — after
    `stage`, `game.has_active_battle()` is true and the round count is 0.
  - `staging_then_running_matches_run_at_the_same_seed` — `run` on a
    1-rep scenario and `stage` + `run_rep` at the same seed produce equal
    `RepRecord`s. **This is the property the whole split rests on.**
  - `staging_reports_the_composition_warnings` — a 9-strong group at zone 1
    comes back with a warning naming the species, the ask and the ceiling
    (move/adapt `exceeding_the_zones_ceiling_warns_...` from `setup.rs`, or
    assert through `Staged::warnings` and leave the setup test alone).
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-engine arena` — expect "cannot find function `stage`".
- [ ] **Step 3: Create `watch.rs`** with `Watch` and the three methods.
      Move `alive`, `hp_fraction_of` and `level_of` out of `run.rs` into it.
- [ ] **Step 4: Add `stage`/`Staged` to `mod.rs`; reduce `run_rep`; rewrite `run` on top of `stage`.** Declare `mod watch;` and re-export `Watch`.
- [ ] **Step 5: Move the two sampling tests onto `Watch`.**
      `a_level_up_on_the_killing_blow_does_not_report_the_fight_as_free` and
      `a_won_fights_transcript_survives_end_battle` move from `run.rs` to
      `watch.rs`, since that is where the logic now lives. They must keep
      asserting the same behaviour, not be rewritten to suit the new shape.
- [ ] **Step 6: Run the gates.** `cargo test -p feral-processes-engine arena`, then `cargo test -p feral-processes-engine` in full — `balance_sim` and the battle suites must be untouched. Then `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 7: Verify the bin is unchanged by hand.** `cargo run --bin arena -- dev-arenas/opening-fight.ron` and `cargo run --bin arena -- dev-arenas/full-group.ron`. The transcripts and the summary must read the same as before the refactor; a diverged seed here means `stage` reordered something the RNG stream sees.
- [ ] **Step 8: Commit.** `refactor(arena): split staging and outcome-reading out of run_rep`

---

## Task 2: the launcher resolves templates once

Pure refactor, no behaviour change. Deletes a copy before the game needs a
second one.

**Files:**
- Modify: `crates/launcher/src/dev_template.rs`, `crates/launcher/src/bin/arena.rs`

**Interfaces produced:**

```rust
// dev_template.rs
pub fn resolve(name: &str) -> Result<PathBuf, String>;
```

Generates the template into its working copy and returns that path,
appending `known()` to the error the way `bin/arena.rs::resolve_template`
does today. The bin's `resolve_template` becomes a call to it and keeps its
`PlayerSource::Template` match, since mutating a `Scenario` is the bin's
business and not `dev_template`'s.

- [ ] **Step 1: Write the failing test** in `dev_template.rs` —
      `resolving_an_unknown_template_names_it_and_lists_the_known_ones`.
      Check the existing tests there first for how they handle the real
      `dev-saves/` directory; follow whatever they already do rather than
      inventing a fixture.
- [ ] **Step 2: Run and watch it fail.** `cargo test -p feral-processes resolve`
- [ ] **Step 3: Add `resolve`; rewrite the bin's `resolve_template` to call it.**
- [ ] **Step 4: Gate.** `cargo test -p feral-processes`, then `cargo run --bin arena -- dev-arenas/geared-vs-boss.ron` — the one shipped scenario that takes the template path. `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 5: Commit.** `refactor(launcher): one template resolver for the bin and the game`

---

## Task 3: the gate, the menu row, and an empty session

The screen becomes reachable and holds a `Scenario`. It cannot fight yet.

**Files:**
- Create: `crates/app-core/src/app/arena.rs`, `crates/app-core/src/tests/arena.rs`
- Modify: `crates/app-core/src/lib.rs`, `app/mod.rs`, `app/input.rs`,
  `app/menus.rs`, `src/tests/mod.rs`, `crates/launcher/src/main.rs`

**Interfaces produced:**

```rust
// lib.rs
pub enum Mode { /* ... */ ArenaBuilder, ArenaLoad, ArenaSave, ArenaPick, ArenaResult }

pub struct DevTemplates {
    pub names: Vec<String>,
    pub resolve: fn(&str) -> Result<PathBuf, String>,
}

impl App {
    pub fn install_dev_templates(&mut self, templates: DevTemplates);
    pub fn arena_enabled(&self) -> bool;
    pub(crate) fn in_arena(&self) -> bool;
}
```

New `App` fields: `arena: Option<ArenaSession>`, `arena_enabled: bool`,
`dev_templates: Option<DevTemplates>`. `ArenaSession` is `pub(crate)` and
lives in `app/arena.rs`; its fields are listed in the spec's §2 — build it
with all of them now, even the ones later tasks fill, so the type does not
churn under five tasks.

`arena_enabled` is read once in `App::new` from `FERAL_DEV_ARENA`, using the
same predicate shape as `crates/engine/src/game/stack_view.rs` — present,
non-empty, not `"0"`. Read the existing one and match it; two answers to
"is a dev flag set" is exactly the drift this repo keeps catching.

Add all five `Mode` variants now and dispatch all five in `input.rs`, even
though three handlers are stubs that only honour Esc. A mode added later
means touching `input.rs`, `render/mod.rs` and the `Mode` doc comments
again for each one.

**Menu wiring.** `handle_main_menu_key` builds its option list dynamically
already (Load Game is conditional on there being saves) — push `'r'` after
`'a'` when `arena_enabled`, and mirror it in `render/meta.rs` in Task 9.
Opening the arena sets `self.arena = Some(ArenaSession::new(...))` and
`Mode::ArenaBuilder`; Esc from `ArenaBuilder` drops the session and returns
to `Mode::MainMenu`, the way `handle_achievements_key` returns rather than
going through `close_screen`.

**Launcher.** `main.rs` calls `app.install_dev_templates(DevTemplates {
names: dev_template::list(), resolve: dev_template::resolve })` right after
`App::new`. Unconditionally — the gate decides visibility, and a launcher
that installs only when gated makes the flag mean two things.

- [ ] **Step 1: Write the failing tests** in `tests/arena.rs`. Look at
      `tests/support.rs` for the existing `App` fixture before writing one.
  - `without_the_dev_flag_the_arena_row_is_absent` — with `arena_enabled`
    false, pressing `r` on the main menu leaves the mode at `MainMenu`.
  - `with_the_dev_flag_r_opens_the_builder` — mode becomes `ArenaBuilder`
    and `app.arena.is_some()`.
  - `esc_from_the_builder_drops_the_session` — back to `MainMenu`, and
    `app.arena.is_none()` so a stale scenario cannot outlive the screen.
  - **Do not set `FERAL_DEV_ARENA` in a test.** Env is process-global and
    the suite is parallel. Make `arena_enabled` a field the fixture can set
    directly, and let `App::new` be the only place that reads the variable.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Add the `Mode` variants, the `App` fields, `DevTemplates`, and the `input.rs` dispatch** (three stub handlers honouring Esc only).
- [ ] **Step 4: Create `app/arena.rs`** with `ArenaSession`, `ArenaSession::new`, `in_arena`, `arena_enabled`, and `handle_arena_builder_key` handling Esc.
- [ ] **Step 5: Wire the main menu and the launcher.**
- [ ] **Step 6: Gate.** `cargo test -p feral-processes-app-core`, `cargo fmt && cargo clippy --workspace`. gui will not compile against the new `Mode` variants until Task 9 — add the arms as `todo!()`-free no-ops there now if `cargo check --workspace` fails, and replace them in Task 9.
- [ ] **Step 7: Commit.** `feat(arena): a dev-gated arena screen holding a scenario`

---

## Task 4: staging a fight, and the three omissions

The largest task. An arena scenario can be fought and lands on a result, and
the session touches no disk.

**Files:**
- Modify: `app/arena.rs`, `app/lifecycle.rs`, `app/battle.rs`,
  `tests/arena.rs`

**Interfaces produced:**

```rust
impl App {
    /// Stages `session.scenario` at `session.seed` and opens `Mode::Battle`.
    /// A staging error stays on the builder with the reason in the status line.
    pub(crate) fn start_arena_fight(&mut self);
}
```

`start_arena_fight` resolves `PlayerSource::Template` through
`self.dev_templates` into a `PlayerSource::Save` on a **clone** of the
scenario before calling `arena::stage` — the session's own scenario keeps
saying `Template(name)`, or saving it back out would rewrite the author's
file into a path. Absent `dev_templates`, a `Template` source is a status
line error naming it. Then it installs the `Game`, stores the `Watch` and
`warnings`, puts the warnings in the status line, and sets `Mode::Battle`.

**`current_save_path` must be `None`.** It is already, for a session opened
from the main menu — assert it rather than assuming it.

**The battle hook** (`app/battle.rs`): `settle_after_round` gains
`watch.observe` and, when `!still_active` and `in_arena()`, sets
`Mode::ArenaResult` and stores `watch.finish(game)` in the session instead
of `Mode::Playing`. First make the `PartyCommandKind::JackOut` arm call
`settle_after_round` unconditionally rather than only when the battle ended
— see the spec §4 for why, and satisfy yourself it is a no-op when the
battle is still live before changing it.

**The three omissions** (`app/lifecycle.rs`): `after_tick` early-returns on
`in_arena()`, covering `flush_profile_writes` and `maybe_autosave` together;
`check_game_over` guards separately and routes to `Mode::ArenaResult`.

- [ ] **Step 1: Write the failing tests.**
  - `an_arena_fight_opens_the_battle_screen` — after `start_arena_fight` on
    a scenario with one opponent, mode is `Battle` and the game has an
    active battle.
  - `winning_an_arena_fight_lands_on_the_result` — drive the fight with the
    real all-attack key until the battle ends; mode is `ArenaResult` and the
    session holds a `RepRecord` whose `won` is true. Use a lopsided
    composition so it cannot stall.
  - `an_arena_fight_writes_no_save` — `current_save_path` is `None` and no
    `.bin` appears in a temp saves dir across a whole fight.
  - `an_arena_fight_writes_no_profile` — `profile.ron` is absent (or
    unchanged) after a fight that kills a boss species. **Assert on the
    file, not on `App::profile`** — the omission being tested is the write.
  - `an_arena_loss_writes_no_run_history` — with a Permadeath save source,
    `run_history.log` is absent and mode is `ArenaResult`, not `GameOver`.
  - `a_failed_jack_out_still_counts_its_round` — the regression the
    unconditional `settle_after_round` fixes. Assert `watch.rounds()`
    advanced, which is what a missed `observe` loses.
  - Each omission gets its own test because an omission is invisible
    otherwise, and the regression is a later change adding one back.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Make `JackOut` call `settle_after_round` unconditionally.** Run `cargo test -p feral-processes-app-core battle` — it must stay green on its own before anything is layered on it.
- [ ] **Step 4: Add `start_arena_fight`.**
- [ ] **Step 5: Add the `settle_after_round` hook.**
- [ ] **Step 6: Add the three guards in `lifecycle.rs`,** with a comment on `check_game_over`'s explaining why it is not inside `after_tick`.
- [ ] **Step 7: Gate.** `cargo test -p feral-processes-app-core`, `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 8: Commit.** `feat(arena): play a staged scenario in the real battle UI`

---

## Task 5: the result screen

**Files:** `app/arena.rs`, `tests/arena.rs`

`handle_arena_result_key`: `r` refights at the same seed, `n` increments
`session.seed` and refights, Esc returns to `Mode::ArenaBuilder` with the
scenario intact. Both refights go through `start_arena_fight`, so there is
one staging path. Up/Down scroll the transcript through the existing
`App::scroll` helper against `record.transcript.len()`.

- [ ] **Step 1: Write the failing tests.**
  - `refighting_keeps_the_seed` — `r` re-enters `Mode::Battle` with the
    session's seed unchanged.
  - `the_next_seed_key_advances_by_one` — `n` leaves `session.seed` one
    higher, matching what rep *n* would have been in the headless run.
  - `esc_from_the_result_returns_to_the_builder_with_the_scenario_intact` —
    mode is `ArenaBuilder` and the scenario equals what was fought.
  - `a_refight_starts_from_a_whole_party` — the regression `arena::run`
    already guards with a fresh `Game` per rep. Fight with a companion,
    lose it, refight, and assert the companion acts in the second fight —
    assert on *behaviour* (a transcript line naming it), not on a count,
    since a lopsided fight can be won without it ever swinging.
  - `jacking_out_records_a_loss` — flee an arena fight and the result says
    lost, because `Watch::finish` reads the standing pack rather than the
    player. The alternative — an abandon that counted as neither — would be
    a third notion of an outcome.
  - The session keeps `warnings` from staging; the result screen must
    surface them (drawn in Task 9), so assert here that they survive the
    fight rather than being cleared when the battle opens.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Implement `handle_arena_result_key`.**
- [ ] **Step 4: Gate.** `cargo test -p feral-processes-app-core`, `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 5: Commit.** `feat(arena): refight, reseed and return from the result screen`

---

## Task 6: the builder's rows

**Files:** `app/arena.rs`, `tests/arena.rs`

**Interfaces produced:**

```rust
pub struct ArenaRow { pub label: String, pub kind: ArenaRowKind }

pub enum ArenaRowKind {
    PlayerSource, PlayerLevel, PlayerZone,
    Equip(usize), AddEquip,
    Inventory(usize), AddInventory,
    Party(usize), AddParty,
    Opponent(usize), AddOpponent,
    Reps, Seed,
}

impl App {
    /// The one source of both the rows the handler dispatches against and
    /// the labels gui draws.
    pub fn arena_builder_rows(&self) -> Vec<ArenaRow>;
}
```

Left/Right adjusts the number on the highlighted row (level, zone, count,
qty, tier, seed, reps — and cycles the player source). Enter on an `Add …`
opens `Mode::ArenaPick`; Enter on an existing spec row reopens the picker
for it. Backspace removes the highlighted spec row. Enter on any other row
does nothing; a separate key (`f`) starts the fight, so Enter never
ambiguously means "edit this" and "fight".

**Row hiding is the point of this task.** `Equip`, `Inventory`, `Party` and
their `Add` rows are omitted entirely when `scenario.player` is not
`Fresh`, because the engine treats them as an *error* on a save or template
rather than ignoring them. `PlayerLevel` and `PlayerZone` likewise.

Clamp every number to something the game can represent: companion level at
`CREATURE_MAX_LEVEL`, opponent count at `MAX_GROUP_SIZE`, opponent rows at
`MAX_ENEMY_GROUPS`, all from `tuning.rs`. Past those, `build_opponents`
hard-errors rather than warning, so a builder that let you author them would
produce a scenario that cannot be fought or saved usefully. **Do not clamp
to the zone ceilings** — those warn deliberately, and clamping them would
delete the tool's main question.

- [ ] **Step 1: Write the failing tests.**
  - `a_save_player_hides_the_loadout_rows` — with `PlayerSource::Save`, no
    row of kind `Equip`/`Inventory`/`Party`/`AddParty`/… is present.
    Assert through `arena_builder_rows`, never against a static table.
  - `the_row_under_the_highlight_is_the_row_that_changes` — with a save
    source selected and the highlight on the row *after* the player source,
    Right adjusts what the label says it adjusts. This is the bug hidden
    rows cause and the reason the rows are one function.
  - `right_on_an_opponent_row_raises_its_count`
  - `an_opponent_count_stops_at_max_group_size`
  - `backspace_removes_the_highlighted_party_row`
  - `a_composition_past_the_zone_ceiling_is_still_authorable` — nine at
    zone 1 builds, because warning rather than capping is the point.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Implement `arena_builder_rows` and the builder key handler.**
- [ ] **Step 4: Gate.** `cargo test -p feral-processes-app-core`, `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 5: Commit.** `feat(arena): edit a scenario row by row in the builder`

---

## Task 7: the picker

**Files:** `app/arena.rs`, `tests/arena.rs`

**Interfaces produced:**

```rust
pub(crate) enum ArenaPickKind { PartySpecies(Option<usize>), OpponentSpecies(Option<usize>), EquipItem(Option<usize>), InventoryItem(Option<usize>) }

impl App {
    /// The picker's rows — species or items, depending on what opened it.
    pub fn arena_pick_rows(&self) -> Vec<String>;
}
```

`Some(index)` replaces an existing row's id, `None` appends a new one. One
mode, four targets, following `Mode::ManifestPick`.

**The catalog.** The screen is reachable from the main menu where `self.game`
is `None`, so app-core loads `SpeciesDb::load_dir(assets/species)` and
`ItemDb::load_dir(assets/items)` itself — the same warn-and-carry-on
contract `App::new` already uses for `AchievementDb`. Load it when the
session opens, into `ArenaSession::catalog`; it dies with the session. Both
dbs skip a malformed file with a warning rather than panicking, so a
missing directory reads as an empty picker, not a crash.

Item rows are filtered by what the target can hold: `EquipItem` offers only
equippable items. There is no `Game` to call `is_equippable` on, so read
`ItemDef::equipment.is_some()` off the db directly.

- [ ] **Step 1: Write the failing tests.**
  - `picking_a_species_appends_an_opponent_row`
  - `picking_into_an_existing_row_replaces_its_id_and_keeps_its_count` —
    the count is the tuning dial; losing it on an id change is the bug.
  - `the_equip_picker_offers_only_equippable_items`
  - `esc_from_the_picker_returns_to_the_builder_adding_nothing`
  - `the_picker_lists_species_without_a_running_game` — the whole reason
    the catalog exists; build the `App` with no `Game` at all.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Add the catalog to `ArenaSession::new` and implement the picker.**
- [ ] **Step 4: Gate.** `cargo test -p feral-processes-app-core`, `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 5: Commit.** `feat(arena): pick species and items from the asset catalogue`

---

## Task 8: loading and saving `dev-arenas/*.ron`

**Files:** `app/arena.rs`, `tests/arena.rs`

`Mode::ArenaLoad` lists `dev-arenas/*.ron` and loads the highlighted one
through `Scenario::load`, which already exists and already reports a
malformed file as an error naming it — put that on the status line and stay
on the picker. `Mode::ArenaSave` takes a filename through the text-input
idiom `Mode::FuseName` uses (read `handle_fuse_name_key` and copy its shape,
including its filename sanitising if it has any) and writes RON beside the
shipped scenarios.

The `dev-arenas/` directory is not a path `App` currently knows. Add it as
a constructor parameter beside `saves_dir` and `history_path` rather than
deriving it — `App` takes its paths from the launcher and resolves none
itself, and that is what keeps app-core testable against a temp dir.
`App::new` grows a parameter; `main.rs` passes `repo_root().join("dev-arenas")`.

- [ ] **Step 1: Write the failing tests.**
  - `a_shipped_scenario_round_trips_through_the_builder` — load
    `opening-fight.ron`, save it under a new name in a temp dir, and the
    two parse to equal `Scenario`s. This is what makes the two tools one
    library rather than two.
  - `a_loaded_template_scenario_keeps_saying_template` — load
    `geared-vs-boss.ron`, save it, and the written file still names
    `Template`, not the resolved save path. The trap Task 4's clone avoids.
  - `a_malformed_scenario_stays_on_the_picker_with_the_reason` — write
    garbage RON to a temp dir and assert the mode did not change and the
    status line names the file.
  - `saving_over_an_existing_name_overwrites_it` — overwriting is the
    behaviour, following `dev_template::generate`, which overwrites
    deliberately and says why: a fixture exists so the same thing comes
    back, and a save that preserved the last version defeats that. Say so
    in the doc comment.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core arena`
- [ ] **Step 3: Add the `arenas_dir` parameter, threading it through `main.rs` and every `App::new` call site in the tests.**
- [ ] **Step 4: Implement the load and save screens.**
- [ ] **Step 5: Gate.** `cargo test --workspace` — the constructor change touches every app-core test fixture, so this is the first full-suite run. `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 6: Commit.** `feat(arena): load and save scenarios from dev-arenas/`

---

## Task 9: drawing it

**Files:**
- Create: `crates/gui/src/render/arena.rs`
- Modify: `crates/gui/src/render/mod.rs`, `crates/gui/src/render/meta.rs`

Four entry points: `draw_arena_builder`, `draw_arena_pick`,
`draw_arena_load`, `draw_arena_result`. `ArenaSave` draws the builder with
the text-entry popup over it, the way `FuseName` does — read
`render/party.rs`'s fuse-name screen and follow it.

The builder draws `app.arena_builder_rows()` and nothing else, and the
picker draws `app.arena_pick_rows()`. Neither may rebuild the list, derive
labels, or filter rows — the builder's rows are hidden dynamically, and a
second opinion about them opens a different row from the one under the
highlight.

The result screen shows won/lost, rounds, HP fraction, companions down, the
seed, the staging warnings, and the scrollable transcript — the warnings
especially, since nothing is ever capped and the only thing making that
honest is that the ask is shown.

`render/meta.rs::main_menu` gains the `[R] Arena` row when
`app.arena_enabled()`, in the same conditional shape Load Game already uses.

Everything draws through `Painter`. Nothing in `render/` may name a graphics
library.

- [ ] **Step 1: Write the failing tests** in the `render/mod.rs` test module (or `render/arena.rs`'s own, matching whatever the neighbouring render modules do — check `render/base.rs` first).
  - `the_arena_screens_draw` — each of the five modes renders without
    panicking through the existing headless painter fixture.
  - `the_main_menu_shows_arena_only_when_enabled`
  - `a_builder_row_fits_its_popup` — measure the widest row against the
    popup width with `paint::with_painter`. Row width **is** testable
    headlessly; `draw_row` clamps vertically but never horizontally, so an
    over-long row runs off the panel silently.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-gui arena`
- [ ] **Step 3: Write `render/arena.rs` and the dispatch arms.**
- [ ] **Step 4: Gate.** `cargo test -p feral-processes-gui`, `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 5: Play it.** `FERAL_DEV_ARENA=1 cargo run`, then: open the arena, load `opening-fight.ron`, fight it, lose or win, reseed, refight, go back, add an opponent, add a companion, equip something, fight again, save it, and run the saved file through `cargo run --bin arena -- dev-arenas/<name>.ron`. **A green suite is not evidence of play** — do this before calling the task done, and report what you actually saw.
- [ ] **Step 6: Commit.** `feat(arena): draw the builder, pickers and result screen`

---

## Task 10: docs and release

**Files:** `dev-arenas/README.md`, `CHANGELOG.md`, `Cargo.toml`,
`CLAUDE.md` (and `cp CLAUDE.md AGENTS.md` — they are gitignored twins with
nothing tracking their drift).

- [ ] **Step 1: `dev-arenas/README.md`** — the interactive half. How to
      reach it (`FERAL_DEV_ARENA=1`), what it does that the bin does not
      (Specials fire, because a person is pressing the keys), and the loop
      between the two: build by feel, save, run for a win rate, pin a loss
      seed, watch it. The existing "What it does not measure" section is
      about the bin specifically — say so there now that it is not the only
      way in.
- [ ] **Step 2: `CLAUDE.md` load-bearing seams** — one entry for the arena
      session's three omissions, in the voice of the entries around it: what
      is absent, what breaks if someone adds it back, and which test holds
      it. This is exactly the kind of fact that reads as missing code.
- [ ] **Step 3: `CHANGELOG.md`** — a new `## 0.5.0` section. Read the
      preamble for which digit moves rather than trusting this line; a new
      feature with no save break is a minor bump.
- [ ] **Step 4: Bump the workspace version** in the root `Cargo.toml` to
      match. The bump happens once, at the merge — not on the branch as it
      goes.
- [ ] **Step 5: Grep for claims this falsifies.** `rg -n "arena"` across
      `docs/`, `CHANGELOG.md` and the `assets/*/README.md` files. Anything
      saying the arena is headless-only is now wrong.
- [ ] **Step 6: Final gate.** `cargo test --workspace` (1545 tests plus what
      this added), `cargo clippy --workspace`, `cargo fmt --check`.
- [ ] **Step 7: Commit.** `docs(arena): the interactive half, and 0.5.0`

---

## Notes for the implementer

- **Ask before working in a worktree.** The user plays from the primary
  checkout, and a worktree commit is invisible there.
- **Push is a separate ask.** Commits are free; `git push` and the tag push
  are not, and `git push` alone does not send annotated tags.
- **Injected LSP diagnostics can be stale.** Verify with a real `cargo
  check` before acting on one.
- If a task's premise turns out to be wrong — and Task 4's claim that
  `settle_after_round` is a no-op when the battle is still live is the most
  likely candidate — stop and say so rather than working around it. The plan
  is a hypothesis about the code, not a description of it.
