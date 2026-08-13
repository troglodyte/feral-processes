# Research Cost and Zone Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Slow the research tree by gating twelve of its twenty-one nodes behind zones 2 and 3 and repricing the ladder from 561 to 1258 Research Data.

**Architecture:** One new `#[serde(default)] min_zone: u32` on `ResearchDef`, checked in `Game::unlock_research` and surfaced through `views::ResearchState::Locked`. Everything else is `.ron` data. No new system, no save-format change, no change to how Research Data is produced.

**Tech Stack:** Rust, `bevy_ecs` (standalone, no Bevy app), `serde` + `ron` for assets.

**Spec:** `docs/superpowers/specs/2026-08-13-research-cost-and-zone-gate-design.md` — read it first. It carries the reasoning for every number and, more importantly, for the two things that must not regress (the softlock guard and the "a gated node stays listed" rule).

## Global Constraints

- **Read `CLAUDE.md` before starting.** It is the project's standing rules and this plan assumes them rather than repeating them.
- **Never push, tag, bump the version, or merge.** Commits on the branch are expected and encouraged; anything outward-facing needs an explicit ask from the user first. Do not infer intent from commit authorship or release conventions.
- **Branch:** `research-cost-and-zone-gate`, already created off `main` (0.8.14). Do not switch branches; `TODO.md` carries an unrelated uncommitted edit belonging to the user — leave it alone and never `git add -A`.
- **The version bump and the `CHANGELOG.md` section happen at merge, not on the branch.** Per `CLAUDE.md`, branch commits stay unversioned. Target is **0.8.15** — a patch, because at `0.x` "breaking" means a player's save stops loading, and a defaulted `.ron` field is not that.
- **`#[serde(default)]` is mandatory** on the new field. A third-party mod's research file must keep parsing untouched.
- **A malformed `.ron` is skipped with a logged warning, never a panic.** `ResearchDb::load_dir` already does this; don't add a validation path that aborts.
- **Gates before calling anything done:** `cargo test --workspace`, then `cargo clippy --workspace` and `cargo fmt`. Fix warnings rather than silencing them.
- **Iterate with `cargo test -p feral-processes-engine research`**, not the full suite. The engine suite is ~24s; a warm `cargo check` is ~1s.
- **If many tests fail at once with `NotFound` on an assets path**, that is stale build artifacts from an old directory rename, not real failures. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — not a full `cargo clean`.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `crates/engine/src/research.rs` | `ResearchDef.min_zone` + its schema test | 1 |
| `assets/research/README.md` | The schema modders read; the two new rules | 1 |
| `assets/research/*.ron` (21 files) | The band and price data — the actual deliverable | 2 |
| `crates/engine/src/tests/research.rs` | Behaviour tests and the two censuses | 2, 3 |
| `crates/engine/src/views.rs` | `ResearchState::Locked` gains the zone | 3 |
| `crates/engine/src/game/unlocks.rs` | The gate: `research_nodes` reports it, `unlock_research` refuses on it | 3 |
| `crates/gui/src/render/progression.rs` | The locked-node label | 3 |

Tasks are ordered so the tree is never in a state that fails to build or fails its own suite. Task 2 lands data that nothing yet reads — that is deliberate and called out in the task.

---

### Task 1: The `min_zone` field

Add the field and nothing that reads it. This task's whole deliverable is that an existing research file still parses and a new one *can* declare a zone.

**Files:**
- Modify: `crates/engine/src/research.rs` — `ResearchDef` (struct at ~line 29) and its `mod tests`
- Modify: `assets/research/README.md` — the schema block and the Rules list

**Interfaces:**
- Produces: `ResearchDef::min_zone: u32`, `#[serde(default)]`. **0 means ungated.** Later tasks compare it against `ZoneLevel.0`, which is 1-based, so a node with `min_zone: 2` is buyable from the moment the player's zone reaches 2. Do not use `Option<u32>` here — 0 and "absent" mean the same thing and a second spelling for it is a second thing to get wrong.

- [ ] **Step 1: Write the failing schema test**

In `research.rs`'s `mod tests`, add a test that a node declaring `min_zone: 3` loads with that value, and extend the existing `a_valid_node_loads_with_defaulted_optional_fields` to assert `min_zone == 0` when the field is absent. Use the module's existing `load()` helper — it writes temp `.ron` files and validates against the real `StructureDb` and `AbilityDb`.

