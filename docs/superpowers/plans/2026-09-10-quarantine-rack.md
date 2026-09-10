# Quarantine Rack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A base structure that stores downed programs, filled and emptied from
the `c` transfer picker, from which posted base staff fetch a carrier when the
rig they are working needs one.

**Architecture:** A new `StructureDef::racks` field and a `Racked(Vec<DownedProgram>)`
component make a third home for an instanced carrier. The transfer picker gains
carrier rows whose basket range is `[-1, +1]`, riding every rule the item rows
already follow. A posted body at a rig with an empty hopper walks to a rack in
`crew_reach`, picks a carrier up in a new `CarryingProgram` component, and loads
it with the tool that rig was last hand-loaded with.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), serde/RON assets, the
existing `Mode::Transfer` screen in app-core + gui.

**Spec:** `docs/superpowers/specs/2026-09-10-quarantine-rack-design.md` — read it
first; this plan argues from it and does not restate its reasoning.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** Every save field here is additive behind
  `#[serde(default)]`. If a task finds itself removing a field or changing one's
  meaning under a name it keeps, stop and raise it.
- **Content stays data.** The rack is `assets/structures/quarantine_rack.ron`.
  Nothing about it is hardcoded in Rust beyond the schema field.
- **New `StructureDef`/`Hopper` fields are `#[serde(default)]`**, and
  `assets/structures/README.md` is updated in the same task that adds one.
- **The basket's sign convention is positive = take, negative = put.** Do not
  change it; carrier rows adopt it.
- **Uppercase letters are screen actions, lowercase are row selectors.** No new
  key is added by this feature, but do not break that rule if one becomes
  tempting.
- **Every refusal lands before anything is spent**, and is asserted *per
  refusal* — one test over one path passes against paths that never spend.
- **Gates:** each task ends green on `cargo test -p feral-processes-engine <name>`
  (or the matching crate). `cargo test --workspace`, `cargo clippy --workspace`
  and `cargo fmt` are the final gate in Task 8, not per task.
- **Version bump, CHANGELOG section and tag happen at the merge**, not on this
  branch.

---

### Task 1: The schema and the content

