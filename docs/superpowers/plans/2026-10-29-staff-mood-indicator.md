# Staff Mood Indicator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every base staff program's mood step shows on its map tile and as counts on the CREW HUD tab, and the two worst steps raise an attention row.

**Architecture:** The engine derives one `MoodStep` per staff body from its stored `Disgruntled` marker plus `Game::morale`. It exposes that step on `EntityView` and as one `MoodCounts` call. gui reads both and computes nothing. `Game::attention` gains one `AttentionKind` for the two alarm steps.

**Tech Stack:** Rust, standalone `bevy_ecs` engine, bevy/egui renderer behind `paint.rs`'s `Painter`.

**Spec:** `docs/superpowers/specs/2026-10-29-staff-mood-indicator-design.md`

## Global Constraints

- Sulking, downed tools and lashing out are read off `Disgruntled`'s `Grievance` and never off raw morale. Only slipping reads morale.
- `MORALE_SLIPPING_AT = -4.0` in `crates/engine/src/tuning/staff.rs`.
- No save-format change, no asset/schema change, no ADR. `Game.world` stays private.
- The program glyph's colour never changes. The mark is drawn beside it.
- `render/` draws only through `Painter`. Text never clips or wraps: what doesn't fit is counted.
- Counts and per-body steps come from one engine derivation (`Game::mood_step`). No second fold in gui.
- No tick vocabulary in player-facing text.
- After each task, run `cargo fmt` and `cargo clippy --workspace --all-targets`, both clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.

## Review Focus

1. **Disgruntled at −7 reads sulking, not slipping** (hysteresis band, −8 to −6). Tested in Task 1.
2. **Staff past CREW's 12 rows are still counted.** Tested in Task 1 (`staff_moods` covers `base_staff()`, not `owned_pets`).
3. **Non-staff bodies are always `Ok`**, even with negative morale: party members, hostiles and the player. Tested in Task 1.
4. **The collapsed CREW bar fits at the narrowest width with all four non-ok steps non-zero.** Tested in Task 4.
5. **A mood mark and a rising mark (`+`/`^`) on the same body both draw** without sharing a rect. Tested in Task 3.

---

### Task 1: Engine — `MoodStep`, its derivation, counts, `EntityView.mood`

**Files:**
- Modify: `crates/engine/src/tuning/staff.rs` (beside `MORALE_RECOVERED_AT`, ~289)
- Modify: `crates/engine/src/views/base.rs` (new types + pure fn)
- Modify: `crates/engine/src/game/base/morale.rs` (two `Game` methods)
- Modify: `crates/engine/src/views/map.rs` (`EntityView.mood`), `crates/engine/src/game/inspection.rs` (~1414, view build)
- Modify: every `EntityView { .. }` literal (23 sites across engine/app-core/gui test fixtures; `rg -n 'EntityView \{'`), adding `mood: MoodStep::Ok`
- Test: `crates/engine/src/tests/staff_mood.rs` (new, registered in `tests/mod.rs`)

**Interfaces — Produces:**
- `pub const MORALE_SLIPPING_AT: f32 = -4.0;` with a doc saying why it sits between neutral and `MORALE_RECOVERED_AT`.
- `#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)] pub enum MoodStep { #[default] Ok, Slipping, Sulking, DownedTools, LashingOut }`, ordered best to worst.
- `impl MoodStep { pub fn label(self) -> &'static str }` returns `"ok" | "slipping" | "sulking" | "downed" | "lashing"`.
- `pub fn mood_step(grievance: Option<Grievance>, morale: f32) -> MoodStep` is pure. `Some(g)` maps 1:1 to Sulking/DownedTools/LashingOut. `None` gives `Slipping` if `morale < MORALE_SLIPPING_AT`, else `Ok`.
- `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub struct MoodCounts { pub slipping: usize, pub sulking: usize, pub downed_tools: usize, pub lashing_out: usize }` with `pub fn is_calm(&self) -> bool` (all zero) and `pub fn worst_first(&self) -> [(MoodStep, usize); 4]` (LashingOut → Slipping).
- `impl Game { pub fn mood_step(&self, who: Entity) -> MoodStep; pub fn staff_moods(&self) -> MoodCounts }`. `mood_step` returns `Ok` for any entity not in `base_staff()`. Callers that loop take the staff set once: the view build collects `base_staff()` into a `HashSet` before its loop and calls a private `fn mood_step_of_staff(&self, who) -> MoodStep` that skips the membership check. `staff_moods` iterates `base_staff()`.
- `EntityView.mood: MoodStep` (doc: `Ok` for anything but base staff).

