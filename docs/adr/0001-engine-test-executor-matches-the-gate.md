# 0001 Engine test build uses the same executor as the gate

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-10-16 (agreed 2026-10-21)
- Pattern: —

## Context

`cargo test -p feral-processes-engine <name>` is the documented iteration command and `cargo test --workspace` is the gate.
They do not build the same engine.
`bevy_ecs` gets the `multi_threaded` feature only under `--workspace`, where `bevy` (via `crates/gui`) enables it; the engine's own `Cargo.toml` asks for `bevy_ecs = "0.19.0"` with defaults only.

bevy_ecs 0.19's `default_executor()` returns `MultiThreadedExecutor` only when `multi_threaded` is on.
`build_schedule` in `crates/engine/src/game/lifecycle.rs` uses `Schedule::default()` and nothing sets `ExecutorKind`.
So the iteration command runs the schedule single-threaded, and the gate and the shipped game run it multi-threaded.

Switching between the two commands rebuilds the engine and its test crate because the feature set changed.
And systems with no explicit ordering can interleave differently under the two executors, which is a credible, untested cause of the tests that flake only in the full suite.

## Decision

Declare on the engine's `bevy_ecs` dependency the union of features the workspace build enables, so `-p` and `--workspace` compile one engine.
Set `ExecutorKind` explicitly in `build_schedule` rather than inheriting the feature-dependent default.

Settle the suite-only flakes by reading each flaky test's code path for a dependence on system order or threading, not by re-running it.
`CLAUDE.md` forbids brute-forcing a flake with repeated runs.
The four known ones are `posted-worker level-up`, `research underground`, `creation allowance` and `sharper-program mining`.
This absorbs question (b) of [0012](0012-open-questions-from-the-review.md).

## Alternatives considered

- Pin `ExecutorKind::SingleThreaded` everywhere. Matches the iteration command, but costs simulation throughput in the shipped game.
- Leave the builds different and document it. Keeps the rebuild on every switch and the flake family unexplained.
- Run each flake thirty times under a pinned executor. Rejected as brute force; it says whether, not why.

## Consequences

One engine build for both commands; no rebuild when switching.
Pinning multi-threaded keeps the shipped behaviour and makes the iteration command match it.
A flake traced to an unordered system pair gets an explicit ordering, not a seed change.

## Evidence

- `crates/engine/Cargo.toml` — `bevy_ecs = "0.19.0"`, no features.
- `crates/gui/Cargo.toml` — `bevy` with `multi_threaded`.
- `crates/engine/src/game/lifecycle.rs` — `build_schedule`, `Schedule::default()`.
- bevy_ecs 0.19.0 `schedule::executor::default_executor`.
- Memory: `single-crate-engine-test-runs-a-different-executor`.
