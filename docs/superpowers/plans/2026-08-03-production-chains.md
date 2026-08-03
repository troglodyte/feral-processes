# Adjacency-fed production chains — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> or `superpowers:executing-plans` to work through this task-by-task.

**Spec:** `docs/superpowers/specs/2026-08-03-production-chains-design.md` — read it first.
**Branch:** `factory-chains`.

**Goal:** Machines get local input/output stock and feed each other by touching, so a
production chain is a physical line across the base and layout becomes a decision.

**Architecture:** A `Stock` component on every deployed structure. Neighbours may take from
a machine's `output`; nothing outside a machine touches its `input`. A new `assembles`
field names an *item*, and the machine runs that item's existing `CraftableDef::cost` as
its recipe — there is no second recipe format. Extractors stop depositing into the player's
inventory and deposit into their own `output` instead, which is what makes clogging real.

**Per this repo's CLAUDE.md, this plan gives file lists, interfaces, test intent and gates
— not finished code.** Code blocks appear only where the constraint is genuinely
non-obvious (borrow scoping, determinism). Write the code yourself; if a task's shape turns
out to be wrong, say so rather than forcing it.

## Global constraints

- **TDD.** Failing test first, every task. `cargo test -p feral-processes-engine <name>`
  to iterate; the engine suite is ~3s, warm builds are ~1s. Don't reach for the workspace
  suite until a task boundary.
- **Gates before any task is called done:** `cargo fmt`, `cargo clippy --workspace` clean,
  `cargo test --workspace` green.
- **Commit per green task.** Already on `factory-chains`; do not push.
- **New schema fields are `#[serde(default)]`,** always — an existing mod's `.ron` must
  keep parsing untouched.
- **A malformed `.ron` is skipped with a logged warning, never a panic.** Follow
  `StructureDb::load_dir`.
