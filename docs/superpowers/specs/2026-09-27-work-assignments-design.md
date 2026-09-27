# Work assignments — design

**Status:** approved design, not yet implemented. Prerequisite to
[drop pods](2026-09-27-drop-pods-design.md).

## Intent

The player tells the base who does what. The existing read-only **Base
staff** screen (`b` → Base staff, `Mode::BaseStaff`) becomes a work table:
one row per owned program, one checkbox column per kind of job, and
`schedule_base_labour` obeys it. Row order is meaningful — it is the order
the scheduler picks bodies in.

Success: "this one digs, that one only guards" holds at the keyboard, and a
restriction that leaves work undone is visible on the same screen rather than
silent.

## Decisions

- **Columns are job kinds**, v1: `Operate`, `Guard`, `Dig`, `Build` — the
  scheduler's four `TaskKind`s (`GatherResource`, `Guard`, `Excavate`,
  `Construct`). Fuel, research, work-order and standing-job wants are all
  `GatherResource` at a machine, so all are `Operate`.
- **Structure-kind columns ("Miner") are a planned extension, not v1.** The
  model is shaped so that adding them is a `Duty` variant plus generated
  columns, with no scheduler or screen change.
- **A checkbox is a restriction, not a preference.** A program takes only
  jobs in its checked columns and idles otherwise. A job no eligible body can
  take goes unworked.
- **Needs still win.** Off shift, downed tools, `Downed`/the Repair Bay and
  `drift_idle_staff` are untouched; the table only narrows which *jobs* an
  on-shift body may be handed.
- **Table order is the scheduler's pick order**, replacing `base_staff()`'s
  sort by `Entity`. One order, player-set.
- **Everything checked is today's game.** A save written before this, and a
  newly tamed program, have every column on.
- **Drop Trooper is not in this deliverable.** Its flag, its column and
  `Game::drop_troopers()` land together in drop pods phase 1, so no switch
  ever exists with nothing reading it. `Duties` gains a field then.

## Components

### 1. Data model — `crates/engine/src/duties.rs` (new)

    pub enum Duty { Operate, Guard, Dig, Build }      // Ord, serde by name
    pub struct PostDesc<'a> { pub kind: TaskKind, pub structure: Option<&'a StructureId> }
    impl Duty { pub fn admits(&self, post: &PostDesc) -> bool }
    pub fn duty_admits(off: Option<&Duties>, post: &PostDesc) -> bool

- `components::Duties { off: BTreeSet<Duty> }` stores the **unchecked** set
  — `DepotFilter`'s denied-set precedent. Absent component = everything on,
  so a future column defaults to checked in every existing save with no
  migration.
- A want is admitted when **some** checked duty admits it. v1 duties
  partition `TaskKind`, so exactly one does; the "some" rule is what lets a
  later `Duty::Structure(id)` column overlap `Operate` without a special case.
- `PostDesc` carries the post's structure def id (a build site's goal; `None`
  for a dig site) although v1 ignores it:
  the scheduler builds it once per want, and that is the whole cost of the
  structure-kind extension being scheduler-free.
