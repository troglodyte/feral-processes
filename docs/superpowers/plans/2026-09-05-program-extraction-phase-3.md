# Program Extraction — Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** The base starts to matter. A Compiler standing anywhere makes an
extraction faster, and every tier above the first makes it richer. A
`Routines`-category tool takes a routine out of a downed program instead of
materials. A sortie's kills travel home as a manifest rather than
teleporting into the store the moment they fall.

**Architecture:** Three seams, all of them existing doors widened rather
than new ones cut. `StructureDef::extracts_programs` joins
`extracts_routines` on the same def and is read by the same "is one
standing?" rule; `extraction_yield` and a new `extraction_ticks` both read
one `extraction_bench_tier()` internally, so a previewed figure and a
granted one still cannot differ; the routine branch and `extract_routine`
share one inner function that does the taking, each caller wording its own
line; `Sortie::programs` is `Sortie::loot`'s shape, banked at the kill and
delivered through `push_downed_program` on return.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-09-04-program-extraction-design.md` —
read sections 3, 4 and 5 first. This plan argues from its ten numbered
decisions and does not restate them.

**Branch:** `claude/extraction-phase-3-135501`, cut from `main` at fea9aaf2
(v0.13.102).

## Global Constraints

- **No `save::SAVE_FORMAT_VERSION` bump.** `SortieSave::programs` is
  additive behind `#[serde(default)]`. `SortieSave::loot` is **not**
  deleted — a save written mid-phase-2 must still parse.
- **No content in Rust.** A tool is `assets/tools/*.ron`, a structure flag
  is a field in `assets/structures/*.ron`, a research unlock is a field in
  `assets/research/*.ron`. Update the matching `assets/*/README.md` in the
  same change as any schema field.
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub
  const`, never inline in a formula.
- **Every refusal lands before anything is spent**, asserted **per refusal** —
  one test over one path passes against all the others.
- **Uppercase letters only for screen actions.** Lowercase are row selectors.
- Follow the repo's comment discipline: comments say *why*, never *what*.
- Gates for every task: `cargo fmt`, `cargo clippy --workspace` (no new
  warnings), `cargo test -p feral-processes-engine <name>` while iterating,
  and `cargo test --workspace` before the task's commit.
- The full plan is TDD: the failing test is written and *seen to fail*
  before the implementation.
- **Do not push.** Commit freely on the branch; the merge and the release
  are the user's call.

## Decisions this plan makes, that the spec left open

1. **The Compiler is the bench.** `assets/structures/compiler.ron` already
   carries `extracts_routines: true` and an upgrade ladder to tier 5; it
   gains `extracts_programs: true` rather than a new structure being
   authored. One bench, one idea — the thing that breaks a program down.
   Rejected: a dedicated Teardown Rig (a second build cost, research gate,
   glyph and upgrade ladder to balance, for a distinction the player would
   read as bookkeeping).
2. **Standing the bench sells speed; upgrading it sells yield.** The spec's
   formula is `tier_scale(tool.tier + structure_tier)`, and
   `TOOL_TIER_SCALE_STEP` is `0.5` — so a *freshly built* Compiler read as
   `structure_tier == 1` would be a flat +50% on the whole material economy
   for a structure most bases already have. Instead the yield term is
   `bench_tier - 1`, which is `Game::best_structure_tier`'s own documented
   rule for the craft quality floor (`game/catalog.rs:723-727`: "the term
   above the first tier is what a bench upgrade sells"), while the *tick*
   term uses the full `bench_tier` so that owning one at all is worth
   something. Yield ×1.0 fresh → ×3.0 at tier 5. Rejected: the spec-literal
   reading (a large unconditional multiplier for a structure already built),
   and a separate `EXTRACT_STRUCTURE_TIER_STEP` (tuning.rs:4390-4392 states
   the two axes are meant to share one curve).
3. **`extraction_yield` keeps its two-argument signature and reads the bench
   itself.** The spec writes `structure_tier` as a parameter. A parameter
   with exactly one correct value, derived one way, is how the screen ends
   up quoting a tier-0 figure while the act grants a tier-3 one — precisely
   the divergence section 3's "one derivation" rule exists to prevent. The
   tier is read inside, from the same world both callers already hold.
4. **A `Routines` tool draws from the species' own kit, weighted.** The pool
   is `SpeciesDef::abilities` gated to the downed program's level, minus
   anything already known (an exclusive routine is never known, so it is
   always in). The draw is random across that pool with
   `tuning::ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT` on the first entry — the
   species' own declaration order, so the earliest routine it never taught
   the player is the likely outcome without being the certain one. An empty
   pool is a refusal *before* the program is spent. Rejected: a second
   drill-down where the player picks the routine (a second pending index and
   a second refusal surface on the screen, against decision 3's "choosing
   the tool *is* the decision").
5. **The routine tool needs no bench.** Spec decision 7: a structure improves
   extraction, it never gates it. `extract_routine`'s own bench requirement
   stays exactly where it is — that is the *tamed-program* door, and a
   researched, forged, installed tool is already a steeper gate than a
   Compiler.
6. **A sortie banks programs and delivers them on return.** Today
   `game/sortie.rs:515` pushes straight into the player's store from an
   off-screen battle. `Sortie::programs` makes the trip carry them, which is
   the only reading under which a sortie is travel rather than telemetry —
   and it is what the field's own doc comment at `resources.rs:1698-1702`
   was left open for.

---

### Task 1: The bench, and what its tier is worth to a yield

**Files:**
- Modify: `crates/engine/src/structures.rs:490-494` — `extracts_programs:
  bool` beside `extracts_routines`, `#[serde(default)]`
- Modify: `crates/engine/src/game/routines.rs:464-473` — `extraction_bench_name`
  takes a predicate so one function serves both flags
- Modify: `crates/engine/src/game/extraction.rs:16-18, 109-125` — the bench
  tier and its term in the unit count
- Modify: `crates/engine/src/tuning.rs` — beside `TOOL_TIER_SCALE_STEP`
- Modify: `assets/structures/compiler.ron`, `assets/structures/README.md`
- Test: `crates/engine/src/tests/extraction.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `Game::best_structure_tier(kind: &str) -> Option<u32>`
  (`game/catalog.rs:728`, already `pub(crate)`), `Game::has_structure`
- Produces:
  - `StructureDef::extracts_programs: bool`
  - `Game::extraction_bench_tier(&self) -> u32` — `0` when no
    `extracts_programs` structure stands, otherwise the best tier standing
    (`>= 1`)
  - `Game::extraction_bench(&self) -> Option<views::ExtractionBenchView>` —
    name and tier, for the screen header (task 5 consumes it)

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/extraction.rs`:

```rust
/// The whole of decision 2's first half: a Compiler that has never been
/// upgraded is worth nothing to a yield. Without this the fresh-bench case
/// silently pays `TOOL_TIER_SCALE_STEP`'s full step, which on the shipped
/// `0.5` is +50% of the entire material economy for a structure most bases
/// already have.
#[test]
fn a_fresh_bench_does_not_change_a_yield() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 5);
    let tool = starter_tool(&game);
    let before = game.extraction_yield(&program, &tool);

    build_program_bench(&mut game, None);

    assert_eq!(game.extraction_bench_tier(), 1, "the bench is standing");
    assert_eq!(
        game.extraction_yield(&program, &tool),
        before,
        "tier 1 is the identity — only upgrades sell yield"
    );
}

#[test]
fn an_upgraded_bench_raises_a_yield_by_the_shared_tier_curve() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 5);
    let tool = starter_tool(&game);
    let before: u32 = game
        .extraction_yield(&program, &tool)
        .iter()
        .map(|(_, qty)| qty)
        .sum();

    build_program_bench(&mut game, Some(5));

    let after: u32 = game
        .extraction_yield(&program, &tool)
        .iter()
        .map(|(_, qty)| qty)
        .sum();
    assert!(
        after > before,
        "tier 5 must pay more than no bench at all: {after} vs {before}"
    );
}

/// Decision 3: the tier is read inside `extraction_yield`, so the figure
/// the screen quotes through `extraction_options` and the figure
/// `extract_program` grants move together. A parameter is how those two
/// come apart.
#[test]
fn the_previewed_yield_tracks_the_bench_tier() {
    let mut game = new_test_game();
    give_downed_program(&mut game, test_program("scrapper", 5));
    build_program_bench(&mut game, Some(4));

    let previewed = game.extraction_options(0);
    let (tool_id, quoted) = previewed.first().cloned().expect("the starter tool");
    let held_before = held_counts(&game, &quoted);

    game.extract_program(0, &tool_id).expect("the extraction runs");

    for (item, qty) in &quoted {
        assert_eq!(
            held(&game, item),
            held_before.get(item).copied().unwrap_or(0) + qty,
            "granted {item} does not match the quoted {qty}"
        );
    }
}
```

In `crates/engine/src/tests/assets.rs`, beside the tool censuses:

```rust
/// A shipped structure must actually carry the flag, or every bench term
/// in `extraction_yield` is unreachable and the phase ships as a no-op.
#[test]
fn some_shipped_structure_extracts_programs() {
    let db = structure_db();
    assert!(
        db.all().any(|def| def.extracts_programs),
        "no shipped structure sets extracts_programs"
    );
}
```

Helpers to add to `crates/engine/src/tests/extraction.rs`'s helper section
(`build_extraction_bench` at `tests/exclusive_routines.rs:556` is the model
— find the def by its flag, never by naming an id in code):

```rust
/// Stands whichever structure `Game::extraction_bench_tier` looks for, at
/// `tier` (`None` for a structure that has never been upgraded, which is
/// what `build_structure` leaves behind — see `best_structure_tier`'s doc
/// on why a missing `StructureTier` reads as tier 1).
fn build_program_bench(game: &mut Game, tier: Option<u32>) {
    let bench = game
        .world
        .resource::<StructureDb>()
        .all()
        .find(|def| def.extracts_programs)
        .map(|def| def.id.clone())
        .expect("some shipped structure extracts programs");
    let entity = spawn_structure_at(game, &bench, 30, 30);
    if let Some(t) = tier {
        game.world.entity_mut(entity).insert(StructureTier(t));
    }
}
```

`spawn_structure_at` (`tests/support.rs:856`) returns `()` today; make it
return the `Entity` it spawns. Every existing caller ignores a return value
without complaint, and a fixture that stands a structure and then cannot
address it is the reason `run_one_full_gather_cycle_at_tier`
(`support.rs:1224-1236`) had to spawn its own by hand rather than call this.

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cargo test -p feral-processes-engine -- extracts_programs a_fresh_bench an_upgraded_bench previewed_yield_tracks
```

Expected: compile failure — `extracts_programs`, `extraction_bench_tier`
and `build_program_bench` do not exist.

- [ ] **Step 3: Add the field and the asset**

`crates/engine/src/structures.rs`, immediately after `extracts_routines`:

```rust
    /// If true, owning one of these anywhere makes an *extraction* better —
    /// `Game::extract_program`, not `Game::extract_routine`. Never a gate:
    /// spec decision 7 is that extraction works in the field, at the base
    /// and in the Stack alike, because the starting tool is useless
    /// otherwise. Ownership rather than proximity, `extracts_routines`'
    /// own rule. `#[serde(default)]` so existing structure files (including
    /// mods) improve nothing, exactly as before this field existed.
    #[serde(default)]
    pub extracts_programs: bool,
```

`assets/structures/compiler.ron` — add `extracts_programs: true,` beneath
`extracts_routines: true,`, and extend the description's last sentence to
"Also the bench that extracts a routine out of a program you own, and the
one that makes tearing a downed program down faster — richer, too, with
every tier above the first."

`assets/structures/README.md` — document the field beside
`extracts_routines`, stating both halves: standing one speeds an extraction,
tiers above the first raise its yield.

- [ ] **Step 4: Add the tuning constant**

`crates/engine/src/tuning.rs`, directly beneath `TOOL_TIER_SCALE_STEP`:

```rust
/// How a standing extraction bench's tier enters `Game::extraction_yield`:
/// as `tier - 1` steps of `TOOL_TIER_SCALE_STEP`, not as `tier`.
///
/// There is no constant here on purpose — this doc is the constant. A
/// freshly built bench reads as tier 1 (`Game::best_structure_tier`: a
/// structure with no `StructureTier` and one never upgraded are the same
/// thing to a player), so a `tier` term would pay `TOOL_TIER_SCALE_STEP`'s
/// full step for a structure most bases already have, which is +50% of the
/// whole material economy on the shipped `0.5`. `tier - 1` makes the
/// *upgrade* the thing that sells yield — `best_structure_tier`'s own
/// documented rule for the craft quality floor — and leaves Task 6's
/// drop-neutrality gate (fitted with no bench standing at all) unmoved.
/// The speed half, `EXTRACT_BENCH_TICK_STEP`, uses the full tier instead:
/// owning the bench is worth something, it is just worth time rather than
/// materials.
```

- [ ] **Step 5: Implement the tier read and its term**

`crates/engine/src/game/extraction.rs`:

```rust
    /// The best tier of any standing structure whose `StructureDef::
    /// extracts_programs` is set, or `0` when none stands — never a gate
    /// (spec decision 7), only a term. Ownership rather than proximity,
    /// `Game::can_extract_routines`' rule (`game/routines.rs:456`) rather
    /// than a distance check.
    pub fn extraction_bench_tier(&self) -> u32 {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| def.extracts_programs)
            .filter_map(|def| self.best_structure_tier(&def.id))
            .max()
            .unwrap_or(0)
    }

    /// The bench a screen names, when one stands. `None` and no name when
    /// none does, rather than a "no bench" string built here — what to say
    /// about an absence is the renderer's business.
    pub fn extraction_bench(&self) -> Option<crate::views::ExtractionBenchView> {
        let tier = self.extraction_bench_tier();
        if tier == 0 {
            return None;
        }
        Some(crate::views::ExtractionBenchView {
            name: self.bench_name(|def| def.extracts_programs),
            tier,
        })
    }
```

In `extraction_yield`, replace the `scale` line:

```rust
        // The bench's term is `tier - 1`, not `tier` — a bench that has
        // never been upgraded pays nothing, and the upgrade is what sells
        // yield. See `tuning::TOOL_TIER_SCALE_STEP`'s neighbouring doc.
        let bench = self.extraction_bench_tier().saturating_sub(1);
        let scale = tier_scale(tool.tier + bench);
```

`tier_scale`'s doc gains a sentence: its argument is now the tool's tier
plus the bench's term, the one shared curve `tuning::TOOL_TIER_SCALE_STEP`
says both axes take.

- [ ] **Step 6: Generalise `extraction_bench_name`**

`crates/engine/src/game/routines.rs:464-473` becomes one function taking the
flag it looks for, since two flags now want the same "name a bench for the
message" answer:

```rust
    /// Display name of a structure carrying `flag`, for a message — no code
    /// names a structure id. `pub(crate)` because `game/extraction.rs`'s
    /// `extraction_bench` wants the same answer for the other flag.
    pub(crate) fn bench_name(&self, flag: fn(&StructureDef) -> bool) -> String {
        self.world
            .resource::<StructureDb>()
            .all()
            .find(|def| flag(def))
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "an extraction bench".to_string())
    }
