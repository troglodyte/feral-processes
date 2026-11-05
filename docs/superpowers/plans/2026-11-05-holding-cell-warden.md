# Holding Cell warden — plan

Spec: `docs/superpowers/specs/2026-11-05-holding-cell-warden-design.md` (read it
first; the decisions there are the user's and are settled).
Worktree: `.claude/worktrees/holding-cell-warden`, branch
`worktree-holding-cell-warden`. Rules to load first:
`.claude/rules/seams-base.md` (scheduler priority = list position; "A Teardown
Rig staffs itself").

**Starting state:** `crates/engine/src/tests/prison.rs` has three
**uncommitted, failing** tests (`an_occupied_cell_is_warded_with_no_standing_job`,
`an_empty_cell_is_not_warded`, `a_prisoner_gets_an_attempt_with_nobody_posted_by_hand`)
plus helpers `a_lit_cell_with_a_spare_body` and `post_of`. Keep them and commit
them with Phase 1's green step.

Three phases, one implementer, sequential. Commit at each green step. Never
push, never `git checkout`/`stash`/`reset` uncommitted work.

## Phase 1 — engine: the cell is a want (TDD)

Files: `crates/engine/src/game/base/prison.rs`,
`crates/engine/src/game/base/work_orders.rs`, `crates/engine/src/tests/prison.rs`.

1. `prison.rs`: extract the room check from `break_down_prisoner` into
   `fn breakdown_fits(&self, cell, prisoner) -> bool`. Read the record with
   `Jailed::record` directly. **Do not call `prisoner_record`**, because it is
   `&mut` and rebuilds the record, and the rebuild rolls a condition. With no
   record (a pre-`record` save) it returns `true`, as `break_down_prisoner`
   already does today. `break_down_prisoner` must call this check, not carry
   its own copy.
2. `prison.rs`: add `pub(crate) fn cell_wants_a_warden(&self, cell) -> bool`.
   It is true when there is a prisoner and either the prisoner is not spent
   (`!prisoner_is_spent`) or `breakdown_fits` passes. Make `holding_cells`
   `pub(crate)`.
3. `work_orders.rs::base_wants`: add the cell loop right after the Teardown
   Rig loop, copying its shape (`!wanted.iter().any(..)` dedupe), and give it
   a comment explaining why it sits at that priority. The existing `dark`
   filter already removes unpowered cells.
4. New tests in `tests/prison.rs`:
   - `a_dark_cell_is_not_warded`: no `stand_ample_grid_supply`, then
     `schedule_base_labour` posts nobody.
   - `a_spent_prisoner_with_no_output_room_frees_its_warden`: set
     `attempts = JAIL_MAX_ATTEMPTS` and fill the cell's `Stock::output` so the
     payout can't fit, and nobody is posted. Empty the output, schedule again,
     and the body is posted. Use the helper the existing clogged-breakdown
     test uses to fill the output (`rg -n Clogged tests/prison.rs`).
5. Gate: `cargo test -p feral-processes-engine prison` and
   `cargo test -p feral-processes-engine racks` both pass. Mutation check:
   comment out the new `base_wants` loop and confirm all four scheduler tests
   fail, then restore it. **Commit.**

## Phase 2 — the view, the sheet, the copy

1. `crates/engine/src/views/base.rs::PrisonerState`: add
   `progress_fraction: f32` (`progress / attempt_ticks`, 0 when spent). Drop
   `progress` and `attempt_ticks` if, after the sheet change, only the gui
   test fixture reads them (`rg -n 'attempt_ticks|\.progress\b' crates/gui
   crates/app-core`). Fill the field in `prison.rs::prison_state`. Engine
   test: 0 on a fresh attempt, and in (0, 1) after a few warded ticks.
2. Bar helper: move `bar(fraction)` and `BAR_CELLS` from
   `crates/gui/src/render/base_report/staff.rs` to a small shared home that
   both `base_report` and `building` can import. Put it in
   `render/base_report/` if `building` can already reach that; otherwise
   `render/meter.rs`. Leave the staff page's output unchanged: its tests at
   `staff.rs:232` must still pass without edits.
3. `crates/gui/src/render/building/structure_sheet.rs` (around lines
   229-251):
   - The clock line becomes `attempt {attempts+1} of {max}  {bar}  {odds}%
     chance`. When spent, it stays `attempts N/N  waiting for room in the
     output`.
   - The no-warden line becomes `waiting for a free worker` (YELLOW). Show it
     only when there is a prisoner, `!warded`, and `s.status !=
     Some(MachineStatus::Unpowered)`.
   - Update `crates/gui/src/render/building/tests/structure_sheet.rs` (the
     fixture at line 248 and any asserts on the old strings). Add tests that
     the line is absent for an unpowered cell and for an empty cell.
4. Copy. Use the exact replacement text from the spec's table:
   `assets/structures/holding_cell.ron`, `assets/research/containment.ron`,
   `assets/help/75-extraction.md`, `crates/engine/src/notifications.rs`
   (`ProgramDecompiled`), `assets/structures/README.md` (`holds_prisoner`),
   `assets/items/reinitialization_protocol.ron`. After editing, run
   `rg -i 'post(ed)? (a )?warden|needs a warden|no warden' assets crates
   --glob '!**/tests/**'`. It must find no player-facing hits.
5. Gate: `cargo test -p feral-processes-engine prison`,
   `cargo test -p feral-processes-gui structure_sheet`, the staff tests, and
   any asset or help census test that pins description text (run
   `cargo test --workspace` once here if unsure). **Commit.**

## Phase 3 — seam, changelog, final gate

1. Seam, three writes in the order the `seams` skill gives:
   - Amend the graph seam behind "A Teardown Rig staffs itself" so it names
     the cell as well (`memory_search("Teardown Rig staffs itself",
     subsystem:"seams")`).
   - Update the skill's trap for it: tests that hand-place the warden's `Task`
     hid a cell that never got a warden for three minor versions.
   - In `.claude/rules/seams-base.md`, add the Holding Cell to that rule's
     line. Don't add a new bullet: the cell uses `cell_wants_a_warden`, which
     is the cell's version of `can_progress`.
2. `CHANGELOG.md`: add an unversioned entry at the top in plain language. The
   Holding Cell never made a decompile attempt, because no worker could be
   sent to it. It now takes a free worker on its own while it has power and a
   prisoner. The sheet shows a progress bar, and the help and descriptions
   have been fixed. Saves load unchanged. (The release bump happens at deploy,
   not here.)
3. `docs/superpowers/INDEX.md`: add a row linking this spec and plan, and
   amend the `2026-10-10-holding-cell` row to say "warden scheduling fixed
   here".
4. Final gate: `cargo fmt`, `cargo clippy --workspace --all-targets` with no
   warnings, `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES` first). If
   anything fails, read the code path; don't rerun to brute-force a flake.
   **Commit.**
5. Final whole-branch review (opus), with the diff given as a file. Ask it to
   check: the priority position against the seams-base rules; that
   `breakdown_fits` is one call shared by both users; that the cell code never
   rebuilds a record and draws no RNG; and that the copy is accurate against
   `prison.rs`. Then stop for the user to deploy.