- **Update the matching `assets/*/README.md` in the same change** as any schema change.
- **Determinism:** no `HashMap` iteration anywhere in this feature's tick path, and no
  draws from `resources::GameRng` in the pull or work phases. See Task 5.
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub const`, in a
  labelled section — never inline in a formula.

## File map

| File | Role |
|---|---|
| `crates/engine/src/components.rs` | `Stock`, `MachineStatus`; delete `PassiveProcessor` |
| `crates/engine/src/structures.rs` | `StructureDef::capacity`, `::assembles`, `AssembleDef`; delete `PassiveProcessDef` |
| `crates/engine/src/systems.rs` | `assembler_system`; extractor payout redirect; delete `passive_process_system` |
| `crates/engine/src/game/building.rs` | spawn `Stock`/`MachineStatus` on deploy |
| `crates/engine/src/game/collect.rs` | **new** — `Game::collect_adjacent` |
| `crates/engine/src/game/inspection.rs` | `StructureReport` gains stock + status |
| `crates/engine/src/game/lifecycle.rs` | save/restore stock (`:428`, `:627`) |
| `crates/engine/src/save.rs` | `StructureSave` fields; `SAVE_FORMAT_VERSION` 19 → 20 |
| `crates/engine/src/tuning.rs` | `DEFAULT_OUTPUT_CAPACITY`, `INPUT_STOCK_BATCHES` |
| `crates/app-core/src/app/playing.rs` | collect key |
| `crates/gui/src/render/base.rs` | stock + status in the structure report rows |
| `assets/items/*.ron`, `assets/structures/*.ron` | the content slice |

## Interfaces

Pin these names and types now; later tasks depend on them.

```rust
// components.rs
pub struct Stock {
    pub input: BTreeMap<ItemId, u32>,
    pub output: BTreeMap<ItemId, u32>,
    pub capacity: u32,   // output only; input is derived, see INPUT_STOCK_BATCHES
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]   // a Component in its own right
pub enum MachineStatus { Running, Starved, Clogged, Idle }

// structures.rs
pub struct AssembleDef { pub item: ItemId, pub ticks_per_unit: u32 }
// StructureDef::capacity: Option<u32>, StructureDef::assembles: Option<AssembleDef>

// game/collect.rs
impl Game { pub fn collect_adjacent(&mut self) -> Vec<(ItemId, u32)> }
```

`BTreeMap`, not `HashMap`. Two reasons and both bite: iteration order feeds the pull phase,
and a `HashMap` would make the save encoding differ run to run.

---

## Task 1: Delete `passive_process`

No shipped structure uses it and `assembles` supersedes it. Removing it first shrinks the
surface everything else is built on.

**Files:** `structures.rs` (`PassiveProcessDef`, `StructureDef::passive_process`),
`components.rs` (`PassiveProcessor`), `systems.rs` (`passive_process_system` ~`:399` and
its tests ~`:774`), `game/building.rs:99`, `assets/structures/README.md` (the section),
plus wherever the system is registered in the schedule.

**Steps:**
- [ ] Grep `passive_process|PassiveProcess` across the workspace; that list is the task.
- [ ] Delete all of it, including the README section and the `load_test_capacitor` fixture
      if it has no other user.
- [ ] Gates. A green workspace suite is the evidence nothing depended on it.
- [ ] Commit.

**Watch for:** a test elsewhere that builds a fixture structure declaring the field. Delete
the fixture, don't neuter the test.

---

## Task 2: `Stock` component and top-level `capacity`

Introduce the storage with no behaviour riding on it yet. Nothing produces into it or takes
from it; deploy just creates it empty.

**Files:** `components.rs`, `structures.rs`, `game/building.rs`, `tuning.rs`,
`assets/structures/README.md`.

**Produces:** `Stock`, `MachineStatus`, `StructureDef::capacity`,
`tuning::DEFAULT_OUTPUT_CAPACITY`, `tuning::INPUT_STOCK_BATCHES` (= 2).

**Why `capacity` is top-level and not `work.capacity`:** an assembler declares `assembles`
and no `work` block at all, and a storage building later declares neither — both still need
an output size.

**Test intent:**
- Deploying any structure gives it a `Stock`, empty, with `capacity` from the def.
- A structure def omitting `capacity` gets `DEFAULT_OUTPUT_CAPACITY`.
- A `.ron` with a malformed `capacity` is skipped with a warning, not a panic.

**Steps:** failing tests → verify they fail → implement → verify pass → README → gates →
commit.

---

## Task 3: Save and restore stock

Do this immediately after Task 2 and before anything fills a buffer, so no interim commit
can silently lose player state.

**Files:** `save.rs` (`StructureSave:165`, `SAVE_FORMAT_VERSION:271`),
`game/lifecycle.rs:428` (restore) and `:627` (capture).

**Changes:** `StructureSave` gains `stock_input: Vec<(ItemId, u32)>` and
`stock_output: Vec<(ItemId, u32)>`, both `#[serde(default)]`. Bump `SAVE_FORMAT_VERSION`
19 → 20 and document the reason at the constant, following the note already at `save.rs:85`.
Leave `resource_amount` alone for now — Task 6 removes it.

Vec-of-pairs on disk rather than the live `BTreeMap`, matching how `build_cost` is already
encoded, and rebuilt into the map on restore.

**Test intent:**
- A base with partially filled input and output buffers survives a dump/pack round trip.
- A save written without the fields (i.e. `default`) restores as empty stock rather than
  failing to parse.

**Gates:** as global, plus round-trip the real `savetool` on a `dev-saves/` template.

---

## Task 4: The collect action

Land this *before* extractors redirect, so the game is never left without a way to get
goods out. Against an empty stock it correctly collects nothing, which is testable now.

**Files:** new `crates/engine/src/game/collect.rs` (register in the `game` module),
`crates/app-core/src/app/playing.rs`, `crates/gui` help/hint text.

**Produces:** `Game::collect_adjacent(&mut self) -> Vec<(ItemId, u32)>` — empties the
`output` of every structure orthogonally adjacent to the player into their inventory and
returns what was taken, so the caller can log it.

**Key:** `C`. Verified free — `playing.rs` binds `c` but not `C`, and uppercase already
means a variant elsewhere (`B`, `F`, `L`, `T`, `U`, `W`).

**Rules:** orthogonal only, never diagonal. `output` only — a collect can no more reach a
machine's `input` than a neighbouring machine can. Respect `Inventory::add_capped`; what
doesn't fit stays in the buffer and says so in the log.

**Test intent:**
- Collects from all four orthogonal neighbours at once, and from none diagonally.
- Leaves `input` untouched.
- A full inventory leaves the remainder in the buffer rather than voiding it.
- Collecting with nothing adjacent is a no-op that doesn't consume a turn's worth of state.

---

## Task 5: Extractors deposit into their own stock

The spec's load-bearing risk. Fragments stop appearing in the player's pocket.

**Files:** `systems.rs` — `task_progress_system` (~`:216`) and `resolve_gather_cycle`.

**Change:** the payout goes to the worked structure's own `Stock::output`, not
`inventories.get_mut(tamed.owner)`. A cycle completing against a full output is **clogged**:
it does not consume the cycle, does not pay out, and logs once on entering that state.

The XP half of the cycle is unaffected — a worker still earns from a completed cycle. Keep
that seam intact; only the item destination moves.

**Test intent:**
- A worked Mining Node accumulates fragments in its own stock; the player's inventory does
  not change.
- A node at output capacity stops accumulating and logs `clogged` exactly once, not once
  per tick.
- Collecting from it un-clogs it and it resumes.
- Worker XP still accrues while clogged or not, per whichever the current behaviour is —
  read `task_progress_system` and preserve it rather than guessing.

---

## Task 6: Retire the deposit pool

`ResourceNode::amount`/`capacity` — the refilling reserve a node is "mined down" from —
is redundant pacing now that output stock paces a node.

**Files:** `components.rs` (`ResourceNode`), `systems.rs` (the `if node.amount == 0` refill
and `resolve_gather_cycle`), `game/building.rs:91` and `:186`, `save.rs` (drop
`resource_amount`), `game/lifecycle.rs:428`/`:630`, `assets/structures/README.md`.

Keep `ResourceNode::resource` and `::level` — `level` is the reliability roll and is still
live, and `WorkDef::level` still feeds it. Only the pool goes.

`SAVE_FORMAT_VERSION` is already 20 from Task 3; dropping a `#[serde(default)]` field needs
no second bump, but confirm a v20 save written before this task still loads.

**Test intent:** an existing test asserting refill behaviour should now fail — delete it
rather than adapting it, and say so in the commit. Node payout across tiers and zones
(`node_payout`, `systems.rs:116`) is unchanged; its tests must stay green untouched.

---

## Task 7: The `assembles` field

Schema only, plus the validation that stops a typo shipping silently.

**Files:** `structures.rs`, `assets/structures/README.md`, the shipped-assets test.

**Produces:** `AssembleDef { item, ticks_per_unit }`, `StructureDef::assembles`.

**Test intent:**
- A structure declaring `assembles` resolves its recipe through the named item's
  `CraftableDef::cost` (`crates/engine/src/items_db.rs:59`) — assert the resolved
  ingredient list, so the "no second recipe format" property is pinned by a test.
- **Shipped-assets test:** every `assembles` in `assets/structures/` names an item that
  actually declares `craftable`. Without this a typo'd mod builds a machine that can never
  run and says nothing.
- An `assembles` naming an unknown item is skipped with a warning at load, not a panic.

---

## Task 8: The pull phase

**Files:** `systems.rs` — new `assembler_system`.

Each tick, for each machine with `assembles`, for each ingredient the recipe needs and the
input lacks, take from the `output` of the four orthogonally adjacent structures, capped at
`INPUT_STOCK_BATCHES × the recipe amount` per ingredient.

**Two non-obvious constraints — get both right or this is subtly broken:**

1. **Deterministic order.** Bevy's query iteration order is not stable. Two machines
   competing for one feeder's scarce output would resolve differently between runs — a
   flaky-test source *and* a base that behaves differently after a reload. Collect the
   machines into a `Vec`, sort by `(Position.x, Position.y)`, then process.

2. **Two passes, because you cannot hold two mutable borrows out of one query.** Reading a
   neighbour's `output` while writing your own `input` is the same `Query<&mut Stock>`.
   Plan the transfers, then apply them:

   ```rust
   // pass 1: immutable read, produces a plan
   let mut transfers: Vec<(Entity, Entity, ItemId, u32)> = Vec::new();  // from, to, what, how many
   // pass 2: apply, one entity at a time
   ```

**Test intent:**
- A diagonal neighbour feeds nothing.
- An orthogonal neighbour feeds.
- Input stops at two batches and leaves the rest in the feeder — a greedy machine cannot
  drain a shared feeder dry.
- **Pull order is stable:** two machines competing for one scarce feeder resolve the same
  way across repeated runs. Assert the specific winner by position, not just that *someone*
  won — an order-independent assertion would pass under the bug this test exists to catch.
- A machine adjacent to nothing pulls nothing and does not panic.

---

## Task 9: The work phase and the three stall states

**Files:** `systems.rs` (`assembler_system`), `components.rs` (`MachineStatus`).

If the input covers a full batch **and** the output has room **and** a program is assigned,
advance progress; on completion spend the batch and add one unit of the assembled item to
output.

Every machine needs an assigned program, assemblers included — reuse the existing cronjob
assignment (`TaskKind::GatherResource`, `Task::target`) rather than inventing a second
assignment concept.

Status resolves each tick to exactly one of `Idle` (no program) → `Starved` (input short) →
`Clogged` (output full) → `Running`, checked in that order. **Log only on transition**, to
`MessageSource::Base` via `log_base` — a stalled base must never flood the pane.

`MachineStatus` is deliberately **not** saved. It initialises to `Running` at spawn and on
load, so a base that loads starved announces it once. That is information the player wants,
and it costs no save field.

**Test intent:**
- A machine with no program advances nothing and reports `Idle`.
- A starved machine does not advance progress.
- A clogged machine does not consume its input — assert the input is *still there*, since
  consuming-then-discarding is the plausible wrong implementation.
- A full three-stage chain produces the terminal item over N ticks.
- A machine stalled for 20 ticks logs once, not 20 times.

---

## Task 10: Surfacing stock and status

The player cannot play this without seeing buffers.

**Files:** `game/inspection.rs` (`structure_report:190`), `crates/gui/src/render/base.rs`.

`StructureReport` gains the structure's stock and its `MachineStatus`.

**Per CLAUDE.md the row count is owned by app-core and drawn by gui**, so any per-row
transform lives in the engine — fold the stock into report rows in `structure_report`, not
in the renderer, or the screen opens on a row that isn't drawn.

**Test intent:** a report row carries stock contents and status; existing
`structure_report` tests in `crates/engine/src/tests/inspection.rs` (`:717`, `:756`,
`:782`, `:812`) stay green.

---

## Task 11: The content slice

One chain end to end. **Numbers are set here, against the balance gate — not guessed.**

**New items** (`assets/items/`): `bytecode_block`, `charge_coil`, `patch_routine`.
`patch_routine`'s `craftable.cost` is `bytecode_block` + `charge_coil` — the first
multi-input recipe in the game.

**New structures** (`assets/structures/`): Refinery (`assembles: bytecode_block`), Winding
Node (`assembles: charge_coil`), Assembly Bay (`assembles: patch_routine`).

**Existing single-input recipes are left alone.** The terminal product is a new item, so
this lands beside the current economy rather than retuning it.

**Size against the roster cap, not against fragment cost.** The pet cap is
`3 + pet_slot_bonus` and every machine needs a program, so:
- Mining Node → Refinery is 2 programs, affordable at a starting roster of 3, and must be
  worth building on its own — give `bytecode_block` standalone trade value.
- The full five-machine chain needs a Data Cache (+2) standing before it can run at all.

**Gate:** `cargo test -p feral-processes-engine balance_sim`. A moved curve is the signal,
not a broken test — if one moves, stop and report it rather than editing the expectation.

**Also:** capture a `dev-saves/` template of a mid-chain base and document it in
`dev-saves/README.md`, so the next session testing this doesn't start with an hour of play.

---

## Task 12: Documentation sweep

- `assets/structures/README.md` — `assembles`, top-level `capacity`, stock and the three
  stall states; `passive_process` and the deposit pool gone.
- `assets/items/README.md` — note that `craftable.cost` is what an assembler runs, so a
  multi-input recipe is automatable by construction.
- Root `README.md` and `CHANGELOG.md` — per the standing rule these are part of the doc
  obligation, not an afterthought. Grep for claims this change falsifies, especially any
  describing cronjob output arriving in your inventory.
- `CLAUDE.md` — add the load-bearing seams this creates: the `output`-only pull asymmetry,
  the `(x, y)` sort, and `BTreeMap`-not-`HashMap`. Then `cp CLAUDE.md AGENTS.md` — they are
  gitignored twins with no tracking to catch drift.
- `TODO.md` — tick "implement factory like interactions at the base".

**Final gate:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

---

## After the plan

A green suite is not evidence this is fun, and the extractor change in Task 5 is the piece
most likely to read as a chore rather than a rhythm. Play it before treating it as done —
`cargo run -- --template <the Task 11 template>`.