```

`extract_routine`'s refusal at `routines.rs:542` becomes
`self.bench_name(|def| def.extracts_routines)`.

- [ ] **Step 7: Add the view type**

`crates/engine/src/views.rs`, beside `DownedProgramRow` (`:2535`):

```rust
/// The extraction bench a screen names — `Game::extraction_bench`. Absent
/// entirely when none stands, so a renderer never has to read a tier of
/// zero as "none".
#[derive(Clone, Debug)]
pub struct ExtractionBenchView {
    pub name: String,
    pub tier: u32,
}
```

- [ ] **Step 8: Run the tests and watch them pass**

```bash
cargo test -p feral-processes-engine -- extracts_programs a_fresh_bench an_upgraded_bench previewed_yield_tracks
```

Expected: PASS.

- [ ] **Step 9: Confirm the phase-1 economy gate is unmoved**

```bash
cargo test -p feral-processes-engine drop_neutral
```

Expected: PASS, unedited. The gate is fitted with no bench standing, where
`extraction_bench_tier()` is `0` and the term is `0` — if this test needed
an edit, the term is wrong, not the test.

- [ ] **Step 10: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/structures.rs crates/engine/src/game/extraction.rs crates/engine/src/game/routines.rs crates/engine/src/views.rs crates/engine/src/tuning.rs crates/engine/src/tests/extraction.rs crates/engine/src/tests/assets.rs assets/structures/compiler.ron assets/structures/README.md
git commit -m "feat(extraction): a bench's upgrades buy a richer teardown"
```

---

### Task 2: The bench buys speed

**Files:**
- Modify: `crates/engine/src/game/extraction.rs` — `Game::extraction_ticks`
  and `extract_program`'s tick loop (`:258-263`)
- Modify: `crates/engine/src/tuning.rs` — `EXTRACT_BENCH_TICK_STEP`
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Game::extraction_bench_tier` (task 1)
- Produces: `Game::extraction_ticks(&self, tool: &ToolDef) -> u64` — what a
  use of `tool` actually costs in ticks here and now. Task 5's preview
  quotes this; nothing re-derives it.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_standing_bench_makes_an_extraction_faster() {
    let mut game = new_test_game();
    let tool = starter_tool(&game);
    let bare = game.extraction_ticks(&tool);
    assert_eq!(bare, tool.ticks, "no bench is the tool's own figure");

    build_program_bench(&mut game, None);

    assert!(
        game.extraction_ticks(&tool) < bare,
        "a fresh bench must already be worth time: {} vs {bare}",
        game.extraction_ticks(&tool)
    );
}

#[test]
fn an_upgraded_bench_is_faster_still_and_never_free() {
    let mut game = new_test_game();
    let tool = starter_tool(&game);
    build_program_bench(&mut game, Some(1));
    let tier_one = game.extraction_ticks(&tool);
    build_program_bench(&mut game, Some(5));
    let tier_five = game.extraction_ticks(&tool);

    assert!(tier_five < tier_one, "{tier_five} vs {tier_one}");
    assert!(tier_five >= 1, "an extraction never costs zero time");
}

/// The tick cost is what the act actually spends, not just what a number
/// says. Without this the formula could be right and unwired.
#[test]
fn the_act_spends_the_bench_reduced_tick_cost() {
    let mut game = new_test_game();
    give_downed_program(&mut game, test_program("scrapper", 5));
    build_program_bench(&mut game, Some(5));
    let tool = starter_tool(&game);
    let expected = game.extraction_ticks(&tool);
    let before = game.ticks_elapsed();

    game.extract_program(0, &tool.id).expect("the extraction runs");

    assert_eq!(game.ticks_elapsed() - before, expected);
}
```

`ticks_elapsed()` is the engine's existing tick counter accessor — if it is
named otherwise in this build, use whatever `tests/extraction.rs`'s existing
tick-cost test uses; do not add a second counter.

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cargo test -p feral-processes-engine -- bench_makes_an_extraction_faster upgraded_bench_is_faster act_spends_the_bench
```

Expected: compile failure — `extraction_ticks` does not exist.

- [ ] **Step 3: Add the constant**

```rust
/// How much each tier of a standing extraction bench divides an
/// extraction's tick cost: `ticks / (1 + EXTRACT_BENCH_TICK_STEP * tier)`,
/// floored at one tick. Uses the bench's **full** tier, unlike the yield
/// term (which uses `tier - 1`) — standing the bench at all is worth time,
/// and upgrading it is worth materials. A quotient rather than a
/// subtraction so no tier can reach zero, and a floor of one so an
/// extraction is never a free action: `Game::extract_program` ticking
/// nothing would make the store a place to stand and think in, which is
/// what every other spend in the game refuses to be.
///
/// A guess, like every other number this feature ships: at `0.25` a fresh
/// bench pays `0.8x` and a tier-5 bench `0.44x`. Nothing in the repo can
/// check it — `balance_sim` models no loot and no time cost.
pub const EXTRACT_BENCH_TICK_STEP: f32 = 0.25;
```

- [ ] **Step 4: Implement**

```rust
    /// What one use of `tool` costs in ticks, here and now — `ToolDef::
    /// ticks` divided down by any standing bench's tier
    /// (`tuning::EXTRACT_BENCH_TICK_STEP`), floored at one. The one
    /// derivation, `extraction_yield`'s rule: `extract_program` spends
    /// exactly this and the screen quotes exactly this, so a promised cost
    /// and a paid one cannot differ.
    pub fn extraction_ticks(&self, tool: &ToolDef) -> u64 {
        let tier = self.extraction_bench_tier() as f32;
        let divisor = 1.0 + tuning::EXTRACT_BENCH_TICK_STEP * tier;
        ((tool.ticks as f32 / divisor).round() as u64).max(1)
    }
