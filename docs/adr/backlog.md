# ADR backlog

Small, independent items that are not architecture decisions.
Do one when a hand is already in the file; strike it out with the release that carried it.
None of these needs an ADR unless doing it would hit a trigger in the [README](README.md#when-an-adr-is-required).

## Build

- [ ] Add `debug = false` or `debug = "line-tables-only"` to `[profile.dev.package."*"]` in the root `Cargo.toml`. Leave `[profile.dev]` alone. From [0002](0002-no-debuginfo-for-dependencies.md).
- [ ] `.claude/rules/launcher-and-build.md` says the launcher's `[lib]` serves "three bins"; there are six. From [0003](0003-headless-dev-bins-without-bevy.md).
- [ ] Workspace manifest: `rand`, `ron` and `serde` are declared in two or three crates; move them to `[workspace.dependencies]`. Consider `[workspace.lints]`, `rust-version` and `rust-toolchain.toml`. From [0010](0010-hygiene-backlog.md).

## Bugs

- [ ] Roster `atk` is raw. The user decided effective atk everywhere ([0012](0012-open-questions-from-the-review.md)), enemy rows included (confirmed 2026-10-21). `Game::owned_pets` (`PetInfo`) and `Game::companion_info` (`CompanionInfo`) in `game/party.rs`, and `enemy_row` (`EnemyGroupView`) in `game/combat_round.rs`, read `stats.atk`; they should call `Game::effective_atk`. Start with a failing test.
- [ ] Stale `watching`: `App::leave_run` and `App::install_game` (`app/lifecycle.rs`) never clear `watching`, so the camera can follow a recycled `Entity` in the next run. Write the failing reproducer first. The structural fix is [0013](0013-a-run-struct-on-app.md).

## Assets and tests

- [ ] Implement [0016](0016-shipped-assets-deny-unknown-fields.md): `#[serde(deny_unknown_fields)]` on every shipped asset kind's def types.
- [ ] `every_shipped_asset_file_loads_without_a_warning` (`tests/assets.rs`) matches only the substring "skipped invalid"; other warning shapes pass it. Assert `AssetDbs.warnings` is empty instead. From [0004](0004-content-loads-once-as-a-value.md).

## Engine

- [ ] `pub(crate)` where nothing outside calls: many engine `pub mod`s and `Game` methods have no caller in app-core, gui or launcher, so dead-code warnings never fire. From [0010](0010-hygiene-backlog.md).
- [ ] Newtype the four `String` id aliases (`AbilityId`, `StructureId`, `SpeciesId`, `ResearchId`). `StructureId` first needs a plan for the minted `"settlement/<id>"` key that `.claude/rules/seams-ground.md` relies on. No save-format change. From [0010](0010-hygiene-backlog.md).
- [ ] `new_with` and `load` in `game/lifecycle.rs` each seat the resources from separate lists with no test that they match; add a census test. From [0010](0010-hygiene-backlog.md).

## app-core and gui

- [ ] `SCRIM` is declared four times (`render/notify.rs`, `level_up.rs`, `perk_bought.rs`, `ending.rs`). From [0009](0009-layout-separate-from-paint.md).
- [ ] `render/settlement_market.rs` copies caravan's `cell` closure; make it a call. From [0009](0009-layout-separate-from-paint.md).
- [ ] Replace `use super::*` in `render/` with explicit imports when a file is touched. From [0009](0009-layout-separate-from-paint.md).
- [ ] `.claude/rules/drawing-seam.md` still says `render/` is ~3,000 lines with fifteen operations; it is tens of thousands, and `Painter` has more. From [0009](0009-layout-separate-from-paint.md).
- [ ] Arrows and hjkl to a direction is written out many times in app-core (`app/building.rs`, `app/field.rs` among them); digit entry with Backspace and empty-means-one is rewritten in `trade.rs`, `crafting.rs`, `inventory.rs` and `building.rs`. From [0010](0010-hygiene-backlog.md).
- [ ] The gui calls five `App` per-frame updates in order (`update_realtime`, `advance_reveal`, `advance_tactical`, `advance_status`, `advance_compile`); one `App::advance(dt)` would own the order. From [0010](0010-hygiene-backlog.md).
- [ ] `crates/app-core/src/tests/support.rs` holds sixteen full `CreatureSave` literals; a `Default` or a builder makes a new save field one edit. From [0010](0010-hygiene-backlog.md).
- [ ] `Fx` (`crates/gui/src/fx.rs`) holds effect timelines and view state (camera, row positions, `map_click`) in one struct. From [0010](0010-hygiene-backlog.md).

## Docs

- [ ] `docs/audit-gen.py` crashes: it reads `crates/engine/src/tuning.rs`, which became `tuning/` in 0.19.16. From [0010](0010-hygiene-backlog.md).
- [ ] The generated reference pages under `docs/` drift from the shipped assets; generating them from the real `*Db` in Rust would remove the second RON parser. From [0010](0010-hygiene-backlog.md).
- [ ] Add a check that every repo path and `paths:` glob cited in rules, skills and code comments resolves. From [0010](0010-hygiene-backlog.md).

## Done

- [x] ~~Hub files become directories~~ — `tuning`, `components` and `views` split by domain in 0.19.16. app-core `lib.rs` and gui `render/mod.rs` follow `CLAUDE.md`'s "split when you're in it" rule instead.
