# Plan: situational thoughts

Spec: `docs/superpowers/specs/2026-09-30-situational-thoughts-design.md` (the
argument; this file is only the order of work). Branch `situational-thoughts`
in the primary checkout. TDD per task, a commit per green step,
`cargo fmt` + `cargo clippy --workspace --all-targets` before each commit.
Read `.claude/rules/seams-memories.md` and `seams-base.md` before Phase 2.

Iterate with `cargo test -p feral-processes-engine <name>`. Phase gate is
the named tests plus clippy; the full suite runs once, at the end.

## Phase 1 — data (engine only)

**T1. `ThoughtDef` / `ThoughtDb` / `Trigger`** in new `crates/engine/src/situations.rs`
(`pub mod situations;` in `lib.rs`).
- `Trigger` (serde, `Copy`, `Eq`): `BesideRival, BesideFriend, Unpowered,
  MachineRunning, NoAmenity`.
- `ThoughtDef { trigger, name, blurb, intensity: f32 }`.
- `ThoughtDb::load_dir(&Path) -> io::Result<(Self, Vec<String>)>` — copy
  `MemoryDb::load_dir`'s shape: absent dir = empty, malformed file skipped
  with a warning, duplicate trigger → first by file name wins + warning.
  `ThoughtDb::get(Trigger) -> Option<&ThoughtDef>`.
- Tests: loads a dir; malformed skipped; duplicate trigger warned and first
  kept; absent dir empty.

**T2. Wire into asset loading** (`game/lifecycle.rs`: the assets bundle
struct ~:3360, loader ~:3485, both `insert_resource` sites ~:508 and
~:1370). Same absent-is-silent comment pattern.

**T3. Ship assets**: `assets/thoughts/{beside_rival,beside_friend,unpowered,machine_running,no_amenity}.ron`
(−3, +2, −2, +1, −2) + `assets/thoughts/README.md` (schema, closed trigger
list, empty-catalogue rule). Census test: every `Trigger` variant has a
shipped def (iterate an exhaustive `match`-backed `Trigger::ALL`).

## Phase 2 — the assessment and the fold

**T4. `SITUATION_MAX_TOTAL: f32 = 5.0`** in `tuning.rs` near the MORALE_*
constants, `const _: () = assert!(SITUATION_MAX_TOTAL < -MORALE_SULKS_AT);`,
doc giving the spec's reason in one line.

**T5. `Situation` + pure fold.** `Situation { thoughts: Vec<Trigger> }`
(`Component`, not saved, doc cites `Stranded` as precedent).
- `pub(crate) fn scaled_rows(&Situation, &ThoughtDb, Disposition) -> Vec<(&ThoughtDef, f32)>`:
  each resolved def's `felt_as.felt(intensity)`; if |raw total| exceeds the
  cap, scale every share by `cap / |total|`.
- `pub(crate) fn sum(...) -> f32` = sum of `scaled_rows`. Doc: free function
  for `sum_intensity`'s reason.
- Tests: clamp at −6 raw → −5; rows sum to `sum`; unresolved trigger
  contributes nothing; empty db → 0; `Abrasive` disposition felt.

**T6. `assess_situation_system`** in `situations.rs`. One pure helper
`assess(...) -> Situation` that both the system and `Game` (T8) call, taking
what the system queries: staff programs with `Position`, `ProgramId`,
`Option<&Task>`, `Option<&Memories>`, `Option<&Disposition>`; `PowerGrid`;
`MachineStatus` of targets; structures for `Amenities::build`; `MemoryDb`,
`GameClock`.
- Staff in base space only (use whatever filter `base_staff()` uses).
- Adjacency: Chebyshev ≤ 1, excluding self (copy `drift_idle_staff`'s
  check, `work_orders.rs:~2446`, adjacency first then bond).
- Bond: `bonds::band(sum_intensity(holder store, …, Read::Opinion, keep:
  subject == Program(other id)))` — must be the same call `Game::opinion_of`
  makes; if that filter isn't already a named fn, extract one and have
  `opinion_of` call it (CLAUDE.md "mirrors must be a call").
- Tests per trigger with nearest miss (spec §5): diagonal fires / distance
  2 not; neutral neighbour neither; dark vs powered; `Starved` vs `Running`;
  one amenity present → no `NoAmenity`. Use a `dev-saves/` base template if
  one fits (`bonds` dev-save) rather than a fresh `Game::new`.

**T7. Schedule.** Insert between `idle_machine_system` and
`task_progress_system` in `build_schedule` (`game/lifecycle.rs:~713`).
Insert only; no reorder.

**T8. Both morale folds.**
- `Game::morale` (`game/memories.rs:483`): memory fold + `situations::sum`
  over the entity's `Situation` (absent = empty).
- `task_progress_system` (`systems.rs:~1400`): `CronjobWorker` query gains
  `Option<&Situation>`, `ThoughtDb` added to `CronjobLookups`;
  `CycleModifiers::morale` = same sum.
- `Game::load` and new-game construction: run `assess` once and insert
  `Situation`s before returning.
- Tests: `Game::morale(w)` equals the `CycleModifiers::morale` built for `w`
  on the same tick; `opinion_of`/`bond` unchanged with `BesideRival` active;
  no memories + every negative thought → not `Sulking` after the morale
  beat; −4 from memories + rival → `Sulking`; empty `assets/thoughts/` →
  morale equals memory fold; loaded save read before any tick includes the
  term. Mutation-check the equality test (break one fold, confirm red).

**Phase 2 gate:** `cargo test -p feral-processes-engine situations memories morale`,
`cargo test -p feral-processes-engine balance_sim` (expect unchanged).

## Phase 3 — the memories page

**T9. `Game::memory_report`** appends a `MemoryRow` per `scaled_rows` entry
(`subject: None`, `age: "now"`) before the existing sort. Test: rows sum to
`Game::morale`, including under the clamp.

**T10. GUI census** (`crates/gui/src/render/party.rs`, the worst-case
height/width census near :145): worst case grows by five thought rows;
use the longest shipped thought name/blurb. No layout change expected — if
the census fails, stop and report rather than shrink rows.

## Phase 4 — the seam and the docs

**T11. Seam update**, three writes in the `seams` skill's order: graph
(`seam:` entity for memories/morale — morale is now memory fold +
situational sum, one `Situation` read by both callers), `seams` skill
entry, and `.claude/rules/seams-memories.md` (the "morale is one addend"
and "`sum_intensity` is the fold" lines).

**T12. Docs.** `docs/superpowers/INDEX.md`: new row for this spec
(built, unplayed; path-pinned to `situations.rs`, `assets/thoughts/`), and
update the roadmap row (F built). CHANGELOG is written at release, not on
the branch.

## Final gate

`cargo test --workspace`, clippy clean, `balance_sim` unchanged. Whole-branch
review by opus with the diff as a file; ask the reviewer to re-derive the
clamp and the "one derivation" claim independently. Then a screenshot of
the memories page on the `bonds` dev-save (`--screenshot`) — a green suite
is not evidence of play.
