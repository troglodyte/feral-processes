# One Build weapon page, laid out like Build routine

**Status:** approved in chat 2026-10-29; not yet built.
**ADR:** [0031](../../adr/0031-player-gear-is-built-from-designs-over-a-slot-generic-frame.md) — unchanged. A design is still a saved recipe whose `ItemDef` is derived; it just stops being a thing the player manages.

## Why

On the Workbench, making a routine is one page: pick parts, name it, Compile spends and mints. Making a weapon is two: **New design** (pick parts, Save stores a recipe) then **Forge** (Enter on a recipe spends and mints). The user found the mismatch confusing. The weapon part lists also show every part, with unresearched ones greyed, where the routine lists show only what the player can use.

## What the player sees

Workbench pages: Build routine · Built routines · **Build weapon** · Salvage · Compile · Info. Forge and New design are gone.

**Build weapon** keeps the New design page's rows and preview:

- Rows: Frame, each modifier category, Affixes, Name, **Compile** (was Save). Modifier rows refuse until a Frame is picked, as now.
- A part list shows only parts that are researched or held (`weapon_part_usable`). A usable part that doesn't fit the weapon so far stays, greyed, with its reason. Picking replaces the category's part; the trailing row clears it. The affix list is unchanged (known affixes only, toggled).
- The preview is unchanged: description, stats, range, power, parts used/missing, materials, uncraftable, the part count against the cap, and why Compile would be refused.
- **Compile** validates, spends held parts, crafts missing ones from materials, pays the frame's bill and puts one ordinary copy in the pack — all or nothing. On success the draft clears (rebuilding means re-picking, as with routines) and the highlight stays on Compile. On refusal the draft is kept and the reason shown.

## Engine

- New `Game::compile_weapon(name, parts) -> Result<GearCopy, GearCraftRefusal>`. It runs every check (`require_forge_free`, `check_design`, `plan_build` on a candidate design) **before** storing anything, so a refusal never leaves a stray design.
- If a stored design has the same trimmed name and the same parts, it is reused, so two compiles of one weapon share an item id. Otherwise a new design is stored. **Builder's call:** whether the comparison is order-sensitive. The app draft orders parts deterministically (Frame first, then by pick), so an exact `Vec` comparison is acceptable.
- Designs are never deleted after this change. They are small, and an orphan design costs one item def.
- Removed, with their tests and views: `delete_design`, `design_rows`, `design_bill`, `design_holder` and `DesignHolder`, the `NoSuchDesign` and `InUse` refusals, `DesignRow` and `DesignBill`. `create_design` and `build_design` either become private steps of `compile_weapon` or fold into it. Engine tests that set up a weapon via `create_design` + `build_design` move to `compile_weapon`. **Builder's call:** whether a test-only helper survives.
- `StructurePage::Forge` and `NewDesign` become one page, `StructurePage::Weapon`, titled "Build weapon", in the same position.
- `design_part_options` filters out parts that aren't usable rather than greying them with `PartNotResearched`.
- No save-format change: `gear_designs` is still saved and loaded as now.

## App-core and gui

- `Mode::Forge` is removed. `Mode::NewDesign`, `DesignPick` and `DesignName` keep their behaviour under the Build weapon page. **Builder's call:** whether to rename them to `WeaponBuild*` to match `RoutineBuild*`.
- `DESIGN_SAVE_ROW` becomes the Compile row and calls `compile_weapon`.
- gui: the Forge renderer goes. The page's last row reads "Compile". Key hints follow the case of the handler.

## Not changing

Salvage. Routine crafting. Fusion's refusal of crafted gear. The preview's numbers. Balance (no tuning or species change).

## Gates

TDD per step. `cargo clippy --workspace --all-targets`, `cargo fmt`, `cargo test --workspace`. Run a `--screenshot` of the Build weapon page from a template with the weapon tree open. CHANGELOG entry at release.
