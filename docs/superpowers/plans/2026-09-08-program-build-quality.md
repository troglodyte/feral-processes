# Program build quality — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to work this plan task by task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A program's two new build rolls decide how fast the machine it was
spent on runs, in both directions, and the build picker quotes that in ticks
before the player commits.

**Architecture:** Two rolls join `components::Potential`; `build_quality`
folds the applicable one with a small rarity lift into a single figure; that
figure is stored on the raised structure as `BuildQuality` and reaches the
one cycle formula, `systems::work_ticks_at_speed`, as a fourth argument.
The picker's preview is a *call* into that same formula through
`Game::build_candidates`, never a percentage re-derived in a view.

**Tech Stack:** Rust, `bevy_ecs` 0.19, serde/RON saves, `bevy_egui` renderer.
Four crates: `engine`, `app-core`, `gui`, `launcher`.

**Spec:** [`docs/superpowers/specs/2026-09-08-program-build-quality-design.md`](../specs/2026-09-08-program-build-quality-design.md)
— read it alongside this plan. Every "why" lives there; this file is the "what
and in what order".

## Global constraints

- **Process weight.** Per `CLAUDE.md`, this plan states interfaces, test
  intent and gates. It deliberately does **not** contain finished
  implementations for a worker to re-emit. Code blocks appear only where the
  shape is genuinely non-obvious.
- **TDD at every size.** Failing test first, minimal implementation, green,
  commit. Each step must fail with the change removed.
- **No `SAVE_FORMAT_VERSION` bump.** All three persisted fields are additive
  field-named RON (`save.rs:539`).
- **Every one of the three new save fields uses `#[serde(default = "…")]`
  returning `1.0`.** A bare `#[serde(default)]` on an `f32` yields `0.0`,
  which is silent and catastrophic — see spec §5.
- **Named constants only.** New tuning values go in
  `crates/engine/src/tuning.rs` with a doc comment carrying the argument.
- **Moddability.** Nothing here adds an asset field, so no
  `assets/*/README.md` change is owed.
- **Player vocabulary.** `assembly_roll` draws as **"Assembly"**,
  `extraction_roll` as **"Extraction"**. No abbreviation anywhere player-
  facing, and no third word.
- **Gates.** Every task ends with `cargo fmt`, `cargo clippy --workspace`
  (fix warnings, do not silence), and the task's own targeted tests. The full
  `cargo test --workspace` gate runs at the end of Task 1 (the RNG
  re-baseline) and again at the end of Task 10.
- **`cargo test -p feral-processes-engine balance_sim` must report the curves
  have NOT moved.** Nothing here should touch them; a moved curve is a bug in
  this work, not a retune.
- **Agents cannot play this.** No task may claim the feature works on a
  screen. Say plainly, once, at the end, that nothing here has been seen.

## Two decisions taken after the spec

The spec left two things under-specified. Both were settled with the owner on
2026-09-08 and are binding:

