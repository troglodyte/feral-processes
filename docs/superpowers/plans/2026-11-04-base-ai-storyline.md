# Base AI Storyline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After 5 to 10 projects are researched, the base staff program with the highest Analysis claims
the nearest Research Station, goes into a fugue, and fuses with it as the base AI. From then on it is the
only researcher, starting at 25% speed. It grows through practice, stalls at level thresholds, and is
levelled by a repeatable "Train your AI" project that consumes a pinned program. Some research is gated
by the AI's level.

**Architecture:** This follows ADR 0042.
- A new `crates/engine/src/storyline/` module holds a plain `Storyline { base_ai: BaseAi }` struct. It is
  owned by `Game` as its first plain field, next to `world` and `schedule`. Every beat is a pure decide
  function, gathered and applied by `Game::advance_storyline` in `tick_inner`.
- The fused program carries a `BaseAiHost` marker component, which gives it a new `ProgramRole::BaseAi`.
  Every "who is free" query excludes it through that role.
- **The AI's research is credited by the arc's shell, not by `task_progress_system`.** The host has no
  Task. Each tick the shell adds `pace` to a fractional accumulator and sends whole units into
  `ActiveResearch` (as a project credit or a study bank) or into the arc's own training progress. Other
  Research Stations stop producing because the labour scheduler never posts anyone to a `studies`
  structure once the arc has left Dormant.

**Tech Stack:** Rust, standalone `bevy_ecs` engine, app-core state machine, bevy + egui GUI.

**Spec:** `docs/superpowers/specs/2026-11-04-base-ai-storyline-design.md`. Read it with this plan; the
plan argues from it.

## Global Constraints

- Gather → decide → apply (ADR 0020). Decide functions take no `&World` or `&Game` and are tested
  directly.
- `Storyline` is plain Rust owned by `Game`, **not a `Resource`**, because a new resource shifts bevy's
  iteration order. The only ECS addition is the `BaseAiHost` marker.
- RNG: the arc draws nothing from `GameRng`. The trigger count is a pure hash of `SaveData.seed`, and AI
  output is deterministic accumulation with no success roll. Any loop over programs or stations sorts by
  `ProgramId` or the station's anchor tile before choosing.
- **Save-stable references:** the program is stored as a `ProgramId` and the station as its anchor tile
  `(i32, i32)`. Never store an `Entity` in the save.
- **Save:** `SaveData.storyline` is `#[serde(default)]` and loads as Dormant. There is no
  `SAVE_FORMAT_VERSION` bump.
- **Tuning:** every figure is in `crates/engine/src/tuning/storyline.rs`, re-exported with `mod
  storyline;` + `pub use storyline::*;` in `tuning/mod.rs`.
- **Player-facing text:**
  - Plain language, in the game's voice.
  - No tick vocabulary ("about a minute", never "120 ticks") and no occult naming.
  - Say "programs", never "monsters".
  - Feeding the AI is called **"training"** it.
- **Strings fixed by the spec:**
  - the notification category label `Storyline`
  - the research project `Train your AI`
  - the gated line `Seems feasible, but we don't know where to start`
- **Rules to read before the subsystems they cover:** `.claude/rules/seams-notifications.md`,
  `seams-research.md`, `seams-base.md`, `seams-hud.md`, `seams-saves.md`, `content-schema.md`. Invoke
  the `seams` skill before changing any of those seams.
- **Every commit gate:** `cargo fmt`, then `cargo clippy --workspace --all-targets` with no warnings,
  then the task's own tests.
- **Subagents** never push, never `git checkout`/`stash`/`reset` uncommitted work, and stage explicit
  paths only.

## Review Focus

1. **Save and load in each stage** (Claiming, Fugue with ticks remaining, Fused, Stalled) restores the
   same host, station, level and accumulator. The test is in Task 2, and Task 5 extends it.
2. **Removing the Home** (the demolish-all cascade in `remove_structure`) must leave the AI station
   standing. The test is in Task 4.