```

In `extract_program`, replace `for _ in 0..tool_def.ticks {` with:

```rust
        // Read before the loop, not inside it: a bench demolished by a raid
        // mid-extraction must not change what this use was already priced
        // at — `commit_caravan_basket`'s rule that a spend is quoted once.
        let ticks = self.extraction_ticks(&tool_def);
        for _ in 0..ticks {
```

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cargo test -p feral-processes-engine -- bench_makes_an_extraction_faster upgraded_bench_is_faster act_spends_the_bench
```

Expected: PASS.

- [ ] **Step 6: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/game/extraction.rs crates/engine/src/tuning.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): a bench makes the teardown quicker"
```

---

### Task 3: The routine branch, shared with `extract_routine`

**Files:**
- Modify: `crates/engine/src/game/routines.rs:536-582` — `extract_routine`
  gives up its two branches to a shared inner function
- Modify: `crates/engine/src/game/extraction.rs` — the routine pool, the
  weighted draw, and `extract_program`'s branch on category
- Modify: `crates/engine/src/tuning.rs` — `ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT`
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Game::routine_is_exclusive` (`routines.rs:188`),
  `Game::knows_routine` (`:175`), `Game::ability_display_name` (`:13`),
  `abilities::weighted_pick(&[u32], u32) -> Option<usize>`
  (`game/spawning.rs:209-210` is the call idiom)
- Produces:
  - `Game::routine_candidates(&self, program: &DownedProgram) -> Vec<AbilityId>`
  - `Game::take_routine(&mut self, ability: &str) -> RoutineTaken` — `&str`
    and not `&AbilityId`, because `abilities::AbilityId` is a `String` alias
    (`abilities.rs:10`) and its two neighbours `knows_routine` (`:175`) and
    `routine_is_exclusive` (`:188`) both take `&str`
  - `enum RoutineTaken { Learned, DiskPopped }` in `crates/engine/src/game/routines.rs`

- [ ] **Step 1: Write the failing tests**

```rust
/// The pool is the species' own kit at the program's level, minus what the
/// player already knows. A level gate that did not apply would teach a
/// level-30 routine off a level-2 kill.
#[test]
fn the_routine_pool_is_the_species_kit_at_that_level() {
    let game = new_test_game();
    let low = game.routine_candidates(&test_program("scrapper", 1));
    let high = game.routine_candidates(&test_program("scrapper", 30));
    assert!(
        low.len() <= high.len(),
        "a higher-level program cannot offer fewer routines: {low:?} vs {high:?}"
    );
    for id in &low {
        assert!(high.contains(id), "{id} vanished at a higher level");
    }
}

#[test]
fn a_routine_already_known_leaves_the_pool() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    let first = game
        .routine_candidates(&program)
        .first()
        .cloned()
        .expect("the scrapper declares a routine");
    game.world.resource_mut::<KnownRoutines>().0.insert(first.clone());

    assert!(
        !game.routine_candidates(&program).contains(&first),
        "a known routine is still being offered"
    );
}

/// The refusal, asserted the way every other refusal in this feature is:
/// nothing spent. A program consumed for a routine the player already had
/// is the exact waste `extract_routine`'s own "already known" check exists
/// to prevent (`routines.rs:520-524`).
#[test]
fn a_routine_tool_with_nothing_left_to_teach_refuses_and_spends_nothing() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    for id in game.routine_candidates(&program) {
        game.world.resource_mut::<KnownRoutines>().0.insert(id);
    }
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);
    let ticks_before = game.ticks_elapsed();

    let refusal = game.extract_program(0, &tool);

    assert!(refusal.is_err(), "it should have refused");
    assert_eq!(game.downed_program_rows().len(), 1, "the program was spent");
    assert_eq!(game.ticks_elapsed(), ticks_before, "time was spent");
}

#[test]
fn a_routine_tool_teaches_a_routine_and_consumes_the_program() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    let pool = game.routine_candidates(&program);
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);

    game.extract_program(0, &tool).expect("the extraction runs");

    assert!(game.downed_program_rows().is_empty(), "the program survived");
    assert!(
        pool.iter().any(|id| game.knows_routine(id)),
        "nothing from the pool was learned"
    );
}

/// The invariant the whole exclusive pool rests on, restated for the new
/// door: an exclusive routine taken off a downed program leaves exactly one
/// copy in the run — the disk — and teaches nothing.
#[test]
fn an_exclusive_routine_from_a_downed_program_leaves_exactly_one_copy() {
    let mut game = new_test_game();
    let (species, ability) = species_declaring_an_exclusive_routine(&game);
    let program = test_program(&species, 30);
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);

    game.extract_program(0, &tool).expect("the extraction runs");

    assert!(!game.knows_routine(&ability), "an exclusive was learned");
    assert_eq!(
        held(&game, &ItemId::etched(&ability)),
        1,
        "exactly one disk, no more and no fewer"
    );
}

/// The first entry is favoured, not guaranteed — decision 4. Run the draw
/// enough times to see both outcomes; a deterministic pick would fail this
/// and a uniform one would fail the skew assertion.
#[test]
fn the_draw_favours_the_first_candidate_without_forcing_it() {
    let mut counts: std::collections::BTreeMap<String, u32> = Default::default();
    let mut first_id = None;
    for seed in 0..200u64 {
        let mut game = test_game_with_seed(seed);
        let program = test_program("scrapper", 30);
        let pool = game.routine_candidates(&program);
        if pool.len() < 2 {
            return; // nothing to skew between; the pool census covers this
        }
        first_id.get_or_insert(pool[0].clone());
        give_downed_program(&mut game, program);
        install_routine_tool(&mut game);
        let tool = routine_tool_id(&game);
        game.extract_program(0, &tool).expect("the extraction runs");
        for id in pool {
            if game.knows_routine(&id) || held(&game, &ItemId::etched(&id)) > 0 {
                *counts.entry(id.to_string()).or_default() += 1;
            }
        }
    }
    let first = first_id.expect("a pool");
    let favoured = counts.get(first.as_str()).copied().unwrap_or(0);
    let rest: u32 = counts
        .iter()
        .filter(|(id, _)| id.as_str() != first.as_str())
        .map(|(_, n)| n)
        .sum();
    assert!(favoured > 0 && rest > 0, "the draw is deterministic: {counts:?}");
    assert!(
        favoured * 2 > rest,
        "the first candidate is not favoured: {counts:?}"
    );
}
```

`install_routine_tool` / `routine_tool_id` / `species_declaring_an_exclusive_routine`
are helpers this task adds beside `build_program_bench`. No `Routines` tool
ships until task 4 — deliberately, so that no commit on this branch ever has
a tool the player can install and reach an unimplemented branch with — so
this task's fixture builds one. `ToolDb::tools` is private and there is no
insert (`tools.rs:130-190`), so add one:

```rust
    /// A tool that no `assets/tools/` file backs, for a test that needs a
    /// category the shipped catalogue does not carry yet. `#[cfg(test)]`
    /// because a runtime caller would be authoring content in Rust, which
    /// `assets/tools/README.md` is the answer to.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: ToolDef) {
        self.tools.insert(def.id.as_str().to_string(), def);
    }
```

Task 4 rewrites these helpers to find the *shipped* tool by its category —
the `build_program_bench` rule, find by property and never by id — and
deletes `ToolDb::insert` with them unless some other test has picked it up
by then. A fixture that outlives its reason is the next person's red
herring.

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cargo test -p feral-processes-engine -- routine_pool routine_tool exclusive_routine_from_a_downed draw_favours
```

Expected: compile failure — `routine_candidates` does not exist.

- [ ] **Step 3: Extract the shared branch out of `extract_routine`**

In `crates/engine/src/game/routines.rs`:

```rust
/// Which of `Game::take_routine`'s two branches ran, so each caller words
/// its own line — the tamed-program door and the downed-program door
/// describe the same effect in different sentences, and a shared function
/// returning a shared string would make one of them wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineTaken {
    /// Ordinary: the knowledge entered `KnownRoutines`. No disk.
    Learned,
    /// Exclusive: the etched disk came back out, and nothing was learned.
    DiskPopped,
}
```

```rust
    /// The one place a routine comes off a program — `extract_routine`'s
    /// tamed-program door and `extract_program`'s `Routines`-tool door
    /// both call this rather than each carrying a copy of the two
    /// branches. Callers own their refusals and their log lines; this owns
    /// only the effect.
    ///
    /// **Exclusive routines pop the disk and teach nothing.** That is what
    /// keeps exactly one copy in the run, and it is the reason this
    /// function exists instead of two implementations that agree today.
    /// `routine_is_exclusive` reads `AbilityDef::exclusive` alone
    /// (`:188`), so nothing about the invariant rested on the program
    /// having been tamed — spec section 4 verified this before the second
    /// door was cut.
    pub(crate) fn take_routine(&mut self, ability: &str) -> RoutineTaken {
        if self.routine_is_exclusive(ability) {
            self.grant_loot(ItemId::etched(ability), 1, LootSource::Etch);
            RoutineTaken::DiskPopped
        } else {
            self.world
                .resource_mut::<KnownRoutines>()
                .0
                .insert(ability.to_string());
            RoutineTaken::Learned
        }
    }
```

