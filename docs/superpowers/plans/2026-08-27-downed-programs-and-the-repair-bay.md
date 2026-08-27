# Downed Programs and the Repair Bay — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Forgiving death of an owned program benches it instead of destroying it; a new passive structure repairs it over ticks; party assignment moves into base space.

**Architecture:** One new door (`Game::bench_or_dissolve`) replaces two direct `dissolve_tamed_program` calls and branches on `DifficultyMode`. A benched program carries a `Downed` marker, is staff by derivation, is excluded from the scheduler's posting half, walks itself to a Repair Bay through the existing `hauling::step_to_post`, and is healed by a system modelled on `power_regen_system`. `RepairDef` is the third member of the `PowerRegenDef` / `ServiceDef` family.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-27-downed-programs-and-the-repair-bay-design.md`

## Global Constraints

- **Read the spec before starting.** This plan argues from it and does not repeat its reasoning.
- **TDD, always.** Failing test first, minimal implementation, green, commit. Every task ends green.
- **Every new test carries a mutation check.** Delete the fix, run the test, watch it fail, restore. A test that passes with the fix removed is not coverage — this repo has shipped two of those. Record the mutation you applied in the commit body.
- **This plan carries no finished code by design.** Per `CLAUDE.md`'s process-weight rule, it gives you the file list, the interfaces, the intent of each test and the gates. Code blocks appear only where something is genuinely non-obvious. Write the implementation yourself; if the plan looks wrong, say so rather than transcribing it.
- **No `SAVE_FORMAT_VERSION` bump.** Every save change here is additive behind `#[serde(default)]`. If you find yourself needing a bump, stop — the design is wrong, not the version.
- **Moddability.** New `StructureDef` fields are `#[serde(default)]`; a malformed `.ron` is skipped with a logged warning, never a panic; `assets/structures/README.md` is updated in the same task as the schema change.
- **Permadeath behaviour is unchanged everywhere.** If a test can't tell the two modes apart, it isn't testing this feature.
- **Gates before calling any task done:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
- **Branch:** work continues on `feat/downed-programs`. Commit freely at each green step. **Do not push.**

---

## File Structure

**Engine (`crates/engine/src/`)**

| Path | Responsibility |
|---|---|
| `components.rs` | `Downed` marker component, beside `OffShift` (~line 1352) which it mirrors in shape and lifetime. |
| `game/trade.rs` | New `Game::bench_or_dissolve` beside `dissolve_tamed_program` (~line 430) — the one door both death sites take. |
| `game/combat_teardown.rs` | `end_battle`'s dead-party loop (line 283) calls the new door. |
| `game/base/upkeep.rs` | Raid defender death (line 413) calls the new door. |
| `game/base/work_orders.rs` | `on_shift` filter (line 873) excludes `Downed`; `drift_idle_staff` (line 1464) gains a `Downed` arm above its `OffShift` arm. |
| `game/base/repair.rs` | **New.** `Bays` (the `offshift::Amenities` analogue) and `Game::step_to_repair` (the `step_off_shift` analogue). |
| `structures.rs` | `RepairDef` beside `PowerRegenDef` (~line 86); `StructureDef::repair` field. |
| `systems.rs` | `repair_system`, modelled on `power_regen_system` (line 1309). |
| `game/lifecycle.rs` | Register `repair_system` in the schedule (~line 330). |
| `game/party.rs` | `require_base` on `add_companion` (line 448); new guarded stand-down verb wrapping `remove_companion` (line 609). |
| `save.rs` | `CreatureSave::downed`, additive; write and read paths. |

**App-core (`crates/app-core/src/`)**

| Path | Responsibility |
|---|---|
| `app/group_menu.rs` | Party row's `locality` becomes `Locality::Base`. |
| `app/party.rs` | Line 139's `remove_companion` call moves to the new guarded verb. |

**Assets & docs**

| Path | Responsibility |
|---|---|
| `assets/structures/repair_bay.ron` | **New.** Passive, ungated, zone-1 affordable. |
| `assets/structures/README.md` | `repair:` schema section. |
| `docs/seams.md` | Two new rows in the `require_base` guard table. |

**Tests** — follow the existing per-subject split in `crates/engine/src/tests/`: `combat_rewards.rs` (Task 1), `raids.rs` (Task 2), `work_orders.rs` (Tasks 3, 6), `assets.rs` (Task 4 census), `building.rs` (Task 5), `party.rs` (Task 7). App-core tests go in `crates/app-core/src/tests/group_menus.rs`. Fixtures live in `crates/engine/src/tests/support.rs` — **look there before writing a new one.**

