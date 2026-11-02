# Work skills: plan

**Spec:** `docs/superpowers/specs/2026-11-02-work-skills-design.md`. Read it
first; it decides the behaviour. **ADRs:** 0038 (the skill core) and 0039
(species starting levels). This plan only says where the work goes.

**Worktree:** `.claude/worktrees/work-skills`, on branch
`worktree-work-skills`.

**Rules for every phase:**

- TDD: write the failing test first.
- Commit at each green step.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets` before each
  commit.
- Never push.
- Never `git checkout`, `git stash` or `git add -A`. Stage explicit paths.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Before Phase 1, run the `design-patterns` and `module-boundaries` skills on
  the `skills/` core and the per-site hook. If either changes a shape named
  here, update this plan before writing code.
- Load these seam rules before touching their areas:
  - `.claude/rules/seams-base.md` and the `seams` skill's labour traps
    (Phase 2)
  - `seams-screens.md` (saves, Phase 1)
  - `content-schema.md` (species, Phase 1)

## Phase 1: Skill core, seeding, species, save (`crates/engine`)

### Files

- **New `src/skills/mod.rs`** (the subsystem directory, 0024). Register it in
  `lib.rs`. It holds `Skill`, `SkillTrack`, `Skills`, `PractiseOutcome`,
  `practise` and `practice_to_next`, with tests in the same directory
  (`src/skills/tests.rs`). It must not name any structure, task, duty or
  `Game`.
- **New `src/tuning/skills.rs`.** Re-export it from `tuning/mod.rs`; don't
  grow `mod.rs` any other way. It holds:
  - `SKILL_LEVEL_CAP = 20`
  - the curve constants
  - `SKILL_SCALE_FLOOR = 0.5` and `SKILL_SCALE_PER_LEVEL = 0.05`
  - a doc comment stating the pace intent and the assumed share of ticks a
    dedicated worker spends working (spec, "Pace target")
  - Check `docs/measurements/` for a zone's length first.
- **`src/species.rs`.** `SpeciesDef.starting_skills: Vec<(Skill, u32)>` under
  `#[serde(default)]`.
  - The loader clamps a level above the cap and warns, following the
    `load_dir` pattern.
  - Add `SpeciesDef::seed_skills(&self) -> Skills`.
- **`src/game/spawning.rs`.** `roster_parts` adds `Skills`, seeded from the
  species. Check that every door listed in the spec reaches it.
- **Release and death.** Find where the other `roster_parts` components are
  removed, and treat `Skills` the same way.
- **`src/save/mod.rs`.**
  - `CreatureSave.skills: Vec<SkillSave>` under `#[serde(default)]`.
  - `SkillSave { skill: String, level: u32, practice: u32 }`.
  - `Skill::name` and `Skill::from_name` follow `Duty`'s, and an unknown name
    is dropped.
  - Don't bump `SAVE_FORMAT_VERSION`.
- **`src/game/lifecycle/save.rs`** writes the field.
- **`src/game/lifecycle/spawn_saved.rs`** restores it. A `Tamed` creature
  with an empty list is seeded from its species.
- **`assets/species/*.ron` (19 files).** Each gets one or two
  `starting_skills` entries at 2 to 5, in the work its existing profile
  points at (`WorkProfile`, `work_resource`).
- **`assets/species/README.md`.** Document the field and give a table of
  every species' values.

### Interfaces

```rust
pub enum Skill { Extract, Assemble, Research, Build, Dig, Farm }   // Copy, Ord, ALL, name/from_name/label
pub struct SkillTrack { pub level: u32, pub practice: u32 }
#[derive(Component)] pub struct Skills { /* one SkillTrack per Skill */ }
impl Skills { pub fn level(&self, s: Skill) -> u32; pub fn track(&self, s: Skill) -> SkillTrack; pub fn track_mut(&mut self, s: Skill) -> &mut SkillTrack }
pub struct PractiseOutcome { pub levels_gained: u32, pub new_level: u32 }
pub fn practice_to_next(level: u32) -> Option<u32>          // None at the cap
pub fn practise(track: &mut SkillTrack, amount: u32) -> PractiseOutcome
```

If the design-patterns skill prefers `practise` to return a new track rather
than mutate one in place, either is fine. It must stay pure, with no `World`
and no RNG.

### Tests (intent)

- Rollover, several levels from one large amount, stopping at the cap, no
  practice past the cap, and a curve that strictly increases.
- `practice_to_next(SKILL_LEVEL_CAP)` is `None`.
- A fresh tame (through the real tame path) has `Skills` equal to its
  species' seed. Breeding gives species levels with nothing inherited. Wild
  and settlement programs and the player have no `Skills`.
- Save then load through the real path keeps level and practice for each
  skill. This can't be only a RON round trip; see memory
  `ron-round-trip-cannot-catch-a-skipped-field`.
- A save whose creatures have no `skills` field loads with species seeds.
- An unknown skill name in a save is dropped, and the load still succeeds.
- Species census: every shipped `starting_skills` level is at most the cap.

### Gate

`cargo test -p feral-processes-engine skills`, the save tests, and the
species census.

## Phase 2: Work sites (`crates/engine`)

### Files

- **New `src/game/base/skill_scale.rs`** (or wherever module-boundaries puts
  it). It holds:
  - `work_skill_for(...) -> Option<Skill>`: an exhaustive match over task
    kind and machine production. Every `None` arm is written out, with no
    wildcard.
  - `skill_scale(level: u32) -> f32`, built from the tuning constants.
  - `fn scaled_ticks(base: u32, scale: f32) -> u32`, which rounds
    deterministically and never returns below 1.
  - An entity without `Skills` reads 1.0.
