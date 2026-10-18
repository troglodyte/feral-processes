# 0010 Hygiene backlog

Status: Proposed (2026-10-16)

Each item here is small and independent; do it when a hand is already in the file.
Strike an item out when it lands, with the release.

## Hub files become directories

Glob re-exports keep every caller path, so this is mechanical.
Commits in the last six months and last month: `crates/engine/src/tuning.rs` 351/101 (it already has subsystem banners), `views.rs` 263/90 (152 structs, no sections), `components.rs` 206/77.
`crates/app-core/src/lib.rs` is 3.7k lines and the file every new screen edits; `crates/gui/src/render/mod.rs` is style, dispatch and helpers in one.
There is no placement rule today: 40 of 71 resources and 39 view types already live with their domain while the rest sit in the hub file.

## `pub(crate)` where nothing outside calls

Fifteen engine `pub mod`s — `systems`, `research`, `talents`, `nemesis`, `statuses` among them — are used by no other crate, and at least 82 `Game` methods have no caller in app-core, gui or launcher.
Items in a `pub mod` never get dead-code warnings; narrowing them turns the lint back on and keeps ECS `systems` out of the renderer's reach.

## Four `String` id aliases

`AbilityId`, `StructureId`, `SpeciesId` and `ResearchId` are `String` aliases beside nineteen newtyped ids, so the four can be swapped for each other silently.
About 263 references; the save format does not change.

## `deny_unknown_fields` on asset RON

It is set only on save, research and achievements.
Everywhere else a modder's typo is silently ignored and the field takes its default.
This is compatible with the `#[serde(default)]` rule — a missing field and an unknown field are different errors.

## Dev flags are handed down, not read from the environment

The engine reads process env in `game/siege/clock.rs:20` and `game/stack_view.rs:56`; the "flag is on" check is copied there and in `crates/gui/src/lib.rs:120` and `crates/app-core/src/app/dev_console.rs:146`, whose own comment warns about the drift.
Because the engine reads env, an exported `FERAL_DEV_NO_SIEGES` changes `cargo test`, which is why `CLAUDE.md` has to warn about it.
A `DevFlags` value parsed once in the launcher and handed to `App` and `Game` (precedent: `Game::dev_set_sieges`) removes both the copies and the warning.

## Workspace manifest

`rand`, `ron` and `serde` are declared in two or three crates with a comment asking that they stay at the same versions; `[workspace.dependencies]` enforces that.
There is no `[workspace.lints]`, no `rust-version` and no `rust-toolchain.toml`, although releases are built on three machines.

## Docs that drift

All seven generated reference pages fail `docs/audit-gen.py`; the structures table is missing 23 shipped ids and lists 15 that no longer exist, and `README.md:69-75` quotes old counts.
Generating the pages from the real `*Db` in Rust removes the second RON parser that the Python transcriptions are.
Five of 33 doc paths cited from code, assets and `.claude/` do not resolve; `crates/engine/src/game/siege/mod.rs:12` is one citer.
`.claude/rules/content-schema.md:12` scopes to `crates/engine/tests/assets.rs`, which does not exist, so the rule never auto-loads for `src/tests/assets.rs`.
`docs/superpowers/INDEX.md` says five specs are path-pinned; 57 code citations pin about twelve.
Stop moving specs on landing and add a check that every repo path and `paths:` glob in rules, skills and code comments resolves.

## Small duplications

Arrows and hjkl to a direction is written out nine times in app-core (`app/building.rs:371`, `app/field.rs:97` among them).
Digit, Backspace and empty-means-one entry is rewritten in `trade.rs`, `crafting.rs`, `inventory.rs` and `building.rs`, each with its own string field.
The gui must call five `App` per-frame updates in the right order (`update_realtime`, `advance_reveal`, `advance_tactical`, `advance_status`, `advance_compile`; `crates/gui/src/lib.rs:850`); one `App::advance(dt)` would own the order.
`crates/app-core/src/tests/support.rs` holds sixteen full `CreatureSave` literals and the struct has no `Default`, so each new save field is sixteen edits.
`new_with` and `load` in `crates/engine/src/game/lifecycle.rs` each seat about a hundred resources in separately maintained lists with no test that the two sets match; a census test is the cheapest guard.
`Fx` in `crates/gui/src/fx.rs` holds effect timelines and view state (camera, row positions, `map_click`) in one struct; map clicks go through it, the sprite editor recomputes its layout and the world map has its own handler.