---

### Task 1: The `Downed` marker and the one death door

**Files:**
- Modify: `crates/engine/src/components.rs` (beside `OffShift`, ~1352)
- Modify: `crates/engine/src/game/trade.rs` (new fn beside `dissolve_tamed_program`, ~430)
- Modify: `crates/engine/src/game/combat_teardown.rs:283`
- Modify: `crates/engine/src/save.rs` (`CreatureSave`, ~122; write and read paths)
- Test: `crates/engine/src/tests/combat_rewards.rs`

**Interfaces:**
- Consumes: `Game::dissolve_tamed_program(Entity) -> String`, `resources::DifficultyMode`, `Game::strip_gear`.
- Produces:
  - `components::Downed` — a unit marker component, `#[derive(Component, Clone, Debug)]`.
  - `Game::bench_or_dissolve(&mut self, creature: Entity) -> String` — `pub(crate)`. Returns the label, matching `dissolve_tamed_program`'s contract so both call sites keep their payout/announcement lines.
  - `save::CreatureSave::downed: bool`, `#[serde(default)]`.

**Design notes** (read before writing):

- `bench_or_dissolve` is **one door, not two paths.** `dissolve_tamed_program`'s own doc comment explains why that function exists: sale and extraction agree through it rather than through a doc comment claiming they mirror. Same argument, one level up. Do not branch on difficulty at each call site.
- On the Forgiving arm the program must: keep `Tamed`, get HP set to 1, gain `Downed`, be retained out of `Party`, lose its `Task`, and have `strip_gear` run. Gear is the player's property on **both** arms.
- `end_battle` is the only legal removal point — `BattleState::planned` indexes `Party` positionally and nothing may leave mid-battle. Do not move the call earlier.
- Save: mirror `CreatureSave::power`'s doc comment shape, which is this repo's worked example of an additive `#[serde(default)]` field earning no version bump.

- [ ] **Step 1: Write the failing tests.** In `tests/combat_rewards.rs`, four intents — (a) a companion that dies in a Forgiving battle is alive, carries `Downed`, is out of `Party` and is still `Tamed` afterwards; (b) the same death under `DifficultyMode::Permadeath` despawns it, asserted with the `world.get::<Stats>(e).is_none()` idiom; (c) gear is stripped on both arms; (d) a downed program survives a **save-then-load** as downed. Intent (d) must be a real save/load, not a RON round-trip — a `#[serde(skip)]` or an unwired write leaves a round-trip test green.
- [ ] **Step 2: Run them, confirm they fail** for the right reason (`Downed` does not exist), not a fixture error. `cargo test -p feral-processes-engine combat_rewards`
- [ ] **Step 3: Add the `Downed` component** in `components.rs`, with a doc comment saying what it is and what clears it.
- [ ] **Step 4: Write `bench_or_dissolve`** and point `end_battle:283` at it.
- [ ] **Step 5: Wire the save field** — struct, write path, read path.
- [ ] **Step 6: Green.** `cargo test -p feral-processes-engine`
- [ ] **Step 7: Mutation check.** Flip the difficulty branch so both arms dissolve; confirm (a) and (d) fail. Restore. Then remove the save field's read; confirm (d) alone fails. Restore.
- [ ] **Step 8: Commit.** Record both mutations in the body.

---

### Task 2: Raid deaths take the same door

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs:413`
- Test: `crates/engine/src/tests/raids.rs`

**Interfaces:**
- Consumes: `Game::bench_or_dissolve` from Task 1.
- Produces: nothing new.

**Design notes:** The existing `Task` removal immediately above the call stays and its comment stays true — `raid_check` finds its defender *by* that `Task`, and leaving it on would make `sale_detachments` write a redundant line under one already naming the structure. Change the call, not its surroundings.

- [ ] **Step 1: Write the failing tests.** Two intents: a defender killed by a raid under Forgiving is downed rather than destroyed; under Permadeath it is still destroyed. `raid_check` needs a posted worker and a damaged structure — check `tests/support.rs` for an existing raid fixture before building one.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine raids`
- [ ] **Step 3: Point the call at `bench_or_dissolve`.**
- [ ] **Step 4: Green.**
- [ ] **Step 5: Mutation check.** Revert the call to `dissolve_tamed_program`; confirm the Forgiving test fails and the Permadeath one still passes — that asymmetry is what proves the test is reading the right thing.
- [ ] **Step 6: Commit.**

