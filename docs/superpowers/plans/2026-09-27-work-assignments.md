# Work assignments — plan

Spec: `docs/superpowers/specs/2026-09-27-work-assignments-design.md`.
Branch: `work-assignments` (off `drop-pods-spec` or `main` once the specs land).
TDD throughout, a commit per green step.

Four phases, one dispatch each. Phases 1, 3, 4 sonnet; **phase 2 opus** (the
matching is the judgment call). Each ends green on `cargo test -p <crate>` +
`cargo clippy --workspace --all-targets` + `cargo fmt`. Full
`cargo test --workspace` once, after phase 4. Final review: opus, whole
branch. Save-schema change → this is the full pipeline, but the schema change
is additive (`#[serde(default)]`, no `SAVE_FORMAT_VERSION` bump).

Read `seams` skill → `base` reference before phase 2.

## Anchors (verified 2026-09-27)

| What | Where |
|---|---|
| `TaskKind` (`GatherResource`, `Guard`, `Excavate`, `Construct`) | `engine/src/components.rs:983` |
| `DepotFilter` (denied-set precedent) | `engine/src/components.rs:407` |
| `schedule_base_labour` (wants 864–925, reach drop ~1040–1116, cut 1135–1136, early return ~1210, diff/keep loop ~1220–1333, hand-out 1334–1402) | `engine/src/game/base/work_orders.rs` |
| `record_labour_demand` / `can_walk_to_post` / `post_route` | `work_orders.rs:1408 / ~1418 / ~1432` |
| `Game::labour_demand` | `work_orders.rs:2788` |
| `LabourDemand { wanted, staff }` + `shortfall()` | `engine/src/resources.rs:1195` |
| `refuses_post` / `willing_index` / `is_on_shift` | `engine/src/game/base/morale.rs:156 / 180 / 251` |
| `crew_reach` / `reaches` / `post_reach` | `engine/src/game/base/hauling.rs:450 / 468 / 489` |
| `base_staff()` (sorts by `Entity`) | `engine/src/game/party.rs:1037` |
| `fuse_companions` (hand-writes its list, spawn ~1465, `roster_parts` at 1464) | `engine/src/game/party.rs:1306` |
| `roster_parts()` + its 5 callers | `game/spawning.rs:414`; callers `spawning.rs:548`, `party.rs:1464`, `outposts.rs:1026`, `lifecycle.rs:3241`, `combat_rewards.rs:1288,1445`; fixture `tests/support.rs:1960` |
| `CreatureSave` (`sortie_index` precedent at 504) | `engine/src/save.rs:460` |
| creature restore / `creature_save_for` | `game/lifecycle.rs:~1911 / ~2342` |
| `BaseStaffRow`, `base_staff_rows`, `handle_base_staff_key` | `app-core/src/app/building.rs:652 / 830 / 870` |
| `Mode::BaseStaff` | `app-core/src/lib.rs:1626`; routed at `app/input.rs:246` |
| `GameKey` (`Char(char)` carries Space, `<`, `>`, `A`) | `app-core/src/lib.rs:~840` |
| `draw_base_staff` (a `draw_popup` Large) / row builder `base_staff_menu_rows` | `gui/src/render/building.rs:1032` |
| gui dispatch | `gui/src/render/mod.rs:937, 1143` |

## Phase 1 — data model, roster doors, save (engine only, no behaviour change)

Files: `engine/src/duties.rs` (new, `pub mod` in `lib.rs`), `components.rs`,
`game/spawning.rs`, `game/party.rs`, `save.rs`, `game/lifecycle.rs`.

- `Duty { Operate, Guard, Dig, Build }` — `Clone, Copy, Eq, Ord, Debug`,
  `Duty::ALL`, `name()`/`from_name()` (the save's string form; `from_name`
  returns `Option`). `PostDesc<'a> { kind: TaskKind, structure:
  Option<&'a StructureId> }`. `Duty::admits` is an exhaustive match on both
  enums (no `_`). `duty_admits(off: Option<&Duties>, post) -> bool`: `None`
  → true; else `Duty::ALL.iter().any(|d| !off.contains(d) && d.admits(post))`.
- `components::Duties { pub off: BTreeSet<Duty> }`,
  `components::StaffRank(pub u32)`.
