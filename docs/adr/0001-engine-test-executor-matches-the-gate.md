# 0001 Engine test build uses the same executor as the gate

Status: Proposed (2026-10-16)

## Context

`cargo test -p feral-processes-engine <name>` is the documented iteration command and `cargo test --workspace` is the gate.
They do not build the same engine.
`cargo tree -e features` shows `bevy_ecs` gets the `multi_threaded` feature only under `--workspace`, where `bevy` (via `crates/gui`) enables it; the engine's own `Cargo.toml` asks for `bevy_ecs = "0.19.0"` with defaults only.

bevy_ecs 0.19's `default_executor()` returns `MultiThreadedExecutor` only when `multi_threaded` is on and `SingleThreadedExecutor` otherwise.
`build_schedule` in `crates/engine/src/game/lifecycle.rs:751` uses `Schedule::default()` and nothing in the workspace sets `ExecutorKind`.
So the iteration command runs the schedule single-threaded, and the gate and the shipped game run it multi-threaded.

Two costs follow.
Switching between the two commands rebuilds the engine and its ~330k-line test crate because the feature set changed.
And systems with no explicit ordering can interleave differently under the two executors, which is a credible — untested — root cause for the recorded "single-crate run shifts the RNG stream" and the tests that flake only in the full suite.

## Decision

Declare on the engine's `bevy_ecs` dependency the union of features the workspace build enables, so `-p` and `--workspace` compile one engine.
Set `ExecutorKind` explicitly in `build_schedule` rather than inheriting the feature-dependent default.

Before changing the default, test the flake hypothesis: pin `ExecutorKind::SingleThreaded` under `--workspace` and re-run the four known suite-only flakes thirty times each.

## Consequences

One engine build for both commands; no rebuild when switching.
If the hypothesis holds, the known flake family has a cause and the "sort in the drawing system, not the seed budget" workaround can be revisited.
Pinning a single-threaded executor would cost simulation throughput in the shipped game; pinning multi-threaded keeps the shipped behaviour and makes the iteration command match it.

## Evidence

- `crates/engine/Cargo.toml` — `bevy_ecs = "0.19.0"`, no features.
- `crates/gui/Cargo.toml` — `bevy` with `multi_threaded`.
- `crates/engine/src/game/lifecycle.rs:751` — `Schedule::default()`.
- bevy_ecs 0.19.0 `src/schedule/executor/mod.rs:45-66` — `default_executor`.
