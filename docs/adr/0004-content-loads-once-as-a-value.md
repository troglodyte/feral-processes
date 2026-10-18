# 0004 Content loads once, as a value

Status: Proposed (2026-10-16)

## Context

`load_asset_dbs(&Path)` in `crates/engine/src/game/lifecycle.rs:3877` is already the one loader, but it takes a path and every `*Db::load_dir` reads the disk itself.
Nothing is cached: each of about 2,788 test `Game` constructions re-parses about 602 `.ron` files, and `Game::load` re-parses too.

Because the door is a path, tests build fixtures by copying files.
`copy_shipped_assets` in `crates/engine/src/tests/support.rs:542-577` copies fifteen asset directories (abilities, achievements, affixes, attributes, behaviours, crops, descriptions, items, perks, policies, research, species, statuses, structures, talents).
The loader also reads classes, implants, outposts, phase_keys, settlements, story and tools, and a missing directory is silent.
So the ~267 fixture uses test a game without that content and nothing complains.
A test fixture species is also injected into the production loader under `#[cfg(test)]`.

The one test that guards loading, `every_shipped_asset_file_loads_without_a_warning` (`crates/engine/src/tests/assets.rs:203`), matches only the substring "skipped invalid"; at least ten other warning shapes ("skipped unreadable nemesis file", "species …: unknown ability", "dropped floor finish") pass it.

## Decision

Introduce a public `Content` value holding every `*Db`, with a pure parser over `(kind, name, text)` and a filesystem adapter that walks `assets/`.
`Game::new_with_content(Content, …)` and `Game::load` take the value; `Game::new` becomes the convenience that loads from a path once.
Tests build a `Content` from the shipped set plus overrides instead of copying directories, which removes the subset trap by construction.
The warning test asserts on `AssetDbs.warnings` (`lifecycle.rs:3868`) being empty, not on a substring.

## Consequences

Test wall-clock drops by the parse cost times 2,788.
Modded, scratch and shipped content become the same type, so a test that overrides one species no longer loses seven other kinds.
Packed assets (an APK, a single archive) become possible through a second adapter — the Android feasibility note named this as its engine blocker.
The `#[cfg(test)]` fixture species moves out of production code.
This is a one-crate change with no save-format bump; `Game::new`'s signature for callers in app-core and launcher stays.

## Evidence

- `crates/engine/src/game/lifecycle.rs:3877` — `load_asset_dbs`.
- `crates/engine/src/tests/support.rs:542-577` — `copy_shipped_assets`.
- `crates/engine/src/tests/assets.rs:203` — substring match.
