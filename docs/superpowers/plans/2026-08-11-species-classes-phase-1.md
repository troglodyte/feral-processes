# Plan: species classes, phase 1 — `base_int` and the manifest WORK box

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:executing-plans`
> or `superpowers:subagent-driven-development`. Steps are checkboxes.

Spec: `docs/superpowers/specs/2026-08-10-species-classes-design.md`, phase 1 of
eight. Phases 2-8 are explicitly **out of scope** — no `base_speed` work-rate
change, no stat shapes, no kits, no class base jobs.

**Goal:** give a species a work-relevant stat that is not confounded with its
tier, so *which* program you post to a Mining Node starts to matter.

**Architecture:** `SpeciesDef` gains `base_int`, read by
`systems::mining_success_chance` as a **deviation from the roster baseline**.
That is the load-bearing choice: an omitted field, a mod's species, and the
player all sit at `DEFAULT_BASE_INT`, where the deviation is zero and the
formula returns exactly what it returns today. The manifest gains a WORK box to
show it, which is also what keeps the SPECIES box off its row cap.

**Crates touched:** engine (schema, formula, tuning, census), gui (manifest box
+ layout fixture), assets (17 species files + README). No app-core change.

## Global constraints

- `base_int` is `#[serde(default = "default_base_int")]` — an existing or
  modded `.ron` without the field must keep parsing. (CLAUDE.md, moddability.)
- **No `SAVE_FORMAT_VERSION` bump.** `base_int` lives on `SpeciesDef`, not on
  `Stats`; nothing about it is serialised into a save. If a task finds itself
  reaching for a save bump, the design has drifted — stop and re-read the spec's
  "Two work dials" section.
- Player-facing name for the stat is **"Analysis"**. `base_int` is the field
  name; the two never need to match, and the manifest row says Analysis.
- Do not touch `assets/species/*.ron` stats other than adding `base_int`. Stat
  shapes are phase 3 and are gated by `balance_sim`; this phase is not.
- `docs/manual.md` and root `README.md` are carved out of the doc obligation.
  `assets/species/README.md` and `CHANGELOG.md` are **not**.
- Version bump, tag, merge and push all need an explicit ask. Do none of them.

## What this phase is *not* gated by

`balance_sim.rs` models no mining, extraction or node payout — verified by
grepping it for `mining`/`gather`/`node_payout`, which returns nothing. So the
balance regression gate is genuinely untouched here, and a green `balance_sim`
is **not** evidence this phase is correct. The evidence is the census test in
Task 4 and playing `--template extraction`.

---

## Task 1: the schema field

**Files:**
- Modify: `crates/engine/src/species.rs` (`SpeciesDef`, beside `base_speed`;
  new `default_base_int()` next to `default_base_speed()` at ~line 258)
- Modify: `crates/engine/src/tuning.rs` (beside `DEFAULT_BASE_SPEED`, ~line 261)
- Test: `crates/engine/src/species.rs` tests, or wherever `SpeciesDb::load_dir`
  parsing is already covered — find the existing test first, don't add a module.

**Produces:** `SpeciesDef::base_int: i32`; `tuning::DEFAULT_BASE_INT: i32 = 10`;
`tuning::MINING_SUCCESS_PER_INT: f64 = 0.02`.

`DEFAULT_BASE_INT` is deliberately one constant, not the `DEFAULT_BASE_SPEED` /
`PLAYER_BASE_SPEED` pair. Those two differ (10 and 11) and so earn separate
names; here the player is *exactly* the roster average by decision, and one
constant read from both sides is what says so.

- [ ] **Step 1** — Write the failing test: a species `.ron` that omits
      `base_int` loads at `DEFAULT_BASE_INT`, and one that declares it loads
      that value. Use the existing modded-assets fixture directory pattern
      rather than editing a shipped file.
