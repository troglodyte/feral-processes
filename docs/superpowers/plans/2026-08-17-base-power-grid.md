# Base power grid implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A base's machines run only while its power supply covers their
draw; Home carries a baseline, each Recharger Node adds more, and a short
base cuts machines in `(x, y)` order.

**Architecture:** One pure function derives the whole ledger from the world's
structures. A new system runs it first each tick and parks the result in a
resource; `idle_machine_system` is the single writer of the new
`MachineStatus::Unpowered`, and the two production systems get a guard
apiece. Nothing is stored and nothing is saved.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), RON assets, `bevy` +
`bevy_egui` (gui).

**Spec:** `docs/superpowers/specs/2026-08-17-base-power-grid-design.md` — read
it alongside this plan. Every design argument lives there; this file is the
order of operations.

## Deviation from the writing-plans skill, on purpose

`CLAUDE.md`'s **Process weight** section governs here and overrides the
skill's "code blocks required for code steps":

> A subagent that has the repo and this file needs the file list, the
> interface it must produce, the intent of each test, and the gates to run —
> not finished code it will merely re-emit. Spelling the code out pays for it
> twice and leaves the subagent no room to notice the plan is wrong. Reserve
> code blocks for the genuinely non-obvious.

So tasks below give **exact paths, exact signatures, exact test names and
exact intent**, and show code only where the shape is not derivable from the
surrounding file — the cut loop, and the two log/format strings that are
player-facing copy. Implementers read the neighbouring code for style.

## Global constraints

Copied from the spec; every task's requirements implicitly include these.

- **No `SAVE_FORMAT_VERSION` bump.** Both new `StructureDef` fields are
  asset schema behind `#[serde(default)]`; `resources::PowerGrid` is not
  saved; `MachineStatus` gains a variant and the save is field-named RON.
  If a task appears to need a bump, stop — the design is wrong, not the
  version.
- **Player-facing name is "Grid", never "Power".** `Power` already means a
  creature's `PowerReserve` in the status column. Code names stay
  `power_draw` / `power_supply` / `Unpowered`; UI copy and log lines say
  Grid.
- **Never re-derive "is this a machine".** `StructureDef::runs_a_job()`
  (`structures.rs:378`) is the one predicate — `work.is_some() ||
  assembles.is_some()`. Its doc comment records this exact rule drifting
  once already, at deploy. The ledger is its fourth agreer.
- **Never re-derive "is this machine dark".** `game::base::power::ledger` is
  the one expression of the rule, per `CLAUDE.md`'s "a doc comment claiming
  to mirror other code must be a call, not a copy".
- **No `GameRng` draws anywhere in this feature.** The ledger is pure.
- **Moddability:** every new asset field is `#[serde(default)]`, and
  `assets/structures/README.md` is updated in the same change as the schema.
- **Gates:** `cargo fmt` and `cargo clippy --workspace` after every change;
  `cargo test --workspace` before any task is called done. Iterate with
  `cargo test -p feral-processes-engine <name>`.
- **Baseline:** `main` at `ac2a165` is green at **2539 tests**. Any failure
  outside the files you touched is a signal, not noise — see `CLAUDE.md` on
  a new `Resource` shifting bevy's query iteration order.

---

### Task 1: The schema — two fields on `StructureDef`

**Files:**
- Modify: `crates/engine/src/structures.rs` (the `StructureDef` struct, near
  `power_regen` at ~line 224)
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `StructureDef::power_draw: u32` and
  `StructureDef::power_supply: u32`, both `#[serde(default)]`.

**Why two fields and not one block:** `power_supply` is deliberately *not* a
member of `PowerRegenDef`. One structure (the Recharger) is about to carry
both with different values, and folding them would force a mod granting grid
supply to grant party regen with it.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/tests/assets.rs`, add
`a_structure_file_without_the_power_fields_still_parses`. Load a minimal
`.ron` written to a temp dir with no `power_draw` or `power_supply` and
assert both read `0`. Follow the temp-dir fixture style already used by
`game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role` in
the same file — do not invent a new one. This is the moddability guarantee,
so it must be a real parse of a real file, not a `Default::default()`.

- [ ] **Step 2: Run it and watch it fail**

`cargo test -p feral-processes-engine a_structure_file_without_the_power_fields`
Expected: fails to compile — no field `power_draw`.

- [ ] **Step 3: Add the two fields**

Both `#[serde(default)]`, doc-commented in the house style of the fields
around them: say what the field is *for* and why it exists separately, not
what its type is. Name `game::base::power` as where they are summed (it does
not exist yet — that is fine, Task 2 creates it).

