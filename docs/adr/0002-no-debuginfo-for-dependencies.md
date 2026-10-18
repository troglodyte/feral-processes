# 0002 No debuginfo for dependencies

Status: Proposed (2026-10-16)

## Context

`[profile.dev.package."*"]` in the root `Cargo.toml` sets `opt-level = 3` for dependencies and nothing else, so every dependency is compiled with full debuginfo.
On 2026-10-16 `target/` was 369G (`deps` 195G, `incremental` 172G), the debug game binary 1.78GB, each dev bin about 330MB and the engine test binary about 500MB.
Each worktree builds its own `target/`, so the cost is per branch.

The `[profile.dev]` block itself must stay; `.claude/rules/launcher-and-build.md` records why a debug build has to be playable.

## Decision

Add `debug = false` (or `debug = "line-tables-only"` if backtraces through dependencies are wanted) to `[profile.dev.package."*"]`.
Leave `[profile.dev]` untouched; our four crates keep full debuginfo.

## Consequences

Smaller binaries, faster links, and a `target/` that fits more worktrees.
Backtraces inside bevy, egui or wgpu frames lose line numbers; frames in our crates keep them.
`gdb` sessions on a dependency crash become harder — `line-tables-only` is the middle setting if that matters.

## Evidence

- `Cargo.toml` — `[profile.dev.package."*"]` block.
- `du -sh target` on 2026-10-16: 369G.
