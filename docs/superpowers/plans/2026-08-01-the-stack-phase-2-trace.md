# The Stack Phase 2 — Trace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Trace meter that rises with what the party takes from a stack and escalates what comes for them, visible in the Stack HUD.

**Architecture:** A `Trace(u32)` resource raised at three hook sites, classified into four bands, feeding three multipliers. Escalation reuses the existing spawn path rather than adding one: encounter chance at the roll, stats folded into `stack_depth_multiplier`, group size threaded into `spawn_pack` as a parameter.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), bincode + RON saves.

**Spec:** `docs/superpowers/specs/2026-07-31-the-stack-design.md`, "Phase 2 — Trace". Read that section before starting. It records four claims an earlier sketch got wrong and why each correction is load-bearing — three of the tests below exist specifically to hold those corrections in place.

## Global Constraints

- **Difficulty tuning is code, not data.** Every number goes in `crates/engine/src/tuning.rs` as a documented `pub const` in one labelled section. Nothing inline in a formula.
- **`SAVE_FORMAT_VERSION` 15 → 16**, bumped exactly once, in Task 1.
- **World generation must never draw from `resources::GameRng`.** No task here generates, but Task 2's measurement test must not perturb the shared stream.
- **No flaky tests.** Seeded RNG only, no wall-clock, no reliance on background systems (habitat spawns and nests keep rolling on every `tick`).
- **`Game::apply_damage` is the only path that lowers HP.** Not used in this phase; noted because Task 3 touches spawn scaling next door to it.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- Engine tests: `cargo test -p feral-processes-engine <name>` to iterate (~3s). `cargo test --workspace` is the gate at Task 5, not per task.
- Test fixtures live in `crates/engine/src/tests/support.rs` — `spawn_tamed`, `spawn_wild_on_player_tile`, `insert_battle`, `set_level`, `resolve_round_with`. Look there before writing a new one.

---

## File Structure

**Engine (`crates/engine/src/`)**

| File | Change | Responsibility |
| --- | --- | --- |
| `tuning.rs` | modify | One labelled `Trace` section: three gains, three thresholds, three multiplier tables. |
| `resources.rs` | modify | `Trace(u32)` resource and `TraceBand` enum with `label()` and `index()`. |
| `game/trace.rs` | **create** | The only module that raises Trace, classifies a band, and hands out multipliers. |
| `game/mod.rs` | modify | Register `mod trace;`. |
| `game/stack.rs` | modify | Clear Trace in `clear_stack`; scale the roll in `maybe_stack_encounter`; fold the band into `stack_depth_multiplier`. |
| `game/stack_features.rs` | modify | Raise on `open_cache` and on a successful `pass_seal`. |
| `game/combat_rewards.rs` | modify | Raise on `award_loot`. |
| `game/spawning.rs` | modify | `spawn_pack` takes group scaling as a parameter. |
| `game/lifecycle.rs` | modify | Insert `Trace::default()` at both world-construction sites; capture and restore it. |
| `game/stack_view.rs` | modify | Populate the new `StackView` field. |
| `save.rs` | modify | New `SaveData` field; bump the version const. |
| `views.rs` | modify | New `StackView` field. |
| `tests/stack.rs` | modify | Tasks 1–3 tests. |

**GUI (`crates/gui/src/`)**

| File | Change | Responsibility |
| --- | --- | --- |
| `render/stack.rs` | modify | Append the band to the existing heading. |

**Docs** — `README.md`, `CHANGELOG.md`, `CLAUDE.md` (+ `AGENTS.md` copy), spec status table. Task 5.

---

## Task 1: The Trace resource, its bands, and its persistence

Trace exists, saves, resets on surfacing, and survives a frame change. Nothing raises it and nothing reads it yet — this task is the storage decision and its guarantees, in isolation.

**Files:**
- Create: `crates/engine/src/game/trace.rs`
- Modify: `crates/engine/src/tuning.rs`, `resources.rs`, `game/mod.rs`, `game/stack.rs` (`clear_stack` only), `game/lifecycle.rs:60,181,446,649`, `save.rs:189-194,222`
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces:**