- **Rank minting:** `roster_parts()` returns a `StaffRank` too (max over
  existing `StaffRank` + 1, 0 on empty) — so every caller gets one by
  construction. `fuse_companions`: carry the dominant parent's `Duties` (if
  any) and `StaffRank` onto the fused entity, overriding the fresh rank.
  No `Duties` inserted anywhere (absent = all on).
- `base_staff()` sorts by `(StaffRank, Entity)`; a body without a rank sorts
  last. Existing suite must stay green — rank order equals spawn order ≈
  entity order in every fixture; if a test moves, investigate before editing.
- Save: `CreatureSave { off_duties: Vec<String>, staff_rank: Option<u32> }`,
  both `#[serde(default)]`. Write: names sorted. Load: unknown names dropped
  silently; empty set → no `Duties`. Missing rank → after restoring all
  creatures, assign ranks to unranked owned programs in load order starting
  at `max + 1`. Update every `CreatureSave { .. }` literal (grep
  `sortie_index: None` — gui has three).
- Tests (engine): `admits` exhaustive (each `TaskKind` admitted by exactly one
  duty); `duty_admits(None, _)` true; every roster door assigns a distinct
  increasing rank (capture, adopt, starting program, outpost, fusion);
  fusion keeps dominant parent's duties + rank; **save→load** keeps
  `off_duties` and `staff_rank` (not only RON round-trip); unknown duty name
  loads inert; a save with neither field loads all-on and ranked.

## Phase 2 — the scheduler (engine)

Files: `game/base/work_orders.rs`, `resources.rs`, maybe
`game/base/morale.rs` (retire `willing_index` if it goes unused).

Replace the **cut** (`record_labour_demand` + `truncate`, ~1135) and **step 5**
(the hand-out loop, 1334–1402) with one priority-ordered bipartite matching.
Everything above the cut is unchanged except the reach drop (below).

1. **Per-body reach, once.** Build `crew_reach` for each on-shift body (lazily
   as today, now for every want kind, not only Construct/Excavate). Edge
   reach = `hauling::reaches(grid, field, from, at, structure_footprint_of(post), &blocked)`.
2. **`can_take(body, post, kind)`** = `duty_admits(body's Duties, &PostDesc{kind, structure})`
   ∧ `!refuses_post` ∧ reach. `structure` = the `Structure.kind` for a
   machine/guard post, the `BuildSite` goal def id for Construct, `None` for
   a `DigSite`.
3. **Unreachable drop** keeps its meaning ("nobody on shift can reach it" →
   announce + drop) but is asked through the same reach lookups; its latch
   clearing is unchanged. Keep "skipped when nobody is on shift".
4. `drop_dry_dig_wants` stays where it is.
5. **Forced edges seeded first:** the existing keep rules — `Carrying`, and
   `shedding` (clogged machine while a Depot stands) — hold the body's
   current post only if `duty_admits` still admits it; otherwise the body is
   freed exactly as a refused post is (Carrying keeps its never-free rule —
   **check**: a loaded body is never freed, so an unchecked column frees it
   only after delivery; test that).
6. **Matching:** for each want in priority order, try candidates — current
   holder first, then table order (`base_staff()` order) — with augmenting
   paths (DFS, visited set per want). A want that finds no body is recorded
   unworked. Wants are never re-sorted; an earlier want is never un-matched
   by a later one (Kuhn in priority order gives this).
7. **Diff:** a body whose match equals its current `Task` keeps it untouched
   (anti-thrash); any other posted on-shift body is freed; matched bodies
   without the task are posted via `post_worker/guard/digger/builder`.
   Keep `post_reach`-as-authority: if a chosen edge fails `can_walk_to_post`
   at posting (only possible past `HAUL_WALK_MAX_TILES / 2` base radius),
   skip silently as today.
8. **Empty-queue early return:** add the exception — a posted on-shift body
   whose current post its duties no longer admit forces the pass through
   (beside `a_posted_body_is_off_the_line`).
9. **`LabourDemand`** gains `unworked: BTreeMap<Duty, usize>` (attributed to
   the first duty in `Duty::ALL` that admits the want). `record_labour_demand`
   stays the one writer, now called after the matching with `wanted.len()`
   taken before it (same figure as today). The `staff.is_empty()` early
   return records every want as unworked.

