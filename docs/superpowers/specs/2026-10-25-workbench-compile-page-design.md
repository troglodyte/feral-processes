# Workbench Compile page

**Status:** Approved by the user in chat, 2026-10-25.

## Intent

Compiling items is a Workbench job. Today the `[b]` base menu's Compile
page has a "Compile an item" row that opens the recipe list (`Mode::Craft`)
anywhere, including out in the field. 34 of the 43 recipes already need a
Workbench (`requires_structure: Some("workbench")`), so most of that list is
only usable at the base anyway. The user wants compiling to live on the
Workbench's own screen, with only a few cheap consumables still
compilable anywhere.

## What changes

1. **The Workbench hub gets a Compile page.** A new `StructurePage::Compile`
   whose screen is `Mode::Craft`. It lists every recipe `Game::craft_recipes`
   returns, which works in the hub because `touched_bench` measures from the
   party's base-space tile and the party is standing beside the bench. It has
   the same list, type-to-filter, quantity prompt and compile bar as today.
   Rail order is Build routine, Subroutines, Built routines, **Compile**,
   Info *(my call)*.
2. **The base menu row offers only field recipes.** `[b]` → Compile →
   "Compile an item" stays and still works anywhere, but its list holds
   only recipes with no `requires_structure`. After step 3, those are Power
   Cell, Breach Kit and Power Outlet. The row's `available` test reads the
   same filtered list, so it hides when that list is empty. "Recipes" (the
   chain reference) stays on the base menu untouched.
3. **Six gear recipes become Workbench-only.** Add
   `requires_structure: Some("workbench")` to the `craftable` block of
   `scrap_ward`, `shiv_routine`, `packet_buffer`, `kinetic_edge`,
   `probe_service` and `handshake_forge`. That is an existing field, so no
   ADR is needed (`docs/adr/0019`). Their drop tables are unchanged. This
   is a deliberate early-game change, and the user chose it item by item:
   before a Workbench exists, these six come only from drops. The builder
   checks whether any test, dev-save or starting loadout compiles one of
   them without a bench, and moves those to a bench fixture or a field
   recipe.
4. **Popups stay inside the hub.** `Mode::CraftQuantity` and
   `Mode::Compiling` open over the Compile page. Add them to
   `base_popup_parent` with parent `Mode::Craft`, so the structure hub
   survives them (the 0.20.2 fix made that check read `base_popup_parent`)
   and the renderer keeps the hub frame behind them.
5. **Leaving a compile returns to where it started** *(my call)*. When the
   structure hub is open, the paths that end in `Mode::Playing` instead
   return to `Mode::Craft` on the hub. Those paths are: a finished batch in
   `advance_compile`, any-key abort in `handle_compiling_key`, a refused or
   zero-quantity `commit_craft`, and a missing `pending_craft`. From the
   base menu they behave exactly as today.

## One list, one decision

Which recipes `Mode::Craft` shows depends on where it was opened:
everything at a bench, field recipes only from the base menu. Today four
readers call `game.craft_recipes()` directly: `craft_visible`,
`handle_craft_key`, `list_filter.rs`'s row count, and gui's
`draw_craft_menu` and its row builder. All of them must go through **one**
function that takes the context, or the cursor and the drawn rows drift
apart. Two shapes look reasonable, and the choice is left to the builder:

- the engine exposes the recipe's bench (`CraftRecipe::bench: Option<String>`,
  or a `field_only` flag) and app-core filters it, or
- `Game::craft_recipes_for(at_bench: bool)`.

The context is `App::structure_hub.is_some()` while in `Mode::Craft`. That
flag is reliable. The end-of-key check in `input.rs` clears the hub whenever
the mode is neither one of the hub's pages nor a `base_popup_parent` popup,
so opening `[b]` drops it before `Mode::Craft` can be reached that way.
Check that `App::hub_at` answers `None` for `Mode::Craft` while
`structure_hub` is `None`, because `Mode::Craft` is now listed as a hub
screen. The base menu's Craft must not be mistaken for a hub page.

## Keys on the Compile page

The Compile page behaves like the **Subroutines** page, which is already a
type-to-filter list on the same hub. Copy what it does for Esc, the rail,
arrows and filter keys (`take_filter_key`, `screen_claims`), and don't
invent new key rules. `handle_craft_key`'s own Esc → `close_screen` stays
for the base-menu path only.

The second open question is answered by the same rule. A compile can only
start from `Mode::Craft`, so at the end of a compile the hub is open
exactly when it was started there. `refresh_structure_hub` already closes
the hub if the bench is gone, and in that case the exit falls back to
`Mode::Playing` as it does today.

## Non-goals

- No change to recipe costs, research gates, hand-compile timing, or
  `Game::craft`'s refusals. The engine already refuses a bench recipe away
  from a bench.
- The Recipes reference screen does not move.
- No new `.ron` field, so no schema README change beyond noting the six
  items in `assets/items/README.md` if it lists field recipes.

## Docs

- `assets/help/20-controls.md`: Compile now holds the field recipes and
  Recipes. Item compiling is on the Workbench screen, opened with the
  existing structure-hub key `c` while standing beside the bench. No new
  key is added.
- `assets/help/51-building-routines.md` (the Workbench help) mentions the
  Compile page.
- `CHANGELOG.md` on landing. Not the manual or the README.

## Tests (intent)

- A Workbench hub lists `StructurePage::Compile`. Opening it shows bench
  recipes. A compile through quantity and the bar returns to the Compile
  page with the hub still open.
- The base menu's list holds exactly the no-bench recipes, even while
  standing beside a Workbench.
- The six items are not in the field list. A census test pins the field
  set to `{power_cell, ice_breaker, outlet}`.
- The existing `tests/crafting.rs` and `list_filter/pickers.rs` tests that
  craft one of the six from the base menu move to a bench fixture or to a
  field recipe.
- The gui census (`render/mod.rs` mode lists near lines 1956 and 2188) still
  covers `Craft`, `CraftQuantity` and `Compiling`.

## Gates

`cargo fmt`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace`, and `balance_sim` (item data changed, though no
stats did).
