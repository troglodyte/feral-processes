# Production lines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Checkbox steps. Per project rule, this plan carries files, interfaces, test intent and gates — **not finished code**; read the cited source before writing.

**Goal:** A feed-connected run of machines is one job staffed by exactly one worker, who pulls from the end of the line.

**Architecture:** A new `game/base/lines.rs` derives lines from the grid on every call (never stored). `base_wants` collapses member `GatherResource` wants into one want per line, naming an active machine. `assign_base_labour` treats any member as the line's holder. Views add `LineReport`. App-core and the gui fold a line into one roster row.

**Spec:** `docs/superpowers/specs/2026-10-02-production-lines-design.md`. § numbers below refer to it.

## Spec amendments (found while grounding)

- **Rigs are not line members in this change.** Reach is fine: a rig's `Stock.output` is in `feeders_by_tile`. But the scheduler never staffs a rig today. `can_progress` (`work_orders.rs:470`) is false for it, so its standing want is dropped at `:1044`, and rigs are staffed only by hand. Rigs also strip with each hopper entry's own tool (`teardown.rs:111-121`), not with `standing_tool`, so an edge built from `standing_tool` could claim a link that never moves anything. Adding a rig gate would widen scope silently, so rigs stay lines of one. Edit spec §1 Members, §2 Rig, and the §7 rig test to match in T1's commit.
- **Roster rows and examine lines are drawn in the gui** (`gui/src/render/building.rs:1376` `draw_structures`, `:1481` `structure_detail_lines`). App-core owns the row index (`app/menus.rs:342`). Phase 4 therefore touches both crates.
- **`dev-saves/chains.ron` becomes one line of five**: Mining (2,0) → Refinery → Assembly Bay, plus Conduit → Winding Node → bay. `launcher/src/dev_template.rs:498` may assert more than one worker. Update it to the new rule; don't weaken it.

## Global constraints

- No save change; `SAVE_FORMAT_VERSION` is untouched, so this is a patch release, cut at landing rather than on the branch.
- An edge is a **call** into the pull's own reach (`collect::feeders_by_tile`, `collect::ORTHOGONAL`, the `With<Stock>` filter) and recipe functions. Never re-derive either.
- A line of one behaves exactly as today. The existing suite is that gate: any edit to an existing test needs a stated reason.
- Never iterate query order unsorted. Sort by `(x, y)`.
- Each task:
  - failing test first, then `cargo test -p <crate> <name>`
  - `cargo fmt` and `cargo clippy --workspace --all-targets`, both clean
  - commit explicit paths; never `git add -A`, never push

## Review focus (each pinned by a test in the owning task)

1. **Cycle boundary.** A mid-cycle worker is not moved. The test must fail with rule 1 removed (mutation-check it). (T4)
2. **Wanted set.** An order for a middle product never posts the worker to the end machine. (T4)
3. **Line holder.** When the active machine moves, the same body is re-posted. No swap, no second body. (T4)
4. **Modded cycle.** No panic, and the rank is deterministic. (T2)
5. **Forced bodies.** A line worker who is `Carrying` or shedding keeps the line's want. (T4)

---

## Phase 1: edges and membership (engine, one context)

### T1: `feeds` predicates

**Files:**
- `crates/engine/src/systems.rs`, near `assembly_recipe` :629, `produced_item` :677 and `intake_recipe` :655
- `crates/engine/src/game/inspection.rs:1444` (`linked_edges_by_structure`)
- the spec amendment above
- tests in `crates/engine/src/tests/` (a new `lines.rs`, registered alphabetically in `tests/mod.rs`)

**Produces:**
- `pub(crate) fn feeds_ingredient(a: &StructureDef, b: &StructureDef, items: &ItemDb) -> bool` is true when `produced_item(a) ∈ assembly_recipe(b)`.
- `pub(crate) fn feeds_fuel(a, b) -> bool` is true when `produced_item(a) == b.power_upkeep`.
- `pub(crate) fn feeds(a, b, items) -> bool` is either of the two.
- `linked_edges_by_structure` calls `feeds_ingredient` instead of its inline check. The map's drawing doesn't change: no fuel edges are added, which follows "no new drawing".

**Tests:**
- `mining_node` → `lathe` is an ingredient edge.
- `power_conduit` → `recharger_node` is a fuel edge.
- `mining_node` → `mining_node` is not an edge.
- `teardown_rig` → anything is not an edge.
- The existing linked-edges tests pass unedited.

