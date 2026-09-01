# Character Creation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A seven-step creation wizard that decides who the player is —
difficulty, class, look, stat points, starter routine and name — before a
run begins.

**Architecture:** One `Mode::CreateCharacter` in app-core holds a
`CharacterChoice` and a `CreationStep` cursor. The engine takes the whole
choice through a new `Game::new_with`; `Game::new` keeps its signature and
delegates with a default that is today's player exactly, so ~1,600 test
call sites are untouched. A class grants affinities only, landing on the
player arm `Game::ability_affinity` already has.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `bevy_egui` renderer.

**Spec:** `docs/superpowers/specs/2026-09-01-character-creation-design.md`
— read it before starting any task. It carries the *why* for every
decision below and the traps each one closes.

## Global Constraints

Copied from `CLAUDE.md` and the spec. Every task's requirements include
these.

- **`crates/engine`'s `Game` is the entire public API the renderer talks
  to via app-core.** Never add a `world` accessor.
- **Never hardcode content in Rust when it can be data.** New schema
  fields are `#[serde(default)]`; a malformed `.ron` is skipped with a
  logged warning, never a panic; update the matching
  `assets/*/README.md` in the same change.
- **`SAVE_FORMAT_VERSION` must not be bumped.** Every new `PlayerSave`
  field is additive behind `#[serde(default)]`.
- **`Game::new`'s signature must not change.** 1,633 call sites.
- **New tuning values go in `crates/engine/src/tuning.rs`** as documented
  `pub const`, never inline in a formula.
- **Run `cargo fmt` and `cargo clippy --workspace` after every change.**
  Fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate** (3842 tests today), not
  just the tests you wrote.
- **Comments explain *why*.** A comment restating what well-named code
  already says is noise.
- **Do not push.** Commit freely; the branch lands separately.
- Work on branch `feat/character-creation`.

**On code in this plan:** per `CLAUDE.md`'s process-weight rule, tasks
give the file list, the interface to produce, the intent of each test and
the gates to run — not finished code. Code blocks appear only where
something is genuinely non-obvious. Read the surrounding source and follow
its idiom.

---

## Phase map

Phases are sequential. Tracks **within** a phase are independent and own
disjoint files — they can run as parallel agents in separate worktrees.

```
Phase 1  Engine foundation           1 agent, 2 tasks   (everything depends on this)
Phase 2  Track A: classes            3 agents in parallel
         Track B: starter routines
         Track C: palette + map draw
Phase 3  app-core wizard             1 agent, 2 tasks
Phase 4  Track D: gui screens        2 agents in parallel
         Track E: profile preview
Phase 5  Finish and verify           1 agent
```

**The one shared file across parallel tracks is
`crates/engine/src/tests/assets.rs`** — Tracks A and B each add a census
to it. Different functions, so a normal merge resolves it; do not
restructure that file.

---

## Phase 1 — Engine foundation

Sequential. Produces every name the later phases consume.

### Task 1: `CharacterChoice` and `Game::new_with`

**Files:**
- Create: `crates/engine/src/game/creation.rs`
- Modify: `crates/engine/src/game/lifecycle.rs` (`Game::new`, the player
  `world.spawn`), `crates/engine/src/lib.rs` (module declaration and
  re-export), `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/creation.rs` (new; register it in the
  test module list)

**Interfaces — Produces:**

```rust
pub struct CharacterChoice {
    pub name: String,
    pub class: Option<AffinityClass>,
    pub glyph: char,
    pub sprite: String,
    pub colour: u8,
    /// Units bought per axis, indexed as `MainStat::all()`; `cost()` prices them.
    pub stats: [u32; 4],
    pub routine: Option<AbilityId>,
}

impl Default for CharacterChoice { /* today's player exactly */ }

impl CharacterChoice {
    /// Points this spend costs, priced per axis. `None` if it exceeds the pool.
    pub fn cost(&self) -> Option<u32>;
}

impl Game {
    pub fn new_with(
        seed: u32,
        difficulty: DifficultyMode,
        assets_dir: &Path,
        choice: &CharacterChoice,
    ) -> std::io::Result<Self>;
}
```