`extract_routine`'s tail (`routines.rs:564-581`) becomes:

```rust
        let name = self.dissolve_tamed_program(creature);
        let ability_name = self.ability_display_name(&ability);
        match self.take_routine(&ability) {
            RoutineTaken::DiskPopped => self.log(format!(
                "You break {name} down and pry its {ability_name} disk back out intact."
            )),
            RoutineTaken::Learned => self.log(format!(
                "You break {name} down and learn its {ability_name} routine."
            )),
        }
        self.tick();
        Ok(())
```

The existing `let exclusive = self.routine_is_exclusive(&ability);` binding
stays where it is — the "already known" refusal above it still needs it, and
that refusal is `extract_routine`'s own, not the shared function's.

- [ ] **Step 4: Add the constant**

```rust
/// Relative weight on the *first* candidate in `Game::routine_candidates`
/// when a `Routines` tool draws — every other candidate weighs 1. The
/// species' own declaration order is the ranking, so the earliest routine
/// it has never taught the player is the likely outcome without being the
/// certain one (plan decision 4). At `3` against a two-candidate pool the
/// first wins three draws in four.
///
/// A guess. Deterministic (weight ∞) would make a `Routines` tool a lookup
/// table the player memorises; uniform would make the kit's own ordering
/// mean nothing.
pub const ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT: u32 = 3;
```

- [ ] **Step 5: Implement the pool and the branch**

In `crates/engine/src/game/extraction.rs`:

```rust
    /// What a `Routines` tool could take out of `program`: every routine
    /// its species declares at or below the program's own level, in the
    /// species file's order, minus anything already known. An exclusive
    /// routine is never known, so it is always in — and it is the one thing
    /// here that cannot be got any other way.
    ///
    /// The level gate is `install_innate_routines`' own
    /// (`game/combat.rs:789`), read off the same `SpeciesDef::abilities`:
    /// a downed program carries no `Routines` component to read, so what it
    /// *would* have been carrying is derived from its species and level
    /// rather than stored — the "derived, never stored" rule that kept
    /// `DownedProgram` a five-field record.
    ///
    /// Deduplicated, because a species may declare the same id at two
    /// levels and a pool with a repeat would weight it twice by accident.
    pub fn routine_candidates(&self, program: &DownedProgram) -> Vec<AbilityId> {
        let Some(species) = self.world.resource::<SpeciesDb>().get(&program.species) else {
            return Vec::new();
        };
        let db = self.world.resource::<AbilityDb>();
        let mut pool: Vec<AbilityId> = Vec::new();
        for declared in &species.abilities {
            if declared.level > program.level {
                continue;
            }
            if db.get(&declared.id).is_none() {
                continue;
            }
            if self.knows_routine(&declared.id) {
                continue;
            }
            if pool.contains(&declared.id) {
                continue;
            }
            pool.push(declared.id.clone());
        }
        pool
    }
```

The draw, and `extract_program`'s branch. In `extract_program`, after the
`tool_def` refusal and **before** anything is removed:

```rust
        // The `Routines` category takes the other branch entirely: no
        // `yields` pool is read, and the refusal below has to land here,
        // above the removal, or a program is spent teaching nothing.
        if tool_def.category == ToolCategory::Routines {
            return self.extract_routine_from_program(index, &program, &tool_def);
        }
```

```rust
    /// A `Routines` tool's use: one routine off `program`, drawn from
    /// `routine_candidates` with `tuning::ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT`
    /// on the first, then `take_routine`'s two branches.
    ///
    /// The `GameRng` draw is the reason this is here rather than inside
    /// `extraction_yield`: that function is `&self` precisely so the
    /// screen's preview can call it with nothing spent, and a preview that
    /// consumed a random draw would make what a player *gets* depend on
    /// whether they looked at the menu first. The screen quotes the pool
    /// instead of an outcome (`views::ExtractionPreview::Routine`), which
    /// is the honest thing to show for a draw that has not happened yet.
    fn extract_routine_from_program(
        &mut self,
        index: usize,
        program: &DownedProgram,
        tool: &ToolDef,
    ) -> Result<(), String> {
        let pool = self.routine_candidates(program);
        if pool.is_empty() {
            return Err("You already know everything that program can teach.".to_string());
        }
        let weights: Vec<u32> = pool
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i == 0 {
                    tuning::ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT
                } else {
                    1
                }
            })
            .collect();
        let total: u32 = weights.iter().sum();
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..total)
        };
        let picked = crate::abilities::weighted_pick(&weights, roll)
            .map(|i| pool[i].clone())
            .unwrap_or_else(|| pool[0].clone());

        let player = self.player_entity();
        self.world
            .get_mut::<DownedPrograms>(player)
            .unwrap()
            .0
            .remove(index);

        let label = self.downed_program_label(program);
        let ability_name = self.ability_display_name(&picked);
        match self.take_routine(&picked) {
            RoutineTaken::DiskPopped => self.log_kind(
                MessageKind::Loot,
                format!(
                    "You read {label} out with the {}: its {ability_name} disk comes back intact.",
                    tool.name
                ),
            ),
            RoutineTaken::Learned => self.log_kind(
                MessageKind::Loot,
                format!(
                    "You read {label} out with the {}: you learn its {ability_name} routine.",
                    tool.name
                ),
            ),
        }

        let ticks = self.extraction_ticks(tool);
        for _ in 0..ticks {
            if self.is_game_over().is_some() || self.has_active_battle() {
                break;
            }
            self.tick();
        }
        Ok(())
    }
```

- [ ] **Step 6: Run the tests and watch them pass**

```bash
cargo test -p feral-processes-engine -- routine_pool routine_tool exclusive_routine_from_a_downed draw_favours
```

Expected: PASS.

- [ ] **Step 7: Confirm the tamed-program door still behaves**

```bash
cargo test -p feral-processes-engine -- extract_routine exclusive
```

Expected: PASS, unedited. `extract_routine` kept every refusal and both log
lines; only the effect moved. If one of these needs an edit, the shared
function took something that was not shared.

- [ ] **Step 8: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/game/routines.rs crates/engine/src/game/extraction.rs crates/engine/src/tuning.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): a tool that reads the routine out"
```

---

### Task 4: The shipped `Routines` tool and its censuses

**Files:**
- Add: `assets/tools/routine_reader.ron`
- Modify: `assets/research/` — one node's `unlocks_tools`
- Modify: `assets/tools/README.md`
- Modify: `crates/engine/src/tests/assets.rs`
- Modify: `crates/engine/src/tests/extraction.rs` — the task-3 helpers stop
  hand-building a `ToolDef` and find the shipped one by category

**Interfaces:**
- Consumes: `ToolCategory::Routines` (`tools.rs:54`), `ResearchDef::unlocks_tools`
- Produces: a shipped tool whose `category` is `Routines`

- [ ] **Step 1: Write the failing census**

In `crates/engine/src/tests/assets.rs`, beside the existing tool censuses:

```rust
/// The `Routines` branch is unreachable content until some shipped tool is
/// in that category — `every_non_routines_tool_has_a_non_empty_yield_pool`
/// only says what a `Routines` tool is *exempt* from, never that one
/// exists.
#[test]
fn a_shipped_tool_reads_routines() {
    let db = tool_db();
    assert!(
        db.iter().any(|def| def.category == ToolCategory::Routines),
        "no shipped tool takes the routine branch"
    );
}