- [ ] **Step 4: Run it and watch it pass**

`cargo test -p feral-processes-engine a_structure_file_without_the_power_fields`

- [ ] **Step 5: Document the schema**

`assets/structures/README.md` gains both fields, in the same voice and the
same section shape as `power_regen`'s entry. State plainly that this is the
base grid and **not** the same resource as `power_regen`, because that is the
exact confusion a modder will otherwise ship.

- [ ] **Step 6: Gates and commit**

`cargo fmt && cargo clippy --workspace && cargo test --workspace`
Commit: `feat(structures): power_draw and power_supply on StructureDef`

---

### Task 2: The ledger

**Files:**
- Create: `crates/engine/src/game/base/power.rs`
- Modify: `crates/engine/src/game/base/mod.rs` (add `pub(crate) mod power;`
  to the list at lines 16-20, alphabetically before `upkeep`)
- Create: `crates/engine/src/tests/power.rs`
- Modify: `crates/engine/src/tests/mod.rs` (register the new test module)

**Interfaces:**
- Consumes: `StructureDef::power_draw` / `power_supply` (Task 1),
  `StructureDef::runs_a_job()`.
- Produces:

```rust
pub(crate) struct PowerLedger {
    pub supply: u32,
    pub draw: u32,
    pub dark: HashSet<Entity>,
}

pub(crate) fn ledger(world: &World, db: &StructureDb) -> PowerLedger
```

`dark` holds `Entity` and so the type stays `pub(crate)` — Task 5 exposes
only `(draw, supply)` to the renderer, and the per-machine half travels as a
`MachineStatus`.

**The rule, in full.** Sum `power_supply` over every deployed `Structure`.
Sum `power_draw` over every deployed `Structure` whose def `runs_a_job()`.
Then:

```rust
// Machines sorted by (x, y): bevy's query iteration order is not stable,
// so two machines competing for the last unit of supply would resolve
// differently between runs. Same reason `assembler_system` sorts.
let mut budget = supply;
for (entity, draw) in machines_sorted_by_position {
    if budget >= draw {
        budget -= draw;          // runs
    } else {
        dark.insert(entity);     // cannot fit
    }
}
```

Note the loop does **not** `break` at the first machine that will not fit: a
3-draw machine that cannot fit a 2-unit budget goes dark while a 1-draw
machine behind it still runs. Breaking would darken an arbitrary tail.

A structure whose def is missing from the `StructureDb` contributes nothing
and is never dark — follow the `unwrap_or` shape the neighbouring systems
already use for that case rather than panicking.

- [ ] **Step 1: Write the failing tests**

All in `crates/engine/src/tests/power.rs`. Use `tests::support`'s existing
fixtures — read `support.rs` before writing a new one, per `CLAUDE.md`.
`spawn_recharger_node` (support.rs:1325) is there; a machine fixture needs
`work_node_parts()`, whose omission reads as a payout curve that moved.

Six tests, by intent:

1. `supply_sums_across_home_and_every_recharger` — a Home and two Rechargers
   report the authored total.
2. `draw_sums_only_over_structures_that_run_a_job` — stand a Depot and a
   Shield beside two machines; draw counts the machines alone.
3. `a_base_exactly_at_capacity_has_nothing_dark` — the boundary is `<=`.
   Write it so it fails against a `<` implementation.
4. `the_cut_order_is_by_position_not_spawn_order` — **spawn the competitors
   in the opposite order to their positions**, the way `assembler_system`'s
   sort test does. Without that inversion the test passes against an
   unsorted implementation and proves nothing.
5. `a_machine_too_big_for_the_budget_does_not_darken_the_one_behind_it` —
   the no-`break` rule.