1. **The picker re-sorts.** Candidates are ordered best-first by the roll the
   pending structure actually reads, and the role headings
   (`companion_page_rows`' "Base staff" / "In your party" runs) do **not**
   survive that sort — a sorted list interleaves roles, so a heading emitted
   on the change would fire repeatedly and mean nothing. The warning those
   headings carried is kept by `PetInfo::activity`, which is already one of
   the row's tags and is asserted in Task 9.
2. **The roster row gains two separate tags**, ` [Assembly: Excellent]` and
   ` [Extraction: Poor]`, so `wrapped_row_lines` can shed one without the
   other.

## File map

| File | Responsibility after this change |
|---|---|
| `crates/engine/src/components.rs` | `Potential` +2 rolls, `roll_label`; new `BuildQuality` component |
| `crates/engine/src/tuning.rs` | `BUILD_QUALITY_PER_RARITY_RUNG`, `BUILD_QUALITY_TICK_WEIGHT` |
| `crates/engine/src/game/spawning.rs` | `roll_potential` rolls six |
| `crates/engine/src/structures.rs` | `cycle_ticks(def) -> Option<u32>` |
| `crates/engine/src/systems.rs` | `work_ticks_at_speed` fourth argument |
| `crates/engine/src/game/base/building.rs` | `build_quality*`, `spawn_structure` parameter, `work_ticks_for`, `cycle_ticks_for` |
| `crates/engine/src/game/base/construction.rs` | both `raise_one_tick` arms write the figure |
| `crates/engine/src/save.rs` | `CreatureSave` +2 rolls, `StructureSave::build_quality`, `CreatureSave::potential()` |
| `crates/engine/src/game/lifecycle.rs` | save write + restore for all three fields |
| `crates/engine/src/views.rs` | `BuildEffect`, `BuildCandidate`, `ManifestPotential` +2, `PetInfo` +2 |
| `crates/engine/src/game/party.rs` | `build_candidates`, `build_roll_labels`, `PetInfo` construction |
| `crates/engine/src/game/catalog.rs` | `structure_kind(entity)` |
| `crates/app-core/src/app/building.rs` | `pending_build_kind`, handler indexes `build_candidates` |
| `crates/gui/src/render/manifest.rs` | two POTENTIAL rows |
| `crates/gui/src/render/manifest_layout.rs` | `MAX_POTENTIAL_ROWS`, worst-case fixture |
| `crates/gui/src/render/party.rs` | roster row's two tags; heading-free row emission |
| `crates/gui/src/render/building.rs` | picker rows: aptitude tag, cycle quote, the no-cycle line |

---

## Task 1: Two rolls on `Potential`

**Files:**
- Modify: `crates/engine/src/components.rs:1393-1453` (`Potential`)
- Modify: `crates/engine/src/game/spawning.rs:574` (`roll_potential`)
- Modify: `crates/engine/src/game/lifecycle.rs:1421` (the restore literal)
- Modify: every other `Potential { … }` struct literal the compiler names
- Test: `crates/engine/src/components.rs` `mod tests` (in-file, beside the
  existing `averaged` tests at ~2530-2600)

**Interfaces produced:**

```rust
pub struct Potential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    pub assembly_roll: f32,
    pub extraction_roll: f32,
}

impl Potential {
    pub const NEUTRAL: Potential;           // both new rolls at 1.0
    pub fn roll_label(roll: f32) -> &'static str;
    pub fn quality_percent(&self) -> u32;   // UNCHANGED: the four combat rolls
    pub fn quality_label(&self) -> &'static str; // now calls roll_label
    pub fn averaged(a: Potential, b: Potential) -> Potential; // folds all six
}
```

`roll_label` maps a single roll onto the same five rungs `quality_label`
already speaks, using the same `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
→ 0..100 mapping `quality_percent` uses on the average. `quality_label` is
rewritten as `Potential::roll_label(average of the four combat rolls)` so the
five-rung ladder exists once. Do not leave the old `match` behind.

Doc comments on both new fields must carry the spec §1 argument: read at
exactly one moment, deliberately independent of the four combat rolls,
never applied to the program itself.

- [ ] **Step 1: Write the failing tests**

Four tests, in `components.rs`'s existing `mod tests`:

- `roll_label_walks_the_same_five_rungs_quality_label_does` — assert
  `roll_label(MIN_INDIVIDUAL_ROLL) == "Poor"`,
  `roll_label(MAX_INDIVIDUAL_ROLL) == "Excellent"`, and that a `Potential`
  whose four combat rolls are all `r` reports `quality_label() ==
  roll_label(r)` for `r` at each of the five rungs. This is the test that
  makes "one ladder, not two copies" real.
- `averaged_folds_all_six_rolls` — spec test 7. Two parents, **six distinct
  values each**, none equal to another (two fields holding the same number
  cannot catch being crossed — `save_roundtrip.rs`'s own rule). Assert all six
  averages, including a fusion of two Excellent builders staying an Excellent
  builder.
- `neutral_is_neutral_on_every_axis` — all six of `NEUTRAL` are `1.0`.
- `quality_percent_still_ignores_the_build_rolls` — a `Potential` with the
  four combat rolls at `MAX_INDIVIDUAL_ROLL` and both build rolls at
  `MIN_INDIVIDUAL_ROLL` still reports 100%. This is the test that pins
  "the tension is the feature".

And one in `crates/engine/src/tests/spawning.rs` (or wherever
`roll_potential` is already exercised — grep first, add there):

- `rolled_potential_puts_both_build_rolls_in_range` — spec test 8. Roll many
  potentials off a seeded `Game` and assert both new fields land inside
  `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`, and that they are **not** all
  identical across the sample (a field wired to a constant passes a range
  check).

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine potential` — expect compile failure
(unknown field), which is the correct failure here.

- [ ] **Step 3: Implement**

Add the fields, `roll_label`, the rewritten `quality_label`, the widened
`averaged` and `NEUTRAL`. Then `cargo check -p feral-processes-engine` and fix
every struct literal it names — `Potential` has no `Default` and no `..` in
its literals, so the compiler is the census here. Known sites:
`lifecycle.rs:1421`, `spawning.rs:574`, the `components.rs` test literals at
~2530-2600, and `crates/gui/src/render/test_support.rs`.

`roll_potential` draws two more `random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL)`
values, in field order after `growth_roll`.

- [ ] **Step 4: Green on the targeted tests**

- [ ] **Step 5: Re-baseline the seeded suite**

`cargo test --workspace`. **Expect failures.** `roll_potential` going from
four draws to six shifts the seeded RNG stream for every creature spawn — the
spec names this in §7 as a known event, not a bug to debug. For each failure:

1. Confirm it is a seed-luck failure (a test asserting a specific spawn, roll,
   or fight outcome) and **not** a real regression, by reading the assertion.
   `docs/measurements/` and the memory entry *"An RNG-stream shift exposes
   seed-luck tests"* describe the symptom.
2. Re-baseline by changing the **seed**, not the assertion, wherever the test
   is about a property rather than a number.
3. If an expected *number* has to move, say so explicitly in the commit
   message with the old and new values.

Do not touch `balance_sim` expectations — it is RNG-free. If a `balance_sim`
curve moves, stop and report: that is a real regression.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src crates/gui/src/render/test_support.rs
git commit -m "Potential carries two build rolls"
```

---

## Task 2: `CreatureSave` carries the two rolls

**Files:**
- Modify: `crates/engine/src/save.rs:379-388` (`CreatureSave`), plus a new
  `impl CreatureSave` block
- Modify: `crates/engine/src/game/lifecycle.rs:1421` (restore),
  `:1757` (write)
- Test: `crates/engine/src/tests/save_roundtrip.rs`

**Interfaces:**
- Consumes: `Potential` from Task 1.
- Produces:

```rust
pub struct CreatureSave {
    // …
    #[serde(default = "neutral_roll")] pub assembly_roll: f32,
    #[serde(default = "neutral_roll")] pub extraction_roll: f32,
}

/// `serde`'s default for an individual roll — the neutral 1.0, because a
/// bare `#[serde(default)]` on an `f32` is 0.0 and would load every program
/// in a pre-feature save as permanently the worst builder in the game.
fn neutral_roll() -> f32;

impl CreatureSave {
    /// The individual rolls this record carries, as the component.
    pub(crate) fn potential(&self) -> Potential;
}
```

`CreatureSave::potential()` is an extraction, not a new expression:
`lifecycle.rs:1421`'s literal moves into it and the restore calls it. Task 3
calls it too, which is the reason it exists — a second walk from a save to a
`Potential` is the copy that drifts.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/save_roundtrip.rs`:

- Extend the file's **exhaustive destructure census** with the two fields.
  That destructure carries no rest pattern, so the file already stops
  compiling until this is done — the work is writing the *value* assertions,
  and they must be two distinct numbers, distinct from every other f32 in the
  fixture.

In `crates/engine/src/tests/construction.rs`:

- `a_cancelled_order_gives_back_a_program_with_its_build_rolls` — spec test
  11. File a build with a program whose two build rolls are known
  non-neutral values, `cancel_build_request`, and assert the restored roster
  program's `Potential` carries both back unchanged.

In `crates/engine/src/tests/save_roundtrip.rs` (a new test, not the census):

- `a_save_written_without_the_build_rolls_loads_them_neutral` — spec test 10.
  **Vacuous if written against a save this build wrote**, because `Game::save`
  always writes the keys. Build it by saving to a scratch path
  (`support::scratch_assets_dir`), reading the file as text, deleting every
  `assembly_roll:` and `extraction_roll:` line, writing it back, then
  `Game::load`. Assert every roster program reports `1.0` on both, and assert
  the file really lacked the keys before the load (or the test proves
  nothing).

- [ ] **Step 2: Run and watch fail** — `cargo test -p feral-processes-engine save_roundtrip construction`

- [ ] **Step 3: Implement** — the two fields, `neutral_roll`,
  `CreatureSave::potential()`, the restore switched to call it, and the write
  at `lifecycle.rs:1757` reading `potential.assembly_roll` /
  `potential.extraction_roll`.

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src
git commit -m "A program's build rolls survive a save"
```

---

## Task 3: `build_quality` and its two constants

**Files:**
- Modify: `crates/engine/src/tuning.rs`
- Modify: `crates/engine/src/structures.rs` (add `cycle_ticks`)
- Modify: `crates/engine/src/game/base/building.rs` (module-level functions
  beside the `impl Game` block)
- Test: `crates/engine/src/tests/construction.rs`

**Interfaces:**
- Consumes: `Potential` (Task 1), `CreatureSave::potential()` (Task 2).
- Produces:

```rust
// crates/engine/src/tuning.rs
pub const BUILD_QUALITY_PER_RARITY_RUNG: f32 = 0.03;
pub const BUILD_QUALITY_TICK_WEIGHT: f64 = 0.5;

// crates/engine/src/structures.rs
/// The cycle this structure ships, or `None` for one that runs no cycle at
/// all. The `(work, assembles)` split `Game::work_ticks_for` already makes,
/// named once so the picker's preview and the real rate cannot disagree
/// about which structures even have a rate.
pub(crate) fn cycle_ticks(def: &StructureDef) -> Option<u32>;

// crates/engine/src/game/base/building.rs, module level
pub(crate) fn build_quality(def: &StructureDef, potential: Potential, rarity: Rarity) -> f32;
pub(crate) fn build_quality_of(def: &StructureDef, program: &CreatureSave) -> f32;

impl Game {
    /// `build_quality` for a live roster entity — the picker's half, where
    /// no `CreatureSave` has been taken yet.
    pub(crate) fn build_quality_for(&self, def: &StructureDef, program: Entity) -> f32;
}
```

Three entry points and **one formula**: `build_quality` holds it,
`build_quality_of` reads `program.potential()` and `program.rarity`, and
`build_quality_for` reads `Potential`/`Rarity` off the entity, each absent
component falling to `Potential::NEUTRAL` / `Rarity::Ordinary`. Do not write
the roll selection or the rarity lift anywhere but `build_quality`.

The rule inside it, verbatim from spec §2:

```text
roll    = if def.work.is_some() { extraction_roll } else { assembly_roll }
quality = roll + rarity.rank() as f32 * BUILD_QUALITY_PER_RARITY_RUNG
```

The `else` is deliberate and must not become a three-armed `Option` — see
spec §2's closing paragraph and §4.

`cycle_ticks` returns `work.ticks_per_unit`, else `assembles.ticks_per_unit`,
else `None`. Rework `Game::work_ticks_for`'s inner `match` to
`cycle_ticks(def).unwrap_or(5)` so the two cannot drift; the `5` fallback and
the missing-def `5` stay exactly as they are.

Both constants' doc comments carry the spec's argument: the rarity lift is
small for `GRADE_PER_RARITY_RUNG`'s reason, and the tick weight is
deliberately half of `WORK_TICKS_PER_SPEED`'s worth because the two multiply.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/construction.rs`:

- `build_quality_reads_extraction_for_a_node_and_assembly_for_a_bench` —
  spec test 1. One program with deliberately opposite rolls (Excellent
  assembly, Poor extraction), asked about `mining_node` (`work`) and about a
  shipped `assembles` structure. Assert the two answers straddle 1.0 in
  opposite directions.
- `rarity_lifts_a_build_by_exactly_one_rung_per_rung` — spec test 2. Same
  rolls at `Ordinary` and at one rung up; assert the difference is exactly
  `BUILD_QUALITY_PER_RARITY_RUNG`. Then a Prismatic program with a Poor roll:
  assert the result is still **below 1.0** — rarity insures, it does not
  rescue.
- `a_structure_with_no_cycle_has_no_shipped_ticks` — `cycle_ticks` returns
  `None` for `depot` and `Some` for `mining_node`, off the real shipped defs.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src
git commit -m "What a program is worth to the machine it becomes"
```

---

## Task 4: `BuildQuality`, the formula, and the two write sites

**Files:**
- Modify: `crates/engine/src/components.rs` (new component)
- Modify: `crates/engine/src/systems.rs:308` (`work_ticks_at_speed`)
- Modify: `crates/engine/src/game/base/building.rs:225` (Home),
  `:410` (`spawn_structure`), `:1072` (`work_ticks_for`)
- Modify: `crates/engine/src/game/base/construction.rs:467-545` (`raise_one_tick`)
- Modify: `crates/engine/src/tests/support.rs:880` (`spawn_structure_at`)
- Test: `crates/engine/src/tests/construction.rs`,
  `crates/engine/src/systems.rs` `mod tests` (the four existing
  `work_ticks_at_speed` assertions at `:2549-2572`)

**Interfaces:**
- Consumes: Task 3's `build_quality_of`, `cycle_ticks`.
- Produces:

```rust
// components.rs
#[derive(Component, Clone, Copy, Debug)]
pub struct BuildQuality(pub f32);

// systems.rs
pub(crate) fn work_ticks_at_speed(
    base_ticks: u32,
    speed: i32,
    class_scale: f64,
    build_quality: f64,
) -> u32;

// game/base/building.rs
pub(crate) fn spawn_structure(
    &mut self,
    def: &StructureDef,
    x: i32,
    y: i32,
    quality: Option<f32>,
) -> Entity;

impl Game {
    /// One cycle of `def` for a worker of `worker_speed`, at `quality`.
    /// `None` for a structure that runs no cycle. The single derivation the
    /// live rate and the picker's preview both call.
    pub(crate) fn cycle_ticks_for(
        &self,
        def: &StructureDef,
        quality: f32,
        worker_speed: i32,
    ) -> Option<u32>;
}
```

**The fourth argument is the raw quality, not a pre-computed scale.** The
spec's `build_scale = 1.0 - (quality - 1.0) * BUILD_QUALITY_TICK_WEIGHT`
belongs *inside* `work_ticks_at_speed`, for the reason its own doc comment
already gives about `class_scale`: a scale computed at the caller is a second
expression of the formula, and there are two callers. The existing
`.max(1.0)` floor covers the new term with no change.

`work_ticks_at_speed`'s doc comment gains a paragraph on the new argument:
whose it is (the *builder's*, baked in at the moment the machine was raised),
how it differs from `speed` (the *posted worker's*), and that the two multiply
— `systems::CycleModifiers`' "whose is this" discipline.

`BuildQuality`'s doc comment must state **absent means 1.0**, and name the
three cases that rely on it (the Home, test fixtures, every pre-feature save),
per spec §3.

Write sites:
- `building.rs:225` (founding the Home) passes `None`.
- `construction.rs`'s `BuildGoal::New` arm passes
  `Some(build_quality_of(&def, program))` where `program` is read off the
  `BuildSite` — extend the existing
  `.get::<BuildSite>(site).map(|b| (b.structure.clone(), b.goal))` tuple to
  carry `b.program.clone()` as well, and read it **before** `consume_site`.
- `construction.rs`'s `BuildGoal::Upgrade` arm inserts
  `BuildQuality(build_quality_of(&def, program))` on the resolved machine,
  beside the `StructureTier(to_tier)` insert. It **overwrites**; it does not
  average and does not keep the better.
- A site with `program: None` (the Home, a hand-spawned fixture) writes
  nothing at all — no component.

`work_ticks_for` reads `BuildQuality` off the structure entity it already
holds, `map_or(1.0, |q| q.0)`, and delegates to `cycle_ticks_for`.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/systems.rs` `mod tests`, widen the four existing
assertions with a neutral `1.0` fourth argument, then add:

- `a_build_quality_scales_a_cycle_the_way_speed_does_but_half_as_hard` —
  at `DEFAULT_BASE_SPEED`, a quality of 1.2 gives fewer ticks than 1.0, which
  gives fewer than 0.8, and the 1.2 figure matches the spec §3 worked table
  (a 20-tick machine at 0.90x). This is where `BUILD_QUALITY_TICK_WEIGHT`'s
  value is pinned.

In `crates/engine/src/tests/construction.rs`:

- `an_excellent_builder_leaves_a_faster_machine_than_a_poor_one` — spec test
  3. Two runs of the same fixture, same def, same worker speed, differing only
  in the committed program's `assembly_roll`/`extraction_roll`. Raise the
  structure to completion with the existing
  `for _ in 0..400 { if structure_at(…).is_some() { break } game.tick(); }`
  idiom, then compare the two machines' real cycle length. Assert strictly
  fewer ticks, and assert the Poor one is strictly *more* than shipped — the
  feature is two-sided, and a one-sided test passes against a bonus-only
  implementation.
- `a_machine_with_no_build_quality_cycles_at_its_shipped_rate` — spec test 4.
  A `spawn_structure_at` fixture (bare, no `BuildQuality`) cycles at exactly
  `work.ticks_per_unit` for a `DEFAULT_BASE_SPEED` worker.
- `the_home_stands_up_carrying_no_build_quality` — spec test 5. Assert the
  component is **absent**, not that it is 1.0: absent is the rule, and a
  fixture writing `BuildQuality(1.0)` would pass a value check while breaking
  the rule the doc comment states.
- `an_upgrade_overwrites_the_figure_with_the_new_programs` — spec test 6.
  Raise with an Excellent builder, upgrade with a Poor one, assert the
  machine's figure is the Poor one — not the average, not the better.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

`cargo check -p feral-processes-engine` names every `spawn_structure` and
`work_ticks_at_speed` call site; `support.rs:880` passes `None`.

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src
git commit -m "A machine runs at the quality of the program that raised it"
```

---

## Task 5: `StructureSave::build_quality`

**Files:**
- Modify: `crates/engine/src/save.rs:912-977` (`StructureSave`) and its
  `default_*` helpers at `:981`
- Modify: `crates/engine/src/game/lifecycle.rs:1986` (write),
  `:1150-1235` (restore)
- Test: `crates/engine/src/tests/save_roundtrip.rs`

**Interfaces:**

```rust
pub struct StructureSave {
    // …
    #[serde(default = "default_build_quality")]
    pub build_quality: f32,
}

fn default_build_quality() -> f32;   // 1.0
```

**It must be restored from the save, never re-derived on load.** `Game::load`
deliberately rebuilds some structure components from the def (`Stock::capacity`
is the stated example), and `ResourceNode::level` already needed carving out
of that rule. The program that raised this machine is gone; there is nothing
on the def to re-derive from, so re-deriving means silently resetting every
machine in the base to neutral. Say that in the field's doc comment.

The write reads `Option<&BuildQuality>` off the structure query — add it to
the tuple at `lifecycle.rs:1966-1974` — and falls back to `1.0` for a
structure carrying none (the Home, a fixture). The restore inserts
`BuildQuality(s.build_quality)` unconditionally; a Home reloaded at `1.0`
carries a component it did not have before, which changes nothing because
`1.0` is the neutral the absent case already means.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/save_roundtrip.rs`:

- `a_machines_build_quality_survives_a_save` — spec test 9. Raise a machine
  with a **non-neutral** builder, record its real cycle length, save to a
  scratch file, `Game::load`, and assert the reloaded machine cycles at the
  same number. A file round trip, not a RON round trip: the memory entry
  *"RON round-trip can't catch a skipped field"* is exactly this failure.
- `a_save_written_without_build_quality_loads_it_neutral` — spec test 10's
  structure half. Same shape as Task 2's: save, strip every `build_quality:`
  line from the text, write back, load, and assert the machine cycles at its
  shipped rate. Assert the stripped file really lacked the key.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src
git commit -m "A machine remembers how well it was built"
```

---

## Task 6: `BuildEffect`, `BuildCandidate` and `Game::build_candidates`

**Files:**
- Modify: `crates/engine/src/views.rs` (new types)
- Modify: `crates/engine/src/game/party.rs:567` (beside `programs_for_build`)
- Modify: `crates/engine/src/game/catalog.rs` (`structure_kind`)
- Test: `crates/engine/src/tests/construction.rs`

**Interfaces:**
- Consumes: Tasks 3 and 4.
- Produces:

```rust
// views.rs
/// What spending one program on one build does to the machine — the engine's
/// answer, drawn by gui. Never a percentage: `work_ticks_at_speed` rounds to
/// whole ticks and floors at one, so on a short cycle a real percentage
/// quotes a change that does not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEffect {
    /// The machine's shipped rate, and its rate as this program would build
    /// it. Both out of `systems::work_ticks_at_speed`.
    Cycle { shipped: u32, built: u32 },
    /// This structure runs no cycle, so quality cannot reach it.
    NoCycle,
}

