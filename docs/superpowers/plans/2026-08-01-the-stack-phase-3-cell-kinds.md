# The Stack phase 3 — cell kinds

Executes the phase 3 section of
`docs/superpowers/specs/2026-07-31-the-stack-design.md`. Read that first;
this file is the ordering and the gates, not a second copy of the design.

Deliberately short. The size rule in CLAUDE.md calls for a plan here — two
crates and a save-format change — but the reason a plan exists is to hand
context to something that lacks it. This is being executed in the session
that wrote it, so the plan carries what survives a compaction (what to build,
in what order, and what each test is *for*) and nothing that would merely be
re-emitted as code.

## Files

| File | Change |
| --- | --- |
| `crates/engine/src/stack.rs` | 3 `CellKind` variants; `place_breakpoint`, `place_faults`, `place_corruption`; bottom-frame guard on faults |
| `crates/engine/src/tuning.rs` | 7 new constants, in the existing Stack and Trace sections |
| `crates/engine/src/resources.rs` | `FrameMemory::jacked`, `#[serde(default)]` |
| `crates/engine/src/save.rs` | `SAVE_FORMAT_VERSION` 16 → 17 |
| `crates/engine/src/game/stack.rs` | `enter_frame` collapsing `descend_to`/`ascend_to`; fault handling in `step` |
| `crates/engine/src/game/stack_features.rs` | `trip_breakpoint`, `breakpoint_spent`, `bleed_corruption` |
| `crates/gui/src/render/stack.rs` | first-person faces for the 3 kinds |
| `crates/gui/src/render/` frame map | glyphs + colours, spent breakpoint greyed |

No app-core change — that is what cutting breakpoint's second option bought.

## Task order

Each task is a failing test first, then the code, then a commit. The order is
chosen so nothing is built on an untested foundation.

1. **`CellKind` variants + `walkable`/`blocks_sight`.** Test: the three new
   kinds are walkable and none blocks sight. This is the door-trap guard, and
   it is one line of test that would have caught the door bug.
2. **Placement passes.** Tests: each kind generates within its tuning count;
   two `generate` calls on one `FrameSpec` agree cell-for-cell; no new kind
   lands on a cache, link, door or lair; the cache count is unchanged by the
   new passes; the bottom frame has no faults; a corruption patch is
   contiguous. The contiguity and cache-count tests are the two that would
   otherwise pass vacuously while the feature was wrong.
3. **Save format.** `FrameMemory::jacked` + version bump. The existing
   `a_save_written_at_the_previous_version_is_refused` is written relative to
   the constant and needs no edit; confirm `dev-saves/extraction.ron` still
   loads (`stack_memory: ({})`, so it should).
4. **`enter_frame` refactor.** Pure refactor, no behaviour change — the
   existing Stack suite is the test, and it must pass untouched. **Invoke
   `design-patterns` before writing this**: it is a third variant of an
   existing pair, which is exactly the trigger.
5. **Breakpoint.** Tests: walking on marks every in-bounds cell seen; raises
   Trace by `TRACE_PER_BREAKPOINT`; is one-shot across leaving and re-entering
   the frame.
6. **Fault.** Tests: lands on `Floor`, not on the new frame's `LinkUp`;
   raises no Trace; depth increased by exactly 1.
7. **Corruption.** Tests: routes through `apply_damage` (assert via a
   Mitigation field buff changing the figure, which a direct `Stats::hp`
   write could not); damages the player and not party members; can reach
   `is_game_over`.
8. **gui.** No test beyond the existing render smoke tests; the glyph table
   stays one definition.

## Gates

- `cargo test --workspace` before calling it done, plus `cargo clippy
  --workspace` and `cargo fmt`.
- `cargo test -p feral-processes-engine balance_sim` — this adds an HP drain.
  A moved curve means progression changed; that is the signal, not a break.
- Iterate with `cargo test -p feral-processes-engine <name>`; the engine suite
  is ~3s.

## Merge gate

**This does not merge until it has been played.** Capture a `dev-saves/`
template a few frames down with Trace live and the new cells present, launch,
crawl it. The three questions the suite cannot answer: whether 40/100/180 are
the right band lines, whether a corruption patch is a decision or an
annoyance, and whether 25 Trace for a free map is a price anyone pays.

## Docs owed

README, CHANGELOG, the manual, and CLAUDE.md's load-bearing seams — the
"a Stack cell that can be used up needs both halves" entry gains the
breakpoint, and `walkable()`/`blocks_sight()` gains three kinds that are
walkable and sight-transparent. Update the spec's Status table on landing.
