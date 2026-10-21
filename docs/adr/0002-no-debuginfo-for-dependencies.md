# 0002 No debuginfo for dependencies

- Status: Withdrawn
- Decided by: user
- Date: 2026-10-16 (withdrawn 2026-10-21)
- Pattern: —

Withdrawn: a one-line build setting, not an architecture decision.
The item is in [backlog.md](backlog.md).

## Context

`[profile.dev.package."*"]` in the root `Cargo.toml` sets `opt-level = 3` for dependencies and nothing else, so every dependency is compiled with full debuginfo.
On 2026-10-16 `target/` was 369G, the debug game binary 1.78GB and the engine test binary about 500MB.
Each worktree builds its own `target/`, so the cost is per branch.

## Decision

Add `debug = false`, or `debug = "line-tables-only"`, to `[profile.dev.package."*"]`.
Leave `[profile.dev]` untouched ([0025](0025-the-debug-build-is-playable.md)).

## Consequences

Smaller binaries and faster links; backtraces inside bevy, egui or wgpu lose line numbers.

## Evidence

- `Cargo.toml` — `[profile.dev.package."*"]`.
