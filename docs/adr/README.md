# Architecture decision records

This directory is where architecture and coding decisions are recorded and agreed between the user and Claude.
A decision is written here before code builds on it, so the user sees it first.

One file per decision, numbered in the order written, never renumbered.
Records are never deleted: a rejected idea stays on file so it is not re-proposed from scratch.
Backlogs and question lists never get a number; open cleanups live in [backlog.md](backlog.md).

## Statuses

| Status | Meaning |
| --- | --- |
| Proposed | Written, awaiting the user's OK. No code may build on it. |
| Agreed | The user approved it; not built yet. |
| Accepted | Landed. Names the release, or "standing" for a rule backfilled from `CLAUDE.md`. |
| Rejected | Turned down; the record says why. |
| Superseded | Replaced; links to the record that replaced it. |
| Withdrawn | Not an architecture decision; its content moved to [backlog.md](backlog.md). |

[0012](0012-open-questions-from-the-review.md) is marked Closed, a status used by no other record.
It predates the rule that question lists get no number and is kept so links to it hold.

## Record shape

Every record opens with a header list:

- `Status:` one of the statuses above.
- `Decided by:` `user`, `user (CLAUDE.md)`, `Claude — proposed, awaiting the user`, `Claude — proposed, never agreed` (superseded before the user ruled), or `user, on Claude's proposal`.
- `Date:` when it was written or decided.
- `Pattern:` the named design pattern it uses, or `—`.
- `Depends on:` other records, when there are any.

The body has five sections: Context, Decision, Alternatives considered, Consequences, Evidence.
Records from the first review that are Withdrawn, Superseded, Closed, or a list of items (0002, 0006, 0010, 0011, 0012) keep their original shape.
Context says what is true now and what it costs.
Decision gives the shape of the change, not its code.
Alternatives considered names the options turned down and why.
Consequences says what it unlocks and what it breaks.
Evidence cites symbols and paths (`render::ALL_MODES`, `Game::effective_atk`, `crates/gui/src/paint.rs`), never line numbers, which rot within days.

One decision per record.
Most records are 20 to 45 lines.

## When an ADR is required

Before writing code, any of these needs a record:

1. **Data or Rust.** A new `assets/<kind>/`, or new fields on existing `.ron` files.
2. **A new pattern or abstraction** not already in the pattern index below.
3. **Crate or module boundaries.** A new crate, a new subsystem directory, or code moving between crates.
4. **Bending a standing record.** A change that contradicts or stretches an Agreed or Accepted record.

Claude writes the record as Proposed and stops for the user's OK.
A spec links to the ADR rather than burying the decision in a bullet.

## Where things live

| Place | Holds |
| --- | --- |
| `CLAUDE.md` | The one-line rule, with a link to its ADR. |
| An ADR | The reasoning and the alternatives turned down. |
| `.claude/rules/`, the `seams` skill, memory graph `seam:<slug>` | The traps on one specific seam. |

Do not duplicate between them; point instead.

## Index

| ADR | Title | Status | Decided by | Pattern |
| --- | --- | --- | --- | --- |
| [0001](0001-engine-test-executor-matches-the-gate.md) | Engine test build uses the same executor as the gate | Agreed | user, on Claude's proposal | — |
| [0002](0002-no-debuginfo-for-dependencies.md) | No debuginfo for dependencies | Withdrawn | user | — |
| [0003](0003-headless-dev-bins-without-bevy.md) | Headless dev bins build without bevy | Rejected | user, on Claude's proposal | — |
| [0004](0004-content-loads-once-as-a-value.md) | Content loads once, as a value | Proposed | Claude — proposed, awaiting the user | Ports and adapters |
| [0005](0005-engine-queries-take-self-not-mut-self.md) | Engine queries take `&self` | Proposed | Claude — proposed, awaiting the user | Command-query separation |
| [0006](0006-per-screen-state-and-a-per-run-struct.md) | Per-screen state structs and a per-run struct on `App` | Superseded by 0013, 0014 | Claude — proposed, never agreed | — |
| [0007](0007-navigation-is-a-stack.md) | Back-navigation is a stack | Proposed | Claude — proposed, awaiting the user | Navigation stack |
| [0008](0008-shared-derivations-as-world-functions.md) | Shared derivations are functions of `&World` | Proposed | Claude — proposed, awaiting the user | Shared pure function |
| [0009](0009-layout-separate-from-paint.md) | Screen layout is separable from paint | Proposed | Claude — proposed, awaiting the user | Functional core, imperative shell |
| [0010](0010-hygiene-backlog.md) | Hygiene backlog | Superseded by backlog.md | Claude — proposed, never agreed | — |
| [0011](0011-restructurings-considered-and-rejected.md) | Restructurings considered and rejected | Accepted (2026-10-16) | user, on Claude's proposal | — |
| [0012](0012-open-questions-from-the-review.md) | Open questions from the review | Closed | user | — |
| [0013](0013-a-run-struct-on-app.md) | A `Run` struct on `App` | Proposed | Claude — proposed, awaiting the user | Aggregate with one lifetime |
| [0014](0014-per-screen-state-and-one-screen-table.md) | Per-screen state and one exhaustive screen table | Proposed | Claude — proposed, awaiting the user | State; table-driven dispatch |
| [0015](0015-engine-reads-no-environment.md) | The engine reads no environment variables | Proposed | Claude — proposed, awaiting the user | Parameter object |
| [0016](0016-shipped-assets-deny-unknown-fields.md) | Shipped asset kinds deny unknown fields | Agreed | user | — |
| [0017](0017-game-is-the-entire-api.md) | `Game` is the entire API | Accepted (standing) | user (CLAUDE.md) | Facade |
| [0018](0018-only-paint-names-a-graphics-library.md) | Only `paint.rs` names a graphics library | Accepted (standing) | user (CLAUDE.md) | Adapter (port) |
| [0019](0019-content-defaults-to-rust.md) | New content defaults to Rust | Accepted (standing) | user (CLAUDE.md) | — |
| [0020](0020-functional-core-imperative-shell.md) | Functional core, imperative shell for `impl Game` | Accepted (standing) | user (CLAUDE.md) | Functional core, imperative shell |
| [0021](0021-plain-rust-over-ecs-for-discrete-logic.md) | Plain Rust over ECS for discrete logic | Accepted (standing) | user (CLAUDE.md) | — |
| [0022](0022-rng-never-depends-on-query-order.md) | RNG never depends on query order | Accepted (standing) | user (CLAUDE.md) | Deterministic ordering |
| [0023](0023-saves-are-field-named-ron.md) | Saves are field-named RON | Accepted (standing) | user (CLAUDE.md) | Tolerant reader |
| [0024](0024-code-is-organized-by-domain.md) | Code is organized by domain | Accepted (standing) | user (CLAUDE.md) | Module per domain; Strategy |
| [0025](0025-the-debug-build-is-playable.md) | The debug build is playable | Accepted (standing) | user (CLAUDE.md) | — |
| [0026](0026-no-ci-manual-releases.md) | No CI; releases are manual per platform | Accepted (standing) | user (CLAUDE.md) | — |
| [0027](0027-refusals-are-typed-per-feature.md) | Refusals are typed per feature | Proposed | Claude — proposed, awaiting the user | Refusal enum |
| [0028](0028-announcements-fire-on-a-crossing.md) | Announcements fire on a crossing | Proposed | Claude — proposed, awaiting the user | Edge trigger |
| [0029](0029-player-routines-are-compiled-from-a-rust-part-catalogue.md) | Player routines are compiled from a Rust part catalogue | Proposed | Claude — proposed, awaiting the user | Builder |

