# The Zone Level Cap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One level cap over the player and every companion, derived from the zone; the Kernel Ring converts from buying levels to unlocking talent tiers; XP earned at the cap buys Perk Points at a sublinear rate.

**Architecture:** A single linear `Game::level_cap()` replaces both the player's `None` and `Game::companion_level_cap`. `CREATURE_MAX_LEVEL` stops being a cap and becomes the level talents begin at; `absolute_companion_level_cap()` stops being the live ceiling and becomes the arena's. `add_xp` gains an overflow report so XP earned at the cap accumulates in the already-saved `Experience::xp` and drains into Perk Points at a price that rises with perk levels held.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-27-zone-level-cap-design.md`

## Global Constraints

- **Read the spec before starting.** This plan argues from it and does not repeat its reasoning.
- **TDD, always.** Failing test first, minimal implementation, green, commit.
- **Every new test carries a mutation check.** Delete the fix, run the test, watch it fail, restore. Record the mutation in the commit body.
- **This plan carries no finished code by design.** Per `CLAUDE.md`'s process-weight rule it gives file lists, interfaces, test intents and gates. The three formulas below are the exception, because they are the thing being specified. If the plan looks wrong, say so rather than transcribing it.
- **Every curve in this feature is linear, and that is not negotiable.** `ZoneLevel::stat_multiplier`'s doc comment carries the argument at length: a compounding curve racing a linear one has an end wherever the coefficients are put. This cap races the enemy curve in the player's favour, which is the same failure wearing the other hat.
- **Never transcribe a number from a doc comment.** Every measured figure in the spec is quoted from one and must be re-derived by calling `balance_sim::min_level_to_clear_zone` live. This repo has been bitten four times by a doc comment claiming to mirror code, all in `balance_sim.rs`.
- **No `SAVE_FORMAT_VERSION` bump.** Nothing here adds, removes or re-means a save field. The two renames are constants and functions. If you need a bump, stop — the design is wrong.
- **`balance_sim` curves are expected to move.** That is the signal, not a broken test. They are **re-derived, not patched to pass** — see Task 5. Do not touch them before then.
- **Gates before calling any task done:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
- **Branch:** `feat/zone-level-cap`, off `main`. Commit freely at each green step. **Do not push.**

---

## File Structure

**Engine (`crates/engine/src/`)**

| Path | Responsibility |
|---|---|
| `tuning.rs` | `ZONE_LEVEL_CAP_FLOOR`, `ZONE_LEVEL_CAP_STEP`, `OVERFLOW_XP_BASE`, `OVERFLOW_XP_STEP`; renames of `CREATURE_MAX_LEVEL` and `absolute_companion_level_cap()`. |
| `game/party.rs` | `Game::level_cap()` replaces `companion_level_cap` (line 329). |
| `game/combat_rewards.rs` | The player's `add_xp` (737) and the companion's (831) both take the cap; the player's converts overflow. |
| `systems.rs` | Cronjob payout (869) takes the cap — a bevy system, so it reads `ZoneLevel` as a resource. |
| `progression.rs` | `LevelGain::overflow`; `add_xp` accumulates at the cap instead of returning early (198). |
| `game/talents.rs` | Talent points formula (32). |
| `game/refactor.rs` | `open_kernel_ring`'s log line (265) — a ring no longer raises a level ceiling. |
| `game/unlocks.rs` | `Game::convert_overflow_xp` beside `unlock_perk` (107). |
| `game/inspection.rs`, `views.rs` | `level_cap` readers re-point. |
| `arena/mod.rs` | Keeps `arena_level_ceiling()`. **Behaviour must not change.** |
| `balance_sim.rs` | `companion_level_for_player_level` (55) and every hardcoded curve — Task 5 only. |

**GUI (`crates/gui/src/`)** — `render/talents.rs:68`, `render/manifest.rs:611` re-point to the one cap.

**Docs** — `docs/measurements/2026-08-27-zone-level-cap.md` (new, Task 5).

**Tests** — `crates/engine/src/tests/`: `level_up.rs` (Tasks 1, 2), `talents.rs` (Task 3), `perks.rs` (Task 4), `assets.rs` (Task 6 census). `balance_sim.rs`'s own test module for Task 5.

---

### Task 1: The cap formula and its constants

**Files:**
- Modify: `crates/engine/src/tuning.rs`
- Modify: `crates/engine/src/game/party.rs` (new fn beside `companion_level_cap`, 329)
- Test: `crates/engine/src/tests/level_up.rs`

**Interfaces:**
- Consumes: `resources::ZoneLevel`, `balance_sim::min_level_to_clear_zone` (tests only).
- Produces:
  - `tuning::ZONE_LEVEL_CAP_FLOOR: u32`, `tuning::ZONE_LEVEL_CAP_STEP: u32`
  - `Game::level_cap(&self) -> u32` — `pub`. Reads `ZoneLevel`. Takes no entity: it is the same number for everyone.

**The formula:**

```rust
max(ZONE_LEVEL_CAP_FLOOR, 1 + ZONE_LEVEL_CAP_STEP * (zone - 1))
```

**Design notes:**

- **Fit the constants, do not copy them.** The spec's starting point is `FLOOR = 6`, `STEP = 5`, quoted from a `balance_sim` doc comment. Before writing them down, call `min_level_to_clear_zone` for zones 1–10 in both the geared and gear-free configurations and fit against what it actually returns. The target: the cap sits **below** the gear-free requirement (a zone cannot be cleared by levelling alone) and **at or above** the geared one (a fully equipped party can).
- **The tolerance is derived, not chosen.** The two curves converge to within a level or two past zone 5, and the integer search is lumpy, so a linear cap cannot sit strictly inside the band at every zone. Fit the constants first, then set the test's tolerance to the smallest value the fit actually achieves — never the other way round, or the test is written to pass rather than to bound. Name it, with the reason attached.
- **`ZONE_LEVEL_CAP_FLOOR` and the renamed `CREATURE_MAX_LEVEL` both start at 6 and that is a coincidence.** They answer different questions and either may be retuned without the other. Do not express one in terms of the other, and say so in the doc comment.
- No caller yet. This task ships a function and its constants; Task 2 wires them.

- [ ] **Step 1: Write the failing tests.** Four intents: the cap rises linearly (the per-zone step is constant across a swept range — `ZONE_STAT_STEP` already has a peer test for this shape); the floor holds zone 1 above the trivial requirement; the cap is bounded by both clear curves within the derived tolerance, asserted against `min_level_to_clear_zone` **called**, never against transcribed numbers; and **depth does not lift it** — the cap four frames underground equals the cap on the surface in the same zone. The last is structural (the formula reads only `ZoneLevel`) but it is a stated design property, and the thing that would quietly break it is somebody adding a depth term to "help" a deep stack.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine level_up`
- [ ] **Step 3: Derive the constants.** Write a throwaway test or `dbg!` that prints `min_level_to_clear_zone` for zones 1–10 both ways. Record the real numbers in the commit body — they are the evidence for the constants and the spec's copies are not.
- [ ] **Step 4: Add the constants and `Game::level_cap`.**
- [ ] **Step 5: Green.**
- [ ] **Step 6: Mutation check.** Make the formula quadratic in `zone`; the linearity test must fail. Set the floor to 1; the zone-1 test must fail.
- [ ] **Step 7: Commit,** with the derived curves in the body.