- Consumes: nothing.
- Produces:

```rust
// resources.rs
#[derive(Resource, Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceBand { Quiet, Noticed, Traced, Hunted }

impl TraceBand {
    pub fn label(self) -> &'static str;   // "Quiet" | "Noticed" | "Traced" | "Hunted"
    pub(crate) fn index(self) -> usize;   // 0..=3, indexes the tuning tables
}

// game/trace.rs
impl Game {
    pub(crate) fn trace(&self) -> u32;
    pub(crate) fn trace_band(&self) -> TraceBand;
    pub(crate) fn band_for(trace: u32) -> TraceBand;  // associated, so it is testable without a Game
}

// save.rs — new field, and the version const goes 15 -> 16
pub struct SaveData { /* ... */ pub trace: u32 }
```

`TraceBand` lives in `resources.rs` beside `Trace` rather than in `views.rs`, because it is domain state the renderer displays, not a shape authored for drawing.

**Tuning constants** (one labelled section, each with a doc comment saying what it is and that it is unplaytested):

```rust
pub const TRACE_PER_CACHE: u32 = 10;
pub const TRACE_PER_SEAL: u32 = 5;
pub const TRACE_PER_KILL: u32 = 2;

pub const TRACE_NOTICED: u32 = 40;
pub const TRACE_TRACED: u32 = 100;
pub const TRACE_HUNTED: u32 = 180;

/// Indexed by `TraceBand::index`. Encounter chance is the gentlest of the
/// three deliberately: it is the only lever that feeds back into its own
/// input, since more encounters mean more kills mean more Trace.
pub const TRACE_ENCOUNTER_MULT: [f64; 4] = [1.0, 1.25, 1.6, 2.0];
pub const TRACE_STAT_MULT: [f32; 4] = [1.0, 1.10, 1.25, 1.45];
/// Inert in zone 1, where `zone_group_cap(1)` pins every group to one
/// member whatever this says. Not a bug.
pub const TRACE_GROUP_MULT: [u32; 4] = [1, 1, 2, 3];
```

- [ ] **Step 1: Write the failing tests.** Five, in `tests/stack.rs`:
  - `trace_survives_descending_and_ascending` — set Trace to 50 at depth 1, `descend()`, assert still 50; `ascend()`, assert still 50. **This is the regression test for the whole storage decision** and fails against a field on `Locale::Stack`, which `descend_to`/`ascend_to` rebuild wholesale. Write it first.
  - `surfacing_clears_trace` — Trace 50 at depth 1, `ascend()` out, assert `Locale::Surface` and Trace 0.
  - `use_symlink_clears_trace` — the other route out of the Stack, which CLAUDE.md documents as going *through* `clear_stack` rather than around it. Same assertion.
  - `trace_round_trips_through_save` — Trace 77 mid-dive, save and load, assert 77.
  - `band_thresholds_are_half_open` — `band_for` at 0, 39, 40, 99, 100, 179, 180. Assert the boundary lands in the *upper* band, so a threshold constant reads as "from".
- [ ] **Step 2: Run them and watch each fail** for the right reason (`Trace` unresolved), not a compile error elsewhere.
- [ ] **Step 3: Implement.** Resource, enum, tuning section, `game/trace.rs`, both `lifecycle.rs` insert sites, capture at `lifecycle.rs:649` and restore near `:446`, `SaveData` field with `#[serde(default)]`, version bump, and one line in `clear_stack`.

  `#[serde(default)]` does nothing for bincode, which is positional and covered by the version bump. It is there for the field-named RON templates — see `crates/launcher/src/dev_template.rs`, which documents that this is exactly what lets `dev-saves/extraction.ron` survive a `SaveData` change without re-capture. Do not re-capture it.

- [ ] **Step 4: Run the tests, then `cargo test -p feral-processes-engine` whole.** The save round-trip tests and `dev_template`'s three guards in the launcher crate are the ones the version bump can break: `cargo test -p feral-processes --lib` too.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

## Task 2: The three sources, and the band-crossing log

