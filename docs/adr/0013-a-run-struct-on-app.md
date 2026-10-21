# 0013 A `Run` struct on `App`

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-21
- Pattern: Aggregate with one lifetime

## Context

`App` (`crates/app-core/src/lib.rs`) holds session state — settings, menus, telemetry opt-in — and run state — the `Game`, the camera, pending choices — side by side.
Nothing ties the run fields to the run's lifetime.
`App::leave_run` clears `game`, `status_line` and `mode` and nothing else; `App::install_game` resets a handful of fields by hand, each with a comment explaining why that one.
`watching: Option<Entity>` is never cleared by either, so after quitting and loading another save the camera can follow whatever entity now holds that id.
About 180 `let Some(game) = &self.game` unwraps exist because the run is optional on the same struct as the session.

Split out of [0006](0006-per-screen-state-and-a-per-run-struct.md), part (a).

## Decision

Move every field whose meaning is tied to one run into a `Run` struct that owns the `Game`.
`App` holds `run: Option<Run>`; `leave_run` sets it to `None` and `install_game` builds a fresh `Run`.
A field is a run field if a stale value would be wrong in the next run; the `install_game` reset comments are the starting list.

## Alternatives considered

- Keep the fields and add each to `leave_run` and `install_game`. The current approach; the next run field is forgotten the way `watching` was.
- A `reset_run_state()` method listing the fields. The same list in a third place, with no compiler check.

## Consequences

A run handle cannot outlive its run, so the stale-`watching` class of bug is closed by construction.
The unwraps become one `Option<Run>`.
Large mechanical change across app-core and its tests; it can move fields over a few at a time, `Game` first.
The `watching` bug is fixed first on its own, with a failing reproducer ([backlog.md](backlog.md)).

## Evidence

- `crates/app-core/src/app/lifecycle.rs` — `App::leave_run`, `App::install_game`.
- `crates/app-core/src/lib.rs` — `App`, `watching`.
- `crates/app-core/src/app/inspection.rs` — `start_watching`.
