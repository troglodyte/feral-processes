# Recipes on the base report rail

**Date:** 2026-11-03
**Status:** Draft, awaiting the user's approval

## Why

A player asked why their Factories could not make Breach Kits. The answer
(a Printer does, after Automation) is in the data but nowhere the player looks.
The game already has a Recipes screen, but it is a row inside the base
report's Compile page, it only lists what machines make, and it never says what
research a recipe needs. The structure descriptions also still say "set what
it makes beside it with R", though recipes moved to the `c` structure hub's
Recipe page some time ago.

The goal is a page that answers "how do I make this, what does it take, and
where" without leaving the base report, plus a build menu that says what a
machine will make before you place it.

## Goals

1. **A Recipes page on the `b` rail.** One entry per item the player can
   make right now, **alphabetical by item name** (the user's choice). Each
   entry shows:
   - **what it is used for** (the user's ask, 2026-11-03), which depends on
     the item's kind (`ItemCategory`):
     - a **material** (`ItemCategory::Material`) shows only a
       `Used in: Hardened Shell, Logic Wafer, …` line, with no prose
     - **anything else** (gear, consumables such as the Honeypot or Breach
       Kit) shows its description, briefly, and adds a `Used in:` line too
       when it is also an ingredient (a Breach Kit goes into a Honeypot)
     - `Used in:` lists only the unlocked recipes that take the item,
       alphabetically, and is computed from the same recipe data as the
       entries, never authored
   - the materials it needs
   - **every** place it can be made: each machine whose recipe list has it,
     the Workbench (gear that needs a bench), and by hand from the base menu
     (a field recipe naming no structure)
   - the chain back to raw materials, as the current screen draws it
     (`inputs -> maker -> output`, taps named for unmade ingredients)
2. **Only unlocked recipes** (the user's choice). A machine recipe whose
   `research` is not done, and a craftable whose `research` is not done, do
   not appear. An item with no unlocked way to make it has no entry. Research
   finishing makes new entries appear on the next view.
3. **The build menu lists a machine's recipes.** In Build → Deploy a
   structure, a machine's entry gets a line under its description:
   `Makes: Breach Kit, Patch Routine` — unlocked recipes only, in the
   machine's recipe order. A machine with no unlocked recipe says so plainly
   (wording is the builder's call, e.g. `Makes: nothing yet — research
   unlocks its recipes`). Non-machines get no line.
4. **Fix stale player-facing text** found by the 2026-11-03 audit (below), in
   the same change.

## Non-goals

- Locked recipes, research hints or a "what unlocks this" view. Explicitly
  out.
- Setting a machine's recipe from the base report. Recipe choice stays on the
  `c` structure hub's Recipe page.
- Grouping by structure, a search/filter box, or a separate detail screen.
- `docs/manual.md` (standing instruction: never update it).
- Renaming the legacy contract ids `commission_an_armory` and
  `stand_up_a_refinery` (their descriptions are correct; an id rename risks
  saves for no player-visible gain).

## Anti-goals

- Two lists that can disagree. The rail page, the build menu line and the
  machine's own Recipe page must answer "is this machine recipe unlocked"
  through **one** predicate (today the inline
  `r.research.is_none_or(is_researched)` in `Game::machine_recipe_view`,
  `crates/engine/src/game/base/machine_recipe.rs:51` — extract it). Likewise
  the hand/bench sources share one "is this craftable researched" predicate
  with `Game::craft_recipes` (`crates/engine/src/game/crafting.rs:105`).
  **Do not read `craft_recipes` itself**: it also requires standing beside
  the bench (`touched_bench`) and applies perk discounts, so from the base
  report it would drop every Workbench recipe.
- An entry claiming a source the player cannot use (a machine recipe still
  locked, a Workbench recipe for gear not researched, a maker structure still
  locked behind `ResearchDef::unlocks_structures`).

## Decisions settled after review

Marked as my call (Claude's), not the user's; override any of them.

- **Machine inputs equal the item's craftable cost.** A machine's recipe is
  `systems::assembly_recipe`, which returns the produced item's
  `craftable.cost`, so a machine, the Workbench and the base menu all take the
  same materials for one item. One materials list and one chain per entry is
  safe; the entry lists its makers, not one chain per maker.
- **A maker counts only when it is unlocked too** (my call). A machine whose
  structure is still gated by `unlocks_structures` research, or a craftable
  whose `requires_structure` is locked, is not listed as a source. Built or
  not does not matter: the page says where it *can* be made.
- **Extractor outputs get entries** (my call). Core Fragment is "made at an
  Extractor, from nothing"; the user asked "where to build it".
- **"Briefly" means the authored description as it stands** (my call).
  Cutting it to its first sentence would leave the Breach Kit reading only
  "A taming catalyst." If a description reads too long on this page, fix the
  description in the asset rather than truncating it in code.
- **A material with no unlocked use** shows no `Used in:` line rather than an
  empty one (my call).
- **Costs shown undiscounted** (my call), as the machines run them; perk
  discounts stay on the Compile screen.
- **Research recipes count.** `ResearchDef::unlocks_recipes` is another way
  to make an item; include it if `craft_recipes` does.
- **Order is alphabetical** (the user's), replacing the "shallowest first"
  sort and its doc-comment rationale. `recipe_chains`' readers
  (`tests/perks.rs`, `tests/chains.rs`, app-core `group_menu.rs`,
  `menus.rs`, `views/work.rs`, gui `crafting.rs`) may be rewritten to the new
  shape; check `views/work.rs` first, since it is not a test.
- **The rail page is always present** (my call); with nothing unlocked it
  says so in a line. `BASE_REPORT_PAGES` stays a fixed table.
- **The build menu line** appears on every machine the deploy list shows
  (locked structures aren't in it), wrapped through the same
  `description_rows` as the description so it cannot run off the popup.

## Constraints (from the repo's rules)

- The renderer never touches `World`; new data reaches it through `Game`
  methods returning plain view structs (`crates/engine/src/views/`).
- `Game::recipe_chains` (`crates/engine/src/game/catalog.rs:161`) is the
  current builder and its doc comment carries the reasoning for root steps vs
  expanded dependencies and for `tap_for`. Change it in place (or replace
  it), don't add a parallel builder. Its only reader today is the Recipes
  screen; check for others before changing its shape.
- The base report rail is the one table `BASE_REPORT_PAGES`
  (`crates/app-core/src/app/base_report.rs:10`). The new page goes there,
  reusing `Mode::Recipes`. The Compile page's `Recipes` row
  (`crates/app-core/src/app/group_menu.rs:235`) is removed, and the tests
  that open it through Compile (`tests/readonly_screens.rs`,
  `tests/base_report.rs:789`, `tests/group_menus.rs`) move to the rail.
- Lists are highlight + Enter, no row digits. The page stays read-only (it
  must not advance the game; the existing test covers that).
- `crates/gui/src/render/crafting.rs::draw_recipes` already scrolls per entry
  with steps as unselectable sub-rows, and a test holds the widest row inside
  the popup (`the_widest_recipe_row_fits_the_popup_it_is_drawn_in`). Keep
  both properties.
- `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test
  --workspace`. No `tuning/` or asset-number change, so `balance_sim` should
  not move; run it anyway.
- No save-format change.
- CHANGELOG entry and `assets/help/20-controls.md` (the rail description and
  "Compile holds the field recipes and Recipes") updated in the same change.

## Stale text to fix

From the audit; each verified against the code at the time.

| Where | Problem | Fix |
|---|---|---|
| `assets/structures/{extractor,factory,assembler,printer}.ron` description | "Set what it makes beside it with R"; `R` does nothing on the map | Point at `c` and the Recipe page |
| `assets/contracts/break_ground.ron`, `keep_the_lights_on.ron`, `tutorial_break_ground.ron` | Same `R` instruction | Same |
| `crates/engine/src/systems.rs:950` (alert) | "stand beside it and press R" | `c` and the Recipe page |
| `crates/gui/src/render/building/structure_sheet.rs:282` | "nothing set — stand beside it and press R" | Same |
| `assets/items/bytecode_block.ron` | "The Armoury forges plate out of these" — retired building | Armour is built from these at an Assembler or a Workbench |
| `assets/help/75-extraction.md:43` | "at the Fabricator" — retired | "at a Workbench" |
| `assets/research/automation.ron` | Omits that it also speeds up Workbench extraction | Add it |
| `assets/items/patch_routine.ron` + `assets/help/70-supplies.md` | "The only thing your base builds that helps you in the field" — Power Cells and Breach Kits are too | Reword (e.g. "the only buff your base builds") |
| `assets/structures/depot*.ron` | Comma splice "fifty units, Walk up" | Full stop |

Before closing the change, `rg` the assets and UI strings for `with R`,
`press R`, `Armoury`, `Fabricator` again (the standing rule: a recipe or
route change sweeps every description that names it). The builder may judge
the Charge Coil / Logic Wafer under-descriptions worth a line; optional.

## Left to the builder

- The view struct's shape (extend `RecipeChain` with a list of makers, or a
  new type).
- How the entry lays out several makers (one step line per maker, or one
  "Made at: Printer, by hand" line above a single chain). Must stay within
  the popup width test.
- Exact wording of the build menu line and the empty case.
- Whether the rail page sits next to Compile or elsewhere in the table.