## Pattern index

The design patterns the codebase uses, where each is decided, and one place to see it.
A pattern not listed here, or listed only as (Proposed), is not yet agreed: it needs an Agreed or Accepted ADR before code uses it.

| Pattern | ADRs | Example in code |
| --- | --- | --- |
| Facade | [0017](0017-game-is-the-entire-api.md) | `Game` in `crates/engine/src/lib.rs` |
| Adapter (port) | [0018](0018-only-paint-names-a-graphics-library.md) | `paint::Painter` in `crates/gui/src/paint.rs` |
| Ports and adapters (Proposed) | [0004](0004-content-loads-once-as-a-value.md) | none yet (proposed `Content`) |
| Functional core, imperative shell | [0020](0020-functional-core-imperative-shell.md), [0009](0009-layout-separate-from-paint.md) | `spawning::roll_summon_tier`, `render::popup::popup_layout` |
| Shared pure function ("mirrors must be a call") | [0008](0008-shared-derivations-as-world-functions.md), [0020](0020-functional-core-imperative-shell.md) | `power::ledger`, `battle::expected_damage` |
| Plain structs owned in one place | [0021](0021-plain-rust-over-ecs-for-discrete-logic.md) | `tactical::TacticalBattle` |
| Deterministic ordering | [0022](0022-rng-never-depends-on-query-order.md) | `TacticalBattle`'s `bodies` `Vec`; `components::ProgramId` |
| Tolerant reader | [0023](0023-saves-are-field-named-ron.md) | `save::SaveData`, `save::SAVE_FORMAT_VERSION` |
| Strategy (one impl per variant) | [0024](0024-code-is-organized-by-domain.md), [0014](0014-per-screen-state-and-one-screen-table.md) | `app::hub::HubKind`, `App::handle_hub_nav` |
| Module per domain | [0024](0024-code-is-organized-by-domain.md) | `crates/engine/src/tactical/`, `crates/engine/src/settlements/` |
| Loader that skips and warns | [0016](0016-shipped-assets-deny-unknown-fields.md), [0019](0019-content-defaults-to-rust.md) | `SpeciesDb::load_dir` |
| Command-query separation (Proposed) | [0005](0005-engine-queries-take-self-not-mut-self.md) | `World::try_query` in `Game` floor and farm reads |
| Parameter object (Proposed) | [0015](0015-engine-reads-no-environment.md) | none yet (proposed `DevFlags`) |
| Aggregate with one lifetime (Proposed) | [0013](0013-a-run-struct-on-app.md) | none yet (proposed `Run`) |
| State, data travels with the mode (Proposed) | [0014](0014-per-screen-state-and-one-screen-table.md) | `RigToolScreen`, `DepotFilterScreen` |
| Navigation stack (Proposed) | [0007](0007-navigation-is-a-stack.md) | none yet |
| Refusal enum (Proposed) | [0027](0027-refusals-are-typed-per-feature.md) | `SortieRefusal`, `ContractRefusal` |
| Edge trigger (Proposed) | [0028](0028-announcements-fire-on-a-crossing.md) | `Outpost::announced` (latch form) |
| Builder, parts compiled into a value (Proposed) | [0029](0029-player-routines-are-compiled-from-a-rust-part-catalogue.md) | none yet (proposed `routine_craft::compile`) |
