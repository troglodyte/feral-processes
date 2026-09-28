---
paths:
  - "crates/launcher/**"
  - "Cargo.toml"
  - "**/Cargo.toml"
  - "dev-saves/**"
  - "dev-arenas/**"
  - "docs/releasing.md"
---

# Launcher crate and build profile

- `savetool` and `arena` are launcher bins, not engine bins, so `default-run`
  keeps a bare `cargo run` unambiguous (`default-members` would also narrow a
  bare `cargo test`). `arena` also resolves `dev-saves/` template names, and
  `dev_template` is the launcher's.
- The launcher's `[lib]` exists only so its three bins share `dev_template`
  and it is unit-tested once. No game logic lives there.
- **`[profile.dev]` in the root `Cargo.toml` is what makes a debug build
  playable** (deps at `opt-level = 3`, workspace crates at `1`). Deleting it
  is a ~22x frame-cost regression no test catches and that reads as an
  animation bug — `docs/measurements/2026-08-19-debug-build-frame-cost.md`.
- Warm builds are fast (~2.5s per touched crate, engine suite ~6.7s); only a
  cold build or dependency change costs ~3.5 minutes. The 557-dep Bevy graph
  is `crates/gui`'s only. No sccache/nextest problem to solve.
- Releases: an executable **plus a loose `assets/` tree**, never a single
  file (fonts and sound cues are `include_bytes!`d; content stays droppable).
  Manual per platform, no cross-compiling — `docs/releasing.md`.
