# Entity memories — Phase 5: The one hook

**Written after the build, not before it.** `CLAUDE.md`'s process-weight
rule puts this phase — one crate, no schema change, no save-format change,
one context — in the brainstorm-then-TDD-inline band rather than the
spec-and-plan band. A forward plan here would have been the feature written
twice. What is kept is the part that is worth reading later: what shipped,
what was decided, and the mutation table.

**Spec:** `docs/superpowers/specs/2026-08-21-entity-memories-design.md`,
section 8. This implements that section and nothing else, closing the
five-phase sequence.

## What shipped

- `tuning::MEMORY_AVOIDANCE_THRESHOLD: f32 = -3.0`, in the memories
  section beside the other four.
- One rejection in `Game::park_idle_staff`
  (`game/base/work_orders.rs`), third after the `Structure` tile and the
  unwalkable tile:
  `opinion_of(worker, BaseTile { x, y }) < MEMORY_AVOIDANCE_THRESHOLD`.
- `Game::opinion_of` loses the phase-2 `dead_code` expectation — this is
  its reader, and the attribute's stated reason was this phase.
- Five tests in `crates/engine/src/tests/memories.rs`.

No `.ron` change, no schema change, no `SAVE_FORMAT_VERSION` bump, no RNG
draw, nothing reaching `Stats`. `balance_sim` is untouched and its curves
did not move.

## Decisions

**-3.0, and not pinned to `stranded_at`'s valence.** The hook asks whether
a program holds anything against a tile, not whether one particular def is
in its store, so a second negative `BaseTile` def must be able to trip it
without editing the constant. At the shipped def — valence -6.0, half-life
3000 — one stranding keeps a program off that tile for exactly one
half-life, and a second inside that window roughly doubles the reach.

**Signed, not a magnitude.** A fondness must never be able to trigger an
avoidance, and the comparison is what makes that structural rather than
something each future def has to be careful about.

**`opinion_of`, never `morale`.** The sum over everything would keep a
program that has had a bad run off every tile in the base at once, which
reads as the parking ring being broken.

The argument for all three, and for why the hook is `park_idle_staff`
rather than `schedule_base_labour`, is in `docs/seams.md` under
**The one hook is `park_idle_staff`, and it is a third rejection rather
than a score**.

## Mutation table

Every test was proved by deleting its fix, running the test, watching it
fail, and restoring. `M3` is the entry that did **not** hold on the first
attempt — the test was hollow, and rewriting it is part of what shipped.

| # | Mutation | Test that failed |
|---|---|---|
| M1 | The rejection deleted outright | `a_program_is_not_parked_on_a_tile_it_holds_a_grudge_against` |
| M2 | `opinion_of` → `morale` | `a_grudge_against_another_tile_does_not_move_a_program` |
| M3 | Threshold → a bare `< 0.0` (a flag, not a threshold) | `a_faded_grudge_stops_keeping_a_program_away` |
| M4 | Store read directly, unweighed, instead of through `opinion_of` | `an_empty_database_leaves_the_parking_hook_inert` **and** M3's test |

`a_program_with_no_grudge_is_parked_on_that_same_tile` is the control the
other four are worthless without: it is what fails if the rejection fires
on every candidate, or if `park_idle_staff` is broken outright, and it is
what pins the tile the sibling tests predict.

### M3 did not hold at first, and that is the finding

`a_faded_grudge_stops_keeping_a_program_away` advances the clock to fade
the memory and then asserts the body parks on the tile anyway. Advancing
the clock also **steps the parking ring**, so the first version measured a
tile the grudge had never been about — a second copy of M2's test, green
against a threshold replaced by a bare `< 0.0`. The suite never saw it; the
mutation pass did.

It now advances by a whole number of ring periods
(`IDLE_STAFF_STEP_TICKS * 8 * IDLE_STAFF_RING_TILES`), reads the half-life
off the def rather than hardcoding it, and asserts as a **precondition**
that the ring is still offering the tile the grudge is about — so a retuned
ring or step fails the test instead of quietly emptying it.

## Gates

`cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test
--workspace` — all green. The seven clippy warnings the workspace carries
were verified present on `origin/main` and are not this phase's.