Doc comments: rewrite the function doc's five steps and every comment that
cites `truncate(staff.len())` as the priority mechanism (grep it — CLAUDE.md
seams and `seams` skill `base` reference too; the rule becomes "priority is
the position in the list; the matching never un-seats an earlier want").

Tests (engine, `tests/work_orders.rs` or new `tests/duties.rs`):
- equivalence: a mixed base (build + fuel + order + standing + dig wants,
  fewer bodies than wants) posts the same bodies to the same posts as before
  — write it **before** the change, green on old code;
- the spec's A/B stranding case works both wants;
- unchecking a column frees the body, including on the empty-queue path;
- a want with no eligible body lands in `unworked[duty]`;
- table order decides which idle body is posted (swap ranks → other body);
- stability: a posted body is not moved when a reshuffle gains nothing;
- forced keep respects duties (Carrying body delivers, then freed);
- off-shift / `Downed` bodies are untouched by duties;
- existing `labour_demand_counts_*` tests stay green.
- Mutation-check the A/B test (revert to greedy → red).

## Phase 3 — engine API + app-core

Files: `engine/src/views.rs`, a `Game` impl file (e.g.
`game/base/duties.rs`), `app-core/src/app/building.rs`, `app-core/src/lib.rs`.

- `views::WorkTable { columns: Vec<WorkColumn>, rows: Vec<WorkRow>,
  on_shift, jobs, unworked_total }`; `WorkColumn { duty, label, unworked }`;
  `WorkRow { program: EntityView, section: WorkSection { Staff, Away }, role,
  doing, cells: Vec<bool>, rank }`. Columns derived from `Duty::ALL`, not a
  const in gui. Rows: staff then away, each in rank order. `doing` is what
  `base_staff_rows` computes today (`staff_activity` / `program_activity`).
- `Game::work_table()`, `set_duty(e, Duty, on)`, `set_duty_column(Duty, on)`,
  `move_staff_row(e, delta: i32)` — all `Result<(), String>`, refusing a
  non-owned entity; `move_staff_row` swaps ranks with the neighbour in the
  **whole** order, clamped at the ends (no error at an end — a no-op).
  `set_duty` removes `Duties` when `off` becomes empty.
- app-core: `BaseStaffRow` becomes (or wraps) `WorkRow`; the one row source
  for handler and renderer is `work_table()`. Keep `work: Option<WorkProfile>`
  only if phase 4's width census fits it — otherwise drop it (the spec
  mockup has no WORK column; **flag to the user**, don't decide silently).
  Column cursor: new `App` field (reset on open). Keys: Up/Down row,
  Left/Right column, `Char(' ')` toggle, `Char('<')`/`Char('>')` move row
  (selection follows the row), `Char('A')` toggle column (on if any cell in
  it is off, else off), Esc closes. Refusals through `App::refuse`. Rewrite
  the "This screen writes nothing" doc.
- Tests (app-core): toggle, column toggle, move keeps selection on the moved
  program, an away row is editable, lowercase `a` does nothing.

## Phase 4 — gui

Files: `gui/src/render/building.rs` (+ `mod.rs` call site).

- Draw the table per the spec mockup: header counts, `N!` on a column with
  unworked, `[x]`/`[ ]`, highlighted cell under the column cursor, away
  section dimmed after a rule, key hint row. Through `Painter` only.
- Census tests (`popup-row-width-is-testable-headlessly`): the widest shipped
  species name + level + widest DOING string fits the Large popup width;
  row count at a realistic roster cap fits the height (no scroll — cap or
  trim with a "+N more" row if it cannot, and say which).
- Screenshot: `cargo run -- --template <one with staff> --keys "b ..."
  --screenshot /tmp/…/staff.png`, Read it, report what it shows.

## Deploy notes

Minor bump (save stays loadable, new feature). CHANGELOG section; update
CLAUDE.md seams (`schedule_base_labour` bullets that cite `truncate`), the
`seams` skill base reference, and the memory graph `seam:` entries per the
`seams` skill's three-write order. Help page for Base staff if one exists
(`rg -l "Base staff" assets/help`).