New `tuning.rs` constants, in the "Player baseline & progression" section
beside `PLAYER_BASE_STATS`, each with a doc comment saying what it is and
why that value: `CREATION_STAT_POINTS`, `CREATION_COST_INTEGRITY`,
`CREATION_COST_ATK`, `CREATION_COST_DECOMPILER`, `CREATION_COST_DEF`,
`CREATION_GAIN_INTEGRITY`, `MAX_CREATION_STAT_POINTS`.

Rates from the spec: Integrity 1 point → +6 `max_hp`; Atk 1 → +1;
Decompiler 1 → +1; Def **3** → +1. Def is a percentage point on a base of
2 that levelling never raises, which is why it is not 1.

- [ ] **Step 1: Write the failing tests**

Four tests in `tests/creation.rs`:

1. `new_and_new_with_default_produce_the_same_player` — construct both at
   the same seed and difficulty, assert `Stats`, `Glyph`, `Inventory` and
   `Routines` are equal. This is the test that protects 1,600 call sites.
2. `creation_points_are_additive_over_the_baseline` — a choice spending
   everything on Integrity gives `PLAYER_BASE_STATS.max_hp +
   points * CREATION_GAIN_INTEGRITY`, and `hp == max_hp` (a run must not
   start damaged — `MainStat::Integrity`'s own trap).
3. `mitigation_costs_more_than_a_point` — spending the whole pool on Def
   raises mitigation by `pool / CREATION_COST_DEF`, not by `pool`.
4. `an_overspent_choice_is_refused` — `cost()` returns `None` above the
   pool, and `new_with` falls back to the default spend rather than
   applying it. Fail closed: a malformed choice must not hand out stats.

Plus a `const` assertion that `CREATION_STAT_POINTS <=
MAX_CREATION_STAT_POINTS`, mirroring `MAX_PROFILE_STAT_POINTS`' reason.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-engine creation`

- [ ] **Step 3: Implement**

`game/creation.rs` holds `CharacterChoice`, `cost()`, and
`Game::apply_character_choice`, which calls four private steps in this
order: stats, look, kit, routine. **Stats and look are complete in this
task. Kit and routine are one-line delegations** to functions Phase 2
owns:

```rust
// Phase 2A owns this; here it is the fallback that keeps today's kit.
crate::classes::apply_kit(self, choice.class);
// Phase 2B owns this; here it is a no-op.
crate::abilities::install_starter(self, choice.routine.as_ref());
```

Create both functions now, in their own modules, with the
today's-behaviour body. **This is what makes Phase 2's tracks
file-disjoint** — neither track has to edit `creation.rs` or
`lifecycle.rs`.

In `lifecycle.rs`, extract the player `world.spawn` so `new_with` can
apply the choice after it. `Game::new` becomes a one-line delegation.

- [ ] **Step 4: Run the tests, then the engine suite**

`cargo test -p feral-processes-engine` — the whole crate, because Task 1
touches the constructor every other test calls.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

### Task 2: Player identity — component, view, save

**Files:**
- Modify: `crates/engine/src/components.rs`,
  `crates/engine/src/views.rs` (`EntityView`, around line 699),
  `crates/engine/src/save.rs` (`PlayerSave`),
  `crates/engine/src/game/creation.rs`
- Test: `crates/engine/src/tests/creation.rs`

**Interfaces — Consumes:** `CharacterChoice` (Task 1).
**Produces:**

```rust
// components.rs — the player's chosen look and class.
pub struct PlayerIdentity {
    pub class: Option<AffinityClass>,
    pub sprite: String,
    pub colour: u8,
}

// views.rs — carried on EntityView, `Some` for the player alone.
pub struct PlayerLook { pub sprite: String, pub colour: u8 }
// EntityView gains: pub look: Option<PlayerLook>,
```

The name reuses `components::CustomName` (`components.rs:85`) — do not
add a second name type. The chosen character goes in the player's
existing `Glyph.ch`; the colour cannot go in `Glyph.color`, because
`GlyphColor` is the eleven-hue *content* palette and the player's choices
are deliberately outside it.

`PlayerSave` gains `name`, `class`, `glyph`, `sprite`, `colour` — every
one `#[serde(default)]`.

- [ ] **Step 1: Write the failing tests**

1. `a_created_player_round_trips_through_a_real_save` — write to a
   `tempfile` path with `Game::save`, load with `Game::load`, assert all
   five survive. **Through a file, not a RON round trip**: a round trip
   cannot catch a field skipped on write.
2. `loading_does_not_re_apply_the_choice` — save a created player, load,
   and assert `Stats` and `Inventory` are unchanged. The points and kit
   are receipts, the shape a `Stat` talent already has.
3. `an_old_save_without_the_fields_still_loads` — a `PlayerSave` RON
   text with the five fields absent parses and yields today's player.
4. `the_player_view_carries_its_look_and_nothing_else_does` — the
   player's `EntityView.look` is `Some`, every other entity's is `None`.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement**
- [ ] **Step 4: `cargo test -p feral-processes-engine`**
- [ ] **Step 5: fmt, clippy, commit**

---

## Phase 2 — Three parallel tracks

All three depend only on Phase 1 and own disjoint files. Dispatch as three
agents.

### Task 3: Classes (Track A)

**Files:**
- Create: `crates/engine/src/classes.rs`, `assets/classes/striker.ron`,
  `bastion.ron`, `medic.ron`, `saboteur.ron`, `leech.ron`,
  `assets/classes/README.md`
- Modify: `crates/engine/src/lib.rs` (module),
  `crates/engine/src/game/lifecycle.rs` (`AssetDbs`, `load_asset_dbs`,
  resource insert — **this is the only lifecycle.rs edit in Phase 2**),
  `crates/engine/src/game/combat.rs:930` (`ability_affinity`),
  `crates/engine/src/tests/assets.rs`
- Test: `crates/engine/src/tests/classes.rs` (new)

**Interfaces — Consumes:** `CharacterChoice::class`,
`classes::apply_kit` (the stub Task 1 created — Track A fills its body).
**Produces:**

```rust
pub struct ClassDef {
    pub class: AffinityClass,
    pub name: String,
    pub description: String,
    #[serde(default)] pub affinities: Affinities,
    #[serde(default)] pub kit: Vec<(ItemId, u32)>,
}

pub struct ClassDb { /* .. */ }
impl ClassDb {
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>;
    pub fn get(&self, class: AffinityClass) -> Option<&ClassDef>;
    /// Sorted by class, because every caller walks it.
    pub fn iter(&self) -> impl Iterator<Item = &ClassDef>;
}

pub fn apply_kit(game: &mut Game, class: Option<AffinityClass>);

impl Game {
    /// The player's spread for `kind`, resolved live through `ClassDb`.
    /// `AFFINITY_NEUTRAL` for no class or an unresolvable one.
    pub(crate) fn player_class_affinity(&self, kind: AffinityKind) -> f32;
    /// One row per loaded class, for the creation screen.
    pub fn class_rows(&self) -> Vec<views::ClassRow>;
}

// views.rs — defined by THIS task.
pub struct ClassRow {
    pub class: AffinityClass,
    pub name: String,
    pub description: String,
    /// Pre-formatted "+Heal  -Damage" style summary of the spread, built
    /// in the engine so the two renderers cannot drift.
    pub axes: String,
    /// Pre-formatted kit summary, same reason.
    pub kit: String,
}
```

`ClassDb::load_dir` follows `PerkDb::load_dir` (`perks.rs:413`) exactly:
skip a malformed file with a returned warning, never panic.

`ability_affinity`'s player arm becomes
`self.player_class_affinity(kind) + perks::affinity_bonus(..)`, clamped by
the `.min(AFFINITY_MAX)` already there. **Additive, not multiplicative**,
matching the shape the arm already has.

Authored spreads must give each class one axis **below** neutral, matching
`ClassShape`'s `damps` (`species.rs:449`): Striker damps Heal, Saboteur
damps Heal, Medic damps Damage, Bastion damps Damage. A class is a trade,
not a bonus.

- [ ] **Step 1: Write the failing tests**

In `tests/classes.rs`:
1. `a_class_moves_its_own_axis_and_nothing_else` — a Medic's `Heal`
   affinity is above neutral and its `Buff`, `Debuff`, `Drain` are
   exactly neutral.
2. `a_class_and_a_perk_stack_and_stay_clamped` — a maxed affinity perk
   plus a class does not exceed `AFFINITY_MAX`.
3. `the_class_kit_replaces_the_default_kit` — a created Medic's
   `Inventory` is the Medic file's kit, and a choice with no class gets
   the four hardcoded items.
4. `an_empty_class_directory_plays_as_todays_game` — point `load_dir` at
   an empty temp dir; every axis resolves neutral and the default kit
   applies. **The supported-install property, held at both ends.**
5. `a_malformed_class_file_is_skipped_with_a_warning` — not a panic.
6. `a_retuned_class_file_reaches_a_loaded_save` — the spread is resolved
   live, not stored. Save a Medic, reload against a db with a different
   Medic spread, assert the new one applies.

In `tests/assets.rs`, two censuses over the **real** assets:
7. `every_class_has_a_file` — exhaustive over `AffinityClass::ALL`, so a
   sixth class fails the build rather than shipping unpickable.
   `cell_mark`'s rule.
8. `every_class_kit_item_exists` — every kit id resolves in `ItemDb`.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement, and write `assets/classes/README.md`** — the
      schema reference a modder reads: every field, which are
      `#[serde(default)]`, the damped-axis convention, and that an empty
      directory is supported.
- [ ] **Step 4: `cargo test -p feral-processes-engine`**
- [ ] **Step 5: fmt, clippy, commit**

### Task 4: Starter routines (Track B)

**Files:**
- Modify: `crates/engine/src/abilities.rs` (`AbilityDef`, and
  `install_starter`'s stub body), `crates/engine/src/tests/assets.rs`,
  `assets/abilities/README.md`, and `starter: true` on the chosen
  ability files
- Test: `crates/engine/src/tests/routines.rs`

**Interfaces — Consumes:** `CharacterChoice::routine`,
`abilities::install_starter` (Task 1's stub).
**Produces:**

```rust
// AbilityDef gains:
/// Offered as a creation starter. Opt-in, like `exclusive` and
/// `wild_weight`, so the pool is defined by the files that ask to be in it.
#[serde(default)] pub starter: bool,

pub fn install_starter(game: &mut Game, id: Option<&AbilityId>);

impl Game {
    /// The starter pool, each row priced through `class`'s affinity.
    pub fn starter_routine_rows(&self, class: Option<AffinityClass>)
        -> Vec<views::StarterRoutineRow>;
}

// views.rs — defined by THIS task.
pub struct StarterRoutineRow {
    pub id: AbilityId,
    pub name: String,
    pub description: String,
    /// What it does *for this class* — the magnitude with the class
    /// affinity already applied. This is the field the step exists for.
    pub effect: String,
    pub power_cost: f32,
}
```

`install_starter` grants **knowledge and the install**: a `KnownRoutines`
entry plus the free slot. Knowledge, not just the slot, so the routine can
be etched onto a disk later like anything else the player knows.

The slot needs no new constant. `tuning::PLAYER_ROUTINE_SLOT_BASE` is 2
and its comment already says `decompile` takes one, leaving one free.

Flag these five, one per affinity axis — all single-target, non-exclusive
and cheap: `stack_smash` (Damage), `checksum_repair` (Heal),
`hyperthread` (Buff), `hard_lock` (Debuff), `siphon_cycles` (Drain).

- [ ] **Step 1: Write the failing tests**

In `tests/routines.rs`:
1. `a_starter_routine_is_known_and_installed` — it is in both
   `KnownRoutines` and the player's `Routines`.
2. `a_starter_routine_does_not_displace_decompile` — both are held, which
   is what the free slot is for.
3. `no_starter_choice_leaves_the_slot_empty` — today's game.
4. `starter_rows_are_priced_through_the_class` — the same routine reads
   differently for a Striker and a Medic. This is the row that teaches
   the affinity system, so it is the row that must be right.

In `tests/assets.rs`, three censuses over the **real** assets:
5. `every_starter_is_single_target` —
   `matches!(target, OneAlly | OneEnemyGroupFront)`.
6. `every_starter_is_not_exclusive` — an `exclusive` routine may never
   enter `KnownRoutines`, and creation must not become a fourth way
   around that gate.
7. `there_is_a_starter_for_every_affinity_axis` — and therefore at least
   one starter at all. **This census is the point:** a
   `#[serde(default)]` field authored nowhere ships documented and dead,
   which this repo has already shipped once (`spread`, used by 0 of 77
   ability files).

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement, and document `starter` in
      `assets/abilities/README.md`** — including that it is opt-in and
      that a starter must be single-target and non-exclusive.
- [ ] **Step 4: `cargo test -p feral-processes-engine`**
- [ ] **Step 5: fmt, clippy, commit**

### Task 5: Player palette and the map draw (Track C)

**Files:**
- Modify: `crates/gui/src/render/hud/palette.rs`,
  `crates/gui/src/render/base.rs` (around line 1249)
- Test: the existing test modules in both files

**Interfaces — Consumes:** `views::EntityView::look` (Task 2).
**Produces:** `pub(crate) const PLAYER_CHOICES: [Color; 6]`.

`palette::PLAYER` **stays** — it also means "an upgradeable item". The map
reads the chosen colour off `EntityView::look` instead, falling back to
`PLAYER` when `look` is `None`.

In `base.rs`, the sprite name comes from `look` rather than the hardcoded
`"player"`. The seam is unchanged: a sprite **substitutes** for the glyph,
and a name the table has nothing under returns `false`, so the caller
draws the glyph. Every option therefore works today on its glyph alone.

- [ ] **Step 1: Write the failing tests**

1. Extend `every_content_hue_is_separable_from_the_others`
   (`palette.rs:209`) to walk `PLAYER_CHOICES` alongside `GlyphColor::ALL`
   and `PLAYER`. **This is the whole of the rule** — there is no second
   place the player's separability is enforced, so a colour that collides
   with the red meaning "this fight will kill you" must fail here.
2. `the_player_sprite_comes_from_the_choice` — following
   `the_player_sprite_stands_in_for_the_at_sign` (`base.rs:2448`), which
   already asserts the mesh **and the absent `@`**. Assert both: painting
   the sprite over a glyph that is still there looks perfect against
   opaque art and breaks the moment one has transparency.
3. `a_missing_sprite_falls_back_to_the_chosen_glyph`.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement.** Six colours, each authored to clear 0.25
      from all eleven content hues and from each other — run the test to
      find them rather than guessing.
- [ ] **Step 4: `cargo test -p feral-processes-gui`**
- [ ] **Step 5: fmt, clippy, commit**

---

## Phase 3 — The wizard

Sequential, one agent. Consumes `class_rows`, `starter_routine_rows` and
`CharacterChoice` from Phases 1–2.

### Task 6: `Mode::CreateCharacter` and the step machine

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode`, `App` fields, the
  modifier-key fold, `needs_status_banner`),
  `crates/app-core/src/app/menus.rs` (creation handlers; **delete**
  `handle_difficulty_key`), `crates/app-core/src/app/input.rs`,
  `crates/app-core/src/app/lifecycle.rs` (`start_new_game`)
- Test: `crates/app-core/src/tests/creation.rs` (new)

**Interfaces — Produces:**

```rust
pub enum CreationStep {
    Difficulty, Class, Look, Points, Routine, Name, Summary,
}
impl CreationStep {
    /// Exhaustive, for `cell_mark`'s reason — a step added as a `_ =>` arm
    /// ships undrawable.
    pub const ALL: [CreationStep; 7];
}

// Mode gains CreateCharacter. Mode::DifficultyPick is DELETED.
impl App {
    pub fn creation_step(&self) -> CreationStep;
    pub fn creation_choice(&self) -> &CharacterChoice;
    pub fn creation_rows(&self) -> Vec<CreationRow>;
    /// Points still unspent on the Points step.
    pub fn creation_points_left(&self) -> u32;
}

/// One row of whichever step is showing — defined HERE, in app-core, not
/// in engine `views`. A read-only screen's row count is owned by app-core
/// and drawn by gui, so the per-step shape belongs on this side.
pub enum CreationRow {
    Difficulty { mode: DifficultyMode, label: String, detail: String },
    Class(views::ClassRow),
    Icon { glyph: char, sprite: String },
    Colour { index: u8 },
    /// One per `MainStat::all()` axis.
    Stat { stat: MainStat, spent: u32, value: i32, cost: u32 },
    Routine(views::StarterRoutineRow),
    Name { typed: String },
    /// A finished line on the Summary step.
    Summary { label: String, value: String },
}
```

`ALL_MODES` in `crates/gui/src/render/mod.rs:1162` **stays at
`[Mode; 86]`** — this adds one variant and deletes another.

Key handling per step: Up/Down moves the cursor through `App::scroll`;
Left/Right adjusts the highlighted row. On the Points step this is
`Mode::Transfer`'s idiom exactly, including `ShiftLeft`/`ShiftRight` as a
target and `CtrlLeft`/`CtrlRight` as a halving step — so
`Mode::CreateCharacter` joins the fold in `App::handle_key` that lists
which screens may see a modifier. The Name step is text entry, following
`Mode::FuseName`. Esc walks back one step; Esc on `Difficulty` returns to
the main menu. `[R]` on any step rolls every remaining choice and jumps to
`Summary`.

The roll spends **exactly** the pool, so it can never beat point-buy and
there is no reason to reroll for size — `[R]` rerolls for shape.

- [ ] **Step 1: Write the failing tests**

1. `the_wizard_walks_forward_and_back` — each step advances, Esc returns,
   Esc on the first leaves to the main menu.
2. `esc_from_the_first_step_does_not_start_a_run` — no `Game` is
   constructed.
3. `the_summary_step_commits_the_choice` — Enter on `Summary` produces a
   `Game` whose player matches every choice made.
4. `roll_everything_spends_exactly_the_pool` — `[R]` leaves
   `creation_points_left()` at 0 and jumps to `Summary`.
5. `points_cannot_be_overspent` — Right on a row with nothing left is
   refused, and the refusal goes through `App::refuse`, the one door.
6. `the_save_list_shows_the_players_name`.
7. `difficulty_is_chosen_in_the_wizard` — assert `Mode::DifficultyPick`
   is gone and the resulting `Game` has the chosen `DifficultyMode`.
8. `the_class_step_cannot_be_left_without_a_class` — there is no
   Unaligned option, so the step advances only once one is chosen. The
   engine's `CharacterChoice::default()` is still classless and neutral,
   deliberately: that is what the ~1,600 test call sites construct and
   what `balance_sim`'s modelled floor corresponds to. **The screen and
   the default disagree on purpose** — assert both halves here, or a
   later reader "fixes" one of them.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement**
- [ ] **Step 4: `cargo test -p feral-processes-app-core`**
- [ ] **Step 5: fmt, clippy, commit**

---

## Phase 4 — Two parallel tracks

Both depend on Phase 3. Track D draws whatever Track E provides, so
neither blocks the other: if E is unfinished, D draws an empty preview.

### Task 7: The creation screens (Track D)

**Files:**
- Create: `crates/gui/src/render/creation.rs`
- Modify: `crates/gui/src/render/mod.rs` (dispatch, module, `ALL_MODES`
  — replace `DifficultyPick` with `CreateCharacter`, count unchanged)
- Test: in `creation.rs`'s own test module

**Interfaces — Consumes:** `App::creation_step`, `creation_rows`,
`creation_points_left`, `App::profile_preview_rows` (Track E).

Seven step draws behind one entry point. The Look step shows a live
preview cell painted with the chosen glyph or sprite and colour.

- [ ] **Step 1: Write the failing tests**

1. `every_creation_step_draws_something` — walk `CreationStep::ALL` and
   assert each paints at least one row. Exhaustive, so a new step cannot
   ship blank.
2. `every_creation_step_paints_a_refusal_exactly_once` — extend the
   existing refusal census to walk `CreationStep::ALL`, **not just the
   mode**. Walking the mode alone asserts against one step and passes
   while six others cannot say why they refused something.
3. `the_tallest_creation_step_fits_its_screen` — at 1280x720. The wizard
   has **no scroll**, so height is a layout constraint and this is the
   only place it is checked. Verify by mutation: make a step one row
   taller and confirm the test fails.
4. `the_look_preview_draws_the_chosen_glyph_and_colour`.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement**
- [ ] **Step 4: `cargo test -p feral-processes-gui`**
- [ ] **Step 5: fmt, clippy, commit**

### Task 8: Profile preview (Track E)

**Files:**
- Modify: `crates/engine/src/achievements.rs` (extract the shared
  derivation), `crates/engine/src/game/lifecycle.rs`
  (`grant_profile_rewards` calls it), `crates/app-core/src/lib.rs`
  (`App` owns an `AchievementDb`), `crates/app-core/src/app/lifecycle.rs`
- Test: `crates/app-core/src/tests/creation.rs`

**Interfaces — Produces:**

```rust
// achievements.rs — the one derivation of what a profile pays.
pub fn profile_rewards(profile: &Profile, db: &AchievementDb) -> Vec<Reward>;

impl App { pub fn profile_preview_rows(&self) -> Vec<String>; }
```

`App` loads its own `AchievementDb` at startup, by the precedent that it
already owns `help_db`. The profile itself is already loaded before
`Game::new` in `start_new_game`, so nothing new is read from disk at
creation time.

**`grant_profile_rewards` must be refactored to call
`profile_rewards`, not keep a copy of the derivation.** A preview that
disagrees with what is actually paid is worse than no preview, and
`CLAUDE.md` records four occasions in this repo where a doc comment
promised a mirror while holding a copy that drifted. This is the one
non-negotiable part of the task.

- [ ] **Step 1: Write the failing tests**

1. `the_preview_matches_what_is_paid` — build a profile with all three
   reward kinds, take the preview, start the run, and assert the actual
   `Stats`/`Perks::points`/roster match it. **Delete the shared call and
   confirm this fails** — otherwise it is not testing the thing it
   exists for.
2. `an_empty_profile_previews_nothing`.

- [ ] **Step 2: Run and confirm failure**
- [ ] **Step 3: Implement**
- [ ] **Step 4: `cargo test -p feral-processes-engine -p
      feral-processes-app-core`**
- [ ] **Step 5: fmt, clippy, commit**

---

## Phase 5 — Finish and verify

Sequential, one agent, after everything merges.

### Task 9: Docs, seams and the full gate

**Files:**
- Create: `assets/help/<nn>-character-creation.md`
- Modify: `CHANGELOG.md`, `CLAUDE.md` and `AGENTS.md`,
  `docs/seams.md`, `.claude/skills/seams/`

**Do not touch** `README.md` or `docs/manual.md` — both are carved out of
the documentation obligation.

- [ ] **Step 1: Write the help page.** `assets/help/`'s filename is both
      the ordering and the id a `[label](topic-id)` link points at, so
      there is no front matter. Five block rules and no more — read
      `assets/help/README.md` first. Check whether an existing page makes
      a claim about starting a run that this falsifies.

- [ ] **Step 2: Amend the routine-install seam in all three places.**
      `CLAUDE.md` currently states *"Installing a routine is the one
      place a `KnownRoutines` entry meets an item, and the item is spent
      last."* Creation is now a door into `KnownRoutines` that spends no
      item. A new seam is three writes and the order matters: the
      argument to `docs/seams.md`, the trap to the `seams` skill, the
      one-sentence rule to `CLAUDE.md`. Note that `CLAUDE.md` and
      `AGENTS.md` are gitignored twins — edit `CLAUDE.md`, then `cp` it.

- [ ] **Step 3: Add the class seam** the same way: *"A player's class
      grants affinities and nothing else, and `ability_affinity`'s player
      arm is where it lands."*

- [ ] **Step 4: `CHANGELOG.md`.** A new `## X.Y.Z` section. Which digit
      moves is decided by the changelog's own preamble; "breaking" means
      a player's save stops loading, which this does not. **The version
      bump happens at the merge, not on the branch.**

- [ ] **Step 5: The full gate.**

```sh
cargo test --workspace          # 3842 tests today
cargo clippy --workspace
cargo fmt --check
cargo test -p feral-processes-engine balance_sim
```

`balance_sim` must be **unchanged**. If a curve moved, something reached
a stat or a formula it should not have — affinity does not enter
`expected_damage`, and the point pool is additive over an unchanged
`PLAYER_BASE_STATS`. A moved curve is a bug in this feature, not a
retune.

- [ ] **Step 6: Arena scenarios.** One per class in `dev-arenas/`, since
      the arena runs real abilities where `balance_sim` runs none — it is
      the only instrument that can see a class at all. Compare within one
      build only: a moved baseline across reports is a reshuffled RNG
      stream, not a difficulty change.

- [ ] **Step 7: Measure whether the stat rates are commensurate.** Run
      two more arena scenarios at an equal pool — everything into Atk
      against everything into Integrity — and report the gap. **Do not
      retune anything from the result; report it.** At level 1 the player
      is 90 HP and 6 atk, so the spec's +6 HP and +1 atk per point are
      +6.7% survivability against +16.7% damage, and Atk looks roughly
      2.5x the value of Integrity per point. That arithmetic is a reason
      to measure, not a licence to change the spec's table blind. Write
      the numbers to `docs/measurements/` following its README — this is
      exactly the case that directory exists for, and a number not
      written down there costs CPU-hours to get back.

- [ ] **Step 8: Commit.**

---

## What a green suite will not tell you

Whether the five classes feel distinct. Whether the pool is the right
size. Whether the colours read against the map's biome tint and vignette.
Whether seven steps is a wizard or a slog.

None of that is in the gate above. Play it:

```sh
cargo run
```

---

## Deliberately not in this plan

- **Seed entry** — cut from v1 during design.
- **Picking a starting program** — `grant_starting_program` already
  exists; held until the flow has been played, and it needs a rule for
  how it meets the profile's own `Reward::StartingProgram`.
- **A rival named at creation** — `NemesisDb` exists; its own feature.
- **A player talent tree** — breaks the perks/talents seam on purpose.