6. `an_unstaffed_machine_still_draws` — the spec's staffing decision, pinned
   so nobody "fixes" it into a staffing-dependent ledger later.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine tests::power`
Expected: fails to compile — no module `power`.

- [ ] **Step 3: Implement `ledger`**

Module doc comment states the two-callers rule and why `dark` is a set of
`Entity` rather than a per-machine flag.

- [ ] **Step 4: Run them and watch them pass**

`cargo test -p feral-processes-engine tests::power`

- [ ] **Step 5: Gates and commit**

`cargo fmt && cargo clippy --workspace && cargo test --workspace`
Commit: `feat(base): the power ledger`

---

### Task 3: `Unpowered`, the system, and the two guards

**Files:**
- Modify: `crates/engine/src/components.rs` (`MachineStatus`, ~line 636)
- Modify: `crates/engine/src/resources.rs` (new `PowerGrid`, beside
  `RunFeats` at ~line 421)
- Modify: `crates/engine/src/systems.rs` — `set_machine_status` (:355),
  `idle_machine_system` (:395), `task_progress_system` (:483),
  `assembler_system`; new `power_grid_system`
- Modify: `crates/engine/src/game/lifecycle.rs` — `build_schedule` (:202)
  and **both** `insert_resource` sites (:133 new-game, :333 load), the way
  `RunFeats` is inserted at exactly those two
- Test: `crates/engine/src/tests/power.rs`

**Interfaces:**
- Consumes: `power::ledger` (Task 2).
- Produces: `MachineStatus::Unpowered`; `resources::PowerGrid { supply: u32,
  draw: u32, dark: HashSet<Entity> }` with a `fn is_dark(&self, e: Entity)
  -> bool`; `systems::power_grid_system`.

**Ordering — this is the load-bearing part of the task.** The new system runs
**first** in the existing chained base group:

```
(
    systems::power_grid_system,     // new — computes the ledger
    systems::idle_machine_system,   // writes Unpowered, else Idle
    systems::task_progress_system,  // guard: skip a dark machine
    systems::player_gather_system,
    systems::assembler_system,      // guard: skip a dark machine
    hauling::haul_step_system,
).chain()
```

The refused alternative — a power system running *last* and overwriting what
the others decided — makes `set_machine_status` log a transition **twice per
tick forever** while a base is short (`task_progress_system` sets `Running`,
the power system sets `Unpowered`). Running the ledger first is what keeps
the log quiet.

**One writer, two guards.** `idle_machine_system` already makes one pass over
every `Structure` and already runs first, so it is where the precedence call
lives: dark wins, else the existing unworked→`Idle` rule. `task_progress_system`
and `assembler_system` each get a one-line `continue` and write no status.

**Precedence:** `Unpowered` outranks all five existing variants. Nothing the
player can do — posting a program, clearing a clog, feeding an input,
building a depot, clearing a route — makes a dark machine run.

`PowerGrid` is a per-tick derived cache and is **not saved**; give it the
same "not saved, and here is why that is sound" doc comment shape `RunFeats`
carries.

The one piece of player-facing copy, added to `set_machine_status`'s match:

```rust
MachineStatus::Unpowered => format!("The {name} is dark — the grid can't power it."),
```

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/power.rs`. Five, by intent:

1. `a_dark_machine_makes_no_progress_on_a_cronjob` — through
   `task_progress_system`.
2. `a_dark_assembler_makes_no_progress` — through `assembler_system`. Two
   tests because they are two separate guards; one test would leave a guard
   unproven.
3. `a_dark_and_unstaffed_machine_reports_unpowered_not_idle` — the
   precedence call. Must fail against an implementation that writes `Idle`
   second.
4. `going_dark_and_coming_back_each_log_exactly_once` — tick several times in
   each state and count matching lines. This is the regression guarding the
   refused last-in-chain ordering.
5. `power_regen_still_refills_the_party_on_a_dark_base` — the two Powers do
   not touch. `power_regen_system` is unchanged and must stay so.

> **The vacuous-test trap, called out because this one is easy to fall
> into.** A machine with nobody posted makes no progress *anyway*, so tests
> 1 and 2 pass with the entire feature deleted unless the fixture has a
> **posted worker on a machine that would otherwise be running**. Per
> `CLAUDE.md`: delete the guard, watch the test fail, put it back. Two tests
> written this way in this repo on 2026-08-09 were vacuous and read as
> coverage.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine tests::power`

- [ ] **Step 3: Add the variant, the resource and the system**

Variant, `PowerGrid`, `power_grid_system`, both `insert_resource` sites, the
schedule entry, the `idle_machine_system` precedence and the two guards.

The variant addition will produce non-exhaustive-match errors — those are the
compiler listing your remaining call sites. `render/base.rs::machine_color`
and `render/building.rs`'s status table are Task 6; give them a placeholder
arm only if the workspace will not otherwise build, and finish them there.

- [ ] **Step 4: Run them and watch them pass**

`cargo test -p feral-processes-engine tests::power`

- [ ] **Step 5: Run the full suite and read the failures carefully**

`cargo test --workspace`

Expect breakage beyond your own tests, and **do not assume it is yours**.
Registering a new `Resource` shifts bevy's query iteration order, and this
repo has latent tests that depend on an unsorted query. A failure in an
untouched subsystem right after a resource registration is that, not a
regression — see `CLAUDE.md`. Fix the fixture's incidental coupling; do not
re-seed to make it pass.

- [ ] **Step 6: Gates and commit**

`cargo fmt && cargo clippy --workspace`
Commit: `feat(base): machines stall when the grid can't power them`

