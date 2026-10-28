# Plan: Forge crafts the missing parts

Spec: [2026-10-28-forge-crafts-missing-parts-design.md](../specs/2026-10-28-forge-crafts-missing-parts-design.md).
Model: [the routine twin's plan](2026-10-28-compile-crafts-missing-parts.md)
and `Game::build_routine` / `routine_craft/plan.rs` as shipped in 0.20.23.
Branch `forge-crafts-missing-parts`, primary checkout (the user plays from
here). First commit: the spec (untracked on `main` now). No save, schema,
`.ron` or tuning change; no ADR.

Two phases, each one green commit or more. Phase 1 adds behaviour and keeps
every caller compiling; Phase 2 deletes the page and draws the new figures.
Both fit one context. For a subagent per phase, brief it with this file plus
the spec, and forbid push, checking out others' work, `git add -A` and stash.

## Phase 1: engine (`crates/engine`)

### Interfaces

`gear_craft/plan.rs`: the pure decide step, shared by `build_design`,
`DesignRow::blocked`, `design_bill` and the preview.

```rust
pub struct BuildInput<'a, H, K, R> {   // or plain fields; the builder's call
    pub design: &'a GearDesign,
    pub held: H,     // Fn(WeaponPart) -> u32, parts in reach
    pub known: K,    // Fn(WeaponPart) -> bool, = weapon_part_known (not _usable)
    pub in_reach: R, // Fn(&ItemId) -> u32, materials in reach
}

pub struct BuildPlan {
    pub spend: Vec<(WeaponPart, u32)>,  // held parts used
    pub craft: Vec<(WeaponPart, u32)>,  // shortfall crafted
    pub bill: Vec<(ItemId, u32)>,       // frame_bill + Σ part_bill(craft), merged per item
}
```

- `plan_build(input) -> Result<BuildPlan, GearCraftRefusal>`. Order: no
  Frame (`Fit(NoFrame)`), missing Proc (`SalvageOnly`), missing unknown
  (`PartNotResearched`), first short item (`ShortOf`). Split per part with
  `let use_held = have.min(need)` (opt-level-1 trap); `craft = need - use_held`.
- One pure helper, `material_bill(design, craft) -> Vec<(ItemId, u32)>`
  (frame bill plus each crafted part's `part_bill × n`, summed per item), and
  `uncraftable(craft, known) -> Vec<WeaponPart>`. The view calls these even
  when `plan_build` errs, so the preview shows every shortfall
  (spec "Refusal order"). Legality (`check_design`) is not consulted.
- Delete `GearCraftRefusal::MissingParts` once nothing produces it.

`views/gear_craft.rs`:
- Reuse `views::MaterialLine` (from `views/routine_craft.rs`; move it to a
  shared views module if the import reads wrong — builder's call).
- `DesignBill { materials: Vec<MaterialLine>, uncraftable: Vec<String> }`,
  where an uncraftable line is the full red text ("Venom proc — salvage a
  weapon first" / "Lance v2.0 — research first"). `DesignPreview` gains the
  same two lists, empty when the draft has no Frame.
- `DesignPartOption` gains `craft_cost: Option<(String, u32)>`: `Some` when
  `weapon_part_known(part)` and not Proc, held or not.

`game/gear_craft.rs`:
- `build_design`: `require_forge_free` → design → gather → `plan_build` →
  apply, copying `build_routine`: `pay_gear_bill(bill)`, then
  `grant_loot(part, n, LootSource::Craft)` per crafted part, then
  `pay_gear_bill(spend + craft as part items)` (cannot fail; `?` anyway),
  then mint the copy. Log "You compile {n} missing part(s) and build a
  {name}." when n > 0, else today's line; "part"/"parts" grammatical.
- `build_block` becomes `require_forge_free` + `plan_build(..).err()`; its
  separate frame-bill check goes.
- New `pub fn design_bill(&self, id) -> Option<DesignBill>`.
- `design_preview` fills the two lists; `design_part_options` fills
  `craft_cost` (item name via `item_name`).
- Leave `craft_weapon_part` / `weapon_part_rows` until Phase 2.

### Tests (`src/tests/gear_craft.rs`, pure ones in `gear_craft/tests.rs`; red first)

1. Pure: a design needing one part twice while holding one → spend 1,
   craft 1 (the miscompile guard).
2. Forge with no parts held but Scrap and frame material in reach builds
   the copy, spends exactly the merged bill, leaves no part items.
3. Some held: held spent first, only the shortfall billed.
4. Frame material equals part material (or two crafted parts): bill sums;
   stock for one but not both → `ShortOf`, nothing spent.
5. Missing Proc → `SalvageOnly`; missing unknown → `PartNotResearched`;
   held-unknown used once builds, needed twice refuses. Nothing spent.
6. Refusal order: no Frame beats Proc beats unresearched beats short.
7. `design_bill` / preview `materials` equal the inventory delta of a
   Forge; empty-but-frame-bill when every part is held.
8. `craft_cost` is `Some` for known non-Proc (held or not), `None` for Proc
   and held-unknown.
9. One `Record::Acquire` with source Craft per crafted part (retarget the
   existing `craft_weapon_part` receipt test if there is one).
10. Reach: Depot stock first, then pack; a Depot is out of reach outside
    base space (mirror the routine bench_stores tests).

Port the tests that set up via `craft_weapon_part` (`tests/gear_craft.rs`
~188–202, ~769) and the `MissingParts` matches (~307, ~340, ~602) now, so
their intent survives Phase 2. Mutation-check 4 and 7 (drop the summing,
drop a bill line).

### Gate
`cargo test -p feral-processes-engine gear_craft`, `cargo clippy --workspace
--all-targets`, `cargo fmt`. Commit.

## Phase 2: remove the Weapon parts page; draw the figures

### Delete (grep first: `rg -n "craft_weapon_part|weapon_part_rows|WeaponPartRow|WeaponParts\b|parts_page_rows|handle_weapon_parts_key|MissingParts" crates assets`)
- engine: `StructurePage::WeaponParts` + `page(forge, …)` line
  (`game/base/structure_hub.rs:30,127`), the `tests/structure_hub.rs:149`
  entry, `craft_weapon_part`, `weapon_part_rows`, `views::WeaponPartRow`,
  their tests (~667–673). Reword `items_db.rs:737` to name `build_design`.
- app-core: `Mode::WeaponParts` (`lib.rs:1744,2527`), the hub page and
  dispatch arms (`app/structure_hub.rs:62,244`), `input.rs:305`,
  `handle_weapon_parts_key`, the `MissingParts` wording arm
  (`app/gear_craft.rs:149`; add wording for any refusal now reachable that
  had none), tests (`tests/gear_craft.rs:89,187,258`).
- gui: `parts_page_rows` and its test (`render/gear_craft.rs:232,326`), the
  `render/mod.rs` arms (~1261, 2024, 2254), the module doc line 6.

### Add (gui `render/gear_craft.rs`; read `.claude/rules/seams-screens.md` and `drawing-seam.md` first)
- One shared `materials_rows(&DesignBill-like lists)`: a "Materials" heading,
  `"{label} {have}/{need}"` per line in the theme's existing good/bad colour,
  one red row per uncraftable line. Copy the routine renderer's version.
- `preview_rows`: append it when the draft has a Frame.
- `forge_page_rows`: append `game.design_bill(id)` under the highlighted
  design only.
- `pick_rows`: `Edge v2.0   held 0   6 Scrap`; nothing for `None`. Respect
  `gui-text-never-clips-or-wraps` and the popup row-width test.

### Help
`assets/help/52-forging-weapons.md` per spec "Docs": page list Forge, New
design, Salvage; Forge crafts missing parts; Materials section; "learn how
to craft them". Plain language, no tick vocabulary.

### Tests
- app-core: Workbench rail has no Weapon parts page; one key-driven Forge
  of a design with materials but no parts builds it.
- gui: headless row test for a green and a red material line and an
  uncraftable line, if the file's existing tests are headless.

### Gate
`cargo clippy --workspace --all-targets`, `cargo fmt`, `cargo test
--workspace` (unset `FERAL_DEV_NO_SIEGES`), `balance_sim` untouched so not
required. Screenshot: a Workbench dev-save (`dev-saves/README`, e.g.
`bench-economy` if the Weapons tree is open there) with `--keys` to the
Forge and New design pages; Read the PNGs for the Materials rows and picker
costs. Commit.

## Finish
Delete the `next-task-forge-crafts-missing-parts` memory and its MEMORY.md
line. Opus whole-branch review (diff as a file). Stop; version, CHANGELOG
and tag wait for the user's "deploy".
