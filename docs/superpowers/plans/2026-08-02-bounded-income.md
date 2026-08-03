# Bounded Income Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rest costs a craftable consumable and scan is deleted, so a maintained base is the only sustainable income.

**Architecture:** Three independent edits sharing one intent. A new `cost` field on `RestDef` prices rest from asset data; `Game::forage` and its perk hook are deleted; `Perk::KeenScavenger` is redirected onto the mining roll so its save index need not move.

**Spec:** `docs/superpowers/specs/2026-08-02-bounded-income-design.md` — read it first. It carries the arithmetic behind every number here.

**Tech stack:** Rust, `bevy_ecs` 0.19 (engine is standalone `bevy_ecs`, not full Bevy), RON assets, `serde`.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** Nothing here is persisted. If a task
  seems to need one, stop and re-read the spec — the perk index not moving is
  the whole reason step 4 repurposes rather than deletes.
- **New schema fields are `#[serde(default)]`**, always, so existing and
  modded `.ron` files keep parsing untouched (`CLAUDE.md`, Moddability).
- **Never hardcode a new item id in Rust.** The outlet is reached through
  asset data only. `ids` in `crates/engine/src/items.rs` is for test setup.
- **Difficulty magnitudes live in `crates/engine/src/tuning.rs`** as documented
  `pub const`, never inline in a formula.
- **TDD:** failing test first, then the minimal implementation, then the test
  passing. Commit per green step.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix
  warnings rather than silencing them.
- Engine fixtures live in `crates/engine/src/tests/support.rs` — look there
  before writing a new one.

---

### Task 1: The Power Outlet item and the `RestDef::cost` field

Deliverable: the outlet exists, crafts, and Home declares its rest price.
Nothing consumes it yet — that is Task 2, and this task's tests must not
assume it does.

**Files:**
- Create: `assets/items/outlet.ron`
- Modify: `crates/engine/src/structures.rs` (`RestDef`, ~line 90)
- Modify: `assets/structures/home.ron`
- Modify: `assets/structures/README.md`, `assets/items/README.md`
- Test: `crates/engine/src/tests/` — the module covering structure defs

**Interfaces produced:**
- `RestDef { radius: i32, cost: Vec<(ItemId, u32)> }`, `cost` defaulting to
  empty. Empty means a free rest, which is today's behaviour and what an
  unmodified mod file gets.

**Item shape:** id `outlet`, name `Power Outlet`, `craftable: Some((cost:
[("core_fragment", 5)]))`. No `requires_structure` — it crafts anywhere, like
`power_cell`. No `consume` block: it is spent by resting, not by `use_item`.

- [ ] **Step 1: Write the failing tests.** Two: a `RestDef` deserialised from
  RON *without* a `cost` field yields an empty cost (the mod-compatibility
  guarantee), and `Game::craft_recipes()` contains a recipe producing the
  outlet at 5 core fragments. Note `craft_cost` applies a `LeanCompiler`
  discount, so assert against a player with no perks.
- [ ] **Step 2: Run them and confirm they fail** — the field and the file do
  not exist yet.
- [ ] **Step 3: Add the field and the asset.** Both READMEs document `cost` in
  the same change; the schema docs are the reference for anyone modding.
- [ ] **Step 4: Run the tests and confirm they pass.**
- [ ] **Step 5: `cargo test -p feral-processes-engine`, then commit.**

---

### Task 2: Rest consumes an outlet

**Files:**
- Modify: `crates/engine/src/game/turn.rs` (`Game::rest`, ~line 404)
- Modify: `crates/engine/src/game/lifecycle.rs` (starting `Inventory`, ~line 83)
- Test: `crates/engine/src/tests/turn.rs`

**Interfaces consumed:** `RestDef::cost` from Task 1.
**Interfaces produced:** none — `Game::rest`'s signature is unchanged.

**The ordering is the feature.** The outlet is looked up from the rest
structure's own def (`nearby_rest_structure` already returns that entity, so
the cost comes from whichever structure granted the rest, not from Home by
name), checked and spent **after** every existing gate — `is_game_over`,
`has_active_battle`, `require_surface`, `nearby_rest_structure` — and
**before** the `REST_TICKS` loop. `Inventory::take` returns how much it
actually removed; a partial take must not be treated as success.

Starting inventory gains 2 outlets beside the existing 3 ICE Breakers, 3
Power Cells and 5 Core Fragments.

- [ ] **Step 1: Write the failing tests.** Intent of each:
  - Rest with no outlet is refused, logs why, and does **not** advance the
    clock (assert against a tick-sensitive observable, not just the log).
  - Rest with one outlet succeeds and leaves zero — exactly one is spent, not
    the whole stack.
  - A rest refused by each earlier gate consumes nothing. One test per gate;
    this is where a regression silently taxes the player.
  - A rest structure whose def carries no `cost` still rests free.
  - A new game starts with 2 outlets.
- [ ] **Step 2: Run them and confirm they fail.**
- [ ] **Step 3: Implement.** Watch the borrow: the def lookup needs
  `StructureDb` while the write needs the player's `Inventory` — resolve the
  cost to an owned `Vec` before touching the inventory, the way
  `nearby_rest_structure` already collects before querying the db.
- [ ] **Step 4: Run the tests and confirm they pass.**
- [ ] **Step 5: `cargo test -p feral-processes-engine`, then commit.**

---

### Task 3: Delete scan

