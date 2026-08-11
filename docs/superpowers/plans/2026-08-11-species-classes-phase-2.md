# Plan: species classes, phase 2 — `base_speed` as worker rate

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:executing-plans`
> or `superpowers:subagent-driven-development`. Steps are checkboxes.

Spec: `docs/superpowers/specs/2026-08-10-species-classes-design.md`, phase 2 of
eight. Phase 1 (`base_int` + the manifest WORK box) is built. Phases 3-8 are
explicitly **out of scope** — no stat shapes, no construct tier move, no
affinity normalisation, no kits, no class base jobs.

**Goal:** make `base_speed` mean pace at a machine as well as initiative in a
fight, so posting a quick program to a Fabricator is visibly different from
posting a slow one.

**Architecture:** the worker's speed is baked into `Task::required` **at
assignment**, as a deviation from `DEFAULT_BASE_SPEED`. `assembler_system` then
reads `Task::required` instead of its def's `ticks_per_unit` — which **inverts a
documented CLAUDE.md seam** (see "The seam" below). Nothing reads a species at
tick time; the rate is decided once, when the program is posted.

**Crates touched:** engine only (tuning, one pure formula, `work_ticks_for` and
its two call sites, `assembler_system`, test fixtures). No gui change, no
app-core change, no asset data change — only two asset READMEs.

## Global constraints

- **No `SAVE_FORMAT_VERSION` bump.** `Task::required` is already serialised
  (`save.rs:220`) and already restored (`lifecycle.rs:576-583`). If a task finds
  itself reaching for a save bump, the design has drifted — stop and re-read.
- **A cronjob in an existing save keeps its old rate until reassigned.** This is
  a known, accepted cost, stated in the spec. Do not add a migration that
  rewrites `required` on load; that would need the species db at load time and
  buys nothing a re-post doesn't.
- `base_speed` stays `#[serde(default = "default_base_speed")]`. A species file
  without it must keep parsing *and* keep working at exactly the machine's own
  rate.
- Do not touch `assets/species/*.ron` at all. The roster's `base_speed` values
  are already authored and are phase 3's business if they change.
- `docs/manual.md` and root `README.md` are carved out of the doc obligation.
  `assets/species/README.md`, `assets/structures/README.md` and `CHANGELOG.md`
  are **not**.
- Version bump, tag, merge and push all need an explicit ask. Do none of them.

## The two settled decisions

Both were put to the user before this plan was written; do not relitigate them.

1. **Damped linear, ~1.5x spread.** `WORK_TICKS_PER_SPEED = 0.05`. On a Mining
   Node (`ticks_per_unit: 10`): construct (speed 6) → 12 ticks, baseline (10) →
   10, sprite (14) → 8. On a Fabricator (30): 36 / 30 / 24.
2. **The player's own rate does not change.** This falls out for free and must
   not be special-cased: `Game::species_base_speed` (`game/combat.rs:291`)
   already returns `DEFAULT_BASE_SPEED` for an entity with no `Creature`, which
   is the player. The deviation is zero, so `work_structure` produces exactly
   today's number. `PLAYER_BASE_SPEED` (11) stays purely an initiative constant
   and is **not** read by any of this.

## The seam this inverts

`CLAUDE.md`'s load-bearing-seams list currently says:

> An assembler's rate comes from its def's `ticks_per_unit`, not from
> `Task::required` — the rate is a property of the machine, not of how its
> program happened to be assigned.

Phase 2 makes the opposite true. The evidence that the inversion is safe:
`upgrade_structure` never touches `ticks_per_unit`, so nothing changes a
machine's rate after assignment, and `displace_task_holder` guarantees one
`GatherResource` task per structure.