/// One row of the build picker: a program, the aptitude this build reads,
/// and what spending it does.
pub struct BuildCandidate {
    pub pet: PetInfo,
    /// "Assembly" or "Extraction" — which roll this build reads.
    pub aptitude: &'static str,
    /// `Potential::roll_label` of that roll.
    pub label: &'static str,
    pub effect: BuildEffect,
}

// game/party.rs
impl Game {
    /// The build picker's rows for an order of `kind` under `goal`: every
    /// program `programs_for_build` qualifies, decorated with what spending
    /// it would do, **ordered best-first by the roll this build reads**.
    ///
    /// The one derivation of both what qualifies and what is drawn — app-core
    /// indexes this list and gui draws it, so a row reordered in either
    /// would spend the program the player read on another row.
    pub fn build_candidates(&mut self, kind: &StructureId, goal: BuildGoal)
        -> Vec<BuildCandidate>;
}

// game/catalog.rs
impl Game {
    /// Which kind of structure stands on `entity`, for a caller holding an
    /// `Entity` where the picker needs a def.
    pub fn structure_kind(&self, entity: Entity) -> Option<StructureId>;
}
```

`build_candidates` internals:

1. `let tier = crate::game::catalog::program_tier_required(goal);` — the
   existing derivation, not a restated copy.
2. `self.programs_for_build(tier)` — unchanged, still the one derivation of
   who qualifies. A def id that resolves to nothing returns the empty list.
3. Per candidate, `self.build_quality_for(&def, pet.entity)`, then
   `self.cycle_ticks_for(&def, quality, DEFAULT_BASE_SPEED)` for `built` and
   `self.cycle_ticks_for(&def, 1.0, DEFAULT_BASE_SPEED)` for `shipped`.
   `None` from either → `BuildEffect::NoCycle`.
4. **Stable** sort, descending by the applicable roll (`f32::total_cmp`), so
   ties keep `programs_for_build`'s role order.

The preview holds the worker at `DEFAULT_BASE_SPEED` because no program is
posted yet — say so in the doc comment, and note that the figure is the
machine's rate as built rather than a promise about whoever ends up at it.

**`shipped` and `built` both go through the same `cycle_ticks_for` the live
rate uses**, including the player's `class_scale`. That is what makes spec
test 13 an equality rather than an approximation.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/construction.rs`:

- `the_picker_orders_candidates_by_the_roll_this_build_reads` — spec test 12.
  Three programs whose assembly and extraction rolls rank them in **opposite**
  orders. Ask for an `assembles` structure and for a `work` one; assert the
  two lists are ordered differently and each matches its own roll. A fixture
  where both rolls rank the same way proves nothing.
- `the_quoted_cycle_is_the_one_the_finished_machine_runs` — spec test 13, and
  the test the whole call-not-a-copy rule exists for. Read a candidate's
  `BuildEffect::Cycle { built, .. }`, file the build **with that same
  program**, run it to completion, and assert
  `work_ticks_for(machine, DEFAULT_BASE_SPEED) == built`. End to end — not two
  expressions compared. It lives in the engine rather than app-core because
  `work_ticks_for` is private and the point is comparing against the *real*
  rate.
- `a_cycle_too_short_for_the_effect_quotes_the_same_number_twice` — spec test
  14. A structure whose `ticks_per_unit` is small enough that rounding eats
  the change (find one in the shipped catalogue; if none is short enough, use
  `assets_dir_with_extra_structure` to add one). Assert `shipped == built`
  **and** that the raised machine genuinely cycles at that number. This is the
  case a percentage would have got wrong.
- `a_depot_reports_no_cycle_at_all` — spec test 15. Every candidate for a
  `depot` build reports `BuildEffect::NoCycle`. Assert on the variant, not on
  a number: `Cycle { shipped: n, built: n }` would be the wrong answer here
  and is the *right* answer for the test above it.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src