---

### Task 3: The scheduler stops posting downed programs

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs:873`
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Consumes: `components::Downed`.
- Produces: nothing new.

**Design notes.** The filter today is:

```rust
self.world.get::<components::OffShift>(w).is_none()
    || self.world.get::<Carrying>(w).is_some()
```

`Downed` joins it **without** the `Carrying` escape. That asymmetry is the point and must be commented: the `Carrying` exception exists because freeing a loaded body destroys the goods, and an off-shift body may legitimately be mid-delivery. A body that just died in a fight is not carrying anything the base needs — and if it somehow is, it is going to the Bay regardless.

`drift_idle_staff` runs *above* this filter and keeps the **whole** staff list. Do not touch it here; Task 6 owns it.

- [ ] **Step 1: Write the failing tests.** Two intents: a downed program is never given a post even when a machine wants one; and `LabourDemand`'s shortfall *grows* to reflect its absence — the same readout the off-shift feature already produces. The second is the one that catches a filter applied after `truncate` instead of before.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine work_orders`
- [ ] **Step 3: Extend the filter.**
- [ ] **Step 4: Green.**
- [ ] **Step 5: Mutation check.** Drop the `Downed` clause; both tests must fail. Then add a `Carrying` escape to the `Downed` clause and confirm a fixture with a downed carrier still stays unposted.
- [ ] **Step 6: Commit.**

---

### Task 4: `RepairDef`, the schema doc, and the shipped Bay

**Files:**
- Modify: `crates/engine/src/structures.rs` (beside `PowerRegenDef`, ~86)
- Create: `assets/structures/repair_bay.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces:

```rust
pub struct RepairDef {
    /// HP restored per tick to a downed program within `radius`.
    pub per_tick: i32,
    /// Chebyshev distance in tiles, a box rather than a circle.
    pub radius: i32,
}
// on StructureDef, #[serde(default)]:
pub repair: Option<RepairDef>,
```

**Design notes:**

- **`i32`, not `f32`.** `Stats::hp` is `i32`, so this needs only half of `PowerRegenDef`'s clamp — negatives floored, and no non-finite case to guard. That deletion is the reason to take the integer type; note it in the doc comment so nobody "fixes" it back to a float for symmetry.
- The `.ron`: model on `assets/structures/recharger_node.ron`. Passive — **no `work` block, no posted worker, no research gate.** `build_cost` must be affordable in zone 1: a gate you cannot afford in the zone where you first need it is a dead run, not pressure. Give it `power_draw: 2` — the shipped band is 1 to 3 (`log_scraper` 1,
  `lathe`/`transcriber`/`winding_node` 2, `assembly_bay`/`armory` 3) — and no
  `power_supply`. Author the `description` by hand; it is not derived from the capability fields.
- The README section is part of *this* task, not a follow-up. Document the field, the units, the clamp and the fact that omitting it is the pre-feature behaviour.

- [ ] **Step 1: Write the failing census** in `tests/assets.rs`: exactly one shipped structure declares `repair`, its `build_cost` is affordable at zone 1 against the same materials `ZONE_MATERIALS` already reasons about, and it declares no `work` block. Follow the existing census idiom in that file.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine assets`
- [ ] **Step 3: Add `RepairDef` and the `StructureDef` field.**
- [ ] **Step 4: Write `assets/structures/repair_bay.ron`.**
- [ ] **Step 5: Update `assets/structures/README.md`.**
- [ ] **Step 6: Green,** and confirm a `StructureDef` `.ron` with no `repair:` key still parses untouched — that is what `#[serde(default)]` is for and it needs an assertion, not an assumption.
- [ ] **Step 7: Mutation check.** Remove `#[serde(default)]`; every existing structure file must fail to parse. Restore.
- [ ] **Step 8: Commit.**

---

### Task 5: `repair_system`

**Files:**
- Modify: `crates/engine/src/systems.rs` (beside `power_regen_system`, 1309)
- Modify: `crates/engine/src/game/lifecycle.rs` (~330, schedule registration)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: `structures::RepairDef`, `components::Downed`.
- Produces: `pub fn repair_system(...)` — a bevy system, signature shaped after `power_regen_system`.

**Design notes:**