---

### Task 2: Point every reader at the one cap, and rename what changed meaning

**Files:**
- Modify: `crates/engine/src/tuning.rs` (both renames)
- Modify: `crates/engine/src/game/party.rs` (remove `companion_level_cap`)
- Modify: `crates/engine/src/game/combat_rewards.rs:737`, `:831`
- Modify: `crates/engine/src/systems.rs:869`
- Modify: `crates/engine/src/game/inspection.rs:1375`, `crates/engine/src/game/refactor.rs:265`, `crates/engine/src/views.rs`
- Modify: `crates/gui/src/render/talents.rs:68`, `crates/gui/src/render/manifest.rs:611`
- Test: `crates/engine/src/tests/level_up.rs`

**Interfaces:**
- Consumes: `Game::level_cap` from Task 1.
- Produces:
  - `tuning::TALENT_START_LEVEL` (was `CREATURE_MAX_LEVEL`)
  - `tuning::arena_level_ceiling()` (was `absolute_companion_level_cap()`)
  - `Game::companion_level_cap` is **deleted**, not deprecated. No back-compat shims — `CLAUDE.md`'s rule.

**Design notes — the two renames are the point of this task:**

- **`CREATURE_MAX_LEVEL` stops being a cap.** It survives only as the level talents begin at. A constant whose meaning changes under a name it keeps is the exact trap the save-format rule warns about, and here *nothing would fail to compile* — that is why the rename is mandatory rather than tidy.
- **`absolute_companion_level_cap()` stops being the live ceiling.** It becomes the arena's alone. Five shipped `dev-arenas/` scenarios author `level: 12` at `zone: 3`; pointing the arena at `Game::level_cap` would silently clamp all five, which is a failure this repo has already had once — the old reports stopped being comparable and nothing said so. `arena/mod.rs:96` must keep taking the absolute ceiling.
- **`systems.rs:869` is the awkward one.** It is a bevy system with no `Game`, and it currently passes `Some(CREATURE_MAX_LEVEL)`. It must pass the zone cap, which means reading `ZoneLevel` as a system parameter. Note the standing trap: adding a resource read can shift bevy's query iteration order, so a failure in an untouched subsystem right after this is a latent unsorted-query test, not your regression.
- **`WORK_XP_LEVEL_CAP` is untouched.** It guards the same call site and looks like a level cap but is not one — it is what stops a developed program being ground up at a Mining Node. Anyone unifying the two deletes that property.
- The player's site (`combat_rewards.rs:737`) changes from `None` to `Some(cap)`. That is the moment the player becomes capped at all.

