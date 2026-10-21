# 0004 Content loads once, as a value

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-16 (revised 2026-10-21)
- Pattern: Ports and adapters

## Context

`load_asset_dbs(&Path)` in `crates/engine/src/game/lifecycle.rs` is already the one loader, but it takes a path and every `*Db::load_dir` reads the disk itself.
Nothing is cached: each of the thousands of test `Game` constructions re-parses about six hundred `.ron` files, and `Game::load` re-parses too.

Because the door is a path, tests build fixtures by copying files.
`copy_shipped_assets` in `crates/engine/src/tests/support.rs` copies a hand-kept list of about sixteen asset directories, while the loader also reads others, and a missing directory is silent.
So a fixture tests a game without that content and nothing complains.
A test fixture species is also injected into the production loader under `#[cfg(test)]` (`SpeciesDb::insert`, `SpeciesDb::retain`).

## Decision

Introduce a `Content` value holding every `*Db`, with a pure parser over `(kind, name, text)` and a filesystem adapter that walks `assets/`.
`Game::new_with_content(Content, …)` and `Game::load` take the value; `Game::new` stays the convenience that loads from a path once.
Tests build a `Content` from the shipped set plus overrides instead of copying directories, which removes the subset trap by construction.

## Alternatives considered

- Cache the parsed dbs behind a `OnceLock` keyed by path. Saves the parse but keeps the copied-directory fixtures and the subset trap.
- Make `copy_shipped_assets` copy every directory. Fixes the trap today; the next new directory reintroduces it.

## Consequences

Test wall-clock drops by the parse cost times the number of constructions.
A test that overrides one species no longer loses other kinds.
The `#[cfg(test)]` fixture species moves out of production code.
One crate, no save-format bump; `Game::new`'s signature for app-core and launcher stays.

## Evidence

- `crates/engine/src/game/lifecycle.rs` — `load_asset_dbs`, `AssetDbs`.
- `crates/engine/src/tests/support.rs` — `copy_shipped_assets`.
- Memory: `engine-test-fixture-copies-a-subset-of-asset-dirs`.