- **The scan centre differs from `power_regen_system` and that is the whole difference.** That one centres on the party's `Locale::Base` coordinates because it serves the player; this one centres on **each downed program's own `Position`**, because a program is where it stands. Do not copy the `Locale` early-return.
- Clamp `per_tick` at zero. It is mod-supplied.
- Clear `Downed` when `hp` reaches `max_hp`, and log the recovery **only on that transition** — `set_machine_status`'s rule that entering a state is news and staying in it is not. Base-sourced (`MessageSource`), so the map's log pane filters it correctly.
- Register it **unchained** if and only if it shares no mutable state with the chained block above it. It writes `Stats` on downed programs and removes a component; check that against `task_progress_system` and `haul_step_system` before deciding, and write the reason into the registration comment the way its neighbours do. Note the standing trap: registering a new system or resource can shift bevy's query iteration order, so a failure in an untouched subsystem right after this is a latent unsorted-query test, not your regression.

- [ ] **Step 1: Write the failing tests.** Five intents: a downed program within radius recovers HP and loses `Downed` at full; one outside radius does not; a negative `per_tick` does not damage; the recovery line is logged once, not every tick; with **no** Bay standing a downed program stays downed indefinitely (advance many ticks). The last is the one that pins the player's "stuck until one is built" decision.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine building`
- [ ] **Step 3: Write `repair_system`.**
- [ ] **Step 4: Register it** with a reasoned comment.
- [ ] **Step 5: Green** — and run the **full** suite here, not just `building`, because of the iteration-order risk. `cargo test --workspace`
- [ ] **Step 6: Mutation check.** Remove the `Downed` removal at full HP; the first test must fail. Remove the transition guard on the log; the fourth must fail. Remove the negative clamp; the third must fail.
- [ ] **Step 7: Commit.**

---

### Task 6: A downed program walks to the Bay