**Files:**
- Modify: `crates/engine/src/structures.rs` — `RackDef` beside `StripDef` (~line 92), `StructureDef::racks` beside `strips` (~line 319)
- Create: `assets/structures/quarantine_rack.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `structures::RackDef { pub slots: u32 }`; `StructureDef::racks: Option<RackDef>`.

**Steps:**

- [ ] **Write the failing census** in `tests/assets.rs`: a def that declares
  `racks` must not `needs_program()`. Key it on `def.racks.is_some()` and write
  it as its own test beside
  `every_shipped_shelf_and_the_portal_cost_no_program` — that one keys on
  `def.stores`, which a rack does not set, so it will never cover this.
  Count the racks found and assert the count is non-zero, or the census passes
  vacuously the day someone deletes the file.
- [ ] **Run it.** Expect FAIL: no `racks` field exists.
- [ ] **Add `RackDef` and the field.** `slots` only. `#[serde(default)]` on the
  field. Doc comments state why `slots` is authored per machine rather than in
  `tuning.rs` (`StripDef::hopper`'s reason) and that the ceiling is `slots * tier`
  derived per read, never stored.
- [ ] **Author `quarantine_rack.ron`** exactly as the spec's block gives it:
  `slots: 8`, `costs_no_program: true`, no `stores`, `upgrade` asking
  `cache_grain` so `every_upgrade_path_asks_for_a_zone_material` stays green.
- [ ] **Run the censuses:** `cargo test -p feral-processes-engine assets`.
  Expect PASS, including the zone-material and structure-schema ones.
- [ ] **Update `assets/structures/README.md`** with the `racks` field — what it
  is, that tier multiplies it, and that a rack is not a haul target.
- [ ] **Commit.**

---

### Task 2: The store, its ceiling, and the save

**Files:**
- Modify: `crates/engine/src/components.rs` — add `Racked`; **rewrite the `Hopper` doc at ~line 600** that says a `DownedProgram` lives in exactly two places
- Modify: `crates/engine/src/game/base/building.rs:451` (spawn) and `crates/engine/src/game/lifecycle.rs` (~908 restore, ~1930 save query)
- Modify: `crates/engine/src/save.rs` — `StructureSave::racked`
- Test: `crates/engine/src/tests/extraction.rs` (or a new `tests/racks.rs` if that file is already unwieldy)

**Interfaces:**
- Produces: `components::Racked(pub Vec<DownedProgram>)`;
  `Game::rack_slots(&self, rack: Entity) -> u32` (the derived ceiling);
  `Game::rack_room(&self, rack: Entity) -> u32`;
  `save::StructureSave::racked: Vec<DownedProgram>`.

**Steps:**

- [ ] **Write three failing tests.** (a) A built rack has a `Racked` component and
  `rack_slots` equal to `slots` at tier 1 and `slots * tier` after an upgrade.
  (b) A save→load round trip on a stocked rack returns the same carriers in the
  same order — a real `Game::save` then `Game::load`, **not** a RON round trip,
  which cannot catch a skipped field. (c) A structure whose def declares no
  `racks` gets no component, and a stored `racked` on such a structure is dropped
  on load (`durability`'s rule, the arm above `strips` in `lifecycle.rs`).
- [ ] **Run them.** Expect FAIL.
- [ ] **Add the component and the two `Game` methods.** `rack_slots` reads the
  def's `slots` and the entity's `StructureTier`, multiplying; a structure with
  no tier reads as tier 1. `rack_room` is `rack_slots` minus the vec's length.
- [ ] **Wire spawn and restore.** Insert `Racked::default()` beside
  `Stock::new(def.capacity)` in `building.rs`, gated on `def.racks.is_some()`;
  mirror it in `lifecycle.rs`'s restore beside the `def.strips.is_some()` arm.
  Both lists are hand-written copies of each other and nothing fails to compile
  when they drift — that is the stated trap, so touch both in this step.
- [ ] **Add the save field and the query arm.** The structure query at
  `lifecycle.rs:1930` goes from a 10-tuple to an 11-tuple; if bevy's tuple arity
  refuses it, split the query rather than dropping a field, and say so in the
  commit message.
- [ ] **Rewrite the `Hopper` doc.** It currently states the two-place instance
  boundary as load-bearing. Name the third place, and restate what still holds:
  what leaves a **rig** is still plain items in `Stock::output`, so hauling,
  depots, `collect::plan_adjacent_take` and work orders are untouched.
- [ ] **Run the tests.** Expect PASS.
- [ ] **Commit.**

---

### Task 3: The engine's carrier moves and the one commit door

**Files:**
- Modify: `crates/engine/src/game/base/transfer.rs`
- Modify: `crates/engine/src/views.rs` — the row types
- Test: same file as Task 2's tests

**Interfaces:**
- Produces: `Game::adjacent_racks(&self) -> Vec<Entity>` (in `(x, y)` order);
  a parameter object for the commit — the existing `transfer_items(take, give)`
  gains two more lists and that is the signature that should become a struct
  rather than a fourth and fifth positional argument;
  `Game::rack_offer(&self) -> Vec<views::TransferCarrier>`, where
  `views::TransferCarrier` carries the `DownedProgram`, its label, which side it
  is sitting on, and — for a racked one — which rack entity holds it. This is the
  engine-side view type; Task 4's `TransferEntry` is the app-core sum of
  `views::TransferRow` and this.

**Steps:**

- [ ] **Write the refusal tests first, one per refusal.** (a) A take that would
  push the pack past `MAX_DOWNED_PROGRAMS` is refused and **nothing moves** —
  assert the rack still holds it *and* the pack is unchanged. (b) A put with no
  room on any adjacent rack is refused the same way. (c) A basket mixing items
  and carriers commits both or neither. (d) With two adjacent racks, a put lands
  in the first with room in `(x, y)` order. Assert per refusal; a single test
  over one path passes against every path that never spends anyway.
- [ ] **Run them.** Expect FAIL.
- [ ] **Introduce the basket parameter object** and move `transfer_items`' two
  lists into it. One caller in app-core; update it in this task so the tree
  stays green.
- [ ] **Implement the carrier halves** inside the existing commit door, keeping
  its stated order: refusals first, then take, then give, so a basket can be
  funded by its own take.
- [ ] **Run the tests.** Expect PASS. Also run
  `cargo test -p feral-processes-engine transfer` to catch the existing screen's
  tests.
- [ ] **Commit.**

---

### Task 4: Carrier rows in the picker

**Files:**
- Modify: `crates/app-core/src/app/basket.rs` (`take_available`, `put_available`, `handle_basket_key`, `commit_transfer`)
- Modify: `crates/app-core/src/lib.rs` — `basket_rows`'s type
- Test: `crates/app-core/src/tests/` — beside the existing transfer tests

**Interfaces:**
- Consumes: Task 3's `rack_offer` and basket object.
- Produces: `App::basket_rows: Vec<TransferEntry>`, where
  `TransferEntry` is the sum of `views::TransferRow` (unchanged) and Task 3's
  `views::TransferCarrier`. `TransferRow` itself is **not** widened — its three
  figures are quantities and a carrier has none.

**Steps:**

- [ ] **Write the failing tests.** (a) A carrier row's amount clamps to `[-1, +1]`
  under Left, Right, and both modifier pairs — the Ctrl half-step must land on
  the end rather than stalling, which is what `half_way_to`'s `div_ceil` exists
  for and is worth pinning on a gap of one. (b) The pack-side ceiling is
  `MAX_DOWNED_PROGRAMS` minus what is already held minus the other rows' pending
  takes. (c) A digit key on a carrier row does not set an amount of 7.
  (d) `[A]` (take everything) fills carrier rows to `+1` and no further.
- [ ] **Run them.** Expect FAIL.
- [ ] **Widen the row list to the sum type** and route each key arm through it.
  Keep the item path byte-for-byte unchanged in behaviour; the existing transfer
  tests are the regression net and must stay green untouched.
- [ ] **Run** `cargo test -p feral-processes-app-core`. Expect PASS.
  Note: `tests::creation` is a known flake in this crate and is unrelated —
  re-run that module rather than debugging it inside this work.
- [ ] **Commit.**

---

### Task 5: Drawing the rows, and the width census

**Files:**
- Modify: `crates/gui/src/render/transfer.rs`
- Test: same file's `#[cfg(test)]` block

**Interfaces:**
- Consumes: Task 4's `TransferEntry`.

**Steps:**

- [ ] **Extend `no_transfer_row_overflows_its_popup` first**, before the rows are
  drawn, so it measures the longest *carrier* label as well as the longest item
  name. Popup row width is testable headlessly — `paint::with_painter` measures
  real text, so this is a real assertion and not an estimate.
- [ ] **Run it.** Expect FAIL if a carrier label overflows, PASS if it fits;
  either way it must be measuring carrier labels, so assert that by temporarily
  feeding it an absurd label and watching it fail.
- [ ] **Draw the carrier rows** through the existing three-column `line`, with
  `Columns::of` measuring them. The figures are the same `projected` call with
  the carrier's 1/0.
- [ ] **Run** `cargo test -p feral-processes-gui transfer`. Expect PASS.
- [ ] **Commit.**

---

### Task 6: The rig's standing tool

**Files:**
- Modify: `crates/engine/src/components.rs` — `Hopper::standing_tool`
- Modify: `crates/engine/src/game/extraction.rs:731` (`load_teardown_rig`)
- Modify: `crates/engine/src/save.rs` + `crates/engine/src/game/lifecycle.rs`
- Modify: `crates/engine/src/game/inspection.rs` — the rig's examine/inspect line
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Produces: `Hopper::standing_tool: Option<ToolId>`, `StructureSave::standing_tool`.

**Steps:**

- [ ] **Write the failing tests.** (a) A hand-load writes the tool used;
  a second hand-load with a different tool overwrites it. (b) It survives a
  save→load round trip. (c) `Routines` and `Gear` are still refused at the
  hand-load, so a standing tool is never one of them — assert on the field after
  a refused load, not just on the refusal. (d) The rig's inspect line names the
  tool, and says it has none when unset.
- [ ] **Run them.** Expect FAIL.
- [ ] **Implement.** The write goes at the end of `load_teardown_rig`, after its
  refusals, so a refused load sets nothing.
- [ ] **Run** `cargo test -p feral-processes-engine extraction`. Expect PASS.
- [ ] **Commit.**

---

### Task 7: The crew fetch

**Files:**
- Modify: `crates/engine/src/components.rs` — `CarryingProgram`
- Modify: `crates/engine/src/game/base/teardown.rs` — the fetch, inside `run_teardown_rigs`
- Modify: `crates/engine/src/game/base/mod.rs` (or wherever `schedule_base_labour` lives) — the never-free rule
- Modify: `crates/engine/src/game/base/upkeep.rs:738` (`damage_structure`) and `crates/engine/src/game/base/building.rs:961` (`remove_structure`)
- Modify: `crates/engine/src/save.rs` + `lifecycle.rs` — the in-transit carrier
- Test: `crates/engine/src/tests/` — beside the hauling tests

**Interfaces:**
- Consumes: Task 2's `Racked`/`rack_room`, Task 6's `standing_tool`.
- Produces: `components::CarryingProgram(pub DownedProgram)`.

**This is the task the spec flags as risky.** Both rules below fail silently —
nothing stops compiling, and the symptom is a lost kill with no error. Each gets
a test that **fails with the line deleted**; verify that by deleting it.

**Steps:**

- [ ] **Write the failing tests.** (a) A body posted at a rig with an empty
  hopper and a stocked rack in `hauling::crew_reach` walks and returns one
  carrier, and the hopper holds it. (b) One trip moves exactly one carrier.
  (c) A rack outside `crew_reach` produces no fetch and the body stays on its
  post. (d) A rig with no `standing_tool` does not fetch, and its
  `MachineStatus` says so — asserted on the transition, since
  `set_machine_status` only speaks on one. (e) `schedule_base_labour` does not
  free a body holding a `CarryingProgram`. (f) Destroying the rig by damage and
  (g) by removal each return the in-transit carrier to a rack with room, or to
  the player's store, rather than dropping it.
- [ ] **Run them.** Expect FAIL.
- [ ] **Implement the fetch in `run_teardown_rigs`.** It is a `&mut Game` pass
  because `extraction_yield`/`extraction_ticks` are `&Game` methods a bevy system
  cannot call — that is why the pass exists, and the fetch belongs in it so the
  fetch and the strip cannot disagree about the hopper's room. Both routes into
  the hopper (this and the hand-load) go through **one** capacity helper.
- [ ] **Extend the never-free rule** in `schedule_base_labour` to name
  `CarryingProgram` alongside `Carrying`.
- [ ] **Extend both destruction paths.** Return the carrier before dropping the
  component; destroying the building must not destroy the kill.
- [ ] **Save the in-transit carrier** on the worker's creature save, additive and
  defaulted.
- [ ] **Delete each of the two rules in turn and confirm its test fails.** Put
  what you saw in the commit message.
- [ ] **Run** `cargo test -p feral-processes-engine`. Expect PASS.
- [ ] **Commit.**

---

### Task 8: Documentation, the seam, and the full gate

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md` — the one-sentence rules
- Modify: `.claude/skills/seams/references/base.md` — the traps behind them
- Memory graph: `seam:` entries for the argument

**Steps:**

- [ ] **Run the full gate:** `cargo test --workspace`, then
  `cargo clippy --workspace` and `cargo fmt`. Fix warnings rather than silencing
  them. Report the actual counts seen.
- [ ] **Run `cargo test -p feral-processes-engine balance_sim`** and confirm the
  curves have not moved. Nothing here touches `tuning.rs`, so a moved curve means
  something unintended changed and is a stop-and-look.
- [ ] **Write the CHANGELOG section** under a new version heading; the bump digit
  is decided by `CHANGELOG.md`'s preamble, and no save format broke.
- [ ] **Write the three seam writes** in the order the `seams` skill documents:
  the argument to the memory graph, the trap to
  `.claude/skills/seams/references/base.md`, the one-sentence rule to
  `CLAUDE.md`. Two rules earn a line: *a downed program lives in three places
  now, and the rack is the only one that is not a queue*; and *a carrier in
  transit is carried, so both the never-free rule and both destruction paths name
  `CarryingProgram` beside `Carrying`*.
- [ ] **Do not touch** `docs/manual.md` or the root `README.md` — both are
  carved out of the documentation obligation.
- [ ] **Commit.**

---

## Notes for whoever executes this

- **A green suite is not evidence of play.** Nothing in this feature will have
  been seen on a screen when the tests pass. Say so plainly once, and leave the
  play question to the user.
- **Don't reach for a fresh `Game::new`** where a `dev-saves/` template would do;
  the `extraction` template already sets up carriers and a rig.
- **Single-crate and workspace test runs shift the RNG stream**, so a test that
  passes under `-p feral-processes-engine` and fails under `--workspace` is
  seed luck, not a bug in this feature — but check the code path before
  theorising, and never brute-force it with repeated runs.