---

### Task 4: The numbers, the census, and the three templates

**One task and one commit on purpose.** Authoring the numbers immediately
breaks `dev_template.rs`'s
`the_chains_template_starts_with_a_chain_that_actually_runs`, so splitting
this would leave the suite red between two commits.

**Files:**
- Modify: 17 files under `assets/structures/` (2 suppliers, 15 machines)
- Modify: `dev-saves/chains.ron`, `dev-saves/contracts.ron`,
  `dev-saves/deep-lair.ron`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: Task 1's fields, Task 3's behaviour.
- Produces: the authored numbers every later task's UI and docs describe.

**The numbers:**

| Structure | field | value |
| --- | --- | --- |
| `home` | `power_supply` | 4 |
| `recharger_node` | `power_supply` | 4 |
| `mining_node`, `log_scraper`, `research_node`, `power_conduit` | `power_draw` | 1 |
| `lathe`, `transcriber`, `winding_node`, `refinery`, `disk_press`, `annealing_node` | `power_draw` | 2 |
| `fabricator`, `compiler`, `armory`, `assembly_bay`, `refactor_bench` | `power_draw` | 3 |

That is 15 machines and 2 suppliers; the other ten shipped structures get
neither field. **These numbers are unmeasured** — `balance_sim` has no
base-production term and gates none of them. They are a starting point for a
session, not a claim, and the plan says so because the next reader will
otherwise treat them as tuned.

**The templates.** Each of the three already stands one Recharger, so supply
is 8 (Home 4 + Recharger 4). Add plain Recharger entries to the checked-in
RON — a template *is* a save file, so no re-capture and no play is needed.

| template | draw | supply now | add | supply after |
| --- | --- | --- | --- | --- |
| `chains` | 15 | 8 | 2 Rechargers | 16 |
| `contracts` | 15 | 8 | 2 Rechargers | 16 |
| `deep-lair` | 17 | 8 | 3 Rechargers | 20 |

`extraction`, `rarity-preview` and `stack` draw 6 against 8 and need nothing.

Copy the shape of the existing `recharger_node` entry in each file, and pick
tiles that are **free and on the slab** — read the file's other structure
positions first rather than guessing, since two structures on one tile is a
corruption the loader will not catch for you.

- [ ] **Step 1: Write the failing census tests**

In `crates/engine/src/tests/assets.rs`, over the **real** shipped assets.
Three, by intent:

1. `every_shipped_machine_declares_a_power_draw` — iterate `StructureDb`,
   filter on `runs_a_job()`, assert `power_draw > 0`. This is what stops the
   sixteenth machine shipping free.
2. `home_alone_powers_a_new_bases_opening_extractors` — Home's
   `power_supply` is at least the summed draw of a Mining Node, a Log
   Scraper and a Research Node. Stated as a concrete sum because "covers the
   opening" is not something a test can check.
3. `no_shipped_structure_both_draws_and_supplies` — a building that does
   both is incoherent, it is a one-line check, and it is cheap enough to
   enforce rather than report.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine tests::assets`
Expected: every machine reports a draw of 0.

- [ ] **Step 3: Author the 17 asset files**

- [ ] **Step 4: Run the engine suite**

`cargo test -p feral-processes-engine`

- [ ] **Step 5: Run the launcher suite and watch the template gate fail**

`cargo test -p feral-processes`
Expected: `the_chains_template_starts_with_a_chain_that_actually_runs` fails
— the chain has gone dark. That failure is the mechanic working.

- [ ] **Step 6: Add a template census test**

`every_checked_in_templates_base_can_power_itself`, beside
`every_checked_in_template_still_loads` in
`crates/launcher/src/dev_template.rs`. Load each template and assert supply
covers draw. This is what stops the next template being captured short.

- [ ] **Step 7: Repair the three templates, then run both suites**

`cargo test -p feral-processes` then `cargo test --workspace`

- [ ] **Step 8: Gates and commit**

`cargo fmt && cargo clippy --workspace`
Commit: `feat(structures): author the grid's draws and supplies`