**The amendment cannot ride this branch.** `CLAUDE.md` and `AGENTS.md` are both
gitignored, and a worktree has neither file — confirmed in this worktree. So the
spec's "in the same commit as the amendment" is not achievable as written. The
amendment is a **merge-time step in the primary checkout**, recorded in Task 5
so it is not silently lost when the worktree is removed. Edit `CLAUDE.md`, then
`cp CLAUDE.md AGENTS.md`; hand-patching both is how they drifted before.

## The blast radius, stated before you start

Two groups of existing tests change behaviour. Neither is a bug; both are the
change working. **Fix the fixtures, never the requirement.**

- **11 `staffed()` call sites in `chains.rs`** hand-build a `Task` with
  `required: 1` against shipped assemblers rated 12-30 ticks. Today the
  assembler *ignores* `required`, so the machine consumes nothing on tick 1 and
  assertions like `input_of(&game, left, CHARGE_COIL) == 1` hold. After the
  inversion, `required: 1` means "produce every tick", the machine eats its
  input immediately, and those assertions fail. Task 3 fixes `staffed()` to
  carry the def's real rate.
- **~28 `assign_cronjob` call sites across `hauling.rs`, `chains.rs`,
  `inspection.rs`, `perks.rs`, `trade.rs`, `building.rs`** use `spawn_tamed`,
  which builds its worker from `support.rs::generic_species` — "first species by
  id with no declared abilities", which today resolves to **construct, whose
  `base_speed` is 6**. So every posted cycle in those tests becomes 20% longer:
  a Mining Node goes 10 → 12 ticks. Any test that ticks a fixed number of times
  and expects a payout will need its loop raised. Raise the loop; do not weaken
  the assertion, and do not "fix" it by changing `generic_species` — relocating
  that fixture is phase 4b's job and doing it early drags an unrelated landmine
  into this phase.

---

## Task 1: the formula, on its own

A pure function first, wired to nothing, so its behaviour is pinned before any
call site can hide it.

**Files:**
- Modify: `crates/engine/src/tuning.rs` — new constant beside
  `MINING_SUCCESS_PER_INT` (~line 1144)
- Modify: `crates/engine/src/systems.rs` — new function beside
  `mining_success_chance` (line 151), tests in the existing test module at the
  bottom of the same file (~line 1135)

**Produces:** `tuning::WORK_TICKS_PER_SPEED: f64 = 0.05`;
`systems::work_ticks_at_speed(base_ticks: u32, speed: i32) -> u32`
(`pub(crate)`).

- [ ] **Step 1** — Write the failing tests, in `systems.rs`'s test module
      alongside `mining_success_chance_rises_with_level_and_caps_at_one`:

```rust
#[test]
fn a_baseline_worker_works_at_exactly_the_machines_own_rate() {
    // The whole moddability contract: a species file with no `base_speed`,
    // and the player (who has no species at all), must reproduce the def's
    // number rather than merely land near it.
    for base in [1, 3, 6, 8, 10, 12, 20, 30] {
        assert_eq!(
            work_ticks_at_speed(base, DEFAULT_BASE_SPEED),
            base,
            "a worker at the roster baseline must cost exactly the def's rate"
        );
    }
}

#[test]
fn a_faster_species_needs_fewer_ticks_and_a_slower_one_more() {
    // The shipped extremes — construct 6, sprite 14 — against a Mining
    // Node's 10 and a Fabricator's 30.
    assert_eq!(work_ticks_at_speed(10, 14), 8);
    assert_eq!(work_ticks_at_speed(10, 6), 12);
    assert_eq!(work_ticks_at_speed(30, 14), 24);
    assert_eq!(work_ticks_at_speed(30, 6), 36);
}

#[test]
fn an_absurd_modded_speed_still_costs_at_least_one_tick() {
    // A `base_speed: 200` mod scales the multiplier straight past zero and
    // negative. Without the floor that is a machine producing on every tick
    // forever, which is also what a `required: 0` would do.
    assert_eq!(work_ticks_at_speed(10, 200), 1);
    assert_eq!(work_ticks_at_speed(1, 14), 1);
}
```

