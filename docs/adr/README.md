# Architecture decision records

One file per structural decision, numbered in the order they were written, never renumbered.
A record is **Proposed** until the work lands, then **Accepted** with the release that carried it; a record overtaken by a later one is **Superseded** with a link.
Records are not deleted: a rejected idea stays on file so it is not re-proposed from scratch.

The first twelve records come from the 2026-10-16 architecture review of all four crates.
Each cites the source lines it was argued from at that date; the per-crate reports behind them were working files and are not in the repo.
Tiers below are the review's suggested order, not a priority the records themselves carry.

Where an ADR touches a load-bearing seam, the seam's own argument still lives in the memory graph as `seam:<slug>` and its rule in `.claude/rules/`; the ADR records the decision to change, not the seam's reasoning.

## Index

| ADR | Title | Status | Tier |
| --- | --- | --- | --- |
| [0001](0001-engine-test-executor-matches-the-gate.md) | Engine test build uses the same executor as the gate | Proposed | 1 |
| [0002](0002-no-debuginfo-for-dependencies.md) | No debuginfo for dependencies | Proposed | 1 |
| [0003](0003-headless-dev-bins-without-bevy.md) | Headless dev bins build without bevy | Proposed | 1 |
| [0004](0004-content-loads-once-as-a-value.md) | Content loads once, as a value | Proposed | 1 |
| [0005](0005-engine-queries-take-self-not-mut-self.md) | Engine queries take `&self` | Proposed | 2 |
| [0006](0006-per-screen-state-and-a-per-run-struct.md) | Per-screen state structs and a per-run struct on `App` | Proposed | 2 |
| [0007](0007-navigation-is-a-stack.md) | Back-navigation is a stack | Proposed | 2 |
| [0008](0008-shared-derivations-as-world-functions.md) | Shared derivations are functions of `&World` | Proposed | 2 |
| [0009](0009-layout-separate-from-paint.md) | Screen layout is separable from paint | Proposed | 2 |
| [0010](0010-hygiene-backlog.md) | Hygiene backlog | Proposed | 3 |
| [0011](0011-restructurings-considered-and-rejected.md) | Restructurings considered and rejected | Accepted | — |
| [0012](0012-open-questions-from-the-review.md) | Open questions from the review | Open | — |

## Adding a record

Copy the shape of an existing record: Status, Context, Decision, Consequences, Evidence.
Context says what is true now and why it costs something; Decision says the shape of the change, not its code; Consequences says what it unlocks and what it breaks.
Evidence is `path:line` at the time of writing — rot is expected and a stale line is still a better pointer than none.
