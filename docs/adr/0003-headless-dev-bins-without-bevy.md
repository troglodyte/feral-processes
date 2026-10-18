# 0003 Headless dev bins build without bevy

Status: Proposed (2026-10-16)

## Context

`crates/launcher` depends on `feral-processes-gui` unconditionally, but only `src/main.rs` uses it.
The other five bins — `arena`, `bench`, `tuner`, `savetool`, `train` — and the launcher's own tests are headless, yet each pays the ~3.5-minute cold bevy build in every fresh worktree.

`.claude/rules/launcher-and-build.md` and the Cargo comment describe "three bins" and two shared modules; there are now six bins and eight modules (`bench/`, `tuner/`, `cem.rs`, `soak.rs`, `tactical_train.rs`, `dev_template.rs`, `paths.rs`, `scratch_assets.rs`).

## Decision

Move the headless bins and their modules to a `crates/devtools` crate that depends on engine and app-core only.
`crates/launcher` keeps `main.rs` and the gui dependency.
`paths.rs` goes with whichever crate owns `dirs`; the other must be handed its paths, which is the discipline the Cargo comment already states.

The alternative — an optional gui dependency with `required-features` on `main` — keeps one crate but makes `cargo run` need a feature flag, which breaks the documented command.

## Consequences

A worktree that only runs arena or savetool never builds bevy.
The rule file and the Cargo comment get rewritten to match the real bin count.
Nothing in the launcher is engine logic, so no code moves down into the engine.

## Evidence

- `crates/launcher/Cargo.toml` — gui dependency.
- `crates/launcher/src/main.rs` — the only gui user.
- `.claude/rules/launcher-and-build.md` — stale bin count.