3. **A host chosen while it is in the party, downed, jailed, under study or on a sortie must never happen,**
   and a host that becomes unfree mid-walk sends the arc back to Dormant without re-alerting. The tests
   are in Task 1 (`pick_host`) and Task 2.
4. **Progress on the active project before the claim survives the fugue untouched,** and resumes at the
   AI's pace. The test is in Task 2.
5. **A subject pinned at a non-AI station at fusion is unpinned,** and can't be re-pinned there. The
   test is in Task 4.

---

### Task 1: The `storyline` module, a pure core

**Files:**
- Create: `crates/engine/src/storyline/mod.rs` (`Storyline`, re-exports)
- Create: `crates/engine/src/storyline/base_ai.rs` (stage types and decide functions)
- Create: `crates/engine/src/storyline/tests.rs`
- Create: `crates/engine/src/tuning/storyline.rs`
- Modify: `crates/engine/src/tuning/mod.rs`, `crates/engine/src/lib.rs` (`mod storyline;`)

**Interfaces (Produces):**
```rust
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Storyline { pub base_ai: BaseAi }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BaseAi {
    Dormant { announced: bool },                       // Default: announced = false
    Claiming { host: ProgramId, station: (i32, i32) },
    Fugue { host: ProgramId, station: (i32, i32), ticks_left: u32 },
    Fused(FusedAi),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusedAi {
    pub host: ProgramId, pub station: (i32, i32),
    pub level: u32,
    pub analysis_at_fusion: i32,   // the receipt: what the arc added = current - this
    pub pace_at_fusion: f64,       // research units/tick that host made on the ordinary formula
    pub practice: u32,             // toward the next Analysis point
    pub carry: f64,                // fractional research not yet credited
    pub training: TrainingTarget,  // Task 6
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrainingTarget { pub selected: bool, pub progress: u32 }

pub struct HostCandidate { pub id: ProgramId, pub analysis: i32, pub free: bool }
pub struct StationSpot { pub anchor: (i32, i32), pub distance: u32 }

pub fn trigger_count(seed: u32) -> u32;                       // in 5..=10
pub fn pick_host(c: &[HostCandidate]) -> Option<ProgramId>;   // max analysis among free; tie → lower id
pub fn pick_station(s: &[StationSpot]) -> Option<(i32, i32)>; // min distance; tie → lower anchor
pub fn analysis_cap(f: &FusedAi) -> i32;     // analysis_at_fusion + BASE_AI_LEVEL_OFFSETS[level]; i32::MAX past the table
pub fn practice_cost(points_gained: i32) -> u32;              // BASE + QUAD * gained^2, the skills shape
pub fn pace(f: &FusedAi, analysis: i32) -> f64;               // pace_at_fusion * BASE_AI_START_SPEED * analysis / analysis_at_fusion
pub struct PracticeOutcome { pub analysis: i32, pub practice: u32, pub stalled: bool }
pub fn practise(f: &FusedAi, analysis: i32, ticks: u32) -> PracticeOutcome;
pub fn training_cost(level: u32) -> u32;
```
The `Dormant → Claiming → Fugue → Fused` step is `pub fn step(arc: &BaseAi, input: &StepInput) ->
StepOutcome`.
- `StepInput`: `researched: u32`, `trigger: u32`, `candidates`, `stations`, `host_free: bool`,
  `host_arrived: bool`, `host_analysis: i32`, `host_pace: f64`.
- `StepOutcome`: `None`, `BeginClaim { host, station, announce: bool }`, `AbandonClaim`,
  `StartFugue`, `TickFugue`, `Fuse(FusedAi)`.

**Tuning (placeholders until Task 8):**
- `BASE_AI_FUGUE_TICKS = 120`
- `BASE_AI_START_SPEED = 0.25`
- `BASE_AI_PRACTICE_BASE = 50`
- `BASE_AI_PRACTICE_QUAD = 2`
- `BASE_AI_LEVEL_OFFSETS: [i32; 5] = [10, 25, 45, 70, 100]`
- `BASE_AI_TRAINING_COST_BASE = 20`
- `BASE_AI_TRAINING_COST_PER_LEVEL = 20`