git commit -m "The picker's quote is a call into the real cycle formula"
```

---

## Task 7: The manifest page

**Files:**
- Modify: `crates/engine/src/views.rs` (`ManifestPotential`)
- Modify: `crates/engine/src/game/inspection.rs:1869-1879`
- Modify: `crates/gui/src/render/manifest.rs:630-642` (`program_sections`),
  `:905` (`roll_readout` — read only, no change expected)
- Modify: `crates/gui/src/render/manifest_layout.rs:47-102` (new constant),
  `:365-375` (`worst_case_program`)
- Test: `crates/gui/src/render/manifest_layout.rs` `mod tests`

**Interfaces:**

```rust
// views.rs
pub struct ManifestPotential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    pub assembly_roll: f32,
    pub extraction_roll: f32,
    pub percent: u32,
    pub label: String,
}

// manifest_layout.rs
/// POTENTIAL's own cap, **wider** than `MAX_SECTION_ROWS` — the other
/// per-box constants narrow their box, this one widens one. Seven because
/// the box lists one row per roll plus the overall tier, and the two build
/// rolls are the sixth and seventh. Held here rather than by raising
/// `MAX_SECTION_ROWS`, which would take a row from ROUTINES' "+N more"
/// budget for a box that does not need it.
pub(super) const MAX_POTENTIAL_ROWS: usize = 7;
```

The POTENTIAL box gains `stat("Assembly roll", roll_readout(q.assembly_roll))`
and `stat("Extraction roll", roll_readout(q.extraction_roll))`, placed after
"Growth roll" and before "Overall" — the four combat rolls, then the two build
rolls, then the aggregate that only folds the first four. It switches from
`section_rows` to `section_rows_capped(…, MAX_POTENTIAL_ROWS)`.

**The layout census is the gate on this task.** `worst_case_program`'s
`section("POTENTIAL", 5, false)` becomes
`section("POTENTIAL", MAX_POTENTIAL_ROWS, false)`, and
`the_real_worst_case_pages_fit_the_tightest_window` must still pass at every
window size it measures. If it fails: **stop and report the measured
clearance.** Do not silently lower another box's cap to pay for this — that
trade is the owner's call, and `MAX_MOVE_ROWS`' doc comment records what
lowering one costs.

- [ ] **Step 1: Write the failing tests**

- Update `worst_case_program` first, and run
  `the_real_worst_case_pages_fit_the_tightest_window`. It should pass with the
  taller fixture before any renderer change — that is the measurement, taken
  before the code that depends on it.
- In `crates/gui/src/render/manifest.rs` `mod tests` (or wherever the
  section census lives — grep for `program_sections`), add
  `the_potential_box_lists_both_build_rolls`: build a `ProgramManifest` whose
  six rolls are six distinct values and assert the POTENTIAL rows name
  "Assembly" and "Extraction" and carry those two values, **and** that no
  `SectionRow::Note("+N more")` was emitted — a silently trimmed row is
  exactly the failure `section_rows_capped` produces and the one this task
  has to rule out.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green** — `cargo test -p feral-processes-gui manifest`

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src crates/gui/src
git commit -m "The manifest names both build rolls"
```

