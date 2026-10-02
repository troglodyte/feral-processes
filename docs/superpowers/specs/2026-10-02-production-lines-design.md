# Production lines

**Status:** design.

**No save change.** A line is derived from the structures on the grid every
time it is asked for and never stored; `SAVE_FORMAT_VERSION` is untouched, so
the release is a patch.

**Out of scope:**
- the player loading a machine's input by hand
- a line-length or completeness bonus
- a cap on line size
- any new map drawing
- per-machine progress that survives a worker switching machines

## 1. Purpose

Today every machine in a production chain costs one base staff member: a
Mining Node → Lathe → Compiler run eats three bodies, even though the
adjacency chain (`collect::plan_adjacent_take`) already moves the items
between them. This feature makes **a connected run of machines one job**:

- **Staff (A).** One worker runs the whole line.
- **One unit (B).** A work order, a standing job and the roster all treat the
  line as one thing.
- **Layout (C).** How you place machines decides what is one line. Connecting
  them saves staff; keeping them apart buys throughput.

Decisions taken in the brainstorm:

| Question | Decision |
|---|---|
| How a line forms | Automatically, from adjacency. No controller structure. |
| What links two machines | A **feed edge** only: A's product is B's ingredient or fuel. Two touching Mining Nodes are two lines. |
| Workers per line | **Exactly one.** More throughput means a second, separate line. |
| How the worker spends its time | **Pull from the end**: work the furthest-downstream wanted machine that can progress. |
| Where the worker stands | It **walks to the active machine's station**. |
| Members | Work nodes, assemblers, Teardown Rigs, fuel burners (`power_upkeep`). |
| An ingredient no member makes | The line's worker fetches it from a Depot, through the existing fetch errand. |
| Layout reward | The staff saving only. |

## 2. Line membership

### Feed edges

There is a directed edge A → B when A and B are orthogonal neighbours within
`collect::plan_adjacent_take`'s reach (`feeders_by_tile`), and one of these
holds:

- **Assembler:** A's product (`systems::produced_item`) is an ingredient of
  B's recipe (`systems::assembly_recipe`).
- **Fuel:** A's product is B's `power_upkeep` fuel.
- **Rig:** A is a Teardown Rig whose fitted tool's yield table
  (`Hopper::standing_tool` resolved against `ToolDb`) includes an ingredient
  or the fuel of B. `produced_item` ignores `strips` today, so this needs a
  new `yields_of(rig)` helper. A rig with no tool fitted has no out-edges.

The edge is a **call into the same reach and recipe functions the pull
uses**, never a re-derivation. A line's links therefore cannot disagree with
what actually moves.

### Lines

A **line** is a weakly connected component of the feed-edge graph. A machine
with no edges is a **line of one**, and a line of one must behave exactly as
the machine does today. Most of the existing test suite already guards this
invariant.

- **Key.** A line is identified by its lowest-`(x, y)` member's tile. The key
  is derived, so it is stable across ticks for an unchanged layout without
  being stored.
- **Downstream rank.** Each member has a rank: the length of the longest edge
  path from it to a sink of the line. Sinks are rank 0. "Furthest downstream"
  means lowest rank, with ties broken by `(x, y)` (the assembler sort's
  reason: query order is not stable).
- **Cycles.** A cycle (A feeds B, B feeds A) would make the rank undefined.
  No shipped recipe creates one, but a mod could. Rank is computed over the
  graph with back edges dropped in `(x, y)` DFS order, and a malformed cycle
  degrades to an arbitrary but deterministic order, never a panic.

**Code home:** a new `crates/engine/src/game/base/lines.rs`:
- `Game::production_lines() -> Vec<Line>`
- `Game::line_of(machine) -> Option<LineKey>`
- `Line { key, members: Vec<Entity> /* rank order */, edges }`

Lines are recomputed on demand, once per scheduling pass. They are never
stored or cached across ticks, so build, demolish, upgrade and tool changes
need no hooks.

## 3. Staffing a line

### Wants collapse to one per line

`base_wants` keeps building its list exactly as now: builds, fuel, research,
`settle_orders`, standing jobs, digs. Then a single pass collapses it:

1. Each `(machine, GatherResource)` want whose machine belongs to a line of
   two or more is replaced by a want for that **line**. The line takes the
   position of its first (highest-priority) member want, and later member
   wants are dropped. Wants for lines of one and all other `TaskKind`s pass
   through untouched.
2. The line's want names its **active machine** (below). The matching
   (`assign_by_priority`) and the posting (`post_worker`) see an ordinary
   `(machine, GatherResource)` want. The worker walks to that machine's
   station, so the station, Stranded, job-mark and hauling rules apply
   unchanged.

**Line holder.** For `holder[want]`, the holder of a line want is the body
whose `Task.target` is **any** member of the line. That keeps the
anti-thrash seat ("existing holders first") working when the active machine
moves. The diff then re-posts the same body to the new machine instead of
swapping bodies.

### Active machine

The **wanted set** of a line is the members that produced a want in step 1.
Only wanted members are candidates. An order for Blank Substrate from a
Mining → Lathe → Compiler line must not run the Compiler.

The active machine is chosen in this order:

1. **Finish the cycle.** If the holder's current target is a wanted member
   and its task is mid-cycle (`0 < progress < required`), the active machine
   stays that member. Today's `Task` keeps progress on the worker and
   `post_worker` resets it, so switching mid-cycle would throw work away.
   Switching only at a cycle boundary costs nothing.
2. **Pull from the end.** Otherwise, the wanted member with the lowest
   downstream rank.