- [ ] **Step 2: Run it and watch it fail**

`cargo test -p feral-processes-engine research::tests -- --nocapture`
Expected: a compile error — no field `min_zone` on `ResearchDef`.

- [ ] **Step 3: Add the field**

One field on `ResearchDef` with `#[serde(default)]` and a doc comment saying what 0 means and that it is compared against the player's current zone. Follow the doc-comment density of the fields around it.

- [ ] **Step 4: Run it and watch it pass**

`cargo test -p feral-processes-engine research`
Expected: PASS, including `the_shipped_tree_loads_clean` — every shipped file omits the field, so every node defaults to 0.

- [ ] **Step 5: Update the schema doc**

`assets/research/README.md`: add `min_zone` to the schema block with a comment in the style of the fields already there, and add a bullet to **Rules** stating that a node must not be gated below its own prerequisite (the prereq lock would outlive the zone lock, so the gate could never fire). Per `CLAUDE.md`, the schema doc moves in the same change as the schema — this is not a follow-up.

- [ ] **Step 6: Commit**

`git add crates/engine/src/research.rs assets/research/README.md`
Message: `feat(research): add min_zone to ResearchDef`

---

### Task 2: The bands and the prices

Pure content plus the two censuses that guard it. Nothing reads `min_zone` yet — the gate arrives in Task 3. Landing the data first means Task 3's behaviour tests can assert against the real shipped tree instead of a hand-built fixture, which is what this repo prefers and what makes a passing test evidence about the game.

**Files:**
- Modify: all 21 files in `assets/research/`
- Modify: `crates/engine/src/tests/research.rs` — two censuses
- Modify: `crates/engine/src/research.rs` — `the_shipped_tree_loads_clean` asserts `cortex` costs 45
- Check: `crates/app-core/src/tests/research.rs` — `picking_an_unaffordable_research_node_reports_why_and_stays_open` is priced against the current ladder

**Interfaces:**
- Produces: the shipped tree's bands. Task 3's tests find their subject by *querying* the db for the cheapest node with a given `min_zone` rather than hardcoding an id, so a later retune does not break them.

**The data.** Every row; `min_zone` omitted entirely where the band is 1 (do not write `min_zone: 0` — absent is the idiom, and it keeps the ungated files unchanged apart from cost).

| File | `min_zone` | cost: from → to |
|---|---|---|
| `automation.ron` | — | 8 → 8 |
| `power_grid.ron` | — | 10 → 10 |
| `commerce.ron` | — | 12 → 14 |
| `self_exec.ron` | — | 12 → 14 |
| `fortification.ron` | — | 15 → 18 |
| `field_ops.ron` | — | 16 → 20 |
| `armor_bench.ron` | — | 18 → 24 |
| `weapon_bench.ron` | — | 18 → 24 |
| `routine_fabrication.ron` | — | 20 → 26 |
| `overclock.ron` | 2 | 22 → 45 |
| `firewall.ron` | 2 | 22 → 45 |
| `neural_amp.ron` | 2 | 25 → 55 |
| `runtime_patching.ron` | 2 | 28 → 60 |
| `adaptive_plating.ron` | 2 | 32 → 70 |
| `program_refactoring.ron` | 2 | 34 → 75 |
| `monofilament.ron` | 3 | 40 → 110 |
| `ablative.ron` | 3 | 40 → 110 |
| `cortex.ron` | 3 | 45 → 125 |
| `deep_analysis.ron` | 3 | 46 → 130 |
| `kernel_privileges.ron` | 3 | 48 → 135 |
| `address_translation.ron` | 3 | 50 → 140 |

Totals to check your work against: band 1 = 158, band 2 = 350, band 3 = 750, **whole tree 1258**.

- [ ] **Step 1: Write the two censuses, failing**

In `crates/engine/src/tests/research.rs`, against a real `Game` (so the assertions are about the shipped assets, not a fixture):

1. `no_research_node_is_gated_below_its_own_prerequisite` — for every node, for every id in `requires`, that prereq's `min_zone` is `<=` this node's. Intent: catches a band edit that makes a gate unreachable. It passes *trivially* today with every node at 0, and stays meaningful the moment bands land — that is fine, it is a guard rather than a demonstration.
2. `nothing_needed_to_breach_is_locked_behind_research` — no node naming `"portal"` in `unlocks_structures` has a non-zero `min_zone`. Intent: the softlock guard. Today no node names `portal` at all, so this is vacuously true — which is the point, per the spec: the property is currently safe by accident and one content edit could remove it silently. Assert against the loaded `ResearchDb`, **not** by reading the files, so a node dropped at load time cannot make it pass for the wrong reason.