- [ ] **Step 2** — Run `cargo test -p feral-processes-engine work_ticks`.
      Expect FAIL: `cannot find function work_ticks_at_speed`.

- [ ] **Step 3** — Add the constant to `tuning.rs`:

```rust
/// What one point of `SpeciesDef::base_speed` **either side of**
/// `DEFAULT_BASE_SPEED` is worth on the length of a work cycle. The shipped
/// roster spans 6 (Construct) to 14 (Sprite), so a cycle ranges 1.2x to
/// 0.8x the machine's own rate — a Mining Node's 10 ticks becomes 12 or 8,
/// and a Fabricator's 30 becomes 36 or 24.
///
/// Sized like `MINING_SUCCESS_PER_INT`: enough that swapping the posted
/// program is visible on one screen, small enough that upgrading the
/// machine still beats re-casting the roster.
pub const WORK_TICKS_PER_SPEED: f64 = 0.05;
```

- [ ] **Step 4** — Add the function to `systems.rs`, directly under
      `mining_success_chance`:

```rust
/// How many ticks one work cycle costs a worker of `speed` at a machine
/// whose def rates it at `base_ticks`.
///
/// Read as a **deviation from `DEFAULT_BASE_SPEED`**, exactly like
/// `base_int`'s term in `mining_success_chance` above. A species at the
/// baseline — and the player, who has no `Creature` and so takes the
/// baseline from `Game::species_base_speed` — gets `base_ticks` back
/// unchanged. That is what keeps a machine's shipped `ticks_per_unit`
/// meaning what it says, and what puts pressure on the posting in both
/// directions: a quick program beats working the node yourself, and a slow
/// one is worse than rolling your sleeves up.
pub(crate) fn work_ticks_at_speed(base_ticks: u32, speed: i32) -> u32 {
    let scale = 1.0 + (DEFAULT_BASE_SPEED - speed) as f64 * WORK_TICKS_PER_SPEED;
    // Floored at one cycle per tick however fast the species: a modded
    // `base_speed: 200` scales straight past zero into negative.
    (base_ticks as f64 * scale).round().max(1.0) as u32
}
```

      Add `DEFAULT_BASE_SPEED` and `WORK_TICKS_PER_SPEED` to the existing
      `use crate::tuning::{...}` list at the top of `systems.rs` — do not add a
      second `use` line for them.

- [ ] **Step 5** — Run `cargo test -p feral-processes-engine work_ticks`.
      Expect PASS, all three.

- [ ] **Step 6** — **Mutation check.** Temporarily change the constant to `0.0`
      and confirm `a_faster_species_needs_fewer_ticks_and_a_slower_one_more`
      fails while `a_baseline_worker_works_at_exactly_the_machines_own_rate`
      still passes — that is the pair proving the two tests are testing
      different things. Restore the constant.

- [ ] **Step 7** — Commit.

## Task 2: bake the rate in at assignment

**Files:**
- Modify: `crates/engine/src/game/building.rs:366-380` (`work_ticks_for`),
  `:447-454` (`work_structure`), `:540-554` (`assign_cronjob`)
- Test: `crates/engine/src/tests/building.rs`

**Consumes:** `systems::work_ticks_at_speed` from Task 1.
**Produces:** `work_ticks_for(&mut self, structure: Entity, worker_speed: i32)
-> u32`.

`Game::species_base_speed(entity)` already exists at `game/combat.rs:291` and is
`pub(crate)`, so `building.rs` can call it. **Do not write a new species lookup**
— that function already returns `DEFAULT_BASE_SPEED` for the player, which is
the entire reason decision 2 above costs nothing.

Read the speed on its own line before calling `work_ticks_for`. The nested form
`self.work_ticks_for(s, self.species_base_speed(w))` does compile under
two-phase borrows, but the two-line version is what the rest of this file looks
like.

