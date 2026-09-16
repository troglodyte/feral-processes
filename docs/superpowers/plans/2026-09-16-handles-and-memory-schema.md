# Handles and the Memory Schema Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every owned program a derived hex handle (`0x435eaD`), and give
`MemoryDef` a `mood:` dial and a `stack_decay:` dial, with no change to any
number the game produces.

**Architecture:** Three phases, each merged green on its own. Phase 1 is the
memory schema (engine only, behaviour-identical). Phase 2 is the handle
derivation and the naming ladder (engine). Phase 3 sorts every name surface
into short or long and fixes the tests that assumed species names.

**Tech Stack:** Rust, standalone `bevy_ecs`, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-09-16-handles-and-memory-schema-design.md`
— read it first; this plan argues from it and does not restate it.

## Global Constraints

- No `SAVE_FORMAT_VERSION` bump, no new save field, no new component.
- No `GameRng` draw anywhere in this work.
- No shipped `.ron` file changes; `balance_sim` must not move
  (`cargo test -p feral-processes-engine balance_sim`).
- New `MemoryDef` fields are `#[serde(default)]`, and
  `assets/memories/README.md` documents them in the same commit.
- Plan executors: **never `git push`**, never `git add -A` (stage explicit
  paths), never `git checkout`/`stash` over uncommitted work.
- Gates at each phase boundary: `cargo fmt`,
  `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Within a
  task, iterate with `cargo test -p feral-processes-engine <name>` only.

## Token budget

This is small work. Run it **inline or with one sonnet subagent per phase**,
not one per task. There is no per-task review gate. One whole-branch review
runs at the end (opus, handed the diff as a file). Don't re-run the full
suite to confirm a subagent's report; spot-check one named test.

## File map

| File | Phase | Change |
|---|---|---|
| `crates/engine/src/memories.rs` | 1 | two fields on `MemoryDef`; `Read` enum; `sum_intensity` takes it |
| `crates/engine/src/components.rs` (`Memory::intensity_with`, ~l.2025) | 1 | geometric strikes term |
| `crates/engine/src/game/memories.rs` | 1 | `memory_sum` takes `Read`; `morale`/`opinion_of`/`memory_report` pass theirs; `evict` untouched |
| `crates/engine/src/systems.rs` (~l.1344) | 1 | `CycleModifiers::morale` passes `Read::Morale` |
| `crates/engine/src/tests/memories.rs` | 1 | `test_def` literal gains fields; new tests |
| `crates/engine/src/tests/assets.rs` (near `MEMORY_TRIGGERS`, l.2801) | 1 | two censuses |
| `assets/memories/README.md` | 1 | table rows + formula |
| `crates/engine/src/handles.rs` (new) + `lib.rs` `pub mod` | 2 | `handles::of` |
| `crates/engine/src/game/party.rs` (`creature_name` l.245, `creature_label` l.269) | 2 | ladder |
| `crates/engine/src/tests/party.rs` or a new `tests/handles.rs` | 2 | ladder tests |
| call sites across `engine`, `app-core`, `gui` | 3 | short vs long |
| `CLAUDE.md`, `.claude/skills/seams/`, memory graph | 3 | the handle seam, three writes |
| `CHANGELOG.md` | 3 | at merge only, per the release rule |

---

## Phase 1: The memory schema

Deliverable: two dials that default to today's behaviour, proven to work by
tests that move them. Merge-ready on its own.

### Task 1.1: `stack_decay`

**Files:** `memories.rs`, `components.rs`, `tests/memories.rs`,
`tests/assets.rs`, `assets/memories/README.md`

**Interfaces — Produces:** `MemoryDef::stack_decay: f32` (serde default
`1.0`, via a `fn one() -> f32` default helper shared with 1.2).

- [ ] Add the field. Update the `MemoryDef {…}` struct literals (two in
  tests: `tests/memories.rs::test_def` and one other; find them with
  `rg -n "MemoryDef \{" crates`), setting `stack_decay: 1.0`. Update the
  struct's doc comment, which currently says "these seven fields": the
  original fields stay required and the new ones default.
- [ ] Failing tests in `tests/memories.rs`:
  - `stack_decay_of_one_is_todays_linear_stacking`: at 1, 2 and cap strikes,
    `intensity` equals `valence × n` exactly (`assert_eq!`, not approx).
  - `stack_decay_shrinks_each_strike_after_the_first`: `r = 0.5`, three
    strikes, zero elapsed → `1.75 × valence`.
  - `stack_decay_still_stops_at_the_cap`: `r = 0.5`, strikes above the cap
    read the same as at the cap.
- [ ] Implement in `intensity_with`. The strikes term is non-obvious only
  here:
  ```rust
  let n = self.strikes.min(def.strike_cap);
  let r = def.stack_decay;
  // `r == 1.0` is the shipped default and must stay bit-identical to the
  // linear term; the closed form would also divide 0 by 0 there.
  let stacked = if r == 1.0 { n as f32 } else { (1.0 - r.powi(n as i32)) / (1.0 - r) };
  ```
  (`#[allow(clippy::float_cmp)]` is not needed if clippy doesn't flag it; if
  it does, compare against `1.0` with `>=`, since the census bounds `r` to
  at most 1.)
