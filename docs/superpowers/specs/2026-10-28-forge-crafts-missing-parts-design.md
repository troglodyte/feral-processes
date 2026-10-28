# Forge crafts the missing parts

**Status:** Proposed, 2026-10-28

The weapon twin of [Compile crafts the missing parts](2026-10-28-compile-crafts-missing-parts-design.md)
(0.20.23). Builds on [the weapon-crafting design](2026-10-26-weapon-crafting-design.md)
and [ADR 0031](../../adr/0031-player-gear-is-built-from-designs-over-a-slot-generic-frame.md),
whose catalogue, `compile`, `design_power` and save are unchanged.

## Why

Building a weapon takes two chores today. The player crafts each part one at a
time on the Workbench's Weapon parts page, then goes to the Forge and builds.
A design is reused, so every copy means crafting the same parts by hand again.
Routines had the same friction until 0.20.23 removed it. The user agreed to
the same four answers here: craft the shortfall, remove the parts page, show
one merged bill, and refuse on a missing salvage-only part.

## What changes

### Forge builds from parts held, then crafts the rest

- `Game::build_design` uses the parts the player holds first. It crafts only
  the shortfall from materials, in the same action.
- All or nothing: if any part or material is short, nothing is spent, and the
  refusal says what is missing.
- Crafting a part costs exactly what `gear_craft::weapon::part_bill` says
  today. No disk is spent and no cost figure is new.
- A missing Proc part cannot be crafted, because it is salvage-only. Forge
  refuses with `SalvageOnly`.
- A missing part that has not been researched cannot be crafted either. Forge
  refuses with `PartNotResearched`. A held part may be used unresearched, as
  it can now.
- Materials come from reach (`held_in_reach` / `pay_gear_bill`): base stores
  first, then the pack.
- The log line says how many parts were crafted, e.g. "You compile 2 missing
  parts and build a Longknife." When nothing was crafted it stays "You build a
  Longknife."

### One merged bill

The frame's bill and every crafted part's `part_bill` are summed per item into
one bill. That one bill is:

- checked against reach,
- shown as the Materials section, and
- paid in the same `pay_gear_bill` call as the held parts.

So green in the preview means Forge goes through. `pay_gear_bill` already sums
lines naming one item, and the frame and part materials may coincide.

### Materials section

The section shows one line per item in the merged bill, written as the item
name and `have/need`. A held line is green and a short line is red, using the
theme's existing status colours. Parts the player holds are not listed. They
appear under "Build uses" as they do now.

- **New design page preview:** under the existing lines, live from the draft
  each frame, like the rest of the preview. It shows only when the draft has a
  Frame.
- **Forge page:** under the highlighted design only, so a long list of designs
  stays short.
- A missing part that cannot be crafted gets its own red line in place of
  materials:
  - a Proc part: "Venom proc — salvage a weapon first";
  - an unresearched part: "Lance v2.0 — research first".

### Part picker shows the craft cost

Each entry in the list that a design row opens gains the cost of crafting one,
beside the held count, e.g. `Edge v2.0   held 0   6 Scrap`. A Proc entry, or
one that is held but not researched, shows no cost.

### The Weapon parts page goes

Once Forge crafts, the page's only remaining job is crafting spare parts ahead
of time. The user agreed to remove it. The Salvage page stays. The Workbench's
weapon pages become Forge, New design and Salvage.

Removed:

- `Mode::WeaponParts` and `StructurePage::WeaponParts`;
- the page's hub entry, `handle_weapon_parts_key`, renderer
  (`parts_page_rows`) and help hints;
- the engine API that only it called: `Game::craft_weapon_part`,
  `Game::weapon_part_rows` and `views::WeaponPartRow`.

Tests that crafted parts through `craft_weapon_part` switch to the Forge path
or grant parts directly. The `items_db.rs` doc comment that names
`craft_weapon_part` as "the one door" is reworded to name `build_design`.
Grep every reference before deleting.

## Constraints

- The engine stays the one source of truth. The Forge build, the Forge row's
  `blocked`, and the preview all share one pure planning function
  (`gear_craft::plan_build`, extended), so the screen cannot promise a build
  that Forge would refuse. `build_block`'s separate frame-bill check folds
  into it.
- `plan_build` may change shape. Prefer a plain input struct (the design, held
  counts, which parts are known, item counts in reach) and a concrete outcome
  (parts spent, parts crafted, merged bill), per the gather/decide/apply rule.
  Update every caller and test, with no shim for the old signature.
- No save-format change, no `.ron` or schema change, and no new tuning
  constants.
- This path draws no RNG today. Keep it that way.
- Parts crafted inside a Forge are consumed in the same action. Copy
  `Game::build_routine` (0.20.23):
  1. pay the merged materials bill;
  2. `grant_loot` the crafted parts with `LootSource::Craft`, so each is
     receipted as a craft;
  3. pay every part the design uses (held plus crafted) from reach.

  The pack ends the action without them.
- Watch for the opt-level-1 miscompile that the routine split hit twice
  (memory `opt-level-1-miscompiled-a-generic-closure-fn`). Write the
  held/craft split with `min`, and add a pure test of a design that needs one
  part twice while holding one copy.

## Refusal order

Busy or not at a bench, then no such design, then no Frame, then a missing
Proc part, then a missing unresearched part, then the first item short in
reach. Forge refuses with only the first of these. The preview's red lines
already show every shortfall.

A part that is held but unresearched and needed twice: the held copy is used.
The second copy is missing and unresearched, so Forge refuses and the preview
shows its "research first" line.

## Docs

- `assets/help/52-forging-weapons.md`: drop the Weapon parts page bullet. The
  page list becomes Forge, New design and Salvage. Describe Forge crafting the
  missing parts and the Materials section. The Salvage bullet's "learn how to
  compile them" becomes "learn how to craft them".
- Add a CHANGELOG entry at release, not on the branch.

## Not doing

- No "craft spares" replacement anywhere.
- No change to salvage, research, the design cap, or how a built weapon is
  equipped or fused.

## Settled after the builder check

- **"Researched" means `weapon_part_known`** (the research node is done). It is
  not `weapon_part_usable`, which also counts held parts. The plan's input
  takes a `known` predicate.
- **Part picker cost:** shown on every entry that is known and not a Proc,
  whether or not it is held. A Proc entry, or an unresearched one, shows none.
- **`GearCraftRefusal::MissingParts`** is deleted if nothing produces it
  after the change. That is expected, since every shortfall becomes
  `SalvageOnly`, `PartNotResearched` or `ShortOf`. Its wording arms in
  app-core and gui go with it, and tests that match on it move to the new
  refusals.
- **The preview bill ignores legality.** Once a Frame is chosen, the Materials
  section is computed from the draft's parts even when the draft fails
  `check_design` (over the cap, say). The blocked line says why separately.
- **The view shape is a new call**, not a new field on every `DesignRow`. For
  example, a `Game::design_bill(id)` returns the material lines and the
  cannot-craft lines for the highlighted design. `DesignPreview` gains the
  same two lists for the draft. One pure planning function feeds both,
  `DesignRow::blocked` and `build_design`.

## Calls I made (not discussed with the user)

- Unresearched missing parts are refused like Proc parts, with their own
  "research first" line, as Compile does.
- Held parts are spent before any are crafted.
- The Materials section lists the frame bill plus the shortfall's cost, not
  the from-scratch cost of every part.
- On the Forge page the section shows under the highlighted design only.
- The refusal and log wording above.
