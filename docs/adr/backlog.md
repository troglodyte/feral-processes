# ADR backlog

Small, independent items that are not architecture decisions.
Do one when a hand is already in the file; strike it out with the release that carried it.
None of these needs an ADR unless doing it would hit a trigger in the [README](README.md#when-an-adr-is-required).

## Build

- [ ] Workspace manifest: `rand`, `ron` and `serde` are declared in two or three crates; move them to `[workspace.dependencies]`. Consider `[workspace.lints]`, `rust-version` and `rust-toolchain.toml`. From [0010](0010-hygiene-backlog.md).

## Bugs

- [ ] Stale `watching`: `App::leave_run` and `App::install_game` (`app/lifecycle.rs`) never clear `watching`, so the camera can follow a recycled `Entity` in the next run. Write the failing reproducer first. The structural fix is [0013](0013-a-run-struct-on-app.md).

- [ ] Roster `mitigation` is raw beside the now-effective `atk`: `PetInfo`, `CompanionInfo`, `EnemyGroupView`, `ManifestView` and `FieldRoutineTargetView` read `stats.mitigation`; `Game::effective_mitigation` exists. [0012](0012-open-questions-from-the-review.md) decided atk only, so ask before doing it.

## Assets and tests

- [ ] Implement [0016](0016-shipped-assets-deny-unknown-fields.md): `#[serde(deny_unknown_fields)]` on every shipped asset kind's def types.

## Engine

- [ ] `pub(crate)` where nothing outside calls: many engine `pub mod`s and `Game` methods have no caller in app-core, gui or launcher, so dead-code warnings never fire. From [0010](0010-hygiene-backlog.md).
- [ ] Newtype the four `String` id aliases (`AbilityId`, `StructureId`, `SpeciesId`, `ResearchId`). `StructureId` first needs a plan for the minted `"settlement/<id>"` key that `.claude/rules/seams-ground.md` relies on. No save-format change. From [0010](0010-hygiene-backlog.md).
- [ ] `new_with` and `load` in `game/lifecycle.rs` each seat the resources from separate lists with no test that they match; add a census test. From [0010](0010-hygiene-backlog.md).

## app-core and gui

- [ ] `render/settlement_market.rs` copies caravan's `cell` closure; make it a call. From [0009](0009-layout-separate-from-paint.md).
- [ ] Replace `use super::*` in `render/` with explicit imports when a file is touched. From [0009](0009-layout-separate-from-paint.md).
- [ ] `.claude/rules/drawing-seam.md` still says `render/` is ~3,000 lines with fifteen operations; it is tens of thousands, and `Painter` has more. From [0009](0009-layout-separate-from-paint.md).
- [ ] Arrows and hjkl to a direction is written out many times in app-core (`app/building.rs`, `app/field.rs` among them); digit entry with Backspace and empty-means-one is rewritten in `trade.rs`, `crafting.rs`, `inventory.rs` and `building.rs`. From [0010](0010-hygiene-backlog.md).
- [ ] The gui calls five `App` per-frame updates in order (`update_realtime`, `advance_reveal`, `advance_tactical`, `advance_status`, `advance_compile`); one `App::advance(dt)` would own the order. From [0010](0010-hygiene-backlog.md).
- [ ] `crates/app-core/src/tests/support.rs` holds sixteen full `CreatureSave` literals; a `Default` or a builder makes a new save field one edit. From [0010](0010-hygiene-backlog.md).
- [ ] `Fx` (`crates/gui/src/fx.rs`) holds effect timelines and view state (camera, row positions, `map_click`) in one struct. From [0010](0010-hygiene-backlog.md).

## Docs

- [ ] The generated reference pages under `docs/` drift from the shipped assets; generating them from the real `*Db` in Rust would remove the second RON parser. From [0010](0010-hygiene-backlog.md).
- [ ] Add a check that every repo path and `paths:` glob cited in rules, skills and code comments resolves. From [0010](0010-hygiene-backlog.md).

## Done

- [x] ~~Hub files become directories~~ — `tuning`, `components` and `views` split by domain in 0.19.16. app-core `lib.rs` and gui `render/mod.rs` follow `CLAUDE.md`'s "split when you're in it" rule instead.
- [x] ~~Dependencies build with `debug = "line-tables-only"`~~ — unreleased.
- [x] ~~The launcher rule counts six bins~~ — unreleased.
- [x] ~~Roster, party, enemy, manifest and ally-picker rows quote `Game::effective_atk`~~ — unreleased.
- [x] ~~The shipped-assets test asserts `AssetDbs.warnings` is empty~~ — unreleased.
- [x] ~~`SCRIM` is declared once, in `render/mod.rs`~~ — unreleased.
- [x] ~~`docs/audit-gen.py` reads `tuning/`~~ — unreleased.