---

## Task 8: `PetInfo`'s build labels and the roster row

**Files:**
- Modify: `crates/engine/src/views.rs` (`PetInfo`)
- Modify: `crates/engine/src/game/party.rs:409-419` (beside
  `potential_quality_label`), `:483-503` (`PetInfo` construction)
- Modify: `crates/gui/src/render/party.rs:243` (`companion_row_lines`)
- Modify: `crates/gui/src/render/mod.rs:1351` (`test_pet`)
- Test: `crates/gui/src/render/party.rs` `mod tests`

**Interfaces:**

```rust
// views.rs, on PetInfo
/// This individual's Assembly rung — see `Potential::roll_label`. `None`
/// for a creature with no `Potential`, `quality`'s own rule.
pub assembly: Option<String>,
/// The same for Extraction.
pub extraction: Option<String>,

// game/party.rs
impl Game {
    /// The `Poor`..`Excellent` rung each of `entity`'s two build rolls sits
    /// on, or `None` for a creature with no `Potential`.
    pub(crate) fn build_roll_labels(&self, entity: Entity) -> Option<(String, String)>;
}
```

`companion_row_lines` gains two tags, ` [Assembly: {a}]` and
` [Extraction: {e}]`, appended after the existing five so they are the first
to shed. `wrapped_row_lines` is what makes that safe — the tags are the units
that come and go, and an over-long head is lost rather than wrapped, which is
why neither goes into the head.