**Files:**
- Modify: `crates/engine/src/game/trace.rs`, `game/stack_features.rs` (`open_cache`, `pass_seal`), `game/combat_rewards.rs` (`award_loot`)
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces:**

- Consumes: `Game::trace_band`, `Game::band_for`, the `TRACE_PER_*` constants from Task 1.
- Produces: `pub(crate) fn raise_trace(&mut self, amount: u32)` in `game/trace.rs`.

`raise_trace` is the single choke point and carries two responsibilities that must not be split to the call sites:

1. **It no-ops unless `is_underground()`.** `award_loot` fires for every kill in the game, the overwhelming majority of them on the surface. One guard here beats three at the hooks.
2. **It compares the band before and after and logs any rise** via `log_kind(MessageKind::Outcome, ...)` — `Outcome` specifically, so the line survives `MessageLog::retain_outcomes_since_battle`, which prunes plain `Info` when a battle ends. A kill-driven crossing is logged *during* a battle teardown, so this is not theoretical.

Crossings are monotonic within a dive — no decay, reset only on surfacing — so only a rise is ever logged and no fall case is needed.

Band-crossing lines, in the game's register:

| Band | Line |
| --- | --- |
| Noticed | `Something in the substrate turns to look at you.` |
| Traced | `You are being traced. The dark is routing around you.` |
| Hunted | `Hunted. Whatever is down here has your address.` |

Hook placement:

- `open_cache` — after the spent-ness record is written, so a re-entered empty cache cannot charge again (the function already early-returns on `looted`).
- `pass_seal` — only on the branch that actually burns a shard. Not when `already_open`, and not when the shard is missing and the attempt is refused.
- `award_loot` — once per dead hostile. It is the one place that knows a hostile died rather than being fled from, which is why `mark_lair_cleared` already lives there.

- [ ] **Step 1: Write the failing tests.**
  - `cracking_a_cache_raises_trace` — by exactly `TRACE_PER_CACHE`; stepping onto the emptied cache again raises nothing.
  - `burning_a_seal_raises_trace` — by `TRACE_PER_SEAL` with a shard in the pack; raises nothing when the seal is refused for want of one, and nothing on re-crossing an opened seal.
  - `killing_a_hostile_raises_trace` — `TRACE_PER_KILL` per dead hostile.
  - `a_surface_kill_raises_no_trace` — the guard inside `raise_trace`. Kill on the surface, assert 0.
  - `a_plain_step_raises_no_trace` — walking is free; this is the design's load-bearing choice, so assert it.
  - `crossing_a_band_logs_an_outcome_line` — raise across `TRACE_NOTICED`, assert a `MessageKind::Outcome` entry appears; raising within a band logs nothing.
  - `a_frame_holds_three_caches_and_two_hundred_walkable_cells` — **the measurement the whole tuning table rests on.** Left unasserted, a later generator change moves the kill-to-cache ratio and silently turns the greed meter into a combat meter with the suite still green.

    Generate frames at depths 1–4 from a fixed `FrameSpec` (`world_seed: 12345, entrance: (30, 30), frames: 4` reproduces the numbers below) and assert:

    | Quantity | Measured, depths 1–4 | Assert |
    | --- | --- | --- |
    | Walkable cells | 206, 208, 209, 204 | in `190..=220` |
    | Caches | 3, 3, 2, 3 | in `2..=3` |
    | Sealed doors | 0, 0, 0, 2 | `0` above the bottom frame, `> 0` on it |

    Ranges rather than equalities because the generator legitimately varies per depth; they are tight enough that a change to frame size or cache count breaks them. Comment the test with *why* it exists — that the kill-to-cache ratio is what makes Trace a greed meter rather than a combat meter — because the numbers alone do not say that.
- [ ] **Step 2: Run them and watch each fail.**
- [ ] **Step 3: Implement `raise_trace` and wire the three hooks.**
- [ ] **Step 4: Run `cargo test -p feral-processes-engine`.**
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

## Task 3: Escalation

The three multipliers, applied. This is the task where the spawn path changes shape, so it opens with a design dialog rather than an edit.