- [ ] **Step 1: Write failing tests in `storyline/tests.rs`:**
  - `trigger_count_is_between_five_and_ten_for_every_seed`: sweep seeds `0..10_000` and assert `5..=10`.
  - `trigger_count_spreads_across_the_range`: across the sweep, all six values appear.
  - `pick_host_takes_the_highest_free_analysis_and_ties_go_to_the_lower_id`
  - `pick_host_skips_unfree_programs` and `pick_host_with_no_free_program_is_none`
  - `pick_station_takes_the_nearest_and_ties_go_to_the_lower_anchor`
  - `step_stays_dormant_below_the_trigger`
  - `step_begins_the_claim_at_the_trigger_even_with_no_research_selected`: `announce == true`.
  - `step_does_not_announce_a_second_claim`: from `Dormant { announced: true }`, `BeginClaim` has
    `announce: false`.
  - `step_waits_when_no_candidate_or_no_station`
  - `step_abandons_the_claim_when_the_host_stops_being_free`
  - `step_starts_the_fugue_on_arrival_and_fuses_after_120_ticks`: the `Fuse` outcome has `level 0`,
    `analysis_at_fusion == host_analysis` and `pace_at_fusion == host_pace`.
  - `pace_starts_at_a_quarter_and_rises_with_analysis_without_a_ceiling`:
    - `pace(f, a0) == 0.25 * p0`
    - `pace(f, 4 * a0) == p0`
    - `pace(f, 40 * a0) == 10 * p0`
  - `practise_raises_analysis_and_each_point_costs_more`
  - `practise_stalls_exactly_at_the_cap_and_drops_overflow`: analysis is `== analysis_cap`,
    `stalled == true`, `practice == 0`.
  - `practise_after_the_last_offset_never_stalls`
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine storyline`. Expected: they fail to compile.
- [ ] **Step 3: Implement the module and the tuning file.**
  - `trigger_count`: `5 + mix(seed ^ 0xBA5E_A1A1) % 6`, using a splitmix32-style mixer local to the
    module.
- [ ] **Step 4:** Run the same command. Expected: PASS.
- [ ] **Step 5: Commit.** `Storyline core: base AI stages and pure decide functions`

### Task 2: Game owns the arc: tick, claim, fugue, fusion and save

**Files:**
- Modify: `crates/engine/src/lib.rs`: add the `storyline: Storyline` field on `Game` (around line 217)
  and initialise it in every constructor.
- Create: `crates/engine/src/game/storyline.rs`: `advance_storyline`, plus the gather and apply
  helpers.
- Modify: `crates/engine/src/game/turn.rs`: call `self.advance_storyline()` in `tick_inner` right after
  `schedule_base_labour()` and before `settle_research()`.
- Modify: `crates/engine/src/game/base/` labour scheduling, wherever `schedule_base_labour` builds
  demand: while the arc isn't `Dormant`, no program is posted to a structure whose
  `StructureDef.studies` is true.
- Modify: `crates/engine/src/components/staff.rs`: add `#[derive(Component)] pub struct BaseAiHost;`
  next to `Jailed`.
- Modify: `crates/engine/src/save/mod.rs` (`SaveData`, around line 1657): add `#[serde(default)] pub
  storyline: Storyline`. Also update `game/lifecycle/save.rs` and `load.rs`.
- Test: `crates/engine/src/storyline/tests.rs` for the game-level tests, and
  `crates/engine/tests/save_roundtrip.rs`.

**Interfaces:**
- Consumes: Task 1's `step`, `trigger_count`, `pick_host`, `pick_station`.
- Produces:
  - `Game::advance_storyline(&mut self)`
  - `Game::storyline(&self) -> &Storyline`
  - `pub(crate) fn Game::base_ai_host(&self) -> Option<Entity>`
  - `pub(crate) fn Game::base_ai_station(&self) -> Option<Entity>`, which resolves the anchor tile to a
    `studies` structure.