Every wanted member already passed `can_progress`. That check counts an
ingredient held by a Depot, so a machine short of an ingredient no member
makes, but which a Depot holds, is a candidate. When it is chosen, its worker
runs the existing `Collect`/`Load` errand (`hauling.rs`). No new hauling code
is needed.

The forced-body rules are unchanged. A line worker who is `Carrying`, or
shedding a clogged member's output to a Depot, keeps its post. The line's
want is held by it until the errand ends.

### Standing jobs

- `Game::set_standing_job(structure, work, guard)` writes `work` to **every
  member** of the structure's line. `guard` stays per-structure: guarding is
  not production.
- A line counts as standing if **any** member carries `StandingJob::work`. In
  that case every member is a standing-job candidate, each still gated on
  `can_progress`. Two lines merged by building between them therefore run
  together if either was running, with no flag migration.
- The flag stays on the structures (`StructureSave::standing_work`), so
  nothing new is saved.

### Work orders and fuel

There is no change to `settle_orders` or `fuel_wants`. They emit machine
wants as now, and the collapse folds them into lines. A burner inside a line
is fed by its neighbour, so it raises no fetch want. If it does raise one (no
fuel within reach), that want collapses into its line like any other.

## 4. Status and views

- **`views::LineReport`** contains:
  - `key`
  - members in feed order (sources first, highest rank to lowest)
  - `active: Option<Entity>`
  - `status: MachineStatus`
  - `missing: Option<ItemId>`

  `status` is the active machine's status. With no active machine, it is the
  first non-`Idle` member status from the end of the line. `missing` is the
  ingredient a `Starved` member lacks that no member makes and no Depot
  holds.
- **`Game::structure_report()`**: each `StructureReport` gains
  `line: Option<LineKey>`. The key is set for lines of two or more and `None`
  otherwise. There is also a new `Game::line_reports()`.
- **Examine:** a machine in a line gets one more examine line:
  "Part of a line: Mining Node → Lathe → Compiler."

`MachineStatus` is still written per machine by its existing writers
(`set_machine_status`, `idle_machine_system`). A line's status is derived
from them, never written.

## 5. App-core and gui

- **Structure roster.** A line of two or more is one row:
  - `Mining Node → Lathe → Compiler · working Lathe` while working
  - `… · Compiler needs Bytecode Block` when `missing` is set
  - otherwise the line's status word

  Its member structures do not get their own rows. The standing-work toggle
  on the line row calls `set_standing_job` on the row's first member, which
  reaches the whole line. Lines of one keep today's rows exactly.
- **Map:** no new drawing. The existing job mark sits on the active machine,
  so the worker is seen moving along the line.
- Row selection follows the existing roster's rules: lowercase letters are
  row selectors and actions are uppercase.

## 6. Docs

- A `CHANGELOG.md` patch section.
- A help page in `assets/help/` explaining lines: what connects, one worker,
  pull from the end, Depot-fed ingredients.
- A new seam in all three places (graph `seam:production-lines`, the `seams`
  skill, `.claude/rules/seams-base.md`):
  - a line is derived and never stored
  - one worker per line
  - the worker switches only at a cycle boundary
  - a line's link is a call into the pull's own reach and recipe functions

## 7. Testing

Engine unit tests, TDD, using `dev-saves/` templates where one fits and
`work_node_parts()` / `park_at_post()` for hand-built fixtures.

**Membership**
- Two touching Mining Nodes are two lines.
- Mining → Lathe is one line, ranked Lathe 0 and Mining 1.
- Fuel edge: a power-cell maker beside a burner joins its line.
- Rig edge: present with a tool whose yields include the neighbour's
  ingredient, absent with no tool fitted.
- Building a linking machine between two lines merges them; demolishing it
  splits them.
- A modded cycle does not panic and ranks deterministically.

**Staffing**
- A three-machine line under a standing job is staffed by exactly one body,
  with two staff free.
- Pull from the end: when the Lathe can progress, the worker is on the Lathe
  rather than the Mining Node.
- Wanted set: an order for the middle product never posts to the end machine.
- Cycle boundary: a worker mid-cycle on Mining is not moved when the Lathe
  becomes workable, and is moved on the tick its cycle completes. This test
  must fail with rule 1 removed.
- Depot feed: a line whose end machine needs an ingredient no member makes
  fetches it from a Depot with its one worker.
- With no Depot stock, `LineReport::missing` names the item.
- Standing: the toggle on one member reaches all members; a merged line with
  one flagged member runs.
- A line of one schedules identically to today: the existing suite stays
  green.

**Save**
- A save → load (not only a RON round trip) with a staffed line keeps the
  same body on the same active machine.

**App-core**
- The roster shows one row per line of two or more.
- The line row's standing toggle reaches every member.

**Gates**
- `cargo test --workspace`.
- `balance_sim` is expected not to move, since it doesn't touch base labour.
  Confirm it.

## 8. Risks

- **Throughput drop in existing bases.** Bases whose machines already feed
  each other by adjacency drop to one worker per line on load. Their lines
  produce more slowly and the freed staff return to the pool. This follows
  from "automatic, exactly one". The CHANGELOG entry says so.
- **Rig "can progress".** A rig's job is a hopper queue, not a recipe. The
  plan must confirm how a rig's want is gated today and reuse that gate as
  its candidacy test.
- **Rig reach.** Confirm that `feeders_by_tile` and `plan_adjacent_take`
  already let an assembler pull from a rig's `Stock.output`. If they do not,
  a rig edge would show a link that moves nothing. In that case the rig edge
  is dropped from scope rather than the pull widened silently.
