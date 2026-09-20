# The Research Station and studying a program — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Research Node grows to a 2x2 Research Station with a pen; a tamed program pinned in the pen is *under study*; every research node from sector 2 up refuses selection until one is; completion spends the subject into a `DownedProgram` record; `program_refactoring` becomes what unlocks fusion.

**Architecture:** A structure's `footprint` is authored, never stored, and derived on read through the square `tactical::footprint_cells_at` already spells. The anchor stays the one blocking cell, so a legacy 1x1 Node needs no migration. Being under study is a fifth `ProgramRole` whose consequences are omissions; the only stored fact is `components::UnderStudy { station }`, and arrival is the subject's own `Position` equalling `Game::study_pen`.

**Tech Stack:** Rust, standalone `bevy_ecs` (engine), app-core state machine, Bevy + egui renderer, RON assets.

**Spec:** `docs/superpowers/specs/2026-09-20-research-station-study-design.md`. **Read it before any task** — this plan records only the file list, the interfaces, the test intent, and where the code differs from the spec.

## Global Constraints

- Branch `feat/research-station-study`. **Never push, never merge, never tag** — landing is the controller's job. Commit freely at each green step; stage explicit paths, never `git add -A`.
- One release for the whole branch, bumped at the merge. No `CHANGELOG.md` section on the branch.
- TDD: failing test first, *watched* failing, then the code. A test that passes with the fix removed is not a test — delete the fix and watch it fail.
- Gates at every task end: `cargo fmt`, `cargo clippy --workspace --all-targets` clean (`--all-targets` is load-bearing — a bare run leaves test modules unlinted), targeted `cargo test -p <crate> <name>`. Full `cargo test --workspace` at the end of Task 3, Task 9 and Task 12 only.
- Cargo's exit code is lost through a pipe: never judge pass/fail from `| tail` or `| grep`.
- Before touching code in a subsystem, invoke the `seams` skill and read the whole matching reference file — `references/base.md` for Tasks 1–9, `references/screens.md` for Task 6, `references/hud.md` for Task 11. Not a grep; the traps cross-reference each other.
- **New asset fields are `#[serde(default)]`** so existing and modded `.ron` keeps parsing. A malformed `.ron` is skipped with a logged warning, never a panic (`StructureDb::load_dir`, `structures.rs:682`).
- **A `u8` field defaulting to 1 needs `#[serde(default = "…")]` with a named function** — a bare `#[serde(default)]` gives 0, and a footprint of 0 claims no cells.
- Update the matching `assets/*/README.md` **in the same task** as the field that changes it.
- New save field behind `#[serde(default)]`, **no `SAVE_FORMAT_VERSION` bump**, and a **save→load** test — a RON round trip cannot catch a field that is not really persisting.
- No new content hardcoded in Rust. Rust may name the *capability* (fusion) and the *geometry rule* (top-left anchor, diagonal pen); which structure studies, which nodes need a subject and which node unlocks fusion are authored.
- Tuning constants go in `crates/engine/src/tuning.rs` as documented `pub const`, never inline in a formula.
- Screen actions are UPPERCASE keys; lowercase letters select rows.
- Vocabulary: a program is *pinned*, *under study*, a *subject*; the building is the *Research Station*. No occult words.
- Don't edit `docs/manual.md`, root `README.md`, or the repo's `TODO.md`.

---

## Where the code differs from the spec (decisions)

These are verified against the source at branch HEAD. The spec was written in a brainstorm and five of its citations did not survive contact with the tree.

1. **`find_blocking_structure_at` is in `game/zone.rs:340`, not `game/base/building.rs`.** Signature `pub(crate) fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity>`. `build_site_at` *is* in `building.rs:793`.

2. **The fifth `ProgramRole` variant does not make four of its five consequences fail to compile.** The spec's central argument for the variant is that every consequence is an exhaustive match. The workspace census is **three** matches on `ProgramRole`: `ProgramRole::roster_rank` (`party.rs:50`), gui's `role_heading` (`render/party.rs:290`), and the rest-repair branch (`game/turn.rs:1322`). Everything else — the labour scheduler, the wander, the party recall, fusion candidacy — compares with `==` / `is_some_and` and **will compile silently**. So: the variant still buys the rest-repair omission for free, and the other four are held by tests and nothing else. Task 4 writes one test per omission and says so in the doc comment. Do not repeat the spec's claim in a comment.

3. **`walks_the_base` is shared with `Game::watch_position`** (`party.rs:106`, `role == Some(Staff) && task != Some(Guard)`). Widening it to `UnderStudy` widens both readers. That is wanted — a subject occupies ground — but the second reader has to be looked at and a decision stated.