- `admits` is an exhaustive match (`cell_mark`'s rule).
- `components::StaffRank(u32)` is the table order. Every door into the
  roster (`roster_parts()` and `fuse_companions`, which writes its own list)
  gives a new program `max + 1`. **Fusion keeps the dominant parent's
  `Duties` and `StaffRank`** — `fuse_companions` is the door that silently
  drops a new component, so a test holds it.

### 2. Save

`CreatureSave` gains, all `#[serde(default)]`, no `SAVE_FORMAT_VERSION` bump:

- `off_duties: Vec<String>` — by name. An unknown name is dropped on load
  (`latch_key`'s precedent: a retired duty must be inert, not a parse error).
- `staff_rank: Option<u32>` — `sortie_index`'s precedent, entity ids are not
  stable across a save. A save without ranks is ranked by load order.

Needs a save→load test, not only the RON round-trip (a skipped field leaves
the round-trip green).

### 3. The scheduler — `schedule_base_labour`

**The problem:** `wanted.truncate(on_shift.len())` assumes any body can take
any want. With restrictions it cuts the wrong wants, and a greedy hand-out
strands work: body A (all on, ranked first) and body B (Build only) against
wants [Build, Dig] — greedy gives Build to A and leaves Dig unworked; the
answer is B→Build, A→Dig.

**The change:** the truncate and step 5's hand-out are replaced by one
**priority-ordered bipartite matching** (augmenting paths; base staff is
tens of bodies). Each want, in priority order, tries to claim a body and may
re-seat an already-claimed body onto another want it can take. The result is
the whole assignment, then diffed — the seam rule "decides the whole
assignment by priority and then diffs it" holds.

- **Priority is unchanged.** Want order still decides what is worked when
  bodies run short; wants are never re-sorted.
- **Candidate order:** the body currently holding that post first, then
  table order. So nobody moves unless moving frees work, and new hands come
  off the top of the table.
- **One edge predicate, `can_take(body, want)`:** `duty_admits` ∧
  `!refuses_post` ∧ reachable from the body's own tile. The unreachable drop
  and `can_walk_to_post` fold into it — reach becomes per edge rather than
  "someone can reach it". **Reach is read from a field computed once per
  on-shift body** (`hauling::crew_reach`'s pattern), never a walk per edge;
  the seam forbidding a walk per want still stands.
- **Forced edges are seeded first:** the existing keep rules (`Carrying`, a
  clogged machine while a Depot stands) — provided the body's duties still
  admit the post; unchecking frees a body at its next unforced moment.
- **The empty-queue early return** keeps its no-standdown rule, with one
  exception: a posted body whose post its duties no longer admit is freed.
  Otherwise unchecking a column on a base with no orders does nothing.
- `drop_dry_dig_wants`, the substrate claim and every want producer are
  unchanged.
- **`LabourDemand`** gains `unworked: BTreeMap<Duty, usize>` — wants with no
  body after the matching, attributed to the duty that admits them. Still
  one writer, `record_labour_demand`, now called after the matching (the
  "before the cut" figure `wanted` is kept as is for the work-order header).

### 4. The screen — `Mode::BaseStaff`, made editable

```
 BASE STAFF                              on shift 5 · jobs 9 · 2 unworked
  #  PROGRAM          DOING             OPERATE  GUARD  DIG 2!  BUILD
  1  Scrapper  Lv 7   Mining Node         [x]     [x]    [ ]     [x]
  2  Sentinel  Lv 5   guarding Shield     [ ]     [■]    [ ]     [ ]
 ── away ──────────────────────────────────────────────────────────────
  6  Striker   Lv 8   in party            [x]     [x]    [x]     [x]
  ↑↓ program  ←→ column  Space toggle  < > move row  A toggle column
```

- Staff rows first, then an **away** section (party, wielded, sortie,
  outpost, under study) dimmed but editable — it takes effect when the
  program comes home. Both sections in table order; `<`/`>` moves within the
  whole order.
- Header: total unworked. Column header: that duty's unworked count, `N!`,
  blank at zero.
- Keys: arrows, `Space`, `<`/`>`, `A` (whole column). No lowercase actions.
- The existing DOING/role text is kept.

### 5. Engine API (the `Game` surface)

- `Game::work_table() -> views::WorkTable` — columns (derived, not a const
  list, so generated structure columns need no screen change), rows in table
  order with section, cells and the DOING text, and the unworked counts. The
  one derivation app-core's row count and gui's drawing both read.
- `set_duty(entity, Duty, on)`, `set_duty_column(Duty, on)`,
  `move_staff_row(entity, delta)` — each returns `Result<(), String>`, refusing
  a non-owned entity.
- Nothing here gives the renderer the `World`.

## Tests (intent)

- Duty/`admits`: exhaustive over `TaskKind`; the "some duty" rule.
- Scheduler:
  - all-checked base schedules identically to today (existing suite green,
    plus an explicit equivalence test on a mixed base);
  - the A/B stranding case above works both wants;
  - an unchecked column frees the body, including on the empty-queue path;
  - a want with no eligible body is counted in `unworked` under its duty;
  - table order decides which idle body is posted;
  - a posted body is not moved when nothing is gained (stability);
  - forced keep edges respect duties;
  - off-shift/downed bodies are untouched by duties.
- Save→load keeps `off_duties` and `staff_rank`; an unknown duty name loads
  inert; an old save loads all-on, ranked.
- Fusion keeps the dominant parent's duties and rank; every roster door
  assigns a rank.
- app-core: key handling on `BaseStaff` (toggle, column, move, away rows).
- gui: the screen fits 1280×720 with the widest shipped name (the page has no
  scroll — row width and height censuses); a screenshot via `--screenshot`.

## Out of scope

- Structure-kind columns (designed for, not built).
- Drop Trooper flag, column and `drop_troopers()` — drop pods phase 1.
- Any change to needs, amenities, the Repair Bay or want priority.