- [ ] **Step 1: Write the failing tests.** Six intents: the player stops levelling at the cap; a companion stops at the *same* number; a breach lifts both; an entity loaded from a save **above** the cap keeps its level and its stats (the `EquippedItem::fusion_tier` rule — clawing back spent growth is the thing not to do); the five `dev-arenas/` scenarios still stage companions at 12; and **`WORK_XP_LEVEL_CAP` still stops cronjob XP at its own level**, unchanged by the zone cap now passed alongside it. That last one is the guard against the tempting simplification of unifying the two caps, which would delete the property that a developed program cannot be ground up at a Mining Node.
- [ ] **Step 2: Run, confirm failure.**
- [ ] **Step 3: Rename both** across the workspace. Mechanical, but grep the **new** vocabulary afterwards as well as the old — grepping only the removed word is blind to what is half-converted around it, and `--type rust` misses `.ron` and player-facing text.
- [ ] **Step 4: Repoint every reader,** deleting `companion_level_cap`.
- [ ] **Step 5: Green** — full workspace, because of the iteration-order risk and the gui readers. `cargo test --workspace`
- [ ] **Step 6: Mutation check.** Give companions `TALENT_START_LEVEL` as their cap instead of the zone cap; the "same number" test must fail. Point `arena/mod.rs:96` at `Game::level_cap`; the scenario test must fail — that one is the whole reason the second rename exists.
- [ ] **Step 7: Commit.**

---

### Task 3: The Kernel Ring buys talent tiers, not levels