4. **`ResearchStatus` is a struct (`views.rs:25`); the enum is `ResearchState` (`views.rs:123`)** with `Unlocked / Active / Available / Locked { missing, min_zone }`. The spec's "`ResearchStatus::Locked`" means `ResearchState::Locked`. `blocked_by` is a field on the struct and the spec is right about that.

5. **`settle_research`'s gate is an early-return `||`, not an `&&`** (`unlocks.rs:1029`): `if progress < def.cost || !spend_bill_from_base(…) { return; }`. The subject term and the store-room term are **negated conditions inserted before `spend_bill_from_base`**, so materials are still spent last. Order: `progress < cost` → no subject → store full → `!spend_bill`.

6. **`RARITY_BAR_PX` is `render/mod.rs:186`**, visible in `marks.rs` only through `use super::*`. `PROGRESS_BAR_PX` is local at `marks.rs:42`.

7. **`BaseGrid` has no dig-mark or build-site query.** Those are `Game::dig_site_at` (`game/base_space.rs:288`) and `Game::build_site_at` (`building.rs:793`), both ECS queries. `place_structure`'s widened refusals call those per footprint cell, not a grid method.

8. **`structure_tiles` has exactly one production caller**, `dig_wants` (`work_orders.rs:1765`). The `tests/hauling.rs:618` hit is a same-named test-local helper, not a call.