**Files:**
- Create: `crates/engine/src/game/base/repair.rs`
- Modify: `crates/engine/src/game/base/work_orders.rs:1464` (`drift_idle_staff`)
- Modify: `crates/engine/src/game/base/mod.rs` (module declaration)
- Test: `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Consumes: `hauling::step_to_post`, `hauling::NoPost`, `Game::structure_tiles`, `components::Downed`.
- Produces:
  - `repair::Bays` — `pub(crate)`, with `build(structures, db)` and a `nearest(from: Position) -> Option<(Position, i32)>`, sorted by tile so ties resolve identically every run.
  - `Game::step_to_repair(&mut self, worker: Entity, bays: &Bays) -> Result<(), NoPost>` — `pub(crate)`.

**Design notes:**

- **`offshift.rs` is the template.** `Amenities` (line 28) and `step_off_shift` (line 281) are the same two shapes for the same reason; read them first and follow them rather than inventing. `Bays` is simpler — it is not keyed by anything, so a sorted `Vec<(Position, i32)>` replaces the `BTreeMap`.
- **Build once per beat**, never per program. `Amenities`' doc comment carries the argument: two cheap builds beat one stale cached copy, and a cached one would be a new `Resource` and another iteration-order shift.
- **Placement in `drift_idle_staff` is load-bearing.** The `Downed` arm goes **above** the `OffShift` arm in the fall-through chain — repair outranks an amenity — and above the wander, which is what everyone else gets.
- On arrival (already in reach), hold. The wander's other shape would walk it straight back out; `step_off_shift` makes exactly this move and says so.
- On `Err(NoRoute)`, hold rather than dropping `Downed`. This is deliberately **not** `step_off_shift`'s behaviour, which latches the need and drops the marker to stop an insert/remove flicker. There is no flicker here — nothing re-inserts `Downed` — and dropping it would silently heal a program that could not reach a Bay.
- `entry_tile` already handles a program whose `Position` is a surface tile (one downed in the Stack). Do not add a second arrival path; verify the existing one covers it.

- [ ] **Step 1: Write the failing tests.** Four intents: a downed program moves toward a Bay rather than wandering; one already in reach holds; one whose `Position` is still a surface tile arrives in base space and then walks; one with no route holds and **keeps** `Downed`. For the third, `drift_idle_staff_for_test` (line 1570) is the existing hook — use it rather than building a scheduler fixture.
- [ ] **Step 2: Run, confirm failure.**
- [ ] **Step 3: Write `repair.rs`** — `Bays` then `step_to_repair`.
- [ ] **Step 4: Add the `Downed` arm** to `drift_idle_staff` above the `OffShift` arm.
- [ ] **Step 5: Green.**
- [ ] **Step 6: Mutation check.** Move the `Downed` arm *below* the `OffShift` arm and confirm a fixture that is both downed and off-shift walks to the wrong place — if no test catches that, the ordering claim is untested and you need one. Then make `Err` drop `Downed`; the fourth test must fail.
- [ ] **Step 7: Commit.**

---

### Task 7: Party assignment requires base

**Files:**
- Modify: `crates/engine/src/game/party.rs:448` (`add_companion`), `:609` (`remove_companion` — **unchanged**, plus a new verb beside it)
- Modify: `crates/app-core/src/app/group_menu.rs` (party row `locality`)
- Modify: `crates/app-core/src/app/party.rs:139`
- Modify: `docs/seams.md` (`require_base` guard table)
- Test: `crates/engine/src/tests/party.rs`, `crates/app-core/src/tests/group_menus.rs`

**Interfaces:**
- Consumes: `Game::require_base`, `app::group_menu::Locality::Base`.
- Produces: `Game::stand_down_companion(&mut self, creature: Entity) -> Result<(), String>` — guarded, logs, wraps `remove_companion`.

**Design notes — read this one twice:**

- **`remove_companion` must stay guard-free.** It returns `()`, and `wield_program` calls it internally (`party.rs:577`) to stand a member down before taking it as a weapon. Putting `require_base` inside it refuses **wielding in the field** as a silent side effect, through a function the player never invoked. So it stays the *mover* and the new verb is the *player verb* — `take_from_adjacent` / `give_to_adjacent`'s exact shape, which are guard-free, log-free and tick-free on purpose so the caller owns the refusal.
- `add_companion` already returns `Result` and takes `require_base()?` directly.
- The refusal text should mention that a downed program can still be sold or have a routine extracted, so a player with a full roster and no Bay is not left guessing. That is the one player-facing consequence of the "stuck until built" decision.
- `Locality` is already a three-state enum with a `Base` variant; the party row changes one field. That table is the **only** source of which rows show — do not fold an `in_base()` check into the row's `available` closure instead.
- `docs/seams.md` gains a row per new caller. The table is what keeps the engine's guard list and app-core's locality table honest; a guard added without its row is the drift the table exists to catch.

- [ ] **Step 1: Write the failing tests.** Engine, four intents: `add_companion` refuses on the open grid and in the Stack; `stand_down_companion` refuses in both; both succeed in base space; and — **the regression this split exists for** — `wield_program` still works outside base space. App-core, one intent: the party row is absent from the group menu outside base space and present inside it.
- [ ] **Step 2: Run, confirm failure.** `cargo test -p feral-processes-engine party` and `cargo test -p feral-processes-app-core group_menus`
- [ ] **Step 3: Add the guard to `add_companion`.**
- [ ] **Step 4: Add `stand_down_companion`,** leaving `remove_companion` alone.
- [ ] **Step 5: Repoint `app/party.rs:139`** and set the row's `locality`.
- [ ] **Step 6: Add the two `docs/seams.md` rows.** The spec lists "the
  `require_base` caller list and the guard table agree" as a test intent; it
  cannot be one. Parsing prose to check a Rust caller list is the same
  category as the player-facing strings the repo already holds by review
  rather than by census. Write the rows carefully and say in the commit body
  that this half is review-held.
- [ ] **Step 7: Green.**
- [ ] **Step 8: Mutation check.** Put `require_base` inside `remove_companion` instead and make it return `Result`; the wielding test must fail. This is the whole reason for the split and it must be pinned by a test, not by this comment. Restore.
- [ ] **Step 9: Commit.**

---

### Task 8: Final gates

**Files:** none — verification only.

- [ ] **Step 1:** `cargo test --workspace`
- [ ] **Step 2:** `cargo clippy --workspace` — fix warnings, never silence them.
- [ ] **Step 3:** `cargo fmt`
- [ ] **Step 4:** `cargo test -p feral-processes-engine balance_sim` — **no curve may move.** Nothing here touches a balance constant, so a moved curve means something was changed that should not have been. Investigate rather than re-baselining.
- [ ] **Step 5:** `git diff --quiet assets/` against the pre-task state for anything you toggled while testing. A timed-out test loop has left a shipped asset mutated in this repo before.
- [ ] **Step 6:** Report what you actually ran and saw. Do not report completion from a partial run.

---

## Deliberately not in this plan

- **The `CHANGELOG.md` section and the version bump.** Per `CLAUDE.md`, those happen once at the merge to `main`, not on the branch — a rebase or squash would invalidate a version already tagged.
- **`docs/manual.md` and the root `README.md`.** Both are carved out of the documentation obligation. `assets/structures/README.md` is not, and is in Task 4.
- **The zone level cap.** Separate spec, separate plan, no dependency either way.
