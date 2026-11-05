# Holding Cell staffs its own warden, and its copy says so — design

**Date:** 2026-11-05

## Intent

Since 0.18.10 a Holding Cell has never made a decompile attempt in play. A
prisoner goes in, the cell waits for a warden, and none ever comes.

- The labour scheduler posts a body only where `base_wants`
  (`crates/engine/src/game/base/work_orders.rs`) lists a want. Nothing in that
  list names a cell.
- A standing work job would add one, but it is filtered by `can_progress`.
  That returns false for a cell: it has no `Hopper`, no recipe and no product.
- `cell_has_warden` (`game/base/prison.rs`) is therefore never true, and
  `step_holding_cell` returns before it advances anything.
- The tests never caught this. Every warded fixture inserts the warden's `Task`
  by hand or calls the test-only `assign_cronjob`
  (`tests/support/base_fixtures.rs`).

The player-facing copy also tells the player to "post a warden". There is no
way to do that, and after this fix they won't need to.

## Goals

1. **A cell staffs itself, the way a Teardown Rig does.** A lit cell that holds
   a prisoner is a want with no standing job set.
2. **The cell lets its body go when it can do nothing.** That covers three
   cases: the cell is empty, the cell is dark, or the prisoner is spent and the
   breakdown has no output room to pay into. This mirrors the rig's
   `can_progress` gate. The body comes back once the player collects with `c`.
3. **The copy is accurate.** No text tells the player to post a warden, no
   raw beat counts are shown, and the cell's power need is stated.

## Non-goals

- No change to the odds, the attempt count, the salvage or the pin flow.
- No change to the save format. `Jailed` and `Task` are already saved.
- No new `.ron` fields. The asset edits change description strings only, so
  no ADR is needed.

## Design

### Engine: the want

`base_wants` gets a cell block right after the Teardown Rig block (the user confirmed this priority). Both are
self-staffing jobs, so both sit at a standing job's priority: below builds,
fuel, research and orders, and above dig and farm wants. Priority is a want's
position in that list, so the placement is the whole decision.

```rust
for cell in self.holding_cells() {
    if !wanted.iter().any(|&(e, _)| e == cell) && self.cell_wants_a_warden(cell) {
        wanted.push((cell, TaskKind::GatherResource));
    }
}
```

`holding_cells()` is already in tile order, so the RNG and query order are
unaffected. It becomes `pub(crate)`.

The new `Game::cell_wants_a_warden(cell)` in `prison.rs` is true when the cell
holds a prisoner and either of these holds:

- the prisoner is not spent, or
- the prisoner is spent and the cell's output has room for its breakdown.

The breakdown's room check is pulled out of `break_down_prisoner` into a
`&self` predicate that both of them call. They must not each carry their own
copy, per CLAUDE.md's "a call, not a copy" rule.

Dark cells are already removed by `base_wants`' existing `dark` filter, which
applies to every `GatherResource` want.

`can_progress` is not widened. Doing that would route cells through the
standing-job path and its stock arithmetic, which says nothing about a
prisoner.

A seated warden is held across ticks by the scheduler's existing `held` diff.
No new component is added.

### GUI: the structure sheet (`render/building/structure_sheet.rs`)

- **Clock line.** It reads `attempts 2/5  beats 17/40  next roll 34%`, and
  "beats N/M" is a raw tick count. The user chose a progress bar to replace
  it, so the line becomes `attempt 3 of 5  ████░░░░░░  34% chance`. The bar
  reuses the text `bar(fraction)` helper in `base_report/staff.rs`, lifted to
  a module both screens can import rather than copied. The engine supplies the
  fraction as a new `PrisonerState::progress_fraction`, because under the
  tick-vocabulary rule the engine does any banding. `progress` and
  `attempt_ticks` leave the view if nothing else reads them.
- **No-warden line.** It reads `no warden — nothing is decompiling`. It becomes
  `waiting for a free worker`, and it is shown only when the cell holds a
  prisoner, is not `Unpowered`, and has no warden. A dark cell already gets the
  unpowered stall line, and an empty cell needs nobody.

### Copy

| File | Now | Becomes |
|---|---|---|
| `assets/structures/holding_cell.ron` description | "…boot one as a prisoner; a posted warden works at decompiling it." | "A 2x2 pen for a downed program. Stand beside it and spend a Reinitialization Protocol to boot one as a prisoner. While the cell has power, a free worker comes to decompile it." |
| `assets/research/containment.ron` description | "…Needs a warden." | "…Needs power and a free worker." |
| `assets/help/75-extraction.md` ¶ | "Post a worker to the cell as its warden and it makes a decompile attempt at intervals…" | "While the cell has power, a free worker comes over as its warden and makes a decompile attempt at intervals, each failure making the next likelier. A success joins it to your roster. After five failures it breaks down and the cell is left holding a little salvage to collect with `c`. A cell holds one prisoner at a time." |
| `crates/engine/src/notifications.rs` `ProgramDecompiled` body | "…A warden kept at the Holding Cell until the decompile took." | "…Its warden kept at it in the Holding Cell until the decompile took." |
| `assets/structures/README.md` `holds_prisoner` | "a posted warden works at decompiling it … `attempt_ticks` is the warden beats…" | "a warden is staffed on its own while the cell has power and a prisoner … `attempt_ticks` is the warded, powered ticks between one roll and the next" (this README is for modders, so "ticks" is allowed here) |

Also (user confirmed): the Reinitialization Protocol's description
(`assets/items/reinitialization_protocol.ron`) becomes "Boots a downed program
as a prisoner in a Holding Cell you stand beside."

The copy audit confirmed the rest of the cell's text is accurate: the refusal
fragments, the log lines, the "Hold a program" row and page, the picker and
the key hints. No text describes the old "reinitialize anywhere" behaviour.

## Tests (engine, `tests/prison.rs`)

The first three are already written in the worktree and fail on `main`:

- `an_occupied_cell_is_warded_with_no_standing_job`: one `schedule_base_labour`
  call posts the spare body to the cell.
- `an_empty_cell_is_not_warded`
- `a_prisoner_gets_an_attempt_with_nobody_posted_by_hand`: the whole loop runs
  on real ticks, with no `Task` inserted by hand.

To add:

- `a_dark_cell_is_not_warded`
- `a_spent_prisoner_with_no_output_room_frees_its_warden`, and the warden comes
  back once the output is emptied.
- A view test: `progress_fraction` is 0 at a fresh attempt and under 1 just
  before a roll.
- A GUI line test: the no-free-worker line is absent when the cell is unpowered
  and absent when the cell is empty.

Mutation check: remove the new block from `base_wants`. The three new
scheduler tests must fail.

## Gates

- `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo fmt`.
- `balance_sim` is not touched, because the base is not simulated there.
- Seam: add one line to `.claude/rules/seams-base.md` ("a Holding Cell, like a
  Teardown Rig, staffs itself") and amend the rig's graph seam to name both.
- CHANGELOG entry: the Holding Cell now actually decompiles. A worker comes on
  its own.