---

### Task 5: The engine's view surface

**Files:**
- Modify: `crates/engine/src/game/base/power.rs` or `crates/engine/src/views.rs`
  — place `Game::base_power` with the other base view methods, following
  whichever file already holds `Game::structure_report`
- Test: `crates/engine/src/tests/power.rs`

**Interfaces:**
- Consumes: `power::ledger` (Task 2).
- Produces: `pub fn base_power(&self) -> (u32, u32)`, returning
  `(draw, supply)`.

**It calls `ledger` directly rather than reading `PowerGrid`.** That is the
point: the base roster is then correct on the first frame after a load,
before any tick has run and while the resource is still `Default`. It also
keeps `PowerLedger` and its `Entity` set out of the renderer. Iterating
fifteen structures per frame is not a cost worth optimising ahead of
evidence.

- [ ] **Step 1: Write the failing test**

`base_power_reports_draw_and_supply_before_the_first_tick` — build a base,
do **not** tick, assert the pair is right. Written that way deliberately: it
fails against an implementation that reads the resource.

- [ ] **Step 2: Run it and watch it fail**

`cargo test -p feral-processes-engine base_power_reports_draw_and_supply`

- [ ] **Step 3: Implement `base_power`**

- [ ] **Step 4: Run it and watch it pass**

- [ ] **Step 5: Gates and commit**

`cargo fmt && cargo clippy --workspace && cargo test --workspace`
Commit: `feat(views): Game::base_power`

---

### Task 6: The renderer

**Files:**
- Modify: `crates/gui/src/render/building.rs` — `draw_structures` (:595) for
  the header row, and the status text table (:699-703) for the new arm
- Modify: `crates/gui/src/render/base.rs` — `machine_color` (:1037)
- Test: the `mod tests` already in `crates/gui/src/render/building.rs`

**Interfaces:**
- Consumes: `Game::base_power` (Task 5), `MachineStatus::Unpowered` (Task 3).
- Produces: nothing later tasks depend on.

**Three changes:**

1. **The `B` roster's header.** `draw_structures` opens with a row reading
   `"{n} structures, {a} programs assigned, {i} idle"`. Add a second header
   row for the grid, red when `draw > supply`:

   ```rust
   text_row(format!("Grid  {draw} / {supply}"))
   ```

   **"Grid", not "Power"** — see the global constraints.

2. **The status table** (`:699`) gains
   `MachineStatus::Unpowered => Some("dark — the grid is short, build a Recharger Node")`.
   It names the fix because it is the only status whose fix is a build.

3. **`machine_color`** gains an `Unpowered` arm returning `RED`. Red rather
   than yellow: yellow in this table means "will resolve itself by waiting"
   (`Starved`, `Unstaffed`) and red means "act on it" (`Clogged`,
   `Stranded`). A dark machine never resolves itself.

- [ ] **Step 1: Write the failing tests**

In `building.rs`'s existing `mod tests`, following the pure-rows pattern the
file already uses so no `Painter` is needed:

1. `the_roster_header_reports_the_grid` — the rows contain the draw and the
   supply.
2. `a_dark_machines_row_names_the_recharger` — the status line tells the
   player what to build.