- [ ] **Step 1** — Write the failing tests in
      `crates/engine/src/tests/building.rs`. There is no helper for spawning a
      tamed program of a *named* species; overwrite the `Creature` after
      `spawn_tamed` rather than touching `support.rs`, whose `spawn_tamed` has
      233 call sites:

```rust
/// A tamed worker forced onto a named species, so a test can post a
/// specific `base_speed`. `spawn_tamed` builds from `generic_species`,
/// which has no `base_speed` of its own worth reasoning about.
fn tamed_of(game: &mut Game, species: &str) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    game.world.get_mut::<Creature>(worker).unwrap().species = SpeciesId::from(species);
    worker
}

#[test]
fn a_quicker_program_is_posted_on_a_shorter_cycle() {
    let mut game = game_with_worked_structure("speed_posting");
    let node = only_node(&game);

    let sprite = tamed_of(&mut game, "sprite");
    game.assign_cronjob(sprite, node).unwrap();
    let quick = game.world.get::<Task>(sprite).unwrap().required;

    let construct = tamed_of(&mut game, "construct");
    game.assign_cronjob(construct, node).unwrap();
    let slow = game.world.get::<Task>(construct).unwrap().required;

    assert!(
        quick < slow,
        "posting a faster species must buy a shorter cycle — got {quick} against {slow}"
    );
}

#[test]
fn working_a_node_by_hand_still_costs_exactly_the_machines_own_rate() {
    // The player has no species, so their deviation is zero. This is the
    // other half of the pressure `base_int` set up: it has to stay true
    // that a dull program is worse than doing the job yourself.
    let mut game = game_with_worked_structure("hand_worked_rate");
    let node = only_node(&game);
    stand_player_at_post(&mut game, node);

    game.work_structure(node).unwrap();

    let required = game.world.get::<Task>(game.player_entity()).unwrap().required;
    assert_eq!(
        required,
        mining_node_ticks_per_unit(&game),
        "the player works a node at the def's own rate, not at PLAYER_BASE_SPEED"
    );
}
```

      Use whatever this file's existing fixture helpers are actually called —
      read the top of `tests/building.rs` first and reuse them rather than
      inventing `game_with_worked_structure` / `only_node` /
      `mining_node_ticks_per_unit` if equivalents exist. `stand_player_at_post`
      does exist (`tests/support.rs:550`) and is required before
      `work_structure`, which refuses a player not at a station tile.

