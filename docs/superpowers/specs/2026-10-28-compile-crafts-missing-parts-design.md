# Compile crafts the missing parts

**Status:** Proposed, 2026-10-28

Builds on [the routine-crafting design](2026-10-24-routine-crafting-design.md)
and [ADR 0029](../../adr/0029-player-routines-are-compiled-from-a-rust-part-catalogue.md),
whose catalogue, `compile` and save are unchanged.

## Why

Building a routine is two chores today. The player crafts each part one at a
time on the Workbench's Subroutines page (one blank Routine Disk plus the
part's material), then goes to Build routine and compiles. In the user's
words: "when i select all the options to create the complete routine, i don't
want to have to manually craft each piece. if i have all the required
materials, it should just build it."

## What changes

### Compile on the Build routine page

- Compile uses the parts the player holds first, then crafts only the shortfall
  from disks and materials, in the same action.
- All or nothing: if any part or material is short, nothing is spent and the
  refusal says what is missing.
- A missing part that is not researched cannot be crafted (a part from an
  extraction or a boss can be *used* unresearched, as now). Compile refuses
  with a message naming it, e.g. "Research Heal v2.0 first."
- Crafting a part costs exactly what `routine_craft::material` says today: one
  Routine Disk plus `count` of one material. No new cost figures.
- Materials come from reach (`held_in_reach` / `pay_from_reach`: base stores
  first, then the pack), as every Workbench verb does now.
- The log line says how many parts were crafted, e.g. "You compile 2 missing
  parts and build Zap." With nothing crafted, it stays "You compile Zap."

### New Materials section in the build preview

Under the existing preview lines (description, Power, cooldown, parts used,
missing, the blocked reason):

- One line per item the shortfall needs: the Routine Disk first, then each
  material, written as the item name and `have/need`.
- Green when held ≥ needed, red when short. The colours come from the existing
  theme, as the page's current status colours do.
- It counts only what this Compile will craft. Parts already held add nothing,
  so the section is absent when every part is held.
- Live, like the rest of the preview: it recomputes from the draft each frame.
- An unresearched missing part shows as a red line of its own ("Heal v2.0 —
  research first") rather than as materials.

### Part list shows the craft cost

Each entry in the list a slot row opens gains the cost of crafting one, beside
the held count, e.g. `Heal v2.0   held 0   1 disk + 4 Bytecode`. An entry the
player holds but has not researched shows no cost.

### The Subroutines page goes

Once Compile crafts, the page's only job is crafting spare parts ahead of time,
and parts only matter inside a build (markets and caravans already sell them).
The user agreed to remove it. The Workbench rail keeps Compile (gear), Build
routine and Built routines.

Removed: `Mode::Subroutines`, `StructurePage::Subroutines`, its key handler,
filter wiring, renderer and help hints, `subroutines_visible`, and the engine
API that only it called (`Game::craft_subroutine`, `Game::subroutine_rows`,
`views::SubroutineRow`). Tests that crafted parts through `craft_subroutine`
switch to the new Compile path or grant parts directly. Grep every reference
before deleting.

## Constraints

- The engine stays the one source of truth: the preview and Compile share one
  pure planning function (gather, decide, apply), so the screen cannot promise
  a build Compile would refuse. Extend `routine_craft::plan_build` (it already
  feeds both) rather than adding a second calculation.
- No save-format change, no `.ron` or schema change, no new tuning constants.
- No RNG is drawn in this path today; keep it that way.
- Parts crafted inside a Compile are consumed in the same action. Whether they
  pass through the inventory as items on the way is the builder's call, as
  long as the base ledger still counts them as crafted the way
  `craft_subroutine`'s `LootSource::Craft` grant does today.

## Docs

- `assets/help/51-building-routines.md`: drop the Subroutines page bullet and
  the "compile Subroutines, then combine them" framing; describe Compile
  crafting missing parts and the Materials section; the page list becomes
  Compile, Build routine and Built routines.
- CHANGELOG entry at release, not on the branch.

## Not doing

- No "craft spares" replacement anywhere.
- No change to research, extraction, markets, or how built routines are
  installed.

## Settled after the builder check

- **One summed bill.** Disks = parts crafted; each material is totalled across
  every crafted part (merged with the disk line if a material is ever the disk
  itself). That one bill is checked against reach, printed as the Materials
  section, and paid in the same `pay_from_reach` call as the held parts, so
  green in the preview means Compile goes through.
- **Refusal order:** name, then `validate_build`, then a missing part that is
  unresearched, then short disks or materials. Compile refuses with the first
  only; the preview's red lines already show every shortfall.
- **`plan_build` may change shape.** Prefer a plain input struct (cap, `held`,
  `known`, item counts in reach) and a concrete outcome (parts spent, parts
  crafted, item bill), per the gather/decide/apply rule. Update every caller
  and test; no shim for the old signature.
- **Ledger:** no grant-then-pay round trip, so no loot messages and no
  mid-action overflow. Record the crafted parts in the base ledger the same way
  a `LootSource::Craft` grant does, through whatever the ledger exposes
  (extract a shared call if `grant_loot` bundles the two).
- **Held but unresearched, needed twice:** the held one is used; the second is
  missing and unresearched, so Compile refuses and the preview shows its
  "research first" line.
- **Log wording** lives in `Game::build_routine`. Check app-core tests for the
  old "You compile {name}." text and update them.

## Calls I made (not discussed with the user)

- Held parts are spent before any are crafted.
- The Materials section lists only the shortfall, not the from-scratch cost.
- The refusal and log wording above.
- Unresearched-and-missing parts are refused, not silently skipped.