Also read `memory: popup row width IS testable headlessly` before assuming
width cannot be checked — `paint::with_painter` measures real text, and
`draw_row` never clips horizontally. The new header row is short and fixed,
so this is a check, not a task: confirm it cannot overrun the popup.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui the_roster_header_reports_the_grid`

- [ ] **Step 3: Make the three changes**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Gates and commit**

`cargo fmt && cargo clippy --workspace && cargo test --workspace`
Commit: `feat(gui): the base roster reports the grid`

---

### Task 7: Documentation

**Files:**
- Modify: `docs/structures-gen.py`, then regenerate `docs/structures.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`
- Modify: `docs/seams.md`

**`docs/structures-gen.py` is a hand transcription, not a parser.** It gains
two columns, `draw` and `supply`, both `0` on the ten structures with
neither. Do **not** fold the numbers into the existing `makes / does` string:
on a producer or assembler row that column holds the **item id**, which the
script reads back to draw the production-line diagram, and appending text to
it corrupts the diagram. Regenerate with
`python3 docs/structures-gen.py` from the repo root.

**`CLAUDE.md` gets the rules and `docs/seams.md` gets the arguments**, under
the same titles — that split is the standing convention and the reason each
file exists. Three seams are worth stating, all under "The base":

- The ledger is one pure function with two callers, and `runs_a_job()` is
  its "is this a machine" predicate — the fourth thing that has to agree
  about it.
- One writer and two guards, and why the power system runs **first**: last
  makes `set_machine_status` log a transition twice a tick forever.
- Why `Unpowered` was allowed a sixth `MachineStatus` variant where
  `output_stranded` was refused one — the `(x, y)` cut order makes darkness
  one machine's own state, which is exactly the test `views.rs:504` sets.

**Not in this task:** `CHANGELOG.md` and the workspace version bump, which
happen once at the merge under the one-release-per-change rule — a bump on a
branch can be invalidated by a rebase. `docs/manual.md` and the root
`README.md` are carved out and stay stale.

- [ ] **Step 1: Update `docs/structures-gen.py` and regenerate**

- [ ] **Step 2: Confirm the diagram survived**

`git diff docs/structures.md` — the production-line section must be
unchanged. If it moved, the columns were added wrong.

- [ ] **Step 3: Write the `docs/seams.md` entries, then the `CLAUDE.md` rules**

In that order. The argument is what makes the rule keepable, and a rule
written first tends to arrive without one.

- [ ] **Step 4: `cp CLAUDE.md AGENTS.md`**

They are gitignored twins with no tracking to catch drift.

- [ ] **Step 5: Gates and commit**

`cargo test --workspace`
Commit: `docs(base): the power grid's rules and their arguments`

---

### Task 8: Play it

**Files:** none.

`CLAUDE.md`: a green suite is not evidence of play, and this feature's whole
numeric surface is ungated — `balance_sim` models battles and has no
base-production term at all.

- [ ] **Step 1: Open a built-out base**

`cargo run -- --template chains`

- [ ] **Step 2: Answer four questions**

1. Does `Grid 15 / 16` read as a budget, or as noise in a header?
2. Is Home's 4 enough that a new base never meets the mechanic confused?
   (Check with a fresh `cargo run`, not the template.)
3. When a machine goes dark, is the `(x, y)` cut order legible — can you
   tell *why that one*?
4. Is a Recharger at 4 supply for 10 core fragments worth building, against
   everything else 10 fragments buys?

- [ ] **Step 3: Report, do not silently retune**

Numbers that move as a result get their own commit with the observation in
the message.

---

## Self-review

**Spec coverage** — walked the spec section by section:

| Spec section | Task |
| --- | --- |
| Two Powers / the Grid naming | Global constraints; Tasks 3, 6 |
| Supply against draw, the cut loop | Task 2 |
| Draws regardless of staffing | Task 2, test 6 |
| The ledger is one pure function | Tasks 2, 5 |
| Damaged structures draw normally | Task 2 (no special case = the behaviour) |
| `MachineStatus::Unpowered` + precedence | Task 3 |
| Where the status is written | Task 3 |
| Schema, both fields | Task 1 |
| Which structures draw | Task 2 (`runs_a_job`), Task 4 (numbers) |
| The numbers | Task 4 |
| Templates load dark | Task 4 |
| What the player sees | Task 6 |
| Save format (no bump) | Global constraints |
| Testing | Tasks 2, 3, 4, 5, 6 |
| Documentation obligations | Task 7 |
| Out of scope | not built, by omission |

No gaps.

**Placeholders:** none. Every test is named with its intent; every file path
is exact and was read this session.

**Type consistency:** `ledger` → `PowerLedger { supply, draw, dark }` is
produced in Task 2 and consumed in Tasks 3 and 5 under those names.
`PowerGrid` (Task 3) is a resource and is distinct from `PowerLedger` (Task
2) — deliberately two types, since the resource is a per-tick cache and the
ledger is the return of a pure function. `base_power` returns `(draw,
supply)` in that order in Task 5 and is destructured in that order in Task 6.

**One thing left to the implementer on purpose:** exactly which file
`Game::base_power` lands in (Task 5), because the answer is "wherever
`Game::structure_report` already lives" and that is a one-grep question best
answered with the file open.