- [ ] Census in `tests/assets.rs`: `every_memory_stack_decay_is_in_range`,
  covering every shipped def with `0.0 < r <= 1.0`. Check it's not vacuous:
  temporarily set `0.0` on one def and watch the census fail, then restore
  the def (use a restore `trap` so a timeout can't leave it edited).
- [ ] README: add a table row, and change the intensity formula line to the
  geometric sum.
- [ ] `cargo test -p feral-processes-engine memories`, then commit.

### Task 1.2: `mood` and the two reads

**Files:** `memories.rs`, `game/memories.rs`, `systems.rs`,
`tests/memories.rs`, `tests/assets.rs`, `assets/memories/README.md`

**Interfaces — Produces:**
- `MemoryDef::mood: f32` (serde default `1.0`)
- `pub(crate) enum memories::Read { Opinion, Morale }`
- `sum_intensity(store, db, now, felt_as, read: Read, keep)`: `Morale`
  multiplies each record's felt intensity by `def.mood`, and `Opinion`
  doesn't.
- `Game::memory_sum(who, read, keep)`

- [ ] Failing tests:
  - `mood_scales_morale_and_leaves_opinion_whole`: one `Program`-subject
    memory on a def with `mood: 0.5`. `morale` is half of `opinion_of` for
    that subject.
  - `the_memories_page_rows_sum_to_its_morale`: with a `mood < 1` def held,
    the sum of `memory_report` row intensities equals `morale`.
  - `eviction_ignores_mood`: two memories of equal raw intensity, one on a
    `mood: 0.0` def. Fill past `MEMORY_CAP_PER_PROGRAM`, and the `mood: 0`
    one is not preferentially dropped. Ties break by insertion order, so put
    the `mood: 0` one first; with the bug it would weigh 0 and go.
  - `mining_morale_is_game_morale`: skip this if a parity test already
    exists (`rg -n "morale" crates/engine/src/tests/chains.rs`). Otherwise,
    for a posted worker holding a `mood: 0.5` memory, the
    `CycleModifiers::morale` a cycle sees equals `Game::morale`. If
    `CycleModifiers` can't be observed from a test, assert through
    `mining_success_chance`'s output against a hand-computed expectation.
- [ ] Implement it. `morale` passes `Morale`, `opinion_of` passes `Opinion`,
  `memory_report` passes `Morale` (row intensity = felt × mood), and
  `systems.rs` passes `Morale`. **`evict` is not touched.** Fix its doc
  comment only if it now claims something false.
- [ ] Census `every_memory_mood_is_in_range_and_read_somewhere`: every def
  has `0.0 <= mood <= 1.0`, and a def with `mood == 0.0` must have a subject
  in `OPINION_READ_SUBJECTS = [Program, BaseTile]`. Give that constant a doc
  comment naming its three readers (`base/tantrum.rs`, `base/morale.rs`,
  `base/work_orders.rs`) and saying a new `opinion_of` reader must extend
  it. Check it's not vacuous, as in 1.1.
- [ ] README row for `mood`.
- [ ] `cargo test -p feral-processes-engine memories` and `balance_sim`,
  then commit.

### Phase 1 gate

- [ ] `cargo fmt && cargo clippy --workspace --all-targets && cargo test --workspace`,
  with every exit code checked (cargo's status is lost through a pipe).
- [ ] Commit. Phase 1 can merge on its own.

---

## Phase 2: Handles

Deliverable: `creature_name`/`creature_label` produce handles. Engine
correct; some engine, app-core and gui tests will fail on names, and Phase 3
fixes them. **Phase 2 is not merged alone.**

### Task 2.1: `handles::of`

**Files:** create `crates/engine/src/handles.rs`, register in `lib.rs`; tests
inline in the module (`#[cfg(test)]`, the `memories.rs` pattern).

**Interfaces — Produces:** `pub fn of(id: ProgramId) -> String`: always
`"0x"` + six hex digits.

- [ ] Failing tests:
  - `handles_are_pinned`: exact strings for ids `0`, `1`, `2`, `4095` and
    `16_000_000`. Write the strings in after the implementation first runs,
    then **mutate the salt and watch the test fail**. The assertion message
    says: changing the salt or the permutation renames every program in
    every save.
  - `handles_are_distinct`: ids `0..100_000`, lowercased, all distinct.
  - `handle_shape`: length 8, `0x` prefix, digits are hex, and uppercase
    appears only on `a`–`f`.
  - `case_is_mixed_somewhere`: across the first 1000 ids, both cases occur,
    so a case hash that's secretly constant fails.
- [ ] Implement it. The only non-obvious part is that the digit permutation
  **must be invertible on 24 bits**. Use two rounds of
  `x = (x * ODD) & MASK; x ^= x >> 12`, with `ODD` an odd constant and
  `MASK = 0xFF_FFFF`. Multiplying by an odd number and xor-shifting are both
  bijections mod 2^24. Take the case bits from a separate FNV-1a fold of the
  id (`derive`'s fold if it's reusable), and upper-case letter digit `i`
  when bit `i` is set. Document the ≥ 2^24 wrap as unguarded, per the spec.
- [ ] `cargo test -p feral-processes-engine handles`, then commit.

### Task 2.2: The naming ladder

**Files:** `game/party.rs`; tests in `tests/party.rs` (fixtures from
`tests/support.rs`)

**Interfaces:**
- Consumes: `handles::of`
- Produces: `creature_name` (short form: `CustomName` › handle › species);
  `creature_label` (long form: tier + name + species when the name is a
  handle + zone tag). No new public function unless Task 3.1's census shows a
  surface needs a short *label* (tier + handle + zone, no species). If it
  does, add `Game::creature_short_label` there, not here.

- [ ] Failing tests:
  - `a_tamed_program_is_named_by_its_handle`
  - `a_wild_creature_and_a_summon_keep_their_species_name`
  - `a_custom_name_outranks_the_handle_and_drops_the_species_from_the_label`
  - `a_fused_child_has_a_handle_neither_parent_had`
  - `a_label_carries_the_species_after_a_handle`: exact string built from
    `handles::of`, the species name and the zone tag.
  - `a_program_from_a_pre_handle_save_reads_a_handle`: save → load round trip
    of a tamed program; its name equals `handles::of` of its restored id. Use
    a `dev-saves/` template or the existing save/load fixture in
    `tests/support.rs`.
- [ ] Implement it and run the new tests. Then run `cargo test -p feral-processes-engine`
  and **write the list of failing tests to the scratchpad**. Don't fix them
  here; they're Task 3.2's input. Commit, noting in the message that the
  engine suite is red on names.

---

## Phase 3: Surfaces and the suite

Deliverable: every name surface reads right, the full workspace is green,
and the seam is written down. Phases 2 and 3 merge together.

### Task 3.1: Short or long, per call site

**Files:** the ~110 call sites
(`rg -n "creature_name\(|creature_label\(" crates`)

- [ ] Sort each site into three groups:
  - **Long:** logs, the roster, the manifest header, popups with wrapping
    rows. These keep `creature_label`.
  - **Short:** anything drawn into a fixed cell. The known case is the party
    battle roster `NAME_W = 18` (`gui/src/render/battle.rs:61`). The
    status-column rows (38.5 cells) are the other suspects.
  - **Species on purpose:** anything whose meaning is the species (for
    example, telemetry and `balance`/arena reports). These read the species
    directly.
- [ ] For each short or species-on-purpose site, change the call and add a
  test only where the width is actually at risk. Measure it with
  `paint::with_painter` in a gui test on the widest realistic label:
  Overclocked tier, a handle and the longest shipped species name, at zone
  two digits.
- [ ] Write the census result as a short table in the commit message (site →
  form → reason), not in a doc file. Commit.

### Task 3.2: Suite repair

**Files:** tests from the Task 2.2 list, plus app-core and gui failures

- [ ] `cargo test --workspace`, collecting failures. For each one:
  - If it asserts *which species*, read the species instead.
  - If it asserts displayed text, build the expectation from `handles::of`
    on the program's `ProgramId`.
  - **Never paste a literal hex string** (only `handles_are_pinned` does).
- [ ] If a failure isn't a name assertion, stop and diagnose it; don't adapt
  it. `tests::creation` in app-core is a known unrelated flake; re-run that
  module rather than fixing it.
- [ ] Commit in batches per crate.

### Task 3.3: Write the seam down

- [ ] Memory graph: `seam:a-handle-is-derived-and-its-salt-is-save-format`,
  holding the argument (stored → derived, no pool, salt = rename).
- [ ] `.claude/skills/seams/`: the trap (nothing fails to compile when the
  salt moves; the pinned test is the only barrier).
- [ ] `CLAUDE.md`: one sentence under **Species and data**: "**A program's
  handle is derived from its `ProgramId` by `handles::of`, never stored, and
  the permutation's salt is save format.**" Also one sentence under **What a
  program remembers**: "**`mood` splits one store into two reads, and `evict`
  reads neither.**"
- [ ] Commit.

### Phase 3 gate (branch done)

- [ ] `cargo fmt`, `cargo clippy --workspace --all-targets`,
  `cargo test --workspace`, and `balance_sim`, with exit codes checked.
- [ ] Whole-branch review: one opus agent, diff handed over as a file, told
  to check the spec's Testing section and Global Constraints line by line.
- [ ] Tell the user plainly that no name has been seen on a screen. The
  battle roster and status column are the two to look at in play.
- [ ] Landing (version bump, CHANGELOG section, tag) goes through the
  `deploy` skill, and only when the user asks.