- [ ] **Step 1: Write failing tests** in `tests/staff_mood.rs`:
  - `slipping_threshold_sits_between_neutral_and_recovery`: `assert!(0.0 > MORALE_SLIPPING_AT && MORALE_SLIPPING_AT > MORALE_RECOVERED_AT && MORALE_RECOVERED_AT > MORALE_SULKS_AT)`.
  - `mood_step_boundaries`: `mood_step(None, 0.0) == Ok`, `mood_step(None, -4.0) == Ok`, `mood_step(None, -4.01) == Slipping`, `mood_step(None, -7.0) == Slipping`, `mood_step(None, -60.0) == Slipping` (no marker means not yet acting), `mood_step(Some(Sulking), -7.0) == Sulking`, `mood_step(Some(Sulking), 0.0) == Sulking`, `mood_step(Some(DownedTools), _) == DownedTools`, `mood_step(Some(LashingOut), _) == LashingOut`.
  - `staff_moods_counts_every_base_staff_body`: build 14 staff (more than `CREW_ROWS` = 12; use `a_base_of`-style setup from `tests/tantrums.rs`) and insert `Disgruntled` directly on 13 of them, split across the three grievances. Don't tick, since `update_disgruntled` would clear markers. Assert `staff_moods()` sums to 13 with the right split.
  - `non_staff_bodies_read_ok`: a party member holding a `Disgruntled` marker and the player both give `mood_step == Ok`, and the `EntityView` for each has `mood == Ok`.
  - `entity_view_carries_the_staff_step`: a sulking staff body's `EntityView.mood == Sulking`.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine staff_mood`. Expected: compile FAIL (missing items).
- [ ] **Step 3: Implement** the Produces block. Fill every `EntityView` literal with `mood: MoodStep::Ok`.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine staff_mood`. Expected: PASS. Then `cargo build --workspace --all-targets`. Expected: builds.
- [ ] **Step 5: Commit** ("Engine: a staff body's mood step, its counts, and EntityView.mood").

### Task 2: Engine + gui tab map — `AttentionKind::StaffUnrest`

**Files:**
- Modify: `crates/engine/src/views/base.rs` (`AttentionKind`)
- Modify: `crates/engine/src/game/inspection.rs` (in `attention`, after the `Unslotted` row, ~2085)
- Modify: `crates/gui/src/render/hud/column.rs:115-118` (map to `InfoTab::Crew`)
- Test: `crates/engine/src/tests/attention.rs`

**Interfaces:**
- Consumes: `Game::staff_moods() -> MoodCounts`.
- Produces: `AttentionKind::StaffUnrest`. At most two rows, lashing first: `"{n} lashing out (Base)"` when `lashing_out > 0`, then `"{n} downed tools (Base)"` when `downed_tools > 0`. Both have `key: 'b'` and `threat: true`.

- [ ] **Step 1: Write failing tests** in `tests/attention.rs`:
  - `a_sulking_or_slipping_staff_raises_no_attention`: with one sulking staff body, `kinds()` has no `StaffUnrest`.
  - `downed_and_lashing_staff_raise_one_row_each`: two `DownedTools` and one `LashingOut` give exactly two `StaffUnrest` rows. Their texts are `"1 lashing out (Base)"` then `"2 downed tools (Base)"`, both `key == 'b'` and `threat`.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine attention`. Expected: FAIL.
- [ ] **Step 3: Implement**, and add the `column.rs` arm next to `Unslotted`'s `InfoTab::Crew`.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine attention` and `cargo test -p feral-processes-gui column`. Expected: PASS.
- [ ] **Step 5: Commit** ("Attention: downed and lashing staff need the player").

### Task 3: gui — the mood mark on a staff tile

Read `.claude/rules/seams-hud.md` and `drawing-seam.md` first.

**Files:**
- Modify: `crates/gui/src/render/marks.rs` (rect fn, mark fn, draw fn, unit tests)
- Modify: `crates/gui/src/render/base.rs`: draw it wherever `draw_rising_mark` is called for an actor (~1366, ~2398; check whether ~2162 is a test)