- **Gather:** `researched = Research.0.len()`.
  - Candidates: `base_staff()` before the exclusion in Task 4, plus `free = role == Staff`, with their
    `Attributes` analysis.
  - Stations: from `research_stations()`, with tile distance from the candidate.
  - `host_pace`: research units per tick that this host would make at this station on the ordinary
    formula, which is success chance divided by cycle ticks. It **must call** the functions the normal
    posting uses (`Game::work_ticks_for` / `cycle_ticks_for` and `systems::mining_success_chance`
    through whatever `resolve_gather_cycle` feeds it). If a reusable function doesn't exist, extract a
    pure `research_pace_per_tick(..) -> f64` that both paths call ("mirrors must be a call").
- **Apply:**
  - `BeginClaim`:
    - Insert `BaseAiHost` on the host.
    - Clear its Task and post.
    - Release every program posted at a `studies` structure from that post.
    - Walk the host to the station using the existing walk-to-a-structure path, which the builder finds.
      Arrival is derived from position, in the same way as `UnderStudy`.
    - Fire the awakening notification when `announce` is true (Task 3 supplies the kind; until then use
      a `todo!`-free stub that Task 3 replaces).
  - `AbandonClaim`: remove `BaseAiHost` and set `Dormant { announced: true }`.
  - `Fuse`: fire the fusion notification.
  - **No research anywhere while the arc is Claiming or in the Fugue.** This follows from the
    scheduler filter and the released posts. Existing `ActiveResearch.progress` is left untouched.