- **Machines.** `Game::work_ticks_for` and `cycle_ticks_for` in
  `game/base/building.rs`, and `systems::work_ticks_at_speed` and
  `SiteScales` in `systems.rs`.
  - Add the worker's skill scale as one more factor. A `SiteScales` field is
    the obvious place, but not if it would muddy what "site" means.
  - Re-bake `Task::required` at each cycle boundary, in:
    - `task_progress_system` (Extract and Research)
    - `assembler_system` (Assemble)
  - Settle the skill at the cycle's start from the recipe it runs (spec,
    "Which skill a machine cycle practises").
- **Research.** Classify a Research Station by a single rule, either
  `studies` or the research payout path. Pick one and say why in a comment.
- **Build.** `raise_one_tick` and `run_build_crew` in
  `game/base/construction.rs`, plus `BuildSite`.
  - Add a per-worker fractional carry or a finer meter.
  - If the carry needs saving, it is a new `#[serde(default)]` field, and
    losing it on load must cost at most one step.
  - Two builders together must still add up correctly.
- **Dig.** `post_digger` and the swing reset in `run_dig_crew`
  (`base_space.rs`). Divide `BASE_DIG_TICKS_PER_SWING` by the scale.
- **Farm.** `post_tender` and the task reset in `run_farm_crew` (`farm.rs`).
  Divide the farm ticks by the scale.
- **Practice.** At each progress site, a tick that advanced the worker's
  progress calls `skills::practise(…, 1)` for that worker's skill.
  - A level gained is returned to the system or shell, which logs it (Phase
    3 writes the message; here, collect the outcomes).
  - Any loop that draws RNG stays sorted by `ProgramId`.
- **Existing tests that count labour ticks.** Fix each one that moves:
  - When the test is about something other than speed, pin the worker at
    level 10 with a test helper such as `set_skill_level`, which belongs in
    the tests' helper module.
  - When the test is about speed, update the number.
  - Never loosen an assertion. List each changed test in the commit message.

### Tests (intent)

- For each of the six skills, a high-level worker finishes measurably sooner
  than a level-0 one. It practises that skill and no other, through the real
  posting.
- A level gained mid-posting changes the rate from the next cycle, swing or
  task.
- A worker without `Skills` runs at exactly today's rate: pin the tick count.
- The player's own work rate is unchanged.
- The player's class scale still multiplies with the skill scale.
- Build: a level-0 builder takes about twice as long as a level-10 one, and
  two builders sum correctly.
- At the cap, practice stops accruing.
- Prefer a `dev-saves/` template (`chains`, `stack`, and so on) to a fresh
  `Game::new`.

### Gate

`cargo test --workspace` green. Then run `cargo test -p
feral-processes-engine balance_sim`: it must not move, and if it does,
something leaked into battle.

## Phase 3: Views and message (`crates/engine`)

### Files

- **`views/work.rs` and `game/base/duties.rs` (`work_table`).** `WorkRow`
  gains `skills: Vec<(Skill, u32)>` (or a small view struct). It is empty
  for a row without `Skills`.
- **`views/manifest.rs` and `game/inspection.rs` (`program_manifest`).**
  `ProgramManifest.skills: Option<Vec<SkillView>>`, which is `None` for an
  untamed program.
  - `SkillView { skill_label, level, practice, practice_to_next:
    Option<u32> }`.
- **Level-up message.** Post it from the outcomes Phase 2 collects, using
  `push_base_kind(MessageKind::LevelUp, …)` inside systems or `log_base_kind`
  from `Game`.
  - The wording is one line in the shape "Kernel Moth is now a level 4
    builder.", with a label per skill.
  - Don't use tick vocabulary.
  - Check how `message_history` condenses repeats of it.

### Tests (intent)

- The view carries the levels. The manifest has no skills for a wild
  program, and does have them for a tamed one.
- One message per level gained, and only one when a single large practice
  gain covers several levels, if the design-patterns pass chose that. Decide
  this and pin it.

### Gate

The engine tests for the views and messages.

## Phase 4: App-core and GUI

### Files

- **`crates/app-core/src/app/building.rs`** (`base_staff_table`): pass the
  skills through if the table rows are re-shaped there. No new keys.
- **`crates/gui/src/render/building/staffing.rs`**
  (`base_staff_menu_rows`, `work_table_header`): show the skill levels per
  row.
  - Choose a layout that obeys "GUI text never clips or wraps" (memory
    `gui-text-never-clips-or-wraps`).
  - The six skills don't map onto the four duty columns.
- **`crates/gui/src/render/manifest/sections.rs`** (`program_sections`): a
  SKILLS section with level and progress to next, drawn only when `skills`
  is `Some`.
- **The drawing seam:** draw only through `Painter`.

### Tests

A headless layout or width test where the existing panel has one (memory
`popup-row-width-is-testable-headlessly`).

### Gate

`cargo test --workspace` green. Then take screenshots and Read both PNGs:

```sh
cargo run -- --template chains --keys "…" --screenshot staff.png
cargo run -- --template chains --keys "…" --screenshot manifest.png
```

## Finish

- **The new seam:** "every work site reads the skill scale through
  `work_skill_for`". Do the three writes in the order the `seams` skill
  documents: the graph `seam:`, the skill reference, then
  `.claude/rules/seams-base.md`.
- Mark ADRs 0038 and 0039 as Accepted at landing, along with the README
  index.
- Add a `CHANGELOG.md` entry at landing. It's save-compatible, so the bump
  is not breaking.
- Final whole-branch review with opus. Hand it the diff as a file.
- Land through the `deploy` skill.