**Files:**
- Modify: `crates/engine/src/game/trace.rs`, `game/stack.rs` (`maybe_stack_encounter`, `stack_depth_multiplier`), `game/spawning.rs` (`spawn_pack` and its four call sites), `game/stack_features.rs` (`rouse_lair`'s `spawn_pack` call), `game/turn.rs:290`
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces:**

- Consumes: `Game::trace_band` from Task 1.
- Produces, in `game/trace.rs`:

```rust
impl Game {
    pub(crate) fn trace_encounter_mult(&self) -> f64;
    pub(crate) fn trace_stat_mult(&self) -> f32;
    pub(crate) fn trace_group_mult(&self) -> u32;
}
```

Three separate accessors rather than one struct, because the three are consumed at three unrelated call sites and never together.

- [ ] **Step 1: Run the `design-patterns` dialog on the `spawn_pack` signature.** Required by CLAUDE.md before a structural choice, and this is one. The question: `spawn_pack` currently takes `(&str, bool, i32, i32, f32)` and adding group scaling makes it six parameters ending in two bare numeric multipliers whose order is easy to transpose silently. The alternative is a small `PackScaling { stats: f32, group: u32 }` with a `SURFACE` constant, which would also let the two surface call sites (`spawning.rs:607`, `turn.rs:290`) stop passing a bare `1.0`. Decide with the dialog, then hold that decision for the rest of the task.

  **Whichever shape wins, group scaling is a parameter, never read off the `Trace` resource inside `spawn_pack`.** That function's doc comment records this precise leak already happening once with `depth_mult`: ambient spawns and nest respawns keep rolling on every `tick` while the party is underground, so a locale-derived multiplier scaled them too and left 3× programs standing around the link mouth for the climb out. A Trace-derived multiplier reproduces it exactly.

- [ ] **Step 2: Write the failing tests.**
  - `trace_scales_the_encounter_roll` — assert `trace_encounter_mult` against each band, and that `STACK_ENCOUNTER_CHANCE * mult` at Hunted is the expected 0.16.
  - `trace_scales_enemy_stats_through_depth` — `stack_depth_multiplier` at depth 3 with Trace at Hunted equals `STACK_DEPTH_STAT_GROWTH.powi(2) * 1.45`. Folding it here is what makes the lair guardian inherit the party's greed for free; its only two callers are the ambush and the lair.
  - `trace_scales_group_size` — the group ceiling handed to the roll is multiplied by the band. If `max_group` is combined with the multiplier in more than one expression, extract that to a pure function and assert it directly.
  - `a_surface_spawn_is_unscaled_while_the_party_is_hunted` — **the leak regression test.** With the party underground at Hunted, drive a surface spawn from a seeded RNG and assert the pack matches the same seeded call with Trace at 0. Seed both identically; this must not depend on the shared `GameRng` stream position.
  - `a_hunted_ambush_is_still_never_a_boss` — Hunted does not open the boss pool. `maybe_stack_encounter` documents the rule it keeps; assert it survives escalation.
- [ ] **Step 3: Run them and watch each fail.**
- [ ] **Step 4: Implement.** Scale the roll in `maybe_stack_encounter`; fold `trace_stat_mult` into `stack_depth_multiplier`; thread group scaling through `spawn_pack` in the shape Step 1 chose, updating all four call sites.
- [ ] **Step 5: Run `cargo test -p feral-processes-engine`, then `cargo test -p feral-processes-engine balance_sim`.** Spawn rates and enemy stats both moved, so a shifted level curve is expected *only* if the sim exercises Stack spawns. It does not model the Stack, so the curves should be **unchanged** — a moved curve here means the band multiplier leaked into surface spawning, which is the bug the leak test above is hunting. Treat it as a failure, not as a retune.
- [ ] **Step 6: `cargo fmt && cargo clippy --workspace`, then commit.**

---

## Task 4: The HUD readout

Without this the phase is a difficulty curve nobody can see, and escalating ambushes with no visible cause read as bad luck rather than as consequence.

**Files:**
- Modify: `crates/engine/src/views.rs` (`StackView`), `game/stack_view.rs:219-226` (populate it), `crates/gui/src/render/stack.rs:145-148` (the heading)
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces:**

- Consumes: `Game::trace_band`, `TraceBand::label` from Task 1.
- Produces: `pub trace: &'static str` on `StackView`.

`&'static str` from `TraceBand::label()`, not the enum. This follows the rule `StackView` already documents for its own `facing` field — a reading for the player, not something the renderer projects with. The renderer draws it verbatim and authors nothing.

The band only, never the raw number: it is a threat readout, not a progress bar, and a visible integer invites playing to the threshold instead of to the risk.

The heading at `render/stack.rs:145` is currently `Facing {} Depth {} / {} ({}, {})`. Append the band to it. Do **not** add the band to the full-screen `g` map in this phase — that is a one-line addition if playtesting says the decision to press on is being made from that screen.

- [ ] **Step 1: Write the failing test.** `stack_view_reports_the_trace_band` — at Trace 0 the view reads `"Quiet"`; past `TRACE_HUNTED` it reads `"Hunted"`. Engine-side, because that is where the label is authored.
- [ ] **Step 2: Run it and watch it fail.**
- [ ] **Step 3: Implement** the field, its population, and the heading.
- [ ] **Step 4: Run `cargo test -p feral-processes-engine` and `cargo test -p feral-processes-gui`.** The existing degenerate-view test in the renderer must still pass — check whether it constructs a `StackView` literal that now needs the field.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

## Task 5: Docs, gates, and a template to play

**Files:**
- Modify: `README.md`, `CHANGELOG.md`, `CLAUDE.md`, `AGENTS.md`, `docs/manual.md`, `docs/superpowers/specs/2026-07-31-the-stack-design.md`
- Possibly create: `dev-saves/<name>.ron`

- [ ] **Step 1: Grep for claims this phase falsifies.** Any doc describing a Stack descent as having no pressure, or listing what the Stack HUD shows, is now wrong. `rg -n "Stack" README.md docs/manual.md` and read the hits — do not trust a memory of what they say.
- [ ] **Step 2: Update `README.md` and `docs/manual.md`** — the manual's "In the Stack:" key list needs no new key, but the descent description needs Trace.
- [ ] **Step 3: Add the `CHANGELOG.md` entry.**
- [ ] **Step 4: Add a CLAUDE.md load-bearing-seams entry** for `Trace` — that it resets in `clear_stack` and nowhere else, that `descend_to`/`ascend_to` rebuild the `Locale::Stack` variant which is *why* it is a resource, and that group scaling is a `spawn_pack` parameter rather than a resource read. Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift.
- [ ] **Step 5: Flip the spec's status table** row 2 to done, and strike the "Before building on phase 2" warning down to just the playtest obligation.
- [ ] **Step 6: Run the full gate.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`. This is the first full-suite run of the phase and the one that counts.
- [ ] **Step 7: Commit.**
- [ ] **Step 8: Capture a `dev-saves/` template and play it.** Get a party a few frames into a stack with caches left to crack, `cargo run --bin savetool -- capture saves/save.bin <name>`, and actually descend. **The spec's closing note makes this the obligation, not an optional extra:** the per-source ratios are grounded in a measured frame, but where the band lines fall at 40 / 100 / 180 is arithmetic and nothing else. Playing is the only thing that can answer whether the meter asks an interesting question — do it before phases 3 and 4 are built on top of it.

---

## Notes for whoever executes this

**The three tests that matter most** are the descend/ascend carry (Task 1), the surface-spawn leak (Task 3), and the frame measurement (Task 2). Each guards a correction the spec had to make against its own earlier reasoning, and each fails silently and plausibly if the correction is undone. If time pressure forces a choice, those three are the ones to keep.

**The lair escape hatch is known and priced, not a bug to fix.** A player can loot a stack to Hunted, climb out to shed it, and walk the remembered shortest path back down to meet the guardian at Noticed. What that costs is ~19 fights of attrition at no reward, since the caches are already empty. The spec records this as an accepted price. Do not close it as part of this phase.

**Group scaling is inert in zone 1.** `zone_group_cap(1)` is 1, so the multiplier changes nothing until zone 2. The tuning constant says so; do not report it as a defect.