/// A `Routines` tool reads no yield pool at all, so a populated one is
/// authored content that silently never runs.
#[test]
fn a_routines_tool_ships_an_empty_yield_pool() {
    for def in tool_db().iter().filter(|d| d.category == ToolCategory::Routines) {
        assert!(
            def.yields.is_empty(),
            "{} is a Routines tool with a yield pool that will never be read",
            def.id
        );
    }
}
```

The phase-2 reachability census
(`every_shipped_tool_other_than_the_starter_is_named_by_some_research_node`)
already covers the research door and needs no edit — it will fail on its own
until step 3 names the new tool, which is the point of writing it that way.

- [ ] **Step 2: Run the censuses and watch them fail**

```bash
cargo test -p feral-processes-engine -- a_shipped_tool_reads_routines routines_tool_ships_an_empty
```

Expected: FAIL — no shipped tool is in the category.

- [ ] **Step 3: Author the tool**

`assets/tools/routine_reader.ron` — follow `core_tap.ron`'s comment style,
including its explicit note that the numbers are untuned:

```ron
(
    id: "routine_reader",
    name: "Routine Reader",
    description: "Reads a downed process for the routines it was running, and keeps one.",
    category: Routines,
    // No `yields`: a Routines tool reads no pool. What comes out is drawn
    // from the program's own species kit — see `Game::routine_candidates`.
    tier: 1,
    // Longer than the material tools: reading a program out is the slowest
    // thing you can do to one. Untuned, like every other figure here.
    ticks: 40,
    forge_cost: [("logic_wafer", 4), ("core_fragment", 12)],
)
```

Check the two ids against `assets/items/` before committing — the
`forge_cost` census will catch a typo, but only if it is run.

- [ ] **Step 4: Put a research door on it**

Add `unlocks_tools: ["routine_reader"]` to `assets/research/cortex.ron`
("Cortex Hacking", cost 125, `min_zone: 3`, requires `neural_amp`) and
extend its `description` to name the tool, the way `deep_analysis.ron`'s
description names the Core Tap. Getting inside a program's head is already
that node's whole subject, and zone 3 is late enough that the Reader does
not undercut `extract_routine` — which costs a whole *tamed* program — in
the early game. The named alternative is `neural_amp.ron` ("Neural
Interfacing", zone 2), rejected as too early for exactly that reason. This
placement is a proposal: **say in the commit body which node you used** so a
reviewer can disagree with the reading rather than with the diff.

- [ ] **Step 5: Document the schema**

`assets/tools/README.md` — the `Routines` category's own paragraph: what an
empty `yields` means, that the pool comes from the program's species kit at
its level, and that the first candidate is favoured rather than certain.

- [ ] **Step 6: Rewrite the task-3 helpers against the shipped tool**

`install_routine_tool` and `routine_tool_id` find the def by
`category == ToolCategory::Routines` and install it directly into
`components::Tools`, the way `build_program_bench` finds its structure by
flag. Delete the hand-built `ToolDef` and any `#[cfg(test)]` `ToolDb` insert
added for it — a test fixture that outlives its reason is the next person's
red herring.

- [ ] **Step 7: Run the censuses and the task-3 tests**

```bash
cargo test -p feral-processes-engine -- a_shipped_tool_reads_routines routines_tool_ships_an_empty routine_tool unlocks_tools forge_cost
```

Expected: PASS, including the phase-2 reachability census.

- [ ] **Step 8: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add assets/tools/routine_reader.ron assets/tools/README.md assets/research crates/engine/src/tests/assets.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): the Routine Reader"
```

---

### Task 5: The preview learns about benches and routines

**Files:**
- Modify: `crates/engine/src/views.rs` — `ExtractionOptionView`,
  `ExtractionPreview`
- Modify: `crates/engine/src/game/extraction.rs:163-176` —
  `extraction_options` returns the new view
- Modify: `crates/gui/src/render/extraction.rs:93-127` —
  `extraction_options_rows`
- Modify: `crates/app-core/src/app/extraction.rs:43-56` — the tool index
  read
- Modify: `crates/engine/src/tests/extraction.rs` — **every existing test
  that destructures `extraction_options`' tuple**, task 1's
  `the_previewed_yield_tracks_the_bench_tier` among them. The return type
  changes here; those tests are correct as written against the shape that
  exists when they are written, and updating them is this task's work, not
  a sign either task got it wrong.
- Test: `crates/gui/src/render/extraction.rs`'s own test module (`:313-330`),
  `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Game::extraction_yield`, `Game::extraction_ticks` (task 2),
  `Game::routine_candidates` (task 3), `Game::extraction_bench` (task 1)
- Produces:

```rust
/// One installed tool and what it would do to the program on the block —
/// `Game::extraction_options`. Carries the tool's display name so the
/// renderer joins nothing: the phase-1 rows zipped this list against
/// `installed_tools()` positionally, which was structural but left the
/// renderer holding two sequences that had to stay in step.
#[derive(Clone, Debug)]
pub struct ExtractionOptionView {
    pub tool: ToolId,
    pub name: String,
    /// What this use costs in time, bench discount already applied.
    pub ticks: u64,
    pub preview: ExtractionPreview,
}

#[derive(Clone, Debug)]
pub enum ExtractionPreview {
    /// `extraction_yield`'s own rows, granted verbatim. Empty when the
    /// grade rounds to no units at all.
    Items(Vec<(ItemId, u32)>),
    /// A `Routines` tool: the pool the draw comes from, in
    /// `routine_candidates`' order (the first is the favourite), as display
    /// names. An outcome cannot be quoted — the draw has not happened, and
    /// making it happen to fill a menu row would let looking at the menu
    /// change what you get.
    Routine(Vec<String>),
    /// A `Routines` tool with an empty pool: the refusal
    /// `extract_program` would answer with, shown before it is spent.
    NothingToLearn,
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/extraction.rs`:

```rust
#[test]
fn a_routine_tools_preview_names_the_pool_it_draws_from() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    let pool = game.routine_candidates(&program);
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);

    let option = game
        .extraction_options(0)
        .into_iter()
        .find(|o| o.tool == routine_tool_id(&game))
        .expect("the routine tool is installed");

    match option.preview {
        ExtractionPreview::Routine(names) => {
            assert_eq!(names.len(), pool.len(), "the pool and the preview disagree")
        }
        other => panic!("expected a routine preview, got {other:?}"),
    }
}

#[test]
fn a_previews_tick_cost_is_the_one_the_act_spends() {
    let mut game = new_test_game();
    give_downed_program(&mut game, test_program("scrapper", 5));
    build_program_bench(&mut game, Some(3));
    let option = game.extraction_options(0).remove(0);
    let before = game.ticks_elapsed();

    game.extract_program(0, &option.tool).expect("it runs");

    assert_eq!(game.ticks_elapsed() - before, option.ticks);
}
```

In `crates/gui/src/render/extraction.rs`'s test module, extend the two
existing layout tests so the worst case includes the bench header line and
a `Routines` row, then **verify by mutation**: add a row to the fixture by
hand and confirm the height test fails.

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cargo test -p feral-processes-engine -- routine_tools_preview previews_tick_cost
```

Expected: compile failure — `extraction_options` still returns tuples.

- [ ] **Step 3: Implement the view**

`extraction_options` becomes:

```rust
    /// Every installed tool and what it would do to the program at `index`,
    /// in `installed_tools`' own slot order. Every figure on a row is a
    /// call into the one derivation that the act itself uses —
    /// `extraction_yield`, `extraction_ticks`, `routine_candidates` — so
    /// the screen and the grant cannot disagree. Empty for an index the
    /// store doesn't hold, rather than a panic.
    pub fn extraction_options(&self, index: usize) -> Vec<crate::views::ExtractionOptionView> {
        let player = self.player_entity();
        let Some(program) = self
            .world
            .get::<DownedPrograms>(player)
            .and_then(|held| held.0.get(index))
            .cloned()
        else {
            return Vec::new();
        };
        self.installed_tools()
            .into_iter()
            .map(|tool| {
                let preview = if tool.category == ToolCategory::Routines {
                    let pool = self.routine_candidates(&program);
                    if pool.is_empty() {
                        crate::views::ExtractionPreview::NothingToLearn
                    } else {
                        crate::views::ExtractionPreview::Routine(
                            pool.iter()
                                .map(|id| self.ability_display_name(id))
                                .collect(),
                        )
                    }
                } else {
                    crate::views::ExtractionPreview::Items(self.extraction_yield(&program, &tool))
                };
                crate::views::ExtractionOptionView {
                    ticks: self.extraction_ticks(&tool),
                    name: tool.name.clone(),
                    tool: tool.id.clone(),
                    preview,
                }
            })
            .collect()
    }
```

- [ ] **Step 4: Update the renderer**

`extraction_options_rows` drops the positional `zip` with
`installed_tools()` entirely — the name is on the row now — and gains a
bench line beneath the program line:

```rust
    let bench = match game.extraction_bench() {
        Some(b) => format!("{} tier {} — faster, and richer above tier 1.", b.name, b.tier),
        None => "No extraction bench standing.".to_string(),
    };
```

Each option row's text:

```rust
        let outcome = match &option.preview {
            ExtractionPreview::Items(rows) if rows.is_empty() => "nothing usable".to_string(),
            ExtractionPreview::Items(rows) => rows
                .iter()
                .map(|(item, qty)| format!("{qty} {}", game.item_name(item)))
                .collect::<Vec<_>>()
                .join(", "),
            ExtractionPreview::Routine(names) => format!("a routine — {}", names.join(" / ")),
            ExtractionPreview::NothingToLearn => "nothing left to teach".to_string(),
        };
        let label = format!(
            "[{}] {}: {outcome} ({} ticks)",
            menu_shortcut(i),
            option.name,
            option.ticks
        );
```

- [ ] **Step 5: Update the key handler**

`crates/app-core/src/app/extraction.rs:53` — `options[tool_idx].0.clone()`
becomes `options[tool_idx].tool.clone()`. Nothing else about the handler
changes: the row count is still re-read from the engine on every keypress
and still never cached.

- [ ] **Step 6: Run the tests and watch them pass**

```bash
cargo test -p feral-processes-engine -- routine_tools_preview previews_tick_cost
cargo test -p feral-processes-gui extraction
```

Expected: PASS.

- [ ] **Step 7: Verify the height test by mutation**

Add a tenth row to the layout fixture by hand, run
`cargo test -p feral-processes-gui the_tallest_extraction_options_page_fits`,
confirm it **fails**, then revert the fixture. A layout test that cannot
fail is not a layout test.

- [ ] **Step 8: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/views.rs crates/engine/src/game/extraction.rs crates/gui/src/render/extraction.rs crates/app-core/src/app/extraction.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): the picker quotes the bench, the clock and the pool"
```

---

### Task 6: A sortie brings its programs home

**Files:**
- Modify: `crates/engine/src/resources.rs:1682-1708` — `Sortie::programs`,
  and `Sortie::loot`'s doc comment, and `test_stub` (`:1729-1743`)
- Modify: `crates/engine/src/save.rs:261-282` — `SortieSave::programs`,
  `#[serde(default)]`, and `loot`'s doc comment
- Modify: `crates/engine/src/game/lifecycle.rs:1237` (load) and `:1489-1500`
  (save) — the field in both directions
- Modify: `crates/engine/src/game/sortie.rs:505-515` — bank instead of push
- Modify: `crates/engine/src/game/sortie.rs:592-616` — `return_sortie`
  delivers
- Modify: `crates/engine/src/tests/sorties.rs` — **`a_sortie_kill_leaves_a_
  downed_program_on_the_player` (`:770-803`) asserts exactly the behaviour
  this task changes.** It must be rewritten, not deleted: keep its seed
  sweep and keep its habitat-pool assertion (a program landing outside the
  pool `resolve_sortie_battle` draws from is a defect a count-only test
  cannot see), and move the assertion from the player's store mid-trip to
  `Sorties::0[0].programs`. Degrading it to a count while it is open is the
  easy mistake here.

**Interfaces:**
- Consumes: `Game::push_downed_program` (`game/combat_rewards.rs:562`),
  `Game::leave_downed_program`'s roll
- Produces: `Sortie::programs: Vec<DownedProgram>`,
  `SortieSave::programs: Vec<DownedProgram>`

- [ ] **Step 1: Write the failing tests**

```rust
/// The whole point of the change: a kill six screens away does not appear
/// in the pack the instant it lands.
#[test]
fn a_sorties_kill_does_not_reach_the_store_until_the_squad_returns() {
    let mut game = sortie_in_flight();
    run_one_sortie_battle(&mut game);

    assert!(
        game.downed_program_rows().is_empty(),
        "a program teleported home"
    );
    assert!(
        !game.world.resource::<Sorties>().0[0].programs.is_empty(),
        "the trip is not carrying anything"
    );
}

#[test]
fn a_returning_sortie_delivers_its_programs() {
    let mut game = sortie_in_flight();
    run_one_sortie_battle(&mut game);
    let carried = game.world.resource::<Sorties>().0[0].programs.len();

    run_until_the_sortie_returns(&mut game);

    assert_eq!(game.downed_program_rows().len(), carried);
}

/// Spec decision 9 at the delivery door: a full store refuses, logs once,
/// and destroys nothing it is already holding.
#[test]
fn a_full_store_refuses_a_delivery_and_destroys_nothing_held() {
    let mut game = sortie_in_flight();
    run_one_sortie_battle(&mut game);
    fill_downed_program_store(&mut game);
    let held_before = game.downed_program_rows();

    run_until_the_sortie_returns(&mut game);

    assert_eq!(
        game.downed_program_rows(),
        held_before,
        "a delivery displaced something already held"
    );
    assert_eq!(
        log_lines_containing(&game, "No room to carry").len(),
        1,
        "the refusal should be said once, not once per program"
    );
}

/// A save→load round trip, not a RON round trip — a RON round trip cannot
/// catch a `#[serde(skip)]`.
#[test]
fn a_save_round_trip_preserves_a_sorties_carried_programs() {
    let mut game = sortie_in_flight();
    run_one_sortie_battle(&mut game);
    let before = game.world.resource::<Sorties>().0[0].programs.clone();

    let loaded = save_and_load(&mut game);

    assert_eq!(loaded.world.resource::<Sorties>().0[0].programs, before);
}
```

The fixtures already exist and none of these tests should build their own:
`a_dispatched_sortie(seed, DifficultyMode::Forgiving) -> (Game, Vec<Entity>)`
(`tests/sorties.rs:735`) stands a Relay, funds a Depot and dispatches three
programs; a trip is advanced with `game.wait()` in a
`for _ in 0..(ticks_total - 1)` loop and *returned* by running past
`ticks_total` (`:789-791, :906` are the two idioms). `sortie_in_flight`,
`run_one_sortie_battle` and `run_until_the_sortie_returns` above are names
for those two loops — write them as thin helpers over `a_dispatched_sortie`
rather than as new fixtures, and note that whether any single battle lands a
kill depends on the habitat draw, so a test that needs a kill sweeps seeds
the way `a_sortie_kill_leaves_a_downed_program_on_the_player` already does.

`log_lines_containing` is whatever the sortie tests already use to read the
log; check before adding one. Remember `message_history` condenses repeats —
if the assertion counts entries, one line logged twice reads as one, so this
test must not be phrased as "exactly one entry" against a mechanism that
could have logged three; the `break` in step 4 is what actually holds it.

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cargo test -p feral-processes-engine -- sortie_delivers sorties_kill_does_not_reach full_store_refuses_a_delivery carried_programs
```

