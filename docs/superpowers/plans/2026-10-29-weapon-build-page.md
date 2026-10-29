# Plan: one Build weapon page

Spec: `docs/superpowers/specs/2026-10-29-weapon-build-page-design.md`. Branch `worktree-weapon-build-page`.
Do it in two phases, with a commit at each green step. Never `git checkout` uncommitted work, never push.

## Phase 1: engine (`crates/engine`)

Files: `game/gear_craft.rs`, `gear_craft/mod.rs`, `views/gear_craft.rs`, `game/base/structure_hub.rs`, `gear_craft/plan.rs` (doc only), `items_db.rs:737` (doc only), `tests/gear_craft.rs`, `tests/gear_combat.rs`.

1. **`compile_weapon`, test first.** `pub fn compile_weapon(&mut self, name: &str, parts: Vec<GearPart>) -> Result<GearCopy, GearCraftRefusal>`. The order is gather, decide, apply:
   - `require_forge_free`
   - `check_design` (returns the trimmed name)
   - find a design with the same name and parts, else build a candidate `GearDesign { id: next_id + 1, .. }`
   - `plan_design(&candidate)`. Every refusal happens before this point.
   - store the candidate only if it's new, then `sync_crafted_gear`, then pay and mint as `build_design` does today.

   Tests:
   - a compile mints one copy and spends exactly the preview's bill
   - a refused compile (short of a material, or an unresearched part) stores no design and spends nothing
   - compiling twice with the same name and parts gives the same item id and one design
   - the same name with different parts gives two designs
2. **Make the old verbs private.** Fold `create_design` and `build_design` into `compile_weapon`, or keep them as private steps. Move every test call site (`tests/gear_craft.rs`, `tests/gear_combat.rs:48`) over to `compile_weapon`, or to a `#[cfg(test)]` helper if a test needs a design without a copy. The tests keep their intent: refusals, the bill, the delta between the bill and the inventory, and range.
3. **Delete the dead pieces:**
   - `delete_design`, `design_holder` and `DesignHolder`
   - `GearCraftRefusal::{NoSuchDesign, InUse}`
   - `design_rows` and `DesignRow`
   - `design_bill` and `DesignBill`. `bill_of` stays because the preview uses it.
   - their tests: the delete and holder tests, and the `design_rows` asserts at about lines 603 and 874. Convert the asserts to `design_preview(..).blocked` where the intent survives.
   - fix the doc comments that name the deleted pieces or "Forge page(s)"
4. **Hide unusable parts.** `design_part_options` skips a part that `!weapon_part_usable(part)`. Drop the `PartNotResearched` branch there, but keep the variant, because `check_design` still returns it. Test: an unresearched part that isn't held is absent from the list, a held but unresearched part is present, and a researched part that doesn't fit is present with `unavailable: Some(Fit(..))`.
5. **One page for weapons.** Replace `StructurePage::{Forge, NewDesign}` with `StructurePage::Weapon`, in the same position (after `Built`, before `Salvage`). Check that `StructurePage` isn't saved: `rg StructurePage crates/engine/src/save*`.

Gate: `cargo test -p feral-processes-engine gear_craft`, `cargo test -p feral-processes-engine balance_sim`, then clippy.

## Phase 2: app-core and gui

Files: `app-core/src/{lib.rs, app/gear_craft.rs, app/structure_hub.rs, app/input.rs, app/list_filter.rs, tests/gear_craft.rs}`, `gui/src/render/{mod.rs, gear_craft.rs}`.

1. **Rename to match routines.** Remove `Mode::Forge` and rename `NewDesign` to `WeaponBuild`, `DesignPick` to `WeaponBuildPick` and `DesignName` to `WeaponBuildName`. `structure_hub.rs` maps `StructurePage::Weapon` to the title "Build weapon" and the screen `&[Mode::WeaponBuild]`. Update `lib.rs:2517-2525` (the list of Workbench modes), `lib.rs:2832` (the parent modes), `input.rs:301-304` and `list_filter.rs:50,177`. Delete `handle_forge_key`.
2. **Compile row, test first.** Rename `DESIGN_SAVE_ROW` to `DESIGN_COMPILE_ROW`. `compile_draft` calls `compile_weapon`. On `Ok` the draft clears and `row = DESIGN_COMPILE_ROW`. On `Err` the draft is restored and `refuse_craft`. Delete `holder_line` and the deleted refusal arms. Update the module doc.

   Tests (`tests/gear_craft.rs`): Enter on Compile puts one weapon in the pack and clears the draft; a refused Compile keeps the draft and sets the status line; the hub shows "Build weapon" and no Forge or New design pages. Rewrite or delete the Forge tests at about lines 187-245.
3. **gui.** Delete `forge_page_rows` and the Forge arm (`mod.rs:1258`) and its entries in the mode lists (`mod.rs:2021-2024`, `2252-2253`). The last row of the build page reads "Compile". Update the module doc of `gear_craft.rs`. Run the gui tests (`cargo test -p feral-processes-gui`).
4. **Screenshot.** Find a `dev-saves/` template with `weapon_bench` researched (`rg weapon_bench dev-saves` or the README). Open the bench and go to Build weapon with `--keys`, take a `--screenshot`, and Read the PNG.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` (with `FERAL_DEV_NO_SIEGES` unset). Then a final opus review of the whole branch diff (as a file).

## Not in scope

Releasing or the CHANGELOG (that happens at deploy), Salvage, and routine crafting.