- [ ] **Step 2: Run them**

`cargo test -p feral-processes-engine research`
Expected: both PASS immediately (vacuously). This is the one place in this plan where a new test is not expected to fail first — say so in the commit message rather than contriving a red step. If either *fails* now, stop: the tree is not in the state this plan assumes.

- [ ] **Step 3: Edit the 21 asset files**

Apply the table above. Keep each file's existing field order and formatting; put `min_zone` next to `cost`.

- [ ] **Step 4: Fix the two tests that are priced against the old ladder**

`research.rs::the_shipped_tree_loads_clean` asserts `cortex` costs 45 → 125. Then run the app-core suite and see whether `picking_an_unaffordable_research_node_reports_why_and_stays_open` still holds — it grants the player some Research Data and expects a node to be unaffordable, so a repricing can make it pass or fail for the wrong reason. Read it before changing it; if it hardcodes a number, restate it in terms of the node's own cost rather than a new constant.

- [ ] **Step 5: Run the suite**

`cargo test --workspace`
Expected: PASS. The censuses now assert something real. `balance_sim` has no research term and should be untouched — if it moves, stop and work out why before continuing.

- [ ] **Step 6: Commit**

`git add assets/research crates/engine/src/research.rs crates/engine/src/tests/research.rs` (plus the app-core test if it changed)
Message: `balance(research): band the tree across zones 1-3 and reprice to 1258`

---

### Task 3: The gate

The behaviour. This is the task a reviewer should look hardest at.

**Files:**
- Modify: `crates/engine/src/views.rs` — `ResearchState` (~line 38)
- Modify: `crates/engine/src/game/unlocks.rs` — `research_nodes` (~line 184) and `unlock_research` (~line 230)
- Modify: `crates/gui/src/render/progression.rs` — the `Locked` arm
- Modify: `crates/engine/src/tests/research.rs` — behaviour tests

**Interfaces:**
- Consumes: `ResearchDef::min_zone: u32` (Task 1); the shipped bands (Task 2).
- Produces: `ResearchState::Locked { missing: Vec<String>, min_zone: Option<u32> }`. `Some(n)` means the player's zone is below `n`; `None` means the zone is satisfied and only prereqs are missing. Both may be unsatisfied at once. `Game::research_nodes(&self) -> Vec<ResearchStatus>` and `Game::unlock_research(&mut self, id: &str) -> Result<(), String>` keep their signatures.

**Three constraints from the spec, each of which is a way to get this wrong:**

1. **A gated node stays listed.** Do not filter it out of `research_nodes`. `CLAUDE.md`'s `upgrade_ceiling` entry carries the argument: filtering the stalled rows would mean a player who never breached never learns the feature exists. The visible zone-3 tier *is* the reason to go breach.
2. **The gate is checked before the cost**, and after the prereq check. `upgrade_structure` checks its ceilings before materials "so the player is never sent to find fragments they couldn't have spent" — same argument, so a player at zone 1 hears about the zone, not about their balance.
3. **The gate is on buying, not on having.** Touch `unlock_research` only. `resources::Research` holds what is already unlocked and must not be re-validated anywhere, or a save from before this change would lose nodes it had paid for.

Read the current zone with `self.world.resource::<ZoneLevel>().0`, the same way `Game::upgrade_ceiling` does.

Refusal message: `Requires Zone {n} first.` — matching the existing `Requires {} first.` used for prereqs.

- [ ] **Step 1: Write the failing behaviour tests**

In `crates/engine/src/tests/research.rs`. Find the subject node by querying `ResearchDb` for the cheapest node whose `min_zone` is 2 (and, where needed, 3) rather than naming an id, so a retune of the bands does not break them.

