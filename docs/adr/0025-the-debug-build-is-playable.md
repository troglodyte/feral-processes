# 0025 The debug build is playable

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: in `CLAUDE.md` by 2026-09-08; recorded here 2026-10-21
- Pattern: —

## Context

`cargo run` builds debug, and the user plays from it.
An unoptimized bevy, egui and wgpu stack is about 25 times slower per frame (`docs/measurements/2026-08-19-debug-build-frame-cost.md`), which reads as an animation bug rather than a build setting, and no test catches it.

## Decision

`[profile.dev]` in the root `Cargo.toml` stays: dependencies at `opt-level = 3`, workspace crates at `1`.
Never delete it.

## Alternatives considered

- Play from `--release` only. Slower to rebuild for every change, and `cargo run` would be the wrong command.
- Optimize only bevy's crates. More configuration for the same result as `package."*"`.

## Consequences

A cold build costs more; warm builds stay fast because only touched workspace crates rebuild.
Dependency debuginfo is a separate setting, in [backlog.md](backlog.md).

## Evidence

- `Cargo.toml` — `[profile.dev]`, `[profile.dev.package."*"]`.
- `docs/measurements/2026-08-19-debug-build-frame-cost.md`.
- `.claude/rules/launcher-and-build.md`.