- [ ] **Step 2** — Run it. Expect a compile failure on the unknown field.
- [ ] **Step 3** — Add the field, the `default_base_int()` fn and the two
      tuning constants. Doc-comment `base_int` in the house style: what it
      governs, that it is a *deviation* from the baseline, and why it is not on
      `Stats` (it would grow on level-up and re-confound role with tier — the
      spec's argument, stated in one sentence, not copied wholesale).
- [ ] **Step 4** — Run the test. Expect pass. Run `cargo test -p
      feral-processes-engine species`.
- [ ] **Step 5** — Commit.

## Task 2: the formula

**Files:**
- Modify: `crates/engine/src/systems.rs:139-144` (`mining_success_chance`),
  `:175-200` (`resolve_gather_cycle` signature), `:445` and `:546` (the two
  call sites), `:326-334` (`CronjobWorker` needs nothing new — `Creature` is
  already in the tuple and `SpeciesDb` is already in `CronjobLookups:342-347`)
- Test: `crates/engine/src/systems.rs` tests, beside
  `mining_success_chance_rises_with_level_and_caps_at_one:1076` and
  `keen_scavenger_adds_to_the_mining_roll_and_still_caps_at_one:1094`

**Consumes:** `DEFAULT_BASE_INT`, `MINING_SUCCESS_PER_INT` from Task 1.
**Produces:** `mining_success_chance(level: u32, keen_scavenger_level: u32,
base_int: i32) -> f64`, and `resolve_gather_cycle(..)` taking `base_int: i32`
in the same position relative to `keen_scavenger_level`.

The term is `(base_int - DEFAULT_BASE_INT) as f64 * MINING_SUCCESS_PER_INT`,
inside the existing `.min(1.0)`. Two properties follow and both need a test:
a baseline species reproduces today's number **exactly** (not approximately),
and the roll still cannot exceed a sure thing. Note the floor: a dull enough
species could in principle drive the chance negative — `random_bool` panics
outside 0..=1, so clamp both ends, and test the low end rather than assuming
the shipped roster never reaches it.

`player_gather_system:508` has no species and passes `DEFAULT_BASE_INT`. Put
the *reason* at that call site in a comment — the player is average by
decision, so posting a sharp program beats doing it yourself and posting a dull
one is worse. That two-sided pressure is the whole point of the phase and is
invisible from the code alone.

- [ ] **Step 1** — Write three failing tests: (a) a baseline `base_int` returns
      exactly what the two-argument formula returned, asserted against the
      literal arithmetic on the tuning constants rather than against a magic
      number; (b) each point of `base_int` above baseline adds exactly
      `MINING_SUCCESS_PER_INT`, mirroring the existing keen-scavenger test's
      shape; (c) an absurdly low and an absurdly high `base_int` clamp to 0.0
      and 1.0 rather than escaping the range.
- [ ] **Step 2** — Run them. Expect failures on arity.
- [ ] **Step 3** — Widen both signatures and thread the value through. The
      cronjob site reads the worker's `Creature` species out of
      `lookups.species`; a species missing from the db takes `DEFAULT_BASE_INT`,
      matching how `node_is_flat_payout:153` already treats a hand-spawned
      fixture whose kind isn't in the db.
- [ ] **Step 4** — Run `cargo test -p feral-processes-engine`. Expect pass.
- [ ] **Step 5** — **Mutation experiment** (required — see memory: mutation
      experiments run on every task, not just fix rounds). Force the deviation
      term to a constant zero and confirm test (b) fails. Restore from a
      scratchpad copy, never `git checkout`. Assert the mutation actually
      applied before believing the result.
- [ ] **Step 6** — Commit.

## Task 3: it changes play

A formula test can pass while nothing reaches the player. This task is the one
that proves a posted program's identity now moves the base's output.

**Files:**
- Test: `crates/engine/src/tests/chains.rs` (production//cronjob fixtures live
  here) — check `crates/engine/src/tests/support.rs` first for `work_node_parts`
  and `park_at_post`, both of which this fixture needs. A node short of `Stock`
  or `MachineStatus` is silently skipped; a worker left where it spawned never
  reaches its station. Both read as a payout curve that moved.

- [ ] **Step 1** — Write the failing test: two identical Mining Nodes, one
      posted with a high-`base_int` species and one with a low, run for a fixed
      number of ticks under a seeded `GameRng`. Assert the sharp worker's
      output strictly exceeds the dull one's. Use modded species files so the
      test does not re-break when Task 4 retunes the shipped roster.
- [ ] **Step 2** — Run it. Expect failure (equal output) if Task 2 somehow did
      not reach this path — which is exactly what is being checked.
- [ ] **Step 3** — No implementation. If it fails, Task 2 is wrong; fix there.
- [ ] **Step 4** — Run `cargo test -p feral-processes-engine chains`.
- [ ] **Step 5** — Commit.

## Task 4: author the roster

**Files:**
- Modify: all 17 of `assets/species/*.ron`
- Modify: `assets/species/README.md` (document `base_int` beside `base_speed`
  at `:37-47`, same voice — what it does, the shipped range, that omitting it
  puts you at the average)
- Modify: `assets/species/README.md:81` — the spec records this line as
  describing a cronjob-assignability gate on `work_resource` that
  `Game::accepts_a_program` does not implement. Verify that against the code,
  then fix or delete the claim. This is the phase that touches the file.
- Test: `crates/engine/src/species.rs` census tests (where
  `the_shipped_roster_has_species_on_both_sides_of_the_opening_ring` and
  `base_roster_growth_multiplier_rises_with_difficulty_tier` live)

Proposed values, decorrelated from tier on purpose. Scale mirrors `base_speed`
(shipped range 6-14, baseline 10):

| species | gm | base_int | | species | gm | base_int |
|---|---|---|---|---|---|---|
| glitch | 1.0 | 5 | | zero_day | 1.5 | 12 |
| drone | 1.0 | 7 | | virus | 1.5 | 12 |
| sprite | 1.0 | 11 | | construct | 1.5 | 5 |
| sub_process | 1.0 | 14 | | sentinel | 1.5 | 8 |
| scrapper | 1.25 | 7 | | rootkit | 1.5 | 13 |
| crawler | 1.25 | 8 | | cipher | 1.5 | 14 |
| worm | 1.25 | 11 | | overseer | 2.0 boss | 16 |
| proxy | 1.25 | 13 | | wintermute | 2.0 boss | 18 |
| trojan | 1.25 | 13 | | | | |

The two bosses are unpostable, so their values are flavour only and must not be
allowed to carry the census below.

- [ ] **Step 1** — Write the failing census test. The property that matters is
      **not** "INT is uncorrelated with tier" — that is fragile to a one-point
      retune. Assert the readable thing instead: every non-boss growth band
      contains at least one species above and one below the non-boss roster
      mean, and the highest-INT non-boss species is not in the highest growth
      band. Both fail today (no field values at all) and both fail if a later
      retune quietly re-aligns INT with the ladder.
- [ ] **Step 2** — Run it. Expect failure — every species is at the default, so
      no band spans anything.
- [ ] **Step 3** — Add `base_int` to the 17 files and write the README section.
- [ ] **Step 4** — Run `cargo test -p feral-processes-engine`. Expect pass.
      Then `cargo test -p feral-processes-engine balance_sim` and record that
      the curves did **not** move — if they did, something outside this phase's
      stated scope changed and needs explaining, not retuning.
- [ ] **Step 5** — **Mutation experiment**: set two species' `base_int` equal to
      the ladder order and confirm the census fails. Restore from scratchpad.
- [ ] **Step 6** — Commit.

## Task 5: the WORK box

**Files:**
- Modify: `crates/gui/src/render/manifest.rs:464-470` — move the Speed row out
  of the SPECIES box and emit a new WORK box holding Speed and Analysis
- Modify: `crates/gui/src/render/manifest_layout.rs:336,493,498` — the fixture
  lists, `section("SPECIES", 6, ..)` → `5`, plus the new WORK entry
- Modify: `crates/engine/src/views.rs` — `ProgramManifest` needs `base_int`
  beside its existing `base_speed`

**The fixture edit lands in the same commit as the renderer edit, and the
fixture is order-sensitive** (memory: the packer's fixtures must match
`sections_for`'s emission *order*, not just row counts — a drifted fixture
once hid a live overflow behind a green suite). Read `sections_for:280` and
`program_sections:402` and place WORK where it is actually emitted; do not
guess from the fixture's current order.

SPECIES goes 6 → 5, which is the point: it sat exactly at `MAX_SECTION_ROWS`,
where a 7th row silently truncates to "+N more" and the data just vanishes.

- [ ] **Step 1** — Write the failing test: for a program view, `sections_for`
      emits a WORK box containing both a Speed row and an Analysis row, and the
      SPECIES box no longer contains Speed and is at most 5 rows. Assert on the
      emitted sections, not on pixels.
- [ ] **Step 2** — Run it. Expect failure — no WORK box exists.
- [ ] **Step 3** — Add `base_int` to `ProgramManifest`, populate it where
      `base_speed` is populated, and make the renderer change.
- [ ] **Step 4** — Run `cargo test -p feral-processes-gui`. Expect pass,
      including the existing layout-overflow tests.
- [ ] **Step 5** — Commit.

## Task 6: gates and handover

- [ ] **Step 1** — `cargo fmt` and `cargo clippy --workspace`; fix warnings
      rather than silencing them.
- [ ] **Step 2** — `cargo test --workspace`. Baseline on this branch before any
      change was **1852 passing, 0 failures** — the new total should be that
      plus the tests this plan adds, with nothing lost.
- [ ] **Step 3** — Add a `CHANGELOG.md` entry. Do **not** bump the workspace
      version or tag; that happens at the merge and needs an explicit ask.
- [ ] **Step 4** — Update `CLAUDE.md`'s load-bearing-seams list with the
      baseline-deviation rule, since "an omitted `base_int` reproduces the old
      number exactly" is precisely the kind of fact that costs tool calls to
      rediscover. Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins
      with no tracking to catch drift.
- [ ] **Step 5** — **A green suite is not evidence of play.** Say so, and offer
      `cargo run -- --template extraction`, which is the template that stands up
      a worked node. The thing to feel is whether swapping the posted program
      reads as a difference or as noise; `MINING_SUCCESS_PER_INT` is the knob if
      it reads as noise.

---

## Self-review against the spec

- Spec's phase 1 is "`base_int` + the manifest WORK box" — Tasks 1-5 cover both
  halves, Task 6 is gates.
- Spec's correction list: the `assets/species/README.md:81` claim is in Task 4.
  The other correction (`game/combat_rewards.rs:56-62` missing
  `grant_nest_cache` as a `work_resource` reader) belongs to a phase that
  touches that file; phase 1 does not, so it is deliberately **not** here.
- Spec's manifest warning (SPECIES at exactly `MAX_SECTION_ROWS`, fixture and
  renderer in one commit) is Task 5.
- Spec's `balance_sim` warning concerns the *stat shapes* in phase 3, not this
  phase; the "not gated by" section above records why it does not bite yet.
- Out of scope and confirmed absent: `base_speed` as work rate, `generic_species`
  relocation, kit authoring, class base jobs, construct's tier move.