9. **The reach machinery widens (the spec's §1 table), as decided.** The direct alternative — footprint as occupancy only — was raised and declined. Two consequences the spec's table omits, both of which this plan tests explicitly:
   - **`at_station` widens with `station_candidates`** or a worker walks to a footprint face, `at_station` answers no, and it spins there for the rest of the run (`station_tiles`' own doc names this failure).
   - **`has_station` and `station_tiles` now disagree** about a Station's floor cells: `has_station` reads the footprint set and calls them taken, while `station_tiles` reads the blocked set and would let a body stand there. So `dig_wants` refuses a marked cell whose *only* free walkable face is a Station floor cell. Narrow, deliberate, and pinned by a test in Task 2 rather than left to be rediscovered.

10. **`tactical::footprint_cells_at(anchor: (i32,i32), side: u8) -> Vec<(i32,i32)>`** (`tactical/mod.rs:583`, `pub(crate)`, top-left anchored) is **called**, not copied — the repo rule that a doc comment claiming to mirror another formula must be a call. It stays where it is: `walk_field` living in `game/pursuit.rs` and being called from tactical is the precedent for a shared geometry helper keeping the home of whatever first needed it.

11. **`ALL_MODES` is `[Mode; 112]`** (`render/mod.rs:1498`). A new variant bumps the length, which is a semantic merge conflict — rebase carefully. The list is hand-written and the draw match ends `_ => {}` at `render/mod.rs:1430`, so a missing arm ships a **blank screen** and nothing fails to compile.

12. **`research_node.ron` has no `description` beyond one line and no `min_zone`.** Current contents are exactly: `id, name, description, glyph: 'R', color: Cyan, build_cost: [("core_fragment", 10)], work: Some((produces: "research_data", ticks_per_unit: 14, level: Some(1))), upgrade: Some((max_tier: 5, cost: [("core_fragment", 10), ("cache_grain", 1)])), power_draw: 1`.

13. **The 19/8 split is confirmed exactly.** 27 shipped research files. The 8 with no `min_zone` are `armor_bench, automation, commerce, fortification, power_grid, routine_fabrication, teardown, weapon_bench` — precisely the set the spec names. The other 19 all carry `min_zone >= 2`.

---

# Part A — the footprint seam

### Task 1: `footprint` and `studies`, and the occupancy readers

**Files:**
- Modify: `crates/engine/src/structures.rs:263-464` (two fields + the named default fn), `crates/engine/src/game/zone.rs:340` (`find_blocking_structure_at`), `crates/engine/src/game/base/building.rs:793` (`build_site_at`), `crates/engine/src/game/base/building.rs:24` (`place_structure`'s refusal ladder)
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Produces: `StructureDef::footprint: u8` (`#[serde(default = "default_footprint")]`, returns 1) and `StructureDef::studies: bool` (`#[serde(default)]`); `pub(crate) fn Game::structure_footprint(&self, kind: &StructureId) -> u8` (1 for an unresolvable id); `pub(crate) fn Game::structure_footprints(&mut self) -> Vec<(Entity, Position, u8)>` — one query over `(Entity, &Position, &Structure)` with `With<Structure>`, the def's side looked up per row.
- Consumes: `crate::tactical::footprint_cells_at(anchor, side)`.

`find_blocking_structure_at` and `build_site_at` become point-in-footprint: the answer is the structure (or site) **any** of whose footprint cells contains `(x, y)`. `place_structure`'s three cell refusals at `building.rs:117` (`is_floor`), `:121` (`find_blocking_structure_at`) and `:129` (`build_site_at`) each widen to *every* cell of the footprint being placed, and a fourth is added for a dig mark via `Game::dig_site_at`. The body refusal at `:148` widens too, keeping its existing exemption for the program paying for the build.

**Fixture trap:** `support.rs`'s `spawn_structure_at` (`:1053`) bare-spawns a `Structure` and is for what a standing structure *enables*, not for the build rules — it bypasses the refusal ladder entirely. Every placement-refusal test here goes through `place_now` (`support.rs:791`) or `Game::place_structure` directly. The squad seam's failure was that every fixture was hand-built at footprint 1, so every anchor-measuring reader stayed green across 5,964 passing tests; the 2x2 fixtures are the ones that matter most in this plan.

- [ ] **Step 1: Failing tests.** In `tests/building.rs`: an unannotated def reads `footprint == 1` and `studies == false`; a def authoring `footprint: 2` reads 2. Then, one refusal case per blocker kind — a structure, a build site, a dig mark, and unfloored rock — each sitting in a *non-anchor* cell of a 2x2 being placed, and each refused; and the same placement succeeding with all four cells clear. Then `find_blocking_structure_at` and `build_site_at` each answering for a non-anchor cell of a placed 2x2.
- [ ] **Step 2:** Watch them fail. Expected: compile failure (no `footprint` field), then assertion failures on the refusals.
- [ ] **Step 3:** Implement. Keep the refusal *order* as it is — the ladder's order is what decides which sentence the player gets, and each widened check must still name the cell it refused.
- [ ] **Step 4:** Targeted `cargo test -p feral-processes-engine building` green.
- [ ] **Step 5:** Document `footprint` and `studies` in `assets/structures/README.md`: the side of a square claim, **anchored top-left**; the anchor is the impassable cell carrying the glyph; the other cells are the structure's own **walkable floor**; the pen is the cell **diagonally opposite** the anchor; the claim is checked only at placement, so a structure already standing is never refused retroactively.
- [ ] **Step 6:** `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.

### Task 2: The reach machinery

**Files:**
- Modify: `crates/engine/src/game/base/hauling.rs:190` (`blocked_tiles`), `:245` (`station_candidates`), `:105` (`at_station`), `:764` (`haul_step_system`'s construction site)
- Modify: `crates/engine/src/game/zone.rs:274` (`structure_tiles`), `:285` (`Game::blocked_tiles`)
- Test: `crates/engine/src/tests/hauling.rs`, `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn blocked_tiles(structures: impl Iterator<Item = (Position, u8)>, bodies: impl Iterator<Item = Position>) -> HashSet<(i32, i32)>` — **emits each structure's anchor only**, and takes the side so the two sets below are built from one input shape and no caller can hand them different rows.
  - `pub(crate) fn footprint_tiles(structures: impl Iterator<Item = (Position, u8)>) -> HashSet<(i32, i32)>` — every cell of every footprint. `Game::structure_tiles`' new body.
  - `pub(crate) fn station_candidates(grid, structure: Position, side: u8, blocked: &HashSet<(i32,i32)>) -> Vec<Position>` — walkable, unblocked orthogonal neighbours of **every** footprint cell, deduplicated, `(x, y)`-sorted.
  - `pub(crate) fn at_station(worker: Position, structure: Position, side: u8) -> bool`.
- Consumes: `Game::structure_footprints()` from Task 1.

`station_tiles`, `post_field`, `reaches`, `post_reach` and `has_station` thread the side through; none of their own logic changes. **The two-iterator shape of `blocked_tiles` stays** — it is what keeps `post_field` and `crew_reach` asking one question about which cells are crossable, and a caller that could pass the structures alone would silently ask a different one.

**The invariant that stops the spin:** `at_station(w, s, side)` is true for exactly the positions `station_candidates(grid, s, side, &empty)` can return, plus nothing else. Test it as an equivalence over a 2x2 rather than as two separate assertions.

- [ ] **Step 1: Failing tests.** A hauler's walk refuses to cross a 2x2's anchor and **does** cross its floor cells. `station_candidates` offers faces of the whole footprint and does not offer the anchor. The `at_station` ↔ `station_candidates` equivalence above. `Game::blocked_tiles` contains the anchor and *not* the floor cells; `Game::structure_tiles` contains all four. `has_station` still answers true for a marked cell whose only face holds the digger standing at it — the dig-crew deadlock the narrower set exists to avoid.
- [ ] **Step 2:** Watch fail (compile failure on the changed signatures first).
- [ ] **Step 3:** Implement.
- [ ] **Step 4: The deliberate divergence, pinned.** Add a test asserting that `dig_wants` drops a marked cell whose only free walkable face is a Station floor cell, with a doc comment saying this is decided rather than discovered: `has_station` reads the footprint set and `station_tiles` reads the blocked set, so a body could physically stand there and the want is refused anyway. State the same rule in a sentence at **both** `Game::structure_tiles` and `Game::blocked_tiles`, extending the "not interchangeable" note already on them.
- [ ] **Step 5:** `cargo test -p feral-processes-engine hauling` and `… work_orders` green; fmt; clippy; commit.

### Task 3: The Station itself, and the pen

**Files:**
- Modify: `assets/structures/research_node.ron` (add `footprint: 2, studies: true`; new name and description; **keep `id: "research_node"`** and every other field)
- Create: `crates/engine/src/game/base/study.rs` (registered in `game/base/mod.rs`) — the home for `study_pen` and, in Task 5, the two pin doors
- Test: `crates/engine/src/tests/building.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `pub(crate) fn Game::study_pen(&self, structure: Entity) -> Option<(i32, i32)>` — `None` unless the structure's def declares `studies`; otherwise the footprint cell diagonally opposite the anchor, `(x + side - 1, y + side - 1)`. **The one door**: the pin, the walk, the block check, the draw and the consumption all call it; nothing re-derives a corner.

- [ ] **Step 1: Failing tests.** `study_pen` is `None` for a Lathe and `Some((x+1, y+1))` for a Station at `(x, y)`. A Station placed with any of its four cells occupied is refused (Task 1's ladder, now reached through the real def). **The legacy case:** a 1x1 Research Node hand-spawned through `spawn_structure_at` with occupied neighbours stands, blocks only its anchor, and is not refused retroactively — load it from a save if `support.rs` makes that cheap, otherwise spawn it and assert the same three facts.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green.
- [ ] **Step 5: Censuses** in `tests/assets.rs`, sited beside the existing structure censuses: exactly one shipped structure declares `studies`; every structure declaring `studies` declares `footprint >= 2`.
- [ ] **Step 6:** `cargo test --workspace` — **the first full gate**, because Task 2 changed a signature every base-space walk goes through. Then fmt, clippy, commit.

---

# Part B — being under study

### Task 4: The fifth role, and the walk

**Files:**
- Modify: `crates/engine/src/game/party.rs:20` (`ProgramRole`), `:50` (`roster_rank`), `:66` (`role_of`), `:106` (`walks_the_base`), `crates/engine/src/components.rs` (`UnderStudy`), `crates/engine/src/game/turn.rs:1322` (the rest-repair match), `crates/engine/src/game/base/work_orders.rs:1954` (`drift_idle_staff`), `crates/gui/src/render/party.rs:290` (`role_heading`)
- Test: `crates/engine/src/tests/party.rs`, `crates/engine/src/tests/work_orders.rs`

**Interfaces:**
- Produces: `ProgramRole::UnderStudy`, **between `Sortie` and `Staff`** (`Staff` stays what is left over); `#[derive(Component, Clone, Copy, Debug)] pub struct UnderStudy { pub station: Entity }` — the only stored fact, never saved directly (Task 6 saves the station's tile instead).
- `role_of` gains an `UnderStudy` arm before its `Staff` fallback, reading the component. `walks_the_base` becomes true for `Staff` (less guards) **and** `UnderStudy`.

**Arrival is derived, never stored**: the subject is in its pen when its `Position` equals `Game::study_pen(station)`. No `arrived: bool`.

`drift_idle_staff` gains an `UnderStudy` arm **above** the `Downed` arm at `work_orders.rs:2033`, gated on laid floor exactly as that arm is — a program pinned while it was in the Stack arrives through `entry_tile` rather than teleporting. It walks toward the pen and stops there. **Pinning writes no `Position`**; a body sets off from the tile it is standing on, `post_worker`'s rule. `drift_idle_staff`'s caller must hand it the staff list **plus** the bodies under study, or a pinned program falls out of the only pass that walks anything and stands still forever.

- [ ] **Step 1: Failing tests — one per omission**, because per decision (2) the compiler holds only one of them. A pinned program: is offered no job by `schedule_base_labour`; does not wander off the pen once it has arrived; is not recalled by a party add; is not a fusion candidate; is not healed by a rest. Then: it walks from where it was standing to the pen and stops; two programs cannot share the pen; `roster_rank` and `role_heading` answer for the new variant.
- [ ] **Step 2:** Watch fail. The three exhaustive matches fail to compile; the five omission tests fail on assertions. If one of the five passes before the code is written, it is vacuous — find out why before continuing.
- [ ] **Step 3:** Implement. **Decide and state in a doc comment what `Game::watch_position` should do with a subject** (decision 3) — it shares `walks_the_base`, so this is a choice, not a side effect.
- [ ] **Step 4:** Targeted green; fmt; clippy; commit.

### Task 5: Pinning and unpinning

**Files:**
- Modify: `crates/engine/src/game/base/study.rs` (created in Task 3 — the pin doors live beside `study_pen`, not in `building.rs`, which is already past 1,200 lines and owns a different responsibility)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Produces: `pub fn Game::pin_subject(&mut self, program: Entity, station: Entity) -> Result<(), String>` and `pub fn Game::unpin_subject(&mut self, program: Entity) -> Result<(), String>`.

**Every refusal lands before anything is written, asserted per refusal** — `select_research`'s rule, and for its reason: a single test over one of several refusals passes against all the ones that never write anyway.

`pin_subject` refuses: game over or an active battle; the program is not a tamed program the player owns; it is already under study; its role is not `Staff` (a partied, wielded or away-on-sortie program comes home first, and saying so beats silently recalling it); the structure does not declare `studies`; the pen is not floor or already holds a body; the program cannot route to the pen.

`unpin_subject` refuses while the active project requires a subject, **naming the project and saying that abandoning it is how you change your mind** — verbatim the shape of `select_research`'s "a project is already active" refusal.

- [ ] **Step 1: Failing tests — one per refusal**, each asserting that nothing was written (no `UnderStudy` component, role unchanged). Plus the success case, and a round trip: pin then unpin with no project active returns the program to `Staff`.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green; fmt; clippy; commit.

### Task 6: The save

**Files:**
- Modify: `crates/engine/src/save.rs:318` (`CreatureSave`), `crates/engine/src/game/lifecycle.rs` (the `CreatureRestore` field ~`:208-213` and its resolution ~`:1392`)
- Test: `crates/engine/src/tests/save.rs`

**Interfaces:**
- Produces: `CreatureSave::study_station: Option<(i32, i32)>`, `#[serde(default)]`.

**The station's tile, not its `Entity`** — entity ids are not stable across a save (`party_slot`'s precedent, and `SortieSave` carries no member list for the same reason). **Resolved after structures restore**, through the `pending_cronjobs` deferral on `CreatureRestore`: `attach_cronjobs` runs at `lifecycle.rs:1392` immediately after `restore_structures` at `:1390`, and `pending_patrols` (resolved after `restore_settlements`, `:1414`) is the tether precedent to copy — including its leniency. **A tile resolving to no `studies` structure leaves the program as ordinary `Staff`**, silently, which is what keeps a save loadable after a modder deletes the Station.

Additive behind a default: **no `SAVE_FORMAT_VERSION` bump**.

- [ ] **Step 1: Failing tests.** A **save→load** test (not only a RON round trip — a round trip cannot catch a field that is not really persisting): pin a subject, save, load, and the program is still `UnderStudy` on the same station. A save whose `study_station` names an empty tile loads with that program as `Staff` and no error. An old save with the field absent loads.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted `cargo test -p feral-processes-engine save` green; fmt; clippy; commit.

---

# Part C — the gate and completion

### Task 7: `requires_subject`, and the block reason

**Files:**
- Modify: `crates/engine/src/research.rs:41` (the field), the **19** `.ron` files under `assets/research/` with `min_zone >= 2` (listed in decision 13), `crates/engine/src/game/unlocks.rs:727` (`research_block`)
- Modify: `assets/research/README.md`
- Test: `crates/engine/src/tests/unlocks.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `ResearchDef::requires_subject: bool`, `#[serde(default)]`.

The gate goes **inside `research_block`** as a third block reason beside the no-Research-Node reason and the per-material `chain_break` line — **not** as an arm on `select_research`. `research_block` is what fills `ResearchStatus::blocked_by`, so the row the screen marks blocked and the refusal the player is handed cannot disagree; a check only in `select_research` leaves the menu offering a row it then refuses. The condition is "the `(x, y)`-first `studies` structure has a body in its pen" — `assembler_system`'s sorting rule, because bevy's query iteration order is not stable and two Stations would otherwise resolve differently between runs.

**Outside `research_block_memo`** (`unlocks.rs:744`), which memoises per `ItemId`: the subject test is per *node*, not per item.

Synthesised routine nodes are **not** gated — `routine_tree::synthesise_nodes` (`routine_tree.rs:178`) leaves the field at its default and must keep doing so.

**Blocked, not hidden.** The 19 nodes stay listed; `ResearchState::Locked` and the block reason are the disclosure.

**Pin first, then select.** There is deliberately no "a project is selected but has no subject" state, so **`MachineStatus` gains nothing** and its four reachable values stay `Idle` / `Unstaffed` / `Stranded` / `Running`. That is the reasoning the seam already recorded when it rejected a `MachineStatus::NoProject`: a missing precondition should make the want unraisable rather than mint a new status every census has to learn and a second writer could contradict.

- [ ] **Step 1: Failing tests.** Every subject-gated node is refused by `select_research` with nobody pinned, **and the refusal's sentence equals `research_block`'s `blocked_by` for the same node — compared against a live call, never hardcoded prose.** Each of the eight ungated nodes is selectable with nobody pinned. **A reachability test, not only refusal tests:** pinning a program makes the same selection *succeed* — a refusal can ship permanent with every refusal test still green. Unpinning is refused while a subject-gated project is active (Task 5's door, now reachable).
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green.
- [ ] **Step 5: Censuses** in `tests/assets.rs`, beside `every_research_material_is_reachable_through_that_nodes_own_prerequisites` (`:4610`): every shipped node with `min_zone >= 2` declares `requires_subject` and none of the eight ungated ones does; no subject-gated node is a prerequisite of a node that is not. Confirm `every_research_material_is_reachable_…` still passes unchanged.
- [ ] **Step 6:** Document `requires_subject` in `assets/research/README.md`; fmt; clippy; commit.

### Task 8: Completion spends the subject

**Files:**
- Modify: `crates/engine/src/game/unlocks.rs:1004` (`settle_research`), `:984` (`research_material_shortfall`)
- Test: `crates/engine/src/tests/unlocks.rs`

Per decision 5, the gate is an early-return `||`. Insert the two new negated terms **before** `spend_bill_from_base` so materials are still spent last:

`progress < def.cost` → no subject pinned → the downed-program store has no room → `!spend_bill_from_base(…)`.

The short-circuit is the load-bearing half. `a_full_bill_alone_does_not_complete_a_project` exists because spending materials on a half-researched project was the failure; consuming the **subject** on one is the same failure with a worse loss, since a program is not refundable and a material on a shelf is.

**Which subject is spent** is the `(x, y)`-sorted first `studies` structure's, the same rule Task 7's block reason reads by and for the same reason: bevy's query iteration order is not stable, so two Stations would otherwise resolve differently between runs and encode differently in the save.

The conversion is **two existing doors and no new function**: `downed_program_for_with_overkill(subject, 0.0)` (`game/combat_rewards.rs:639`) → `push_downed_program` (`:739`) → despawn. `0.0` is `overkill_term`'s identity value, so the condition roll is best-case — a controlled dissection yields better material than overkilling something in the field, which is the right way round and is free.

`push_downed_program` returns `false` at `tuning::MAX_DOWNED_PROGRAMS` (10) and logs. **That refusal blocks completion and is reported the way a material shortfall is**, through `research_material_shortfall`. The failure to head off is the naive ordering — consume, push, discover the push failed — which eats the body *and* the knowledge.

- [ ] **Step 1: Failing tests.** Full progress + full material bill + no subject does **not** complete, and the materials are still on the shelf. Full progress + a subject completes, the subject leaves the world, and exactly one `DownedProgram` appears whose species and level match it — **level asserted against the subject's real `Experience`**, since `ability_user_level` reads it where a wild kill falls back to `ZoneLevel`. A routine installed from a disk comes back as `carried` (`carried_routine` reads the live `Routines` minus the species kit). A full `DownedPrograms` store blocks completion, reports, and leaves **both** the subject and the progress intact.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green; fmt; clippy; commit.

### Task 9: Losing the station, and a template to play from

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs:783` (`damage_structure`, beside its `clear_pending_build_at` at `:849`), `crates/engine/src/game/base/building.rs:1148` (`remove_structure`, beside its at `:1241`)
- Modify: `dev-saves/README.md`
- Test: `crates/engine/src/tests/building.rs`

Both destruction doors release the subject back to `Staff` and abandon the project through the existing `Game::abandon_research` (`unlocks.rs:892`), so `withdraw_research_orders` runs and the material orders leave the queue. This is `clear_pending_build_at`'s rule with a second subject: **the door left out silently strands a program in a role nothing can get it out of, and nothing fails to compile.**

- [ ] **Step 1: Failing tests — one per door.** Destroying a Station under a running subject-gated project releases the subject to `Staff`, abandons the project, and withdraws its material orders.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green.
- [ ] **Step 5:** `cargo test --workspace` — **the second full gate**.
- [ ] **Step 6: Capture a `dev-saves/` template.** An engine-side test drives the state — a base with a Station, a pinned subject and an active subject-gated project — and `cargo run --bin savetool -- capture saves/save.bin <name>` names it. No display needed. Document it in `dev-saves/README.md`. Testing this by hand otherwise starts with an hour of play, and that cost is what makes features ship unplaytested.
- [ ] **Step 7:** fmt; clippy; commit.

---

# Part D — fusion, drawing, and the screen

### Task 10: Fusion becomes a researched capability

**Files:**
- Modify: `crates/engine/src/research.rs` (the field), `assets/research/program_refactoring.ron`, `crates/engine/src/game/unlocks.rs` (the query, beside `routine_tree_open` at `:317`), `crates/engine/src/game/party.rs:1169` (`fuse_companions`), `crates/app-core/src/app/group_menu.rs:303` (the fusion row)
- Modify: `assets/research/README.md`
- Test: `crates/engine/src/tests/party.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `ResearchDef::unlocks_fusion: bool` (`#[serde(default)]`); `pub fn Game::fusion_unlocked(&self) -> bool`.

`fusion_unlocked` is **`routine_tree_open`'s exact shape including its lenient rule** (`unlocks.rs:317`): if no loaded node carries the flag, fusion is open from the start, so a mod that replaces the research tree is not stranded without a capability the base game had. `ResearchDef` gets a bool per capability rather than an `unlocks_actions: Vec<String>` registry — the split `perks.rs` already documents, where the catalogue is data and the effect is a named query in Rust.

The flag goes on **`program_refactoring`** (sector 2, cost 75, requires `automation`), the node whose shipped description is already about surgery on tamed programs. `fuse_companions` gains one refusal placed with its existing four, all of which land before anything is consumed. The menu row's `available:` closure gains one term beside `owned_pets().len() >= 2`. Fusion keeps `Locality::Anywhere` and needs no structure standing. **Existing saves lose fusion until they research it, with no grandfathering** — the decision taken.

- [ ] **Step 1: Failing tests.** `fuse_companions` refuses before the research and succeeds after, consuming nothing on the refusal. A `ResearchDb` with no node carrying the flag reads `fusion_unlocked() == true`. The menu row is hidden before and shown after.
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green.
- [ ] **Step 5: Census** in `tests/assets.rs`: exactly one shipped node carries `unlocks_fusion`. Document the field and its lenient rule in `assets/research/README.md`.
- [ ] **Step 6:** fmt; clippy; commit.

### Task 11: Drawing the Station and the pin

**Files:**
- Modify: `crates/gui/src/render/marks.rs` (the bracket geometry, sited beside `nemesis_mark_rect:101` / `patrol_mark_rect:127` / `staffed_mark_rect:375`), `crates/gui/src/render/base.rs:552` (`draw_surface_map` — the tile fill at `:844`), `crates/gui/src/render/base.rs:147` (`corner_marker`)
- Test: `crates/gui/src/render/marks.rs`'s own test module, `crates/gui/src/render/base.rs`'s

**Interfaces:**
- Produces: a free function in `marks.rs` returning the four bracket rects from `(px, py, tile_px)`, matching the shape of every other mark there so it is unit-testable **without a `Painter`**.
- Consumes: two engine-side doors, added in this task and **the renderer's only way to ask** — `pub fn Game::view_station_floor_at(&mut self, x: i32, y: i32) -> bool` (is this cell a non-anchor footprint cell of a standing `studies` structure) and `pub fn Game::view_pinned_at(&mut self, x: i32, y: i32) -> bool` (does a body under study stand here). `view_finishes_at` is the precedent: the derivation stays in the engine and gui holds no copy of the geometry.

**The floor cells take a dark yellow/brown fill and draw no outline.** `marks::outline_open` (`:451`) already owns the tile-edge ring, drawing a machine's walls as 2px lines flush at the tile's edges, and the existing corner marks carry insets specifically so nothing reads as painting an absent wall back in. Keeping the ring clear is what leaves it available for the brackets. Drawn only on cells the Station actually owns — which, because the footprint is derived per read, excludes a legacy Node's cells that turned out to hold something else.

**Four red L-brackets on the tile-edge ring**, clear of `RARITY_BAR_PX` (`render/mod.rs:186`) at the top and `PROGRESS_BAR_PX` (`marks.rs:42`) at the bottom, drawn `* vig` like every other mark so an edge-of-light tile does not leave them burning. **The top-left bracket yields to the Alt `?` marker**, extending `corner_marker`'s existing "borrows the top-left while `reveal` is held" rule rather than inventing an arbitration.

- [ ] **Step 1: Failing tests.** The bracket rects clear the rarity bar at the top and the progress bar at the bottom. The top-left bracket is absent while the Alt marker draws and present otherwise. The three floor cells take the floor fill and the anchor does not. A legacy 1x1 Node draws no floor cells.
- [ ] **Step 2: The corner census — a proof, not a suppression.** Assert that a body under study can wear none of the four corner marks: the con earmark (`ConRead::of` answers `None` for anything non-hostile), the nemesis mark and the patrol mark (both wild-only), and the staffed mark (`wears_job_mark` is about a posted program, and `UnderStudy` is not `Staff`). All four are unreachable today, so this is a proof — and it fails loudly the day con reads apply to owned programs, which is exactly when the ring needs re-examining.
- [ ] **Step 3:** Watch fail. **Step 4:** Implement. **Step 5:** Targeted `cargo test -p feral-processes-gui` green; fmt; clippy; commit.

### Task 12: The screen

**Files:**
- Modify: `crates/app-core/src/lib.rs:1300` (`Mode`), `crates/app-core/src/app/group_menu.rs` (the row, with the base entries near `:124`), `crates/app-core/src/app/input.rs:227` (dispatch), a new handler beside `crates/app-core/src/app/building.rs:311`
- Modify: `crates/gui/src/render/mod.rs:1498` (`ALL_MODES`, length 112 → 113), `:1430`-area (the draw arm, **above** the `_ => {}`), `:561` (`needs_status_banner`), a new draw fn beside `crates/gui/src/render/building.rs:665`
- Test: `crates/app-core/src/tests/`, `crates/gui/src/render/mod.rs`'s mode coverage tests

**Interfaces:**
- Produces: `Mode::PinSubject` — a program picker over `Game::base_staff()` (`party.rs:911`), **modelled on `Mode::BuildProgram`** (`lib.rs:1450`, handler `App::handle_build_program_key` at `app/building.rs:311`, draw `draw_build_program` at `render/building.rs:665`, dispatched at `input.rs:227` and `render/mod.rs:1059`). Copy that flow's shape.
- One `GroupEntry` with the base rows: a study label, `locality: Locality::Base`, and an `available:` closure requiring a `studies` structure standing.

**One row, not two.** It reads as unpin when a subject is already pinned, because "at most one subject per station" is the rule and a second row would be a second place to state it.

Two traps, both recorded and both silent: **`ALL_MODES` does not fail to compile** — the list is hand-written and the draw match ends `_ => {}`, so a missing entry ships a **blank screen**; the variant goes in `ALL_MODES`, in the draw match and in `needs_status_banner`. And **lowercase letters are row selectors** — any key this screen binds beyond row selection and Enter must be UPPERCASE, or one keypress both picks a row and fires it.

- [ ] **Step 1: Failing tests.** The row is hidden with no Station standing and shown with one; selecting it opens `Mode::PinSubject`; picking a program pins it; the row reads as unpin with a subject pinned and unpinning works through it. `ALL_MODES` contains the new variant and the mode-coverage test passes (it will fail on the length until bumped).
- [ ] **Step 2:** Watch fail. **Step 3:** Implement. **Step 4:** Targeted green.
- [ ] **Step 5:** `cargo test --workspace` — **the final gate**. fmt; clippy; commit.

---

## Closing out the branch

- [ ] **The seam's three writes**, in this order (the `seams` skill documents them): the **argument** to the memory graph as `seam:structure-footprint` and `seam:under-study` via `memory_add_observations(entityType: "seam", subsystem: "seams")`, two observations each with the title first; the **trap** as a bullet in `.claude/skills/seams/references/base.md` in the house style; the **rule** in `CLAUDE.md` under *The base*, **exactly one sentence each**. That budget is the point — `CLAUDE.md` reached 151 KB by letting traps creep back in beside rules.
  - Candidate rules: "**A structure's footprint is a claim on placement and a derivation on read**, never stored, and the anchor is the one cell that blocks." / "**A pinned program is a fifth `ProgramRole` whose consequences are omissions**, and only the rest-repair one fails to compile."
- [ ] **Archive the spec on landing**, not by a later sweep: move `docs/superpowers/specs/2026-09-20-research-station-study-design.md` to `docs/superpowers/archive/specs/` and add its row to `docs/superpowers/INDEX.md`. If any source doc comment cites its path, move the citation in the same change — never during the deploy itself.
- [ ] Report to the controller. **Do not merge, tag or push.**

## What the tests cannot see

**Nobody can play this.** `cargo run` needs a display and there is none in an agent session, so every claim here is a claim about source and tests. A green suite is not evidence of play. Three things a test can confirm the geometry of while they still look wrong: the dark yellow/brown floor against the rock palette, the brackets' weight against the rarity bar, and whether a 2x2 building reads as one thing or as a building next to some floor.

**`balance_sim` will report nothing.** Every sector-2+ node just got strictly more expensive by one program, and `balance_sim` models no research at all — no active project, no Research Data, no materials bill. Run `cargo test -p feral-processes-engine balance_sim` anyway; a curve that moves means something unintended happened. This plan changes no tuning value, and whether `cost` and `materials` should come down to pay for the subject is a question for after the feature has been played.

**Nineteen bodies is an unmeasured price.** Reaching sector 6 now means pinning and spending nineteen programs. Captures, gifts, adoptions and Stack orphans all supply them and the roster cap is 200, so there is no deadlock. Whether the *pace* is right is exactly the thing only play will say.

**A cramped legacy footprint is a state nobody will think about again.** A Research Node in an old save whose neighbours are occupied stands forever with a pen it cannot use, and the only surface saying so is a pin refusal the player sees only when they try. That is the deliberate price of taking "same structure id" with no migration.