**Interfaces:**
- Consumes: `EntityView.mood`, `MoodStep`.
- Produces:
  - `pub(super) fn mood_mark(step: MoodStep) -> Option<(MoodShape, Color)>`. `Ok` gives `None`. Slipping gives `palette::WARN`, Sulking gives `palette::ATTENTION`, DownedTools and LashingOut give `palette::THREAT`. `MoodShape` is a small local enum and must tell DownedTools from LashingOut without colour: the builder picks, e.g. a bigger pip, or a glyph versus a square.
  - `pub(super) fn mood_mark_rect(px: f32, py: f32, tile_px: f32) -> Rect`. Pick a corner a staff body never otherwise uses (staff never wear nemesis, patrol or con-read marks; check each), below `RARITY_BAR_PX` and above `progress_bar_rect(..).y`, in `pin_mark_rects`' style. The doc comment says why that corner.
  - `pub(super) fn draw_mood_mark(painter: &Painter, ev: &EntityView, px: f32, py: f32, tile_px: f32, vig: f32)`, which applies `at_level(color, vig)` like every other mark.

- [ ] **Step 1: Write failing tests** in `marks.rs`'s test module:
  - `mood_mark_colours_by_step`: covers all five steps per the mapping above, and `mood_mark(DownedTools).0 != mood_mark(LashingOut).0`.
  - `mood_mark_clears_both_bars`: for `tile_px` in `[12.0, 16.0, 24.0, 32.0]`, the rect lies inside the tile, with `y >= py + RARITY_BAR_PX` and `y + h <= progress_bar_rect(..).y`.
  - `mood_mark_does_not_overlap_rising_mark`: compare against the rising mark's anchor rect, using whatever `draw_rising_mark` computes (factor that into a rect fn if it isn't one already).
- [ ] **Step 2: Run** `cargo test -p feral-processes-gui mood_mark`. Expected: FAIL.
- [ ] **Step 3: Implement** the mark and the call sites in `render/base.rs`.
- [ ] **Step 4: Run** `cargo test -p feral-processes-gui mood_mark`. Expected: PASS.
- [ ] **Step 5: Commit** ("Map: a staff tile wears its mood step").

### Task 4: gui — CREW mood counts (collapsed bar + open pane)

**Files:**
- Modify: `crates/gui/src/render/hud/panes.rs`: `PaneData` (~214), CREW rows (~527), `summary` (~710), tests (~843)
- Modify: `PaneData` construction at `crates/gui/src/render/hud/column.rs:377` and `crates/gui/src/render/base.rs:554`, filling `moods: game.staff_moods()` (or through whatever app-core already hands gui; follow how `labour: LabourDemand` arrives)

**Interfaces:**
- Consumes: `MoodCounts`, `MoodStep::label`.
- Produces: `PaneData.moods: MoodCounts`.
  - `summary(InfoTab::Crew, d)`: if `d.moods.is_calm()`, the output is unchanged from today. Otherwise it's the non-zero steps, worst first, as `"{n} {label}"` joined by `" · "`, e.g. `"1 lashing · 2 sulking · 1 slipping"`. If it would exceed the bar's width, drop the mildest steps and append `" +{n}"` for the bodies dropped, using the existing fitting/ellipsis helpers.
  - CREW open pane: one row after ROSTER, `label("MOOD    ")` plus the same counts. Each count is coloured by its step's `mood_mark` colour. When calm, it shows `"all ok"` in `BODY`.

- [ ] **Step 1: Write failing tests** in `panes.rs` tests:
  - `crew_summary_is_unchanged_when_calm`: compare against today's string for the existing fixture.
  - `crew_summary_counts_worst_first`: `{slipping:1, sulking:2, lashing_out:1}` gives `"1 lashing · 2 sulking · 1 slipping"`.
  - `crew_summary_fits_the_narrowest_bar`: all four non-zero with double-digit counts, at the narrowest supported column width (the existing census uses one; reuse it). Assert it fits and that the dropped count appears.
  - `crew_pane_has_a_mood_row`.
- [ ] **Step 2: Run** `cargo test -p feral-processes-gui panes`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p feral-processes-gui panes`. Expected: PASS.
- [ ] **Step 5: Commit** ("HUD: CREW counts staff by mood").

### Task 5: Gate and look

- [ ] **Step 1:** `cargo test --workspace` passes (check the exit code directly, not through a pipe). Then `cargo test -p feral-processes-engine balance_sim` passes; nothing in it should move.
- [ ] **Step 2:** Make a dev-save with disgruntled staff: `savetool dump` a base template, set `disgruntled` on several staff in the RON across the steps, `pack`, then `capture` as `unrest`. Take a screenshot: `cargo run -- --template unrest --screenshot out.png`. Read the PNG and confirm the marks are visible and distinct, the CREW bar shows counts or the attention alarm, and nothing clips.
- [ ] **Step 3:** Add the `unrest` template to `dev-saves/README.md` (not the repo-root README). Commit.