**Files:**
- Modify: `crates/engine/src/game/turn.rs` — delete `Game::forage` (~line 498)
  and `forage_chance` (~line 536)
- Modify: `crates/engine/src/tuning.rs:774-776` — delete the three
  `FORAGE_CHANCE_*` constants (the unwalkable arm was a bare `0.0`, not a
  constant)
- Modify: `crates/app-core/src/app/playing.rs:182` — the surface `g` arm
- Modify: `crates/engine/src/tests/turn.rs` — delete the forage tests
- Modify: `crates/app-core/src/tests/stack.rs:247-250` —
  `g_still_forages_on_the_surface`

**Leave alone:** `g` in the Stack is the frame map and is a different code
path. `crates/app-core/src/tests/stack.rs`'s surrounding tests assert that
separation and must still pass.

**The crate still compiles after this task.** `KeenScavenger` appears in
`perks.rs` only as a variant and a doc comment; its one effect wiring is the
`turn.rs:513` call this task deletes along with `forage`. The perk is left
inert until Task 4 gives it a new job — that is expected, not a broken build.
Also update the module doc at `perks.rs:18`, which cites `forage_chance` as
its example of a perk hook.

- [ ] **Step 1: Change the failing test first.** The `g` arm currently returns
  `true` (it acted); deleting it drops `g` to the `_ => false` arm, so on the
  surface the key becomes a no-op. Rewrite `g_still_forages_on_the_surface`
  accordingly — renamed — and run it to watch it fail.
- [ ] **Step 2: Delete the production code and the obsolete tests.**
- [ ] **Step 3: Verify nothing dangles:** `rg -i 'forage'` across `crates/`
  and `assets/` returns only the Keen Scavenger references Task 4 removes.
- [ ] **Step 4: `cargo test --workspace`** (this task crosses crates, so the
  narrow gate is not enough), then commit.

---

### Task 4: Repurpose Keen Scavenger onto the mining roll

**Files:**
- Modify: `crates/engine/src/systems.rs` — `mining_success_chance` (line 111)
  and its call site (line 132)
- Modify: `crates/engine/src/balance_sim.rs:206` — the shared call
- Modify: `crates/engine/src/tuning.rs:928` — re-document
  `KEEN_SCAVENGER_BONUS_PER_LEVEL` as a mining constant
- Modify: `crates/engine/src/perks.rs:37` — the doc comment on the variant
- Modify: `assets/perks/keen_scavenger.ron` — name stays, description rewritten
- Test: `crates/engine/src/tests/perks.rs`, `crates/engine/src/systems.rs`
  (the existing `mining_success_chance_rises_with_level_and_caps_at_one`)

**Interfaces produced:**
`mining_success_chance(level: u32, keen_scavenger_level: u32) -> f64`, still
capped at 1.0.

**Two traps, both real:**

1. **`balance_sim.rs` calls the real function** rather than copying it — that
   is deliberate and load-bearing (`CLAUDE.md`, Code principles). Update the
   call to pass `0`: the sweep models a mid-grade party with no perks, and
   saying so in a comment there is part of this task. Curves must not move.
2. **The perk is the player's; the roll runs per worker.** The call site at
   `systems.rs:132` sits in a system iterating worker programs.
   `task_progress_system` already solves this for `XpBoost` — read once from a
   player query *outside* the loop, because it cannot vary per worker. Follow
   that, and check whether `player_gather_system` shares the same helper; if
   it does, it needs the same value threaded, not a second lookup.

- [ ] **Step 1: Write the failing test.** A level-1 node with Keen Scavenger
  at level N rolls better than the same node at level 0, by
  `KEEN_SCAVENGER_BONUS_PER_LEVEL * N`, and the result still caps at 1.0.
- [ ] **Step 2: Run it and confirm it fails.**
- [ ] **Step 3: Implement**, including the `.ron` description and the tuning
  doc comment. The `.ron` must no longer say "Scanning (g)".
- [ ] **Step 4: Run the tests.** Then
  `cargo test -p feral-processes-engine balance_sim` specifically — **a moved
  curve here is a bug in this task**, not a signal, because passing 0 should
  reproduce the old number exactly.
- [ ] **Step 5: Commit.**

---

### Task 5: Docs and the full-suite gate

**Files:**
- Modify: `docs/manual.md` — the `g` key, and how rest is paid for
- Modify: `README.md` — grep it for claims this change falsifies before editing
- Modify: `CHANGELOG.md` — an `## Unreleased` entry
- Modify: `dev-saves/README.md` if any template's described state is now wrong

The CHANGELOG entry states plainly that this is **engine-only with no
save-format bump**, and says why the base is now the farm — the arithmetic
table in the spec is the source for it.

- [ ] **Step 1: `rg -i 'scan|forage|scaveng'` across `docs/`, `README.md` and
  `assets/*/README.md`** and fix every hit that is now false.
- [ ] **Step 2: Write the CHANGELOG entry.**
- [ ] **Step 3: `cargo test --workspace` and `cargo clippy --workspace`** —
  both clean. This is the gate; passing only the tests written here is not
  evidence of correctness.
- [ ] **Step 4: Commit.**

---

## After the plan

**This is unplayed balance.** Five fragments per rest and two starting outlets
are arithmetic, not evidence — the same footing the Trace bands were on before
playing moved them 40/100/180 → 25/70/140. A green suite is not evidence the
opening is playable. Capture or reuse a `dev-saves/` template and actually
play the first twenty minutes before treating any number here as settled; the
starting outlet count is the softener most likely to be wrong.