- `a_node_above_the_players_zone_reports_its_zone` — at zone 1, that node's state is `Locked` with `min_zone: Some(2)`.
- `a_zone_gated_node_is_still_listed` — it appears in `research_nodes()`. Guards constraint 1.
- `unlock_research_refuses_a_node_above_the_players_zone` — with the cost banked and prereqs met, the call errs **and the Research Data is not spent**. The unspent half is what fails if the refusal is ever moved below the payment; without it the test passes against a build that charges and then refuses.
- `breaching_makes_a_zone_gated_node_available` — after advancing the zone, the same node reads `Available`. This is the pairing that fails when the gate is deleted; per `CLAUDE.md`, a test that still passes with the fix removed is not coverage. Verify that by hand before you commit.
- `a_node_can_report_both_a_missing_prereq_and_its_zone` — a zone-3 node with its prereq unresearched carries a non-empty `missing` *and* `min_zone: Some(3)`.
- `the_zone_gate_is_refused_before_the_cost` — a player at zone 1 with no Research Data gets the zone message, not the "Not enough Research Data" one. Guards constraint 2.

For advancing the zone, use whatever the existing zone tests already use rather than writing to `ZoneLevel` directly — check `crates/engine/src/tests/` for the established route, and prefer the real breach path so the test exercises what a player does.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine research`
Expected: compile error first (no `min_zone` on the `Locked` variant), then real failures once the variant exists.

- [ ] **Step 3: Extend `ResearchState::Locked`**

Add the field. **Before writing this, invoke the `design-patterns` skill** — this is a branch on a state enum with two independent reasons, which is exactly the structural choice `CLAUDE.md` asks for a dialog on. The spec records why a separate `ZoneLocked` variant and a stringly-typed `"Zone 3"` pushed into `missing` were both rejected; confirm that reasoning still holds against the code in front of you, and say so if it doesn't.

- [ ] **Step 4: Compute the state in `research_nodes` and refuse in `unlock_research`**

Both read the zone the same way. Keep `research_nodes`'s Available/Locked/Unlocked sort as it is — a zone-locked node sorts as Locked.

- [ ] **Step 5: Update the gui label**

`render/progression.rs` builds the ` (needs …)` suffix. Join the prereq names and the zone into one list so a doubly-locked node reads `(needs Neural Interfacing, Zone 3)`. It is the only place a locked node is labelled — build the string there, not in the engine.

- [ ] **Step 6: Run the tests**

`cargo test -p feral-processes-engine research` then `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Prove the tests are real**

Delete the refusal in `unlock_research`, run the suite, confirm `unlock_research_refuses_a_node_above_the_players_zone` and `breaching_makes_a_zone_gated_node_available` both fail, then restore it. Restore from a scratchpad copy you made first — never `git checkout` a file to undo a deliberate mutation, and confirm the mutation actually applied before reading the result.

- [ ] **Step 8: Commit**

`git add crates/engine/src/views.rs crates/engine/src/game/unlocks.rs crates/gui/src/render/progression.rs crates/engine/src/tests/research.rs`
Message: `feat(research): gate nodes behind the zone they unlock in`

---

### Task 4: Gates and a real look at it

**Files:** none necessarily; this task's deliverable is evidence.

- [ ] **Step 1: Full suite**

`cargo test --workspace`. Report the actual count and any failure output verbatim. Passing only the tests you wrote is not evidence of correctness.

- [ ] **Step 2: Lints and formatting**

`cargo clippy --workspace` and `cargo fmt`. Fix warnings; do not silence them.

- [ ] **Step 3: Look at the screen**

A green suite is not evidence of play, and this feature is almost entirely a *screen*: whether a gated node reads as "go breach" rather than as a bug is not a property any test asserts.

```sh
cargo run                 # new game, zone 1: open the research menu (T)
```

Check three things and report what you saw: the zone-2 and zone-3 tiers are visible rather than hidden; their reason reads clearly; and a node locked on both a prereq and a zone says both. Then confirm the gate releases:

```sh
cargo run --bin savetool -- warp saves/save.bin 3
```

and reopen the menu. **`warp` runs the real breach** rather than editing the zone number, which is why it is the right instrument here.

- [ ] **Step 4: Report, do not release**

Summarise: suite result, what the screens looked like, and the one thing this change cannot know — whether 1258 actually lands at zone 3, which the spec records as unmeasured and deliberately left as data. Do **not** bump the version, write the `CHANGELOG.md` section, tag, push, or merge. Those happen at merge time and need an explicit ask.

---

## At merge (for the user, not the executor)

- Bump the workspace version in the root `Cargo.toml` to **0.8.15**.
- Add a `## 0.8.15` section to `CHANGELOG.md`.
- Tag `v0.8.15`, annotated. Note that a bare `git push` does not send tags — `--follow-tags` does.
- `TODO.md` line 28 (*"increase the amount of reserch points required overall"*) is what this implements and can come off.