**Files:**
- Modify: `crates/engine/src/game/talents.rs:32`
- Modify: `crates/engine/src/game/refactor.rs:265` (`open_kernel_ring`'s log line)
- Test: `crates/engine/src/tests/talents.rs`

**Interfaces:**
- Consumes: `tuning::TALENT_START_LEVEL`, `tuning::LEVELS_PER_RING`, `components::KernelRing`.
- Produces: no new public surface — the derivation changes shape in place.

**The formula:**

```rust
min(level.saturating_sub(TALENT_START_LEVEL), rings * LEVELS_PER_RING)
```

**Design notes:**

- **`saturating_sub`, not `-`.** These are `u32` and a companion below the talent start level is the common case, not an edge one. The existing derivation already saturates for the same reason.
- **Both gates survive.** You must be developed *and* hold rings. Tree depth is unchanged, the 1+2+3 guardian cost is unchanged, and the six talent censuses in `tests/assets.rs` must pass **untouched** — if one needs editing, the formula is wrong.
- **Migration is a non-event and the tests should prove it.** A 3-ring level-12 companion has `min(12-6, 6) = 6` under the new rule and had 6 under the old; a ringless level-6 one has 0 under both. `Talents` stays a receipt like `Refactors`, and a `Stat` node still bakes into `Stats` at purchase with load not re-applying it.
- `open_kernel_ring`'s log line currently announces a new level ceiling. Rewrite it to say what a ring now does. It still grants no stats, no level and no XP — that existing rule is unchanged and should stay pinned.

- [ ] **Step 1: Write the failing tests.** Five intents: points are the saturating `min`; a companion **below** the talent start level yields 0 rather than underflowing; a ringless companion at the zone cap earns none; a 3-ring companion earns exactly a full tree and no more; and existing receipts survive — a companion built to the old shape has the same points under the new rule.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine talents`
- [ ] **Step 3: Change the derivation.**
- [ ] **Step 4: Rewrite `open_kernel_ring`'s log line.**
- [ ] **Step 5: Green,** including the untouched `tests/assets.rs` censuses.
- [ ] **Step 6: Mutation check.** Drop the `rings` term; the ringless test must fail. Swap `saturating_sub` for `-`; the below-start test must panic rather than return 0.
- [ ] **Step 7: Commit.**

---

### Task 4: Overflow XP buys Perk Points

**Files:**
- Modify: `crates/engine/src/progression.rs:25` (`LevelGain`), `:194` (`add_xp`)
- Modify: `crates/engine/src/game/unlocks.rs` (new fn beside `unlock_perk`, 107)
- Modify: `crates/engine/src/game/combat_rewards.rs:737` (the player's site converts)
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/perks.rs`

**Interfaces:**
- Consumes: `components::Perks` (`points: u32`, `unlocked: Vec<Perk>`, `level(perk)`).
- Produces:
  - `progression::LevelGain::overflow: u32` — XP that arrived while capped and was not spent on a level.
  - `Game::convert_overflow_xp(&mut self) -> u32` — `pub(crate)`. Drains `Experience::xp` into `Perks::points`, returns points minted.
  - `tuning::OVERFLOW_XP_BASE: u32`, `tuning::OVERFLOW_XP_STEP: u32`.

**The price:**

```rust
xp_per_point = OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * perks_held
```

where `perks_held` is `Perks::unlocked.len()`.

**Design notes:**

- **Why it must be sublinear.** Perks are already uncapped and repeatable at a flat price, and `Perk::Attacker` writes `stats.atk +=` straight into `Stats`. A flat exchange makes the perk track a linear, unbounded power source and the grind returns wearing a different hat. A linear *cost* makes points earned grow like √XP, which loses the race against a linear zone curve forever. That race is the whole feature.
- **`perks_held` is derived, never stored** — `unlocked.len()`, matching this repo's idiom throughout. No save field.
- **The accumulator is `Experience::xp`,** which is already saved and already idle at the cap. `add_xp` currently returns early at the cap **without accumulating at all** (`progression.rs:198`); change it to accumulate and report the unabsorbed amount. Conversion drains it. Whatever has not been converted when a breach lifts the cap becomes real levels on the spot — banking and taxing are the same accumulator, which is why this needs no new field.
- **`add_xp` stays pure.** It reports; the caller converts. That is why it takes `level_cap` as a parameter today rather than reading the world, and the property is what makes it testable without a `Game`.
- **`Experience::xp_to_next` is still derived on load** by both load paths and never read back from the save. Do not disturb that.
- **Companions do not convert.** They have no Perk Points, so a capped companion's overflow simply is not spent — the behaviour creatures have today. Test it so it is not read as an oversight later.

- [ ] **Step 1: Write the failing tests.** Six intents: at the cap, XP accumulates rather than being discarded; it converts to Perk Points; the price rises with perk levels held; points earned against XP spent is **sublinear** across a swept range (assert the property, not a magic number); unconverted overflow becomes levels when a breach lifts the cap; and a capped companion's overflow is not spent and does not panic.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine perks`
- [ ] **Step 3: Add `LevelGain::overflow`** and change `add_xp`'s cap arm to accumulate.
- [ ] **Step 4: Add the constants and `convert_overflow_xp`.**
- [ ] **Step 5: Wire the player's call site.**
- [ ] **Step 6: Green.**
- [ ] **Step 7: Mutation check.** Set `OVERFLOW_XP_STEP` to 0; the sublinearity test must fail. Restore the early return in `add_xp`; the accumulation and breach-release tests must fail.
- [ ] **Step 8: Commit.**

---

### Task 5: Re-derive `balance_sim`, and measure

**Files:**
- Modify: `crates/engine/src/balance_sim.rs:55` (`companion_level_for_player_level`) and every hardcoded curve in its test module
- Create: `docs/measurements/2026-08-27-zone-level-cap.md`

**Interfaces:** none new. This task changes numbers and writes down why.

**Design notes — this is the largest risk in the feature and it is not optional:**

- `min_level_to_clear_zone` currently fields companions at `player_level / √2`. With companions taking the player's cap exactly, the live party is stronger than the gate has ever modelled — companions go from a ceiling of 12 to roughly the player's cap. `companion_level_for_player_level` must become the cap.
- **Re-derive, do not patch to pass.** Every hardcoded empirical curve in that file is recomputed from the live constants and the new party model. A curve edited until the test goes green is a gate that no longer guards anything.
- Meanwhile wild programs scale by `ZoneLevel::stat_multiplier`, a zone term and not a level term, so the surface does not scale to meet a stronger party. **Expect it to want retuning — and do not do it here.** Measure first; a `ZONE_STAT_STEP` change folded into this task on the assumption it will be needed is a difficulty change nobody asked for.
- Arena runs: compare **deltas, never absolutes**. A moved baseline is a reshuffled RNG stream, not a difficulty change, and arena numbers compare within one build only.
- `docs/measurements/README.md` is the convention and the bar. The data behind these runs is gitignored; a number not written down costs CPU-hours and an afternoon to recover.

- [ ] **Step 1: Capture the "before".** Run `cargo test -p feral-processes-engine balance_sim` and the shipped `dev-arenas/` scenarios on the current build; save the reports.
- [ ] **Step 2: Change `companion_level_for_player_level`** to the cap.
- [ ] **Step 3: Re-derive every curve** from what the sim now returns. Record the old and new side by side.
- [ ] **Step 4: Green.** `cargo test -p feral-processes-engine balance_sim`
- [ ] **Step 5: Re-run the arena scenarios** and diff against Step 1's reports as deltas.
- [ ] **Step 6: Write `docs/measurements/2026-08-27-zone-level-cap.md`** — the commands that produced each number, the numbers, and what the run was blind to. State plainly whether the surface looks like it needs a retune; that is a decision for a later change, and this file is its evidence.
- [ ] **Step 7: Commit.**

---

### Task 6: Final gates

**Files:** none — verification only.

- [ ] **Step 1:** `cargo test --workspace`
- [ ] **Step 2:** `cargo clippy --workspace` — fix warnings, never silence them.
- [ ] **Step 3:** `cargo fmt`
- [ ] **Step 4:** Confirm both renames are complete by grepping the **new** vocabulary as well as the old, across `.rs` **and** `.ron` and docs. A half-converted rename is invisible to a grep for the removed word.
- [ ] **Step 5:** `git diff --quiet assets/` — nothing in this feature should have touched a shipped asset. If something did, find out why.
- [ ] **Step 6:** Confirm `SAVE_FORMAT_VERSION` is unchanged and a save written before this branch still loads, with an over-cap entity keeping its level and stats.
- [ ] **Step 7:** Report what you actually ran and saw. Do not report completion from a partial run.

---

## Deliberately not in this plan

- **Retuning the surface.** Downstream of Task 5's measurement and gets its own decision once there are numbers.
- **`CHANGELOG.md` and the version bump.** Once, at the merge to `main`.
- **`docs/manual.md` and the root `README.md`.** Both carved out of the documentation obligation.
- **Downed programs and the Repair Bay.** Separate spec and plan, no dependency either way.