- [ ] **Step 1: Write failing tests,** starting from a `dev-saves/` template with base staff and a
  Research Station (check `dev-saves/README.md`; don't use a fresh `Game::new`):
  - `the_awakening_fires_once_research_reaches_the_trigger`: set `Research` to `trigger` ids, tick
    once, and the arc is `Claiming` on the highest-Analysis staff.
  - `a_save_already_past_ten_claims_on_the_first_tick`: the `fred` case.
  - `no_research_lands_during_the_claim_or_the_fugue`: `ActiveResearch.progress` and `.study` don't
    change over the fugue.
  - `progress_on_the_active_project_survives_the_fugue`
  - `the_host_fuses_after_the_fugue_and_has_no_task`
  - `a_host_downed_mid_walk_abandons_and_the_next_claim_does_not_re_announce`
  - In `save_roundtrip.rs`: `the_storyline_round_trips_in_every_stage`, covering Claiming, Fugue (with
    `ticks_left` preserved) and Fused. Each case saves and loads, then compares `storyline()`.
  - `a_save_without_a_storyline_field_loads_dormant`
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine storyline save_roundtrip`. Expected: FAIL.
- [ ] **Step 3:** Implement as above.
- [ ] **Step 4:** Run the same tests. Expected: PASS. Then run `cargo test -p feral-processes-engine
  balance_sim`. Expected: unchanged, because the arc stays Dormant in balance runs or draws no RNG.
- [ ] **Step 5: Commit.** `Base AI: the claim, the fugue and the fusion, saved`

### Task 3: Storyline notifications and the stall alert

**Files:**
- Modify: `crates/engine/src/notifications.rs`:
  - `NotificationCategory { Storyline }` with `label() -> &'static str` returning `"Storyline"`.
  - `category: Option<NotificationCategory>` on `NotificationDef` (line 136) and on `Notification`
    (line 328).
  - Three kinds: `BaseAiAwakening`, `BaseAiFused`, `BaseAiLevelUp`.
  - Bump the `all()` array length.
  - Add `def` and `slug` arms.
- Modify: `crates/engine/src/alerts.rs`: `AlertKind::BaseAiWaiting`.
- Modify: `crates/engine/tests/notifications.rs`: census `site()` arms.
- Modify: `crates/gui/src/render/notify.rs`: `draw_notification` draws the category label above the
  title when `Some`, and the height check accounts for it.
- Modify: `crates/engine/src/game/storyline.rs`: replace the Task 2 stubs with
  `notify`/`notify_filled`.

**Interfaces:** Produces the kinds above. `BaseAiLevelUp` is filled with the new level through
`notify_filled`.

- [ ] **Step 1: Failing tests:**
  - `storyline_kinds_carry_the_storyline_category_and_others_carry_none`
  - The existing census tests pick up the new kinds.
  - `the_awakening_notification_names_the_host`
- [ ] **Step 2: Run the tests.** Expected: FAIL.
- [ ] **Step 3: Implement.**
  - The awakening and fusion text is the builder's, in plain language. The awakening names the program
    and says something is going wrong with it. The fusion says it seems to have fused with the
    Research Station.
  - The alert text for `BaseAiWaiting` says the AI is waiting to be trained.
- [ ] **Step 4:** Run `cargo test --workspace notification`. Expected: PASS.
- [ ] **Step 5: Commit.** `Notifications gain a category; the base AI's storyline alerts`

### Task 4: The host can't be lost; there is only one station

**Files:**
- Modify: `crates/engine/src/game/party.rs`:
  - `ProgramRole::BaseAi`, decided in `role_of` before `Staff` from `BaseAiHost`.
  - New arms in the `ord` match and `walks_the_base`.
  - The direct `UnderStudy`/`Jailed` checks in `add_companion`, `wield_program`, `programs_for_build`,
    `fuse_companions` and `commit_program` also refuse `BaseAiHost`.
- Modify the other direct-check sites to treat `BaseAiHost` as they treat `Jailed`:
  - `game/trade.rs` `sell_companion`
  - `game/refactor.rs` `open_kernel_ring`
  - `game/routines.rs` (the door after `routine_holder_label`)
  - `game/siege/mod.rs:127`
  - `game/base/tidy.rs:88`
  - `game/inspection.rs`
  - `game/base/healing.rs`
  - Refusal strings are in storyline voice, for example "It won't leave the Research Station."
- Modify: `crates/engine/src/game/base/building.rs`:
  - `place_structure` refuses a second `studies` structure once the arc is Fused, next to the Home
    uniqueness check.
  - `remove_structure` refuses the AI station.
  - The Home demolish cascade skips the AI station.
- Modify: `crates/engine/src/game/base/study.rs`: at fusion (`Fuse` apply), subjects pinned at other
  stations are released with `release_study_station`, and `pin_subject` refuses any station except
  the AI's.
- Modify: `crates/engine/src/views/programs.rs` `RosterRow`: add `base_ai: bool`, filled in
  `manifest_roster`.
- Modify: `crates/gui/src/render/party.rs`: arms for `role_heading` (the heading is `"Base AI"`) and
  `pet(...)`.

- [ ] **Step 1: Failing tests,** with an arc forced to Fused on a template:
  - `the_base_ai_is_never_base_staff`
  - `the_base_ai_cannot_join_the_party`, and likewise `..._be_fused`, `..._be_sold`, `..._go_on_a_sortie`
    and `..._be_spent_on_a_build`
  - `a_second_research_station_is_refused_after_fusion`
  - `the_ai_station_cannot_be_removed`
  - `removing_the_home_leaves_the_ai_station`
  - `a_subject_pinned_elsewhere_is_released_at_fusion_and_cannot_be_re_pinned_there`
  - `the_manifest_marks_the_base_ai_and_shows_its_true_analysis`
- [ ] **Step 2: Run the tests.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit.** `Base AI: locked to its station, the only Research Station`

### Task 5: The AI researches and grows

**Files:**
- Modify: `crates/engine/src/game/storyline.rs`. Each tick while Fused:
  - Compute `pace(f, analysis)`, add it to `carry`, and credit the whole units.
  - If `training.selected`, the credit goes to `training.progress`.
  - Otherwise, if `ActiveResearch.id` is `Some`, it goes through `ActiveResearch::credit`.
  - Otherwise it goes through `credit_study`.
  - The credit uses the same caps `deliver_payout` passes. Reuse its logic through a shared call, not a
    copy.
  - Then `practise(f, analysis, 1)` for each tick that credit could land, and write the new Analysis to
    the host's `Attributes` `"analysis"` entry.
  - Post `AlertKind::BaseAiWaiting` when practice first stalls, and clear it at level-up.
- Test: `crates/engine/src/storyline/tests.rs` and `save_roundtrip.rs`.

**Interfaces:** Consumes Task 1's `pace` and `practise`. Produces the Fused tick behaviour that Task 6
builds on.

- [ ] **Step 1: Failing tests:**
  - `the_ai_researches_at_a_quarter_of_its_old_pace_at_fusion`: over N ticks, the credited units are
    `floor(N * 0.25 * pace_at_fusion)` within ±1.
  - `the_ai_banks_study_when_nothing_is_selected`
  - `analysis_grows_while_it_researches_and_shows_on_its_attributes`
  - `analysis_stops_at_the_threshold_and_the_waiting_alert_posts`
  - `a_stall_does_not_stop_the_running_project`
  - `other_stations_produce_nothing_after_fusion`: post a staffer by hand if the scheduler would, then
    assert no credit from it.
  - Extend `the_storyline_round_trips_in_every_stage` with a stalled Fused arc whose `carry` and
    `practice` are non-zero.
- [ ] **Step 2: Run the tests.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** Run `cargo test -p feral-processes-engine storyline save_roundtrip balance_sim`.
  Expected: PASS, with balance unchanged.
- [ ] **Step 5: Commit.** `Base AI: researches at its own pace and grows by practice`

### Task 6: "Train your AI"

**Files:**
- Modify: `crates/engine/src/game/storyline.rs` and `game/unlocks.rs`:
  - `pub fn Game::select_ai_training(&mut self) -> Result<(), String>`: refused unless the AI is Fused
    and stalled. It sets `training.selected` and `ActiveResearch.id = None`. The progress map is kept.
  - `Game::select_research` clears `training.selected`.
  - When `training.progress >= training_cost(level)` and a subject is pinned and arrived at the AI
    station: spend it through `spend_subject` (make it `pub(crate)`), add one level, reset `practice`
    and `training`, notify `BaseAiLevelUp` and clear the alert.
- Modify: `game/base/study.rs` `research_wants_subject`: true while `training.selected`.
- Create: `crates/engine/src/views/storyline.rs`: `pub struct BaseAiView { level: u32, analysis: i32,
  next_threshold: Option<i32>, stalled: bool, training_available: bool, training_selected: bool,
  training_progress: u32, training_cost: u32 }`.
  - Add `Game::base_ai_view(&self) -> Option<BaseAiView>`, which is `Some` once Fused.
  - Re-export it from `views/mod.rs`.
- Modify: `crates/app-core/src/app/progression.rs`:
  - The research list gets a `Train your AI` row while `training_available`.
  - Enter calls `select_ai_training`, then opens `Mode::PinSubject` exactly as `select_research` does
    for a body-gated node.
- Modify: `crates/gui/src/render/progression.rs`: add the row to `research_menu_rows` and
  `state_tag`, and show progress against cost. Follow the rules that lists are highlight + Enter and
  that GUI text never clips.

- [ ] **Step 1: Failing tests:**
  - Engine: `training_is_refused_until_the_ai_stalls`
  - Engine: `training_wants_a_subject_and_consumes_it_on_completion_only`
  - Engine: `unpinning_before_completion_costs_nothing`
  - Engine: `switching_away_from_training_keeps_its_progress`
  - Engine: `completing_training_raises_the_level_and_practice_resumes`
  - Engine: `the_level_up_notification_names_the_level`
  - App-core: `the_research_menu_offers_train_your_ai_only_when_available` and
    `entering_train_your_ai_opens_the_pin_subject_screen`
- [ ] **Step 2: Run the tests.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit.** `Base AI: Train your AI levels it with a pinned program`

### Task 7: Research gated on the AI's level

**Files:**
- Modify: `crates/engine/src/research.rs`: `ResearchDef` gains `#[serde(default)] pub
  requires_ai_level: Option<u32>`, next to `min_zone`.
- Modify: `crates/engine/src/views/research.rs`: `ResearchState::Locked` gains `ai_level:
  Option<u32>`, set when the gate isn't met.
- Modify: `game/unlocks.rs`:
  - The state build (around lines 795-810) reads the AI level from `storyline()`. The level is 0 before
    the fusion.
  - `select_research` refuses with the gated line.
- Modify: `crates/gui/src/render/progression.rs` `state_tag`/`row_color` and
  `render/research_graph.rs`: a node locked only by the AI level shows `Seems feasible, but we don't
  know where to start`.
- Modify: `assets/research/README.md`: document the field. No `.ron` gets a value until Task 8.

- [ ] **Step 1: Failing tests:**
  - `a_node_gated_on_ai_level_is_locked_with_the_gated_reason_before_and_after_fusion`
  - `it_opens_when_the_ai_reaches_the_level`
  - `selecting_a_gated_node_is_refused`
  - `a_research_ron_without_the_field_still_parses`
- [ ] **Step 2: Run the tests.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit.** `Research can be gated on the base AI's level`

### Task 8: The balance table, a STOP gate for the user

**Files:**
- Create: `docs/measurements/2026-11-xx-base-ai-research-pace.md`. Follow `docs/measurements/README.md`
  for the bar; read the existing measurements first.
- Then modify: `assets/research/*.ron` (costs and `requires_ai_level`) and `tuning/storyline.rs`.

- [ ] **Step 1: Measure.**
  - Derive the ordinary research pace per tick from live constants and real assets, using the Task 2
    pace function and a typical researcher's Analysis.
  - Compute the time to finish the tree for three players: no fusion, fused and never trained, and
    trained at every threshold.
  - Use whatever deterministic harness already fits (`balance_sim`'s style or an arena or sweep tool
    named in `docs/measurements/`). Don't use the wall clock.
- [ ] **Step 2: Propose one table:**
  - number of levels
  - `BASE_AI_LEVEL_OFFSETS`
  - practice figures
  - training costs
  - which projects get `requires_ai_level` and at which level
  - the new costs for post-awakening projects

  Leave the projects players finish before the awakening (roughly the 8 to 30 cost tier) unchanged.

  Show the three completion times against the spec's bars:
  - an untrained AI can finish every ungated project, but slowly
  - a player who trains at every threshold ends up faster than before the fusion
  - every level in this build unlocks at least one project
- [ ] **Step 3: STOP.** Present the table to the user and wait for approval. Do not apply it before that.
- [ ] **Step 4:** Apply the approved table. Run `cargo test -p feral-processes-engine balance_sim`, and
  update any curve it moves only with the user's OK, noting it in the measurement doc. Run the asset
  census tests.
- [ ] **Step 5: Commit.** `Base AI: research costs and AI-level gates`

### Task 9: Dev saves, help page, index and final gate

**Files:**
- `dev-saves/`: add two templates and their README entries.
  - **`base-ai-ready`:** one research short of its trigger. Build it with `savetool dump`, edit
    `Research` in the RON, `pack`, and `capture`.
  - **`base-ai-stalled`:** fused at level 0, stalled. Reach it by ticking from `base-ai-ready` in a
    small launcher or test helper, or by editing `storyline` in the dumped RON.
- Create: `assets/help/48-the-base-ai.md`, a short player-facing explanation in plain language. Link it
  from `47-attributes.md`'s "Read on" line. Check whether a help census test lists pages.
- Modify: `docs/superpowers/INDEX.md`: add a row for `2026-11-04-base-ai-storyline-design` marked
  `unreleased`.
- Don't touch the manual, the README or TODO.md. The CHANGELOG entry is written at release, through the
  deploy skill.

- [ ] **Step 1:** Create the templates and verify each one loads: `cargo run -- --template base-ai-ready`,
  and screenshot it with `--screenshot` if `DISPLAY` is set.
- [ ] **Step 2:** Write the help page and the INDEX row.
- [ ] **Step 3: Final gate.**
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets` with no warnings
  - `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES` first)
- [ ] **Step 4: Commit.** `Base AI: dev saves and help page`
- [ ] **Step 5: Final review.** A whole-branch review on opus, with the diff passed as a file. Then land
  the branch through the deploy skill when the user asks.
