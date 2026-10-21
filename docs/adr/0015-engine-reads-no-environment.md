# 0015 The engine reads no environment variables

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-21
- Pattern: Parameter object

## Context

The engine reads process environment in two places: `dev_no_sieges_env` in `game/siege/clock.rs` (`FERAL_DEV_NO_SIEGES`) and `dev_reveal` in `game/stack_view.rs` (`FERAL_DEV_REVEAL`).
Each caches its answer in a `LazyLock`, so a test cannot turn one off once the process has read it.
The "present, non-empty and not `0`" check is written out in both, again in `crates/gui/src/lib.rs` (`FERAL_DEV_ALT`), and a fourth time as `value_is_on` in `crates/app-core/src/app/dev_console.rs`.

Because the engine reads env, an exported `FERAL_DEV_NO_SIEGES=1` fails siege tests in the engine and the launcher, which is why `CLAUDE.md` carries a warning to unset it before `cargo test`.
A precedent for handing a switch in already exists: `Game::dev_set_sieges`.

## Decision

The engine reads no environment variables.
The launcher parses the dev switches once into a `DevFlags` value and hands it to `App`, which hands the engine's part to `Game` through setters like `Game::dev_set_sieges`.
One `value_is_on` in the launcher is the only spelling of "the flag is on".

## Alternatives considered

- Keep the env reads but drop the `LazyLock`, so tests can set and clear them. Process env is shared across test threads, so tests would race each other.
- One shared helper for the env check, still called from the engine. Removes the copies but keeps `cargo test` sensitive to the shell.

## Consequences

`cargo test` no longer depends on the shell it runs in, and the `CLAUDE.md` warning can be deleted.
A test can turn a dev switch on for one `Game` without touching the process.
Three crates change: launcher, app-core, engine; no save-format change.

## Evidence

- `crates/engine/src/game/siege/clock.rs` — `dev_no_sieges_env`, `Game::dev_set_sieges`.
- `crates/engine/src/game/stack_view.rs` — `dev_reveal`.
- `crates/app-core/src/app/dev_console.rs` — `value_is_on`.
- `crates/gui/src/lib.rs` — `FERAL_DEV_ALT`.