- [ ] **Step 2** — Run `cargo test -p feral-processes-engine -- posted_on_a_shorter_cycle machines_own_rate`.
      Expect FAIL on the first (both requireds equal the def's rate) and PASS on
      the second (nothing has changed yet). That split is the point: the second
      test is a **regression guard from the moment it is written**.

- [ ] **Step 3** — Change `work_ticks_for` to take the worker's speed, and
      rewrite its doc comment — the current one claims "a program and the player
      grind at the same rate", which this task makes false:

```rust
    /// How many ticks one work cycle against `structure` takes for a worker
    /// of `worker_speed`, from the structure's def rate scaled by
    /// `systems::work_ticks_at_speed`.
    ///
    /// Shared by `assign_cronjob` and `work_structure`, which is what makes
    /// the comparison legible: the player has no species and so works at
    /// the baseline, and a posted program is faster or slower than that by
    /// its own `base_speed`.
    fn work_ticks_for(&mut self, structure: Entity, worker_speed: i32) -> u32 {
        let kind = self.world.get::<Structure>(structure).unwrap().kind.clone();
        let db = self.world.resource::<StructureDb>();
        let base = match db.get(&kind) {
            None => 5,
            Some(def) => match (&def.work, &def.assembles) {
                (Some(work), _) => work.ticks_per_unit,
                (None, Some(assembles)) => assembles.ticks_per_unit,
                (None, None) => 5,
            },
        };
        crate::systems::work_ticks_at_speed(base, worker_speed)
    }
```

- [ ] **Step 4** — Update the two call sites. In `work_structure`, hoist
      `let player = self.player_entity();` above the `ticks` line (it currently
      sits below it at `:448`):

```rust
        let player = self.player_entity();
        let speed = self.species_base_speed(player);
        let ticks = self.work_ticks_for(structure, speed);
```

      In `assign_cronjob`, replace `let ticks = self.work_ticks_for(structure);`
      at `:540` with:

```rust
        let speed = self.species_base_speed(worker);
        let ticks = self.work_ticks_for(structure, speed);
```

- [ ] **Step 5** — Run the two tests. Expect both PASS.

- [ ] **Step 6** — Run `cargo test -p feral-processes-engine`. **Expect
      failures**, in the ~28 `assign_cronjob` tests described in "The blast
      radius" above: `spawn_tamed` builds a construct at `base_speed: 6`, so
      every posted cycle is 20% longer and any fixed tick loop may now fall a
      cycle short. For each failure, raise the loop count to match the new
      cycle length. Do not weaken an assertion, do not change
      `generic_species`, and do not lower `WORK_TICKS_PER_SPEED` to make a test
      pass.

- [ ] **Step 7** — Commit. Mention in the message that the fixture loop counts
      moved because the generic test worker is a construct.

## Task 3: invert the assembler seam

**Files:**
- Modify: `crates/engine/src/systems.rs:781-792` (`assembler_system`)
- Modify: `crates/engine/src/tests/chains.rs:771-782` (the `staffed` helper)
- Test: `crates/engine/src/tests/chains.rs`

**Consumes:** `Task::required` now carrying the worker's baked rate, from Task 2.

- [ ] **Step 1** — Fix `chains.rs`'s `staffed` helper **first**, before touching
      the system. It hand-builds `required: 1` against shipped assemblers rated
      12-30, which the assembler currently ignores; make it carry the machine's
      real rate so the 11 tests built on it keep measuring what they were
      written to measure (pull order and feeding, not production speed):

```rust
/// A staffed structure of `kind` at an absolute tile.
///
/// The task carries the machine's own `ticks_per_unit`, because
/// `assembler_system` reads `Task::required` — these tests are about which
/// machine pulls what, and a worker that finished a batch on tick 1 would
/// eat the very input they assert on.
fn staffed(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    let machine = deployed(game, kind, x, y);
    let required = game
        .world
        .resource::<StructureDb>()
        .get(kind)
        .and_then(|d| d.assembles.as_ref())
        .map(|a| a.ticks_per_unit.max(1))
        .unwrap_or(1);
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required,
    });
    machine
}
```

      Leave `assembler_at` (`chains.rs:95-116`) alone — its `required: 3`
      already matches `test_assembler`'s `ticks_per_unit: 3`. Fix the Compiler
      fixture at `chains.rs:1157-1163` the same way as `staffed`: its
      `required: 1` against the Compiler's `ticks_per_unit: 8` would otherwise
      make that test pass eight times over for the wrong reason.

- [ ] **Step 2** — Write the failing test in `chains.rs`:

```rust
#[test]
fn a_quicker_program_runs_the_same_assembler_harder() {
    // Two identical machines, identically fed, differing only in who is
    // posted. This is the phase's whole claim, and it is invisible until
    // `assembler_system` stops reading the def and starts reading the task.
    let mut game = game_with_assembler("assembler_speed", 1000);

    let quick = assembler_at(&mut game, 40, 40, false);
    feeder_at(&mut game, 41, 40, 200);
    post_species(&mut game, quick, "sprite");

    let slow = assembler_at(&mut game, 40, 50, false);
    feeder_at(&mut game, 41, 50, 200);
    post_species(&mut game, slow, "construct");

    for _ in 0..60 {
        game.tick();
    }

    let fast_out = output_of(&game, quick, "test_item");
    let slow_out = output_of(&game, slow, "test_item");
    assert!(
        fast_out > slow_out,
        "the sprite-run machine should be ahead — got {fast_out} against {slow_out}"
    );
}
```

      `post_species` is a new local helper: spawn a tamed worker, force its
      `Creature.species`, then go through `game.assign_cronjob` so the rate is
      baked by the real code path rather than hand-written. Check what
      `assembler_at`'s `test_assembler` actually produces and substitute the
      real item id for `"test_item"`; `output_of` already exists in this file.
      The worker must be standing at the machine — `assign_cronjob` starts it
      from the player's tile, so put the player beside the machine first, the
      same way `chains.rs:531`'s test does.

- [ ] **Step 3** — Run it. Expect FAIL: both machines produce identically,
      because `assembler_system` reads the def.

- [ ] **Step 4** — Make the inversion in `systems.rs`, replacing the
      `let ticks_per_unit = def.assembles...` block at `:781-792`:

```rust
        let Ok((_, mut task)) = tasks.get_mut(worker) else {
            continue;
        };
        // The rate comes from the task, not from `def.assembles`: it was
        // baked at assignment out of the machine's `ticks_per_unit` and the
        // posted program's `base_speed` (`Game::work_ticks_for`). Safe
        // because `upgrade_structure` never touches `ticks_per_unit`, so
        // nothing changes a machine's rate after a program is on it, and
        // `displace_task_holder` allows only one `GatherResource` per
        // structure. The cost is that a cronjob in an old save keeps its
        // pre-`base_speed` rate until it is re-posted.
        //
        // `.max(1)` for the reason the def read had it: a zero would
        // produce on every tick forever.
        task.progress += 1;
        if task.progress < task.required.max(1) {
            continue;
        }
        task.progress = 0;
```

- [ ] **Step 5** — Run the new test. Expect PASS.

- [ ] **Step 6** — **Mutation check.** Revert only the `systems.rs` hunk (keep
      the fixtures and the test) and confirm the new test fails. A copy of the
      file in the scratchpad, not `git checkout` — never `git checkout` work in
      progress. Restore, and assert the mutation actually applied before
      trusting the result.

- [ ] **Step 7** — Run `cargo test -p feral-processes-engine`. Expect the 11
      `staffed()`-based chain tests to be green again from Step 1's fix. Any
      still failing is a fixture whose `required` disagrees with its def —
      check that first, before suspecting the system.

- [ ] **Step 8** — Commit.

## Task 4: the two schema docs

**Files:**
- Modify: `assets/species/README.md:38-47` (the `base_speed` paragraph)
- Modify: `assets/structures/README.md:33-49` and `:107-123` (the two
  `ticks_per_unit` paragraphs)

No code, no tests. Both files are the modding contract and both now describe
something that is only half true.

- [ ] **Step 1** — Rewrite the `base_speed` paragraph in
      `assets/species/README.md`. It currently describes initiative only. It has
      to say that the same number is also the species' pace at a machine, that
      it is read as a **distance from 10** there (so an omitted field costs
      nothing, the same contract `base_int` documents six lines below), and what
      the spread is worth: 6 → a fifth longer per cycle, 14 → a fifth shorter.
      Say plainly that initiative and work rate cannot be tuned apart — that is
      a deliberate design decision and a mod author will otherwise read it as an
      oversight.

- [ ] **Step 2** — Amend both `ticks_per_unit` paragraphs in
      `assets/structures/README.md` so they describe the number as the
      machine's **baseline** rate, which the posted program's `base_speed`
      scales. Currently `work` says "produce one unit of `produces` every
      `ticks_per_unit` ticks" and `assembles` says "one unit every
      `ticks_per_unit` ticks", both flatly.

- [ ] **Step 3** — Commit.

## Task 5: gates and handover

- [ ] **Step 1** — `cargo fmt --all` and
      `cargo clippy --workspace --all-targets`. Fix warnings rather than
      silencing them. `work_ticks_for` gaining an argument is the likely source
      of a `too_many_arguments` complaint elsewhere if it appears — do not add
      an `#[allow]` without saying why.

- [ ] **Step 2** — `cargo test --workspace`. Report the number, and report it
      against the count at the head of this branch rather than against a
      remembered figure. Every test this plan touched should be green *and*
      every fixture loop raised in Task 2 Step 6 should be named in the
      handover, because "I raised a loop count" and "I broke a test and papered
      over it" look identical in a green suite.

- [ ] **Step 3** — Add a `CHANGELOG.md` entry under a new unreleased heading.
      Do **not** bump the workspace version and do **not** tag; both happen at
      the merge and need an explicit ask.

- [ ] **Step 4** — **Merge-time, not now, and not on this branch:** amend
      `CLAUDE.md`'s load-bearing-seams list where it says "An assembler's rate
      comes from its def's `ticks_per_unit`, not from `Task::required`". That
      claim is now backwards. Replace it with the inversion *and* its two
      guarantees (`upgrade_structure` never touches `ticks_per_unit`;
      `displace_task_holder` allows one worker per structure) and the known
      cost (an old save's cronjob keeps its old rate until reassigned). Then
      `cp CLAUDE.md AGENTS.md`. Both files are gitignored and **absent from
      this worktree**, so this edit cannot ride the branch — it must be applied
      to the primary checkout at merge or it is lost. Say so in the handover
      rather than leaving it as a checkbox someone may believe was ticked.

- [ ] **Step 5** — **A green suite is not evidence of play.** Say so plainly and
      offer `cargo run -- --template chains`, which stands up an assembler line,
      and `--template extraction` for the node side. The thing to feel is
      whether swapping the posted program reads as a difference or as noise over
      a couple of minutes of ticks; `WORK_TICKS_PER_SPEED` is the knob if it
      reads as noise. The cronjob row already prints `progress/required`
      (`gui/src/render/building.rs:509`), so the rate is visible on the base
      screen without any UI work — check that it actually reads differently for
      two different programs.

---

## Self-review against the spec

- Spec's phase 2 is "`base_speed` as worker rate, baked into `Task::required` at
  assignment" — Tasks 1-3 cover the formula, the bake, and the read.
- Spec's "Phase 2 inverts a documented seam" paragraph is Task 3 plus Task 5
  Step 4, with the correction that the amendment **cannot** land in the same
  commit as the spec asks, because the file does not exist on this branch.
- Spec's stated known cost (a cronjob in an existing save keeps its old rate
  until reassigned) is in Global Constraints and repeated in the code comment
  Task 3 Step 4 writes, so it survives in the source rather than only here.
- Spec's `base_speed` claim that "its sole reader today is `roll_initiative`, so
  the wiring is nearly free" checks out — `species_base_speed` is the only
  lookup and it already handles the player.
- No UI task: phase 1's WORK box comment (`gui/render/manifest.rs:471-476`)
  already tells the player Speed "means initiative in a fight and pace at a
  machine". That was written forward; this phase makes it true. Nothing to
  change, which is worth stating rather than leaving as an apparent gap.
- Not gated by `balance_sim`: it models no base economy, no machines and no
  initiative. A green `balance_sim` is **not** evidence this phase is correct.
  The evidence is Task 3's two-machine comparison and playing `--template
  chains`. The `balance_sim` gate belongs to phase 3.
- Spec's correction list: neither stale claim
  (`combat_rewards.rs:56-62`'s missing `grant_nest_cache`,
  `assets/species/README.md:81`'s cronjob-assignability gate) lives in a file
  this phase edits — the README correction was already made in phase 1. Both
  deliberately absent.
- Out of scope and confirmed absent: stat shapes, construct's tier move,
  affinity normalisation, `generic_species` relocation, kit authoring, class
  base jobs, any `SAVE_FORMAT_VERSION` change.