- [ ] failing tests → implement → green → fmt/clippy → commit

### T2: `lines.rs`: components, rank, key

**Files:**
- new `crates/engine/src/game/base/lines.rs`, registered as `pub(crate) mod lines;` in `base/mod.rs`, alphabetically after `hauling`
- `crates/engine/src/tests/lines.rs`
- fixtures: `spawn_machine_at` (`tests/support.rs:1520`)

**Produces:**
- `pub struct LineKey(pub (i32, i32))`, the lowest `(x, y)` member, with `Copy`, `Eq`, `Hash` and `Ord`.
- `pub struct Line { key, members: Vec<Entity> /* rank asc, (x,y) tiebreak */, rank: Vec<u32> }`.
- A pure `fn group(nodes: &[(Entity, (i32,i32))], edge: impl Fn(usize, usize) -> bool) -> Vec<Line>`:
  - weak components
  - longest path to a sink, with back edges dropped in `(x, y)` DFS order
- `Game::production_lines() -> Vec<Line>`, built over `(Entity, &Structure, &Position), With<Stock>` exactly as the pull queries (`systems.rs:1826`), with neighbours via `ORTHOGONAL`.
- `Game::line_of(e) -> Option<LineKey>`, which is `Some` only for lines of two or more.

**Tests (§7 Membership, rigs excluded):**
- two touching Mining Nodes form two lines
- Mining → Lathe has ranks Lathe 0, Mining 1
- Conduit + Recharger form one line
- Building a Lathe between two Mining Nodes and a Compiler merges them into one line, and demolishing it splits them.
- A `group` with a hand-built cycle doesn't panic, and the same input gives the same order twice.

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Phase 2: staffing (engine, one context; read `.claude/rules/seams-base.md` and the `seams` skill's `base-labour.md` first)

### T3: standing jobs reach the line

**Files:**
- `game/base/work_orders.rs`: `set_standing_job` :2492, `standing_wants` :2584
- `tests/lines.rs`

**Produces:**
- `set_standing_job` writes `work` to every member of `line_of(structure)`, after the `accepts_a_program` check on each member.
- `guard` stays per-structure.
- `standing_wants`: when any member carries `work`, every member is a candidate. Each candidate is still gated on `can_progress` at `:1044`.

**Tests:**
- toggling one member reaches all of them
- After building a link between a flagged line and an unflagged one, the merged line yields wants for both sides' members.

- [ ] failing tests → implement → green → fmt/clippy → commit

### T4: collapse, active machine, line holder

**Files:**
- `game/base/work_orders.rs`: the end of `base_wants` :1001-1060, and the `assign_base_labour` holder at :1287-1290 and its signature at :1064
- `game/base/lines.rs`
- `tests/lines.rs`

**Produces:**
- In `lines.rs`, `fn collapse(wants: Vec<(Entity, TaskKind)>, lines: &[Line], holding: impl Fn(LineKey) -> Option<&Task>) -> Vec<(Entity, TaskKind)>`. It follows §3 steps 1–2:
  - The first member want sets the line's position, and later member wants are dropped.
  - The emitted entity is the active machine. Rule 1: if the holder's `Task.target` is a wanted member with `0 < progress < required`, the active machine stays that member. Rule 2: otherwise, the wanted member with the lowest rank.
- `base_wants` calls `collapse` last.
- `assign_base_labour` builds `holder[i]` as follows:
  - For a want whose target is in a line of two or more, it matches a body whose `Task.target` is **any** member with `kind == GatherResource`.
  - Otherwise it keeps today's exact match.
  - Pass in a `HashMap<Entity, LineKey>` from the same `production_lines()` call. Compute lines once per pass.
- Forced bodies (`:1252-1274`) stay unchanged. Check that a forced line worker removes the **line's** want from `open`, not just its exact target's.

**Tests (§7 Staffing):**
- A three-machine line under a standing job has exactly one body posted, and two staff stay free.
- Pull from the end: when the Lathe can progress, the worker is on the Lathe.
- Wanted set: an order for `blank_substrate` with Mining → Lathe → Compiler never posts to the Compiler.
- Cycle boundary: the worker stays on Mining mid-cycle and moves on the completing tick. Mutation-check it with the fix committed.
- Holder: the same `Entity` stays the worker across the move.
- A `Carrying` line worker keeps its post.
- Depot feed: an end machine short of an ingredient no member makes, held by a Depot, gets it fetched by the one worker through `Collect`/`Load`.
- The full `cargo test -p feral-processes-engine` stays green.

- [ ] failing tests → implement → green → fmt/clippy → commit

### T5: save → load and the chains template

**Files:**
- the save tests: find the existing save→load helper with `rg 'fn .*save.*load' crates/engine/src/tests`
- `crates/launcher/src/dev_template.rs:498`

**Tests:**
- A real save → load, not only a RON round trip, of a staffed line keeps the same body on the same active machine.
- Update the chains template test to the one-line rule and record why in the commit message.
- `cargo test -p feral-processes-engine balance_sim` doesn't move. Confirm it; don't assume it.

- [ ] tests → green → commit

---

## Phase 3: views (engine)

### T6: `LineReport`, `StructureReport.line`

**Files:**
- `crates/engine/src/views.rs` (`StructureReport` :1624)
- `game/inspection.rs` (`structure_report` :1501)
- `game/base/lines.rs`
- `tests/lines.rs`

**Produces:**
- `views::LineReport { key, members: Vec<Entity> /* sources first */, names: Vec<String>, active: Option<Entity>, status: MachineStatus, missing: Option<ItemId> }`, with fields as described in §4.
- `active` is a held body's `Task.target` among the members.
- `status` is the active machine's status, or the first non-`Idle` member status from the end.
- `missing` is the ingredient a `Starved` member lacks that no member makes and no Depot holds. Use `can_progress`'s depot count; don't copy it.
- `StructureReport.line: Option<LineKey>`.
- `Game::line_reports() -> Vec<LineReport>`, sorted by key.

**Tests:**
- `missing` names the item when no Depot stocks it, and is `None` once a Depot does.
- `status` follows the active machine.
- a line of one has `line: None`

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Phase 4: roster and examine (app-core + gui, one context)

### T7: one row per line

**Files:**
- `crates/app-core/src/app/menus.rs:342` (`handle_structures_key`)
- `crates/app-core/src/app/building.rs:19-142` (`Staffing`, `handle_structure_assign_key`)
- `crates/gui/src/render/building.rs`: `draw_structures` :1376, `structure_detail_lines` :1481
- `crates/app-core/src/tests/building.rs`

**Produces:**
- An app-core `enum RosterRow { Structure(usize /* report idx */), Line(LineKey) }` and a `fn roster_rows(reports, lines) -> Vec<RosterRow>`:
  - a line row sits at its first member's position
  - its members get no rows of their own
- Both `handle_structures_key` and `draw_structures` index this list, so a selected row and a drawn row can't disagree.
- The line row's text follows §5: `A → B → C · working B`, `… · C needs <item>`, or the status word.
- On a line row, the staffing picker's toggle calls `set_standing_job` on the first member.
- `structure_detail_lines` adds "Part of a line: A → B → C." when `report.line` is `Some`.
- All drawing stays inside `Painter` (`.claude/rules/drawing-seam.md`).
- Follow the existing roster's key bindings: lowercase letters select rows, and actions are UPPERCASE.

**Tests (app-core):**
- one row per line of two or more, and lines of one keep today's rows
- Enter on a line row and toggling standing reaches every member
- Update existing roster tests only where a fixture now forms a line, and say so.

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Phase 5: docs and seam (inline, no subagent)

### T8

- A help page `assets/help/NN-production-lines.md`, numbered next to `60-your-base.md` and following the `assets/help/README.md` format. It explains:
  - what connects machines
  - one worker per line
  - pull from the end
  - Depot-fed ingredients
  - Teardown Rigs stand alone
- The seam `seam:production-lines`, written in the order the `seams` skill documents: the graph, then `.claude/skills/seams/references/base-labour.md`, then `.claude/rules/seams-base.md`. It records four rules from §6 and one more:
  - a line is derived, never stored
  - one worker per line
  - the worker switches only at a cycle boundary
  - a line edge is a call into the pull's reach and recipe functions
  - rigs are excluded until a rig gate exists
- The `CHANGELOG.md` entry goes in at landing, not on the branch. It names the throughput drop from spec §8.

- [ ] write → commit

---

## Final gate

- `cargo test --workspace`, then `cargo clippy --workspace --all-targets` clean.
- A whole-branch review on opus, given the diff as a file and the Review focus list above.
- Cheap play check: `cargo run -- --template chains --screenshot out.png`, then Read the PNG to see the line row and the job mark.
