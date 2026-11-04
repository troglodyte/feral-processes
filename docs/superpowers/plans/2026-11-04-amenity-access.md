# Amenity access: plan

Spec: `../specs/2026-11-04-amenity-access-design.md`. Worktree:
`.claude/worktrees/amenity-access` (branch `amenity-access`). Two crates (engine, gui).
TDD, one commit per green step. Unset `FERAL_DEV_NO_SIEGES` before testing.
Before task 1, read `.claude/rules/seams-base.md` and `seams-instrumentation.md`.

## Phase A: engine (one context)

**A1. Ledger tally.** `base_ledger.rs`:
- Add `pub enum VisitOutcome { Served, Waited, Stranded }`.
- Add `VisitTally { served, waited, stranded: u32 }`, with Serialize/Default and a
  `blocked()` helper.
- Add `Event::AmenityVisit { need: NeedId, outcome }`.
- Add `Bucket::visits: BTreeMap<NeedId, VisitTally>` with `#[serde(default)]`.
- Fold it in `fold`.
- Add `BaseLedger::recent_visits(&self, need, now_tick) -> VisitTally`, summing the
  last `tuning::AMENITY_ACCESS_WINDOW_BUCKETS` (new const = 3, in `tuning/staff.rs`
  or wherever `AMENITY_CAPACITY`'s siblings live, re-exported).
- Telemetry: `Record` variant optional. Skip it if `emit` doesn't need one.

Tests: fold adds to the right bucket; the window excludes a bucket 3+ back; a ledger
saved before this field loads (a RON without `visits`).

**A2. Record visits.**
- `components::OffShift { need, waited: bool }`. Fix every constructor (the 3 sites
  in `rg "OffShift \{"`, tests included). The save keeps writing `off_shift: Option<NeedId>`
  only, and load builds `waited: false`.
- `systems.rs` allocation (~239): after `allocate`, for each staffer with
  `OffShift(need)` that is in reach (`offshift::in_reach`) of a site servicing
  `need` and is not granted a place for it, set `waited = true`. The `programs`
  query needs `Option<&mut OffShift>`. Check the borrow against the later
  `get_mut` loop.
- `offshift.rs` ~538: on `(Some(_), None)`, emit `AmenityVisit` (`Waited` if the flag
  is set, else `Served`). On `(Some(cur), Some(need))` with the same need, **don't
  re-insert** (that would reset `waited`). Only insert when the need changes.
- `strand_off_shift`: emit `Stranded` before removing.
- Emit through `base_ledger::emit` (the Game-side door). Draw no RNG.

Tests (`tests/amenity_capacity.rs` or a new `tests/amenity_access.rs`, using
`tests/support` fixtures and the `respite.rs` helpers `a_base_with_an_amenity`,
`box_in_the_amenity`):
- 1 staffer reaching `content` → 1 served.
- 3 critical staffers at one Sandbox (2 places) → ≥1 waited.
- an unreachable Sandbox (respite.rs:339 shape, with OffShift) → stranded.
- A repeated beat for the same need keeps `waited`.
- Mutation-check the re-insert guard.

**A3. Coverage view.** `views/base.rs` `CoverageRow`:
- Drop `needed`.
- Add `visits`, `waited`, `stranded` (from `recent_visits`).
- `is_short` per the spec.
- `staff_report.rs`: delete the `needed` computation. Keep `off_shift_share` if
  `lost_share` still uses it.
- Fix `tests/staff_report.rs:103,116` to the new meaning: a base with enough
  amenities but a waited visit is short, and one with no blocks is not.

**A4. Alert.**
- `alerts.rs`: append `AlertKind::AmenityBlocked`. Add the unsaved
  `AlertBoard::amenity_blocked: BTreeSet<NeedId>` latch.
- Post and clear at the visit-emit point (one helper `Game::note_amenity_visit(need, outcome, site)`
  that A2's two sites call, so the emit and the alert can't drift).
- Text resolves the structure name at `site`: the tile it waited at
  (`Needs::serviced_at` is None, so pass the `route` target), or the
  routed target for a strand.
- Wire the gui's alert view if `AlertKind` is matched exhaustively there
  (`rg "AlertKind::" crates/gui`).
- Tests: the first block posts; a second while latched doesn't add a row;
  clean visits after the window empties clear the latch; a block after that posts again.

Gate A: `cargo test -p feral-processes-engine`, `balance_sim` unchanged, clippy, fmt.

## Phase B: gui + docs

**B1.** `gui/src/render/base_report/staff.rs` `coverage_rows`:
- New format and the cause lines per the spec.
- Update the tests at 221–344 (the `covered(built, needed, …)` helper takes access fields instead).
- Keep `the_widest_coverage_row_fits_the_content_rect` passing, with the worst
  case using 3-digit visits.

**B2.**
- `assets/structures/README.md` / `assets/needs/README.md`: only if they mention "needed".
- Add one line to `.claude/rules/seams-base.md`: **"Amenity access is a visit
  tally through `base_ledger::emit`, and Coverage's `!` reads it; `needed` no longer exists."**
- Add the matching trap to the `seams` skill and `seam:amenity-access` in the graph (three writes).
- `rg -i "needed" docs/ assets/` for stale Coverage copy.

Gate B: `cargo test --workspace`, clippy `--all-targets`, fmt. Then the opus
whole-branch review (per-task gates off). Then the CHANGELOG entry at deploy (patch:
no save break).