Expected: compile failure — `Sortie::programs` does not exist.

- [ ] **Step 3: Add the field, in three places**

`resources.rs`:

```rust
    /// Downed programs the squad is carrying home — banked at the kill and
    /// delivered by `return_sortie`, never written to the player's store
    /// mid-trip. A sortie is travel: a kill six screens away appearing in
    /// the pack the instant it lands would make the trip telemetry rather
    /// than a journey, and would let the store's own cap be hit by
    /// something the player was not present for.
    pub programs: Vec<crate::items::DownedProgram>,
```

`Sortie::loot`'s doc comment is corrected while the file is open: it stays
empty, and `programs` is the record phase 3 added — the field itself is kept
only so a save written mid-branch still parses.

`save.rs` — `pub programs: Vec<crate::items::DownedProgram>` with
`#[serde(default)]`, and the same correction to `SortieSave::loot`'s doc
comment (`:274-278` currently says phase 3 refills *it*; phase 3 adds a
sibling instead).

`resources.rs`'s `test_stub` (`:1729`) gets `programs: Vec::new()` — the
struct is spelled out there precisely so a new field is a compile error
rather than a silent default.

`lifecycle.rs` both directions: `programs: s.programs` on load (`:1237`),
`programs: s.programs.clone()` on save (`:1497`).

- [ ] **Step 4: Bank at the kill, deliver at the return**

`game/sortie.rs:505-515` — replace the `self.leave_downed_program(hostile);`
call and its comment:

```rust
            // Banked onto the trip, not pushed into the player's store: the
            // squad is carrying these home, and `return_sortie` is where
            // they arrive. `downed_program_for` is the same roll the field
            // kill uses, called rather than copied — `Perk::Teardown`'s old
            // trap.
            if let Some(program) = self.downed_program_for(hostile) {
                banked.push(program);
            }
```

with `let mut banked: Vec<DownedProgram> = Vec::new();` above the loop and
`sortie.programs.append(&mut banked);` beside `sortie.kills += ...` in the
block that already takes `&mut` on the resource.

This needs `leave_downed_program` (`combat_rewards.rs:533`) split at its
last line: the roll becomes `downed_program_for(&mut self, wild: Entity) ->
Option<DownedProgram>` — everything above the `push_downed_program` call —
and `leave_downed_program` becomes `self.downed_program_for(wild).is_some_and(|p|
self.push_downed_program(p))`. Both doors keep the identical roll, which is
the whole reason the function was one call rather than two copies.

`return_sortie` (`:592`), after the "came back" line:

```rust
        let carried = sortie.programs.len();
        let mut delivered = 0;
        for program in sortie.programs {
            // Stops at the first refusal rather than trying each: once the
            // store is full it stays full, so continuing would log the same
            // line once per remaining program. `push_downed_program` says
            // it once and spec decision 9 keeps everything already held.
            if !self.push_downed_program(program) {
                break;
            }
            delivered += 1;
        }
        if carried > 0 {
            self.log_base(format!(
                "They brought back {delivered} downed program(s) of {carried}."
            ));
        }
```

Word that line properly rather than shipping `program(s)` — follow whatever
pluralisation idiom the surrounding log lines use.

- [ ] **Step 5: Run the tests and watch them pass**

```bash
cargo test -p feral-processes-engine -- sortie_delivers sorties_kill_does_not_reach full_store_refuses_a_delivery carried_programs
```

Expected: PASS.

- [ ] **Step 6: Run the whole sortie and save suites**

```bash
cargo test -p feral-processes-engine -- sortie save
```

Expected: PASS. A save test failing here means the field was added to one
direction and not the other.

- [ ] **Step 7: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/resources.rs crates/engine/src/save.rs crates/engine/src/game/lifecycle.rs crates/engine/src/game/sortie.rs crates/engine/src/game/combat_rewards.rs crates/engine/src/tests
git commit -m "feat(extraction): a sortie carries its programs home"
```

---

### Task 7: Documentation and the release

**Files:**
- Modify: `CHANGELOG.md` — a new `## X.Y.Z` section; the digit is decided by
  `CHANGELOG.md`'s own preamble. No save-format break, so a minor at most.
  Write the heading against `origin/main`'s version, not the newest local
  tag — the local `main` ref is stale on this machine.
- Modify: `Cargo.toml` — the workspace version bump, **at the merge, not on
  the branch**
- Modify: `CLAUDE.md` and `AGENTS.md` — one sentence per new seam. They are
  gitignored twins with no tracking to catch drift, so edit `CLAUDE.md` then
  `cp CLAUDE.md AGENTS.md`. **They cannot ride a branch out of a worktree** —
  land these in the primary checkout or hand them back.
- Modify: `docs/seams.md` and `.claude/skills/seams/` — the argument and the
  trap behind each new seam
- Modify: `docs/superpowers/specs/2026-09-04-program-extraction-design.md` —
  an amendment note under section 3 recording that `structure_tier` is read
  inside `extraction_yield` rather than passed, and that the yield term is
  `tier - 1` while the tick term is `tier` (plan decisions 2 and 3). The
  spec is the thing phases 4 and 5 will be read against; an undocumented
  divergence from its formula is how phase 5 re-derives the wrong one.
- Do **not** touch `docs/manual.md` or the root `README.md`; both are carved
  out of the doc obligation. `assets/help/` is not carved out, but no help
  page covers extraction at all yet — that is its own piece of work.

**Steps:**

- [ ] **Step 1: Write the seam sentences**

Four:
1. `extraction_yield` and `extraction_ticks` both read
   `extraction_bench_tier()` themselves; neither takes it as an argument,
   because a caller that could pass a different tier is how a quoted figure
   and a granted one come apart.
2. The bench's yield term is `tier - 1` and its tick term is `tier` —
   standing one buys time, upgrading one buys materials. Neither is a gate.
3. `take_routine` is the only place a routine comes off a program; both
   doors (`extract_routine`'s tamed program, `extract_program`'s `Routines`
   tool) call it, and the exclusive branch popping a disk instead of
   teaching is what keeps exactly one copy in the run.
4. A sortie banks programs onto the trip and delivers them in
   `return_sortie`; nothing writes the player's store from an off-screen
   battle.

- [ ] **Step 2: Amend the spec**

Add the amendment note to section 3, dated, in the style of section 6's
existing `**amended 2026-09-04**` note.

- [ ] **Step 3: Write the changelog section**

One bullet per player-visible change: the Compiler improves extraction, the
Routine Reader, sorties bringing programs home. Say plainly in the section
that none of it has been played.

- [ ] **Step 4: Full gates**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add CHANGELOG.md docs/seams.md docs/superpowers/specs/2026-09-04-program-extraction-design.md .claude/skills/seams
git commit -m "docs(extraction): phase 3's seams"
```

---

## Not in this phase

Named so an implementer does not build them speculatively: the bulk
work-order path (`Carrying`, `Stock`, `collect::plan_adjacent_take` and work
orders becoming instance-aware — phase 4, larger than 1-3 together), and
gear drops moving behind extraction (phase 5). Also not here: turning
`FIGHT_CONDITION_WEIGHT` on (it ships at `0.0` deliberately and wants a play
session behind any other value), replacing `MAX_DOWNED_PROGRAMS`' flat count
with a carried-weight metric, and a help page for extraction.