- [ ] **Step 1: Write the failing tests**

In `crates/gui/src/render/party.rs` `mod tests`:

- `a_roster_row_names_both_build_rolls` — a `test_pet` with both labels set
  draws them; one with `None` on both draws neither and emits exactly the
  line it did before.
- Extend the existing roster width census (the one whose doc comment records
  "Six tags at their widest run a roster row 382px past a `PopupSize::Large`
  body") to seven tags: a worst-case `PetInfo` with every optional tag at its
  widest, measured through `paint::with_painter` against the real
  `PopupSize::Large` body, exactly as
  `no_deploy_row_overflows_its_popup_in_pixels` does at
  `crates/gui/src/render/building.rs:1851`. Assert **no line** overflows —
  the shed is allowed to add lines, it is not allowed to run wide.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green**

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src crates/gui/src
git commit -m "The roster row carries both build rolls"
```

---

## Task 9: The build picker

**Files:**
- Modify: `crates/app-core/src/app/building.rs:304-356`
  (`handle_build_program_key`), plus a new `pending_build_kind`
- Modify: `crates/gui/src/render/building.rs:443-566` (`build_commit`,
  `build_program_rows`, `draw_build_program`)
- Modify: `crates/gui/src/render/party.rs:296-330` (`companion_page_rows`
  split)
- Test: `crates/app-core/src/tests/building.rs`,
  `crates/gui/src/render/building.rs` `mod tests`

**Interfaces:**
- Consumes: `Game::build_candidates`, `Game::structure_kind` (Task 6);
  `PetInfo`'s two labels (Task 8).
- Produces:

```rust
// app-core, impl App
/// Which structure the pending order is for — the def id for a deploy, and
/// the standing structure's own kind for an upgrade. One resolution, because
/// both the handler and gui's `build_commit` need it and a second walk is
/// where the screen and the keypress would name different structures.
pub fn pending_build_kind(&mut self) -> Option<StructureId>;

// gui
pub(super) struct BuildCommit {
    pub label: String,
    pub to_tier: Option<u32>,
    /// What `Game::build_candidates` is asked about, from
    /// `App::pending_build_kind`.
    pub structure: StructureId,
}

impl BuildCommit {
    /// The goal this order carries, derived from `to_tier` rather than
    /// stored beside it — `None` is a deploy, `Some(t)` an upgrade to `t`.
    pub(super) fn goal(&self) -> BuildGoal;
}

pub(super) fn build_program_rows(
    commit: Option<&BuildCommit>,
    candidates: &[BuildCandidate],
    selected: usize,
) -> Vec<Row>;

// gui/render/party.rs
/// One program's rows in the roster's own format — the identity line and its
/// shed continuations. `extra` are tags appended after the roster's own,
/// empty on the roster itself.
pub(super) fn companion_rows(
    p: &PetInfo,
    shortcut: char,
    selected: bool,
    extra: &[String],
) -> Vec<Row>;
```

`companion_page_rows` keeps its signature and becomes the role headings plus
a `companion_rows(p, menu_shortcut(i), i == selected, &[])` per program. The
picker calls `companion_rows` directly and emits **no headings** — see
"Two decisions taken after the spec".

`handle_build_program_key` swaps `programs_for_build(tier)` for
`build_candidates(&kind, goal)` and indexes that. Its existing comment about
`program_tier_required` moves inside `build_candidates` and is replaced by one
about ordering: the list this handler indexes is the list gui draws, in the
same order, and both come from the one call.

Each picker row's `extra` tags are:

```text
 [{aptitude}: {label}]                  // e.g. " [Assembly: Excellent]"
 [{structure} cycle {shipped} -> {built} ticks]   // BuildEffect::Cycle
```

and for `BuildEffect::NoCycle` the row carries **no** cycle tag at all. The
picker instead prints one line, once, above the list:

> *This build does not run a work cycle, so it does not care which program you spend.*

`Row::Text`, so `popup_layout` pins it above the scrolling body with the
warning — a fact about the whole screen belongs above the list, not repeated
on every row. **Rows do not grey**: every program is equally valid here, and
that is the message.

**No new key.** The picker's rows are selectors and lowercase letters are row
selectors; nothing here adds a binding, and `draw_build_program` keeps taking
`&mut Game` after `build_commit(app)` has been hoisted — so `structure` and
`goal` must reach the renderer on `BuildCommit`, not by a second borrow of
`App`.

- [ ] **Step 1: Write the failing tests**

In `crates/app-core/src/tests/building.rs`:

- `the_picker_spends_the_program_on_the_row_the_player_read` — build a roster
  whose `build_candidates` order differs from `owned_pets` order, press the
  shortcut for a middle row, and assert the program actually committed is the
  one that row named. This is the test that catches the handler and the
  renderer indexing different lists.
- `an_upgrade_picker_asks_about_the_structure_standing_there` —
  `pending_build_kind` on a `PendingBuild::Upgrade` returns the machine's own
  kind, not the deploy path's.

In `crates/gui/src/render/building.rs` `mod tests`:

- `a_picker_row_quotes_the_cycle_in_ticks` — a `BuildCandidate` with
  `Cycle { shipped: 20, built: 18 }` draws both figures and no percent sign
  anywhere on the row.
- `a_no_cycle_build_says_so_once_above_the_list` — every candidate at
  `NoCycle`: the sentence appears exactly once, is a `Row::Text` (so it is
  pinned, not scrolled), and **no** row carries a cycle tag. Assert on the
  absence too — a row drawing `Cycle { shipped: n, built: n }` here is the
  reading §4 spends a paragraph refusing.
- `the_picker_still_says_what_each_program_is_doing` — the activity tag
  survives on every row, since the role headings did not. This is what keeps
  "spending it also empties a job" on the screen.
- Extend `no_program_picker_row_runs_past_the_popup_body` and add a pixel
  counterpart in the shape of
  `no_deploy_row_overflows_its_popup_in_pixels` — spec test 16 — with the
  worst case now carrying the aptitude tag, the cycle quote, an overlong
  structure name and an overlong program name.
- The existing `every_picker_row_stays_inside_the_scrollable_body` must still
  pass: nothing may follow the last `Row::Item`, so the no-cycle sentence goes
  **above** the list and never below it.

- [ ] **Step 2: Run and watch fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Green** — `cargo test -p feral-processes-app-core building`
  and `cargo test -p feral-processes-gui building`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src crates/gui/src
git commit -m "The build picker quotes the machine it would leave you"
```

---

## Task 10: Documentation, seams and the full gate

**Files:**
- Modify: `docs/seams.md` (the argument)
- Modify: `.claude/skills/seams/references/base.md` (the trap)
- Modify: `CHANGELOG.md`
- Note for the merge: `CLAUDE.md` (the one-sentence rules) — it is
  **gitignored**, so it cannot ride this branch. Write the two rules into the
  primary checkout's copy at merge time, not here.

Three seam rules earned by this work, one sentence each in `CLAUDE.md`'s
**The base** section, with the argument in `docs/seams.md` under the same
title and the trap in the `seams` skill:

- **A structure remembers how well it was built, and absent means neutral.**
  `components::BuildQuality`, written only by `Game::spawn_structure` and the
  upgrade arm, restored from the save and never re-derived from the def.
- **The build term goes inside `work_ticks_at_speed`, not at its callers** —
  `class_scale`'s own argument, and the picker's preview is a *call* into that
  function through `Game::build_candidates` rather than a percentage.
- **`Potential`'s two build rolls are deliberately independent of the four
  combat rolls, and `quality_percent` still folds only the four.**

`CHANGELOG.md` gets a `## 0.13.126` section (the version bump itself happens
at the merge, per the release-per-change policy — not on this branch).

- [ ] **Step 1: Write the three documents**

- [ ] **Step 2: Grep for claims this change falsifies**

`rg -n "four rolls|quality_percent|work_ticks_at_speed" docs/ assets/*/README.md`
and fix anything now wrong. `docs/manual.md` and the root `README.md` are
carved out of the documentation obligation and stay stale.

- [ ] **Step 3: The full gate**

```sh
cargo fmt --check
cargo clippy --workspace
cargo test --workspace
cargo test -p feral-processes-engine balance_sim
```

`balance_sim`'s curves must be **unmoved**. It models no base and no
abilities, so both new constants are ungated by it — that is expected and is
recorded in spec §7, not a gap to close here.

- [ ] **Step 4: Commit**

```bash
git add docs/ .claude/skills/seams/ CHANGELOG.md
git commit -m "Document the build-quality seam"
```

- [ ] **Step 5: Report honestly**

State plainly, once: the suite is green and **nothing in this feature has been
on a screen**. The build picker, the roster row and the manifest box are all
screens an agent in this environment cannot see. The two constants —
`BUILD_QUALITY_PER_RARITY_RUNG` and `BUILD_QUALITY_TICK_WEIGHT` — are
unmeasured by any instrument the repo has, and the check on them is play.

---

## What this plan does not do

- **No species-level build aptitude.** Spec's open question; `base_speed`
  already gives a species a cycle axis.
- **No second effect for the fourteen structures that run no cycle.**
  Durability was considered and rejected in spec §4 — a second formula with
  its own balance argument, aimed at a number invisible until a raid lands.
  The picker says so outright instead.
- **No structure names the program it ate.** Still flavour, still out of
  scope.
