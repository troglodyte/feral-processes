# 0003 Headless dev bins build without bevy

- Status: Rejected
- Decided by: user, on Claude's proposal
- Date: 2026-10-16 (rejected 2026-10-21)
- Pattern: —

## Context

`crates/launcher` depends on `feral-processes-gui` unconditionally, but only `src/main.rs` uses it.
The other five bins — `arena`, `bench`, `tuner`, `savetool`, `train` — are headless, yet each pays the cold bevy build in a fresh worktree.

## Decision

Proposed: move the headless bins and their modules to a `crates/devtools` crate that depends on engine and app-core only.

Rejected, for three reasons:

- The saving is small. `cargo test --workspace` is the gate and builds bevy anyway, so a worktree that runs the gate pays the cold build regardless.
- The logic is already in the engine (`crates/engine/src/bench/`, `crates/engine/src/arena/`); the launcher bins are thin entry points.
- A fifth crate adds a boundary to keep honest for no change in what can be tested.

## Alternatives considered

- An optional gui dependency with `required-features` on `main`. Makes `cargo run` need a feature flag, which breaks the documented command.

## Consequences

The launcher keeps all six bins.
`.claude/rules/launcher-and-build.md` still says "three bins"; that is stale and is in [backlog.md](backlog.md).

## Evidence

- `crates/launcher/Cargo.toml` — gui dependency, six `[[bin]]` targets.
- `crates/engine/src/bench/`, `crates/engine/src/arena/` — where the logic lives.
