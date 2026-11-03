# Recipes Rail Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Recipes page on the base report rail listing every unlocked
recipe A–Z with what it's for, what it needs and every place it can be made,
plus a `Makes:` line on machines in the build menu, plus the stale-text
fixes.

**Architecture:** The engine's `Game::recipe_chains` is reshaped to one
entry per makeable item, built from two new unlock predicates that the
machine Recipe page also uses. app-core moves `Mode::Recipes` from a
Compile-page row onto the `BASE_REPORT_PAGES` table. The gui draws it as a
base-report page (`render/base_report/recipes.rs`) instead of a popup.

**Tech Stack:** Rust workspace, bevy_ecs (engine), bevy + egui (gui).

**Spec:** `docs/superpowers/specs/2026-11-03-recipes-rail-page-design.md`
(read it first; its "Decisions settled after review" section is binding).

## Global Constraints

- Only unlocked recipes and unlocked makers appear, anywhere in this change.
- Entries are alphabetical by item display name. `Used in:` lists are
  alphabetical.
- `ItemCategory::Material` entries carry no description. All others carry
  their authored description unchanged, never truncated in code.
- Costs are undiscounted (`craftable.cost` as written). No perk discount.
- One predicate per question: "is this machine recipe unlocked" and "is this
  craftable/research recipe unlocked". `machine_recipe_view`, the rail page
  and the build menu all call them. Never read `Game::craft_recipes` for this
  (it gates on `touched_bench` and discounts).
- The renderer never touches `World`, and the `world` field gets no accessor.
- No save-format change, no `tuning/` change, no new asset fields.
- Lists are highlight + Enter. The page is read-only and never advances the
  game.
- `cargo fmt`; `cargo clippy --workspace --all-targets` clean;
  `cargo test --workspace` green; `cargo test -p feral-processes-engine
  balance_sim` unchanged. Unset `FERAL_DEV_NO_SIEGES` before testing.
- Never `git checkout` or `git stash` over uncommitted work, never
  `git add -A`, never push. Commit at each green step with explicit paths.

## Review Focus

1. **Locked maker structure.** A recipe is researched but its structure
   (`ResearchDef::unlocks_structures`) is still locked: the structure must
   not be listed, and the item has no entry if that was its only maker. Test
   in Task 1.
2. **Workbench recipes while away from the bench.** From the base report the
   player is never beside a Workbench, but Workbench gear (e.g. Scrap Ward)
   must still be listed with the Workbench as its maker. Test in Task 1.
3. **Research finishing mid-session.** An entry appears after
   `unlock_research_chain` without reopening anything (the views recompute
   every call). Test in Task 1 (before/after on one `Game`).
4. **The same item from several makers.** Breach Kit lists both Printer and
   by hand, once each, with one chain. Power Cell (Extractor and by hand)
   likewise. Test in Task 1.
5. **Long rows.** The deepest chain, the longest `Used in:` line and the
   longest `Makes:` line all fit their panel. Width tests in Task 3.

---

### Task 1: Engine — unlocked makers, reshaped chains, machine products

**Files:**
- Modify: `crates/engine/src/views/work.rs:252-297` (`RecipeInput`, `RecipeStep`, `RecipeChain`)
- Modify: `crates/engine/src/game/catalog.rs:133-260` (`recipe_chains`, `expand_recipe`, `recipe_inputs`, `tap_for`) and its doc comments
- Modify: `crates/engine/src/game/base/machine_recipe.rs:51` (use the new predicate)
- Modify: `crates/engine/src/game/crafting.rs:105` and the `unlocks_recipes` loop at `:117` (use the new predicate)
- Test: `crates/engine/src/tests/catalog.rs` (new tests), rewrite the `recipe_chains` tests in `crates/engine/src/tests/chains.rs` (~1190–1240) and `crates/engine/src/tests/perks.rs:271` to the new shape
- Check: `crates/engine/src/views/work.rs` has no other reader of the old fields

**Interfaces (Produces):**

```rust
// views/work.rs
pub enum RecipeMaker { Structure(String), ByHand }   // Debug, Clone, PartialEq, Eq
pub struct RecipeStep {
    pub inputs: Vec<RecipeInput>,      // unchanged type
    pub makers: Vec<RecipeMaker>,      // replaces `maker: Option<String>`; every unlocked maker of `output`, structures in StructureDb id order, ByHand last
    pub output: String,
    pub output_qty: Option<u32>,       // unchanged meaning
}
pub struct RecipeChain {
    pub product: String,
    pub description: Option<String>,   // None for ItemCategory::Material
    pub used_in: Vec<String>,          // display names of unlocked entries whose own step takes `product`, A–Z, deduped
    pub steps: Vec<RecipeStep>,        // deepest first, product's own step last
}

// game/catalog.rs, impl Game
pub fn recipe_chains(&self) -> Vec<RecipeChain>;              // one per item with ≥1 unlocked maker, sorted by product
pub fn machine_products(&self, def: &StructureDef) -> Vec<String>; // unlocked recipe display names in recipe order; empty for a non-machine or nothing unlocked
pub(crate) fn machine_recipe_unlocked(&self, recipe: &RecipeDef) -> bool;  // recipe.research done
pub(crate) fn craft_unlocked(&self, research: Option<&ResearchId>, requires_structure: Option<&StructureId>) -> bool; // research done and, if a structure is named, structure_unlocked
```

`fn makers_of(&self, item: &ItemId) -> Vec<RecipeMaker>` (private) is the one
place makers are gathered: each `structure_unlocked` structure whose
`productions()` include `item` with `machine_recipe_unlocked`, then the
item's `craftable` if `craft_unlocked` (`requires_structure` → that
structure's name, none → `ByHand`), then researched `unlocks_recipes` with
`result == item` the same way. Dedup, keeping the first occurrence.

`RecipeInput::source` keeps its meaning (the tap for an ingredient no
recipe makes) but reads `makers_of`: an ingredient whose only makers are
extract productions names the first, and one with any assemble/craft maker
is expanded as a step instead. Expansion (`expand_recipe`) only adds a step
for an item with a non-empty `makers_of`, so locked intermediates show as
plain ingredients.

- [ ] **Step 1: Write the failing tests** in `tests/catalog.rs`, using
  shipped assets and `unlock_research_chain` from `tests/support`:

```rust
#[test] fn recipe_entries_are_alphabetical_and_unlocked_only()
//   fresh game: no entry named "Breach Kit" from a Printer maker (Automation locked),
//   no "Honeypot" entry; names == sorted(names)
#[test] fn breach_kit_lists_the_printer_and_by_hand_once_after_automation()
//   unlock "automation" → Breach Kit's last step makers == [Structure("Printer"), ByHand]
//   (assert both present exactly once; order per the interface rule)
#[test] fn research_finishing_adds_entries_on_the_next_call()
//   honeypot absent; unlock_research_chain(&mut g, "deception"); honeypot present
//   with makers containing Structure("Assembler") and Structure("Workbench")
#[test] fn workbench_gear_is_listed_away_from_the_bench()
//   unlock the research Scrap Ward needs (read assets/items/scrap_ward.ron);
//   player not beside any Workbench; entry "Scrap Ward" has Structure("Workbench")
#[test] fn a_material_has_no_description_and_lists_its_uses()
//   "Bytecode Block": description == None, used_in non-empty and sorted,
//   every name in used_in is itself an entry
#[test] fn a_consumable_keeps_its_description_and_its_uses()
//   unlock "deception": "Breach Kit".description == Some(item's authored text),
//   used_in contains "Honeypot"
#[test] fn a_recipe_whose_structure_is_locked_lists_no_maker()
//   scratch asset (tests/support/scratch_assets.rs): a machine gated by a
//   ResearchDef::unlocks_structures node that is NOT done, with an unlocked
//   recipe for an item nothing else makes → no entry for that item
#[test] fn machine_products_names_only_unlocked_recipes()
//   Printer def: fresh → ["Patch Routine"]; after "automation" → ["Breach Kit", "Patch Routine"]
//   (recipe order from printer.ron); Depot def → []
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine catalog` —
  expected: compile errors / FAIL on the new fields and methods.
- [ ] **Step 3: Implement** the interfaces above. Rewrite the
  `recipe_chains` doc comment: keep the "a machine's recipe *is* the item's
  recipe" and `tap_for` reasoning, drop "shallowest first", state the
  unlocked-only and alphabetical rules and why (spec link). Swap
  `machine_recipe_view:51` and `craft_recipes:105`/`:117` onto the
  predicates with no behaviour change.
- [ ] **Step 4: Rewrite** the old `recipe_chains` assertions in
  `tests/chains.rs` and `tests/perks.rs:271` for the new shape (unlock
  research they now need; `maker` → `makers`). Don't delete what they prove.
- [ ] **Step 5: Run** `cargo test -p feral-processes-engine` and
  `cargo test -p feral-processes-engine balance_sim` — PASS, curves
  unchanged. Then `cargo build --workspace` to list the app-core/gui compile
  breaks Tasks 2–3 fix (expected; don't fix them here beyond `maker` →
  `makers` if needed to keep the workspace building).
- [ ] **Step 6: Commit** the engine files.

### Task 2: app-core — Recipes on the base report rail

**Files:**
- Modify: `crates/app-core/src/app/base_report.rs:10` (`BASE_REPORT_PAGES`: add `HubPage { title: "Recipes", screens: &[Mode::Recipes] }` directly after Compile)
- Modify: `crates/app-core/src/app/base_report.rs:146` (`handle_base_list_key`: add `Mode::Recipes => game.recipe_chains().len()`)
- Modify: `crates/app-core/src/app/input.rs:400,414` (route `Mode::Recipes` to `handle_base_list_key`)
- Delete: `handle_recipes_key` (`crates/app-core/src/app/menus.rs:343-357`)
- Delete: the `Recipes` row in `COMPILE_ROWS` (`crates/app-core/src/app/group_menu.rs:235-246`)
- Modify: `crates/app-core/src/lib.rs:2328` doc comment if it names the Compile route
- Test: `crates/app-core/src/tests/readonly_screens.rs:160-210`, `tests/base_report.rs:30,789`, `tests/group_menus.rs:86,137`

**Interfaces:** Consumes `Game::recipe_chains().len()` (Task 1). Produces
nothing new: the gui keys off `Mode::Recipes` being a base-report screen.

- [ ] **Step 1: Write the failing tests** (rewrite the existing ones):

```rust
#[test] fn the_recipes_page_is_on_the_base_report_rail_after_compile()
//   BASE_REPORT_PAGES titles: index of "Recipes" == index of "Compile" + 1
#[test] fn the_compile_page_no_longer_lists_recipes()
//   base_report.rs:789 expectation becomes ["Compile an item"]; group_menus rows lack "Recipes"
#[test] fn up_and_down_scroll_the_recipes_by_entry()      // was ..._by_chain; open via the rail
#[test] fn the_recipes_screen_does_not_advance_the_game() // open via the rail; same keys; tick unchanged
#[test] fn esc_on_the_recipes_page_goes_to_the_rail()     // replaces the Compile→Esc test
```

  Open the page the way the other rail tests do (find the helper in
  `tests/base_report.rs` that walks the rail to a page).

- [ ] **Step 2: Run** `cargo test -p feral-processes-app-core recipes` — FAIL.
- [ ] **Step 3: Implement** the table entry, routing and deletions above.
- [ ] **Step 4: Run** `cargo test -p feral-processes-app-core` — PASS.
- [ ] **Step 5: Commit.**

### Task 3: gui — the page, the build menu line

**Files:**
- Create: `crates/gui/src/render/base_report/recipes.rs` holding `recipe_rows`, plus `chain_rows`/`step_rows`/`wrap_step_line` moved out of `render/crafting.rs:291-` with their doc comments and the width test
- Modify: `crates/gui/src/render/base_report/mod.rs:~182` (`Mode::Recipes => recipes::recipe_rows(&game.recipe_chains(), selected)`), `frame.rs:~297` (keybar: `↑/↓ scroll`), `frame.rs:~418` (the per-mode list there)
- Delete: `draw_recipes` (`render/crafting.rs`) and its arm at `render/mod.rs:1842`. Move `Mode::Recipes` in `render/tests/refusals.rs:134` to the base-report group at `:148`
- Modify: `crates/gui/src/render/building/build_menu.rs` (`BuildEntry` gains `makes: Option<Vec<String>>`, `None` for a non-machine; filled from `game.machine_products(def)` at `:243`; drawn by `push_entry_rows` under the description through `description_rows`)
- Test: in `recipes.rs` and `build_menu.rs`'s test module

**Interfaces:**
- Consumes `RecipeChain`, `RecipeStep`, `RecipeMaker` and `Game::machine_products` (Task 1).
- Produces `pub(super) fn recipe_rows(chains: &[RecipeChain], selected: usize) -> Vec<Row>`.

Row shape per entry (selectable product row, then unselectable sub-rows,
then a blank row), keeping `crafting.rs`'s `Row::Item` sub-row reasoning:

```
Breach Kit
  A taming catalyst. Spent on a decompile to raise the odds ...   (wrapped description, when Some)
  Used in: Honeypot                                              (when used_in non-empty, wrapped)
  Core Fragment x3 -> Printer, by hand -> Breach Kit             (one line per step, arrows aligned)
```

Makers in a step join with `, `; `RecipeMaker::ByHand` reads `by hand`.
A page with no entries draws one line: `Nothing to make yet. Research
unlocks recipes.` The build menu line is `Makes: A, B`, or for a machine
with nothing unlocked `Makes: nothing yet — research unlocks its recipes`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn the_widest_recipe_row_fits_the_page_it_is_drawn_in()
//   moved from crafting.rs; measure recipe_rows' text against the base
//   report page body width (find the constant/metric the other
//   base_report tests use), all shipped research unlocked
#[test] fn a_step_names_every_maker()     // synthetic chain with [Structure("Printer"), ByHand] → line contains "Printer, by hand"
#[test] fn a_material_entry_draws_no_description_line()
#[test] fn an_empty_list_says_research_unlocks_recipes()
#[test] fn a_machine_entry_says_what_it_makes()   // build_menu_rows with makes Some(["Breach Kit","Patch Routine"]) has a row "Makes: Breach Kit, Patch Routine"
#[test] fn a_machine_with_nothing_unlocked_says_so()
#[test] fn a_non_machine_entry_has_no_makes_line()
#[test] fn the_longest_makes_line_fits_the_build_menu()
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-gui recipe` and
  `... build_menu` — FAIL. (Check the gui crate's package name in
  `crates/gui/Cargo.toml`.)
- [ ] **Step 3: Implement** the files above.
- [ ] **Step 4: Run** `cargo test -p <gui crate>` — PASS. Then take a
  screenshot of the page from a template with research done, e.g.
  `cargo run -- --template <one from dev-saves/README.md> --keys "b ..." --screenshot /tmp/<scratch>/recipes.png`,
  and Read the PNG: rail entry present, entries A–Z, nothing clipped.
- [ ] **Step 5: Commit.**

### Task 4: Stale text, help, changelog

**Files:** every row of the spec's "Stale text to fix" table;
`assets/help/20-controls.md:26-37` (rail list gains Recipes after Compile;
"Compile holds the field recipes and Recipes" → field recipes only);
`assets/help/62-production-lines.md:9` (mention the Recipes page in a
clause); `CHANGELOG.md` (an unreleased entry at the top in the file's
style — no version bump on the branch).

- [ ] **Step 1:** Apply the table's fixes. Plain language, no slogans.
  Keep each structure description one or two sentences.
- [ ] **Step 2: Sweep:** `rg -n -i 'with R\b|press R\b|Armou?ry|Fabricator|Decoy Bench|Refinery|Mining Node|Disk Press' assets crates --glob '!**/tests/**'`.
  Only legitimate hits remain: `R` in the splice rig, rig tool, outposts,
  refit or inventory contexts; the legacy contract ids; test fixtures.
- [ ] **Step 3: Run** `cargo test --workspace` (help and asset census tests
  read these files), `cargo clippy --workspace --all-targets`, `cargo fmt
  --check`. All PASS.
- [ ] **Step 4: Commit.**

### Final gate

- [ ] `cargo test --workspace` green; `balance_sim` unchanged.
- [ ] Whole-branch review on opus, given the spec and `git diff main...` as a
  file, checking the Global Constraints and Review Focus above.
- [ ] Add the shipped feature to `docs/superpowers/INDEX.md`.
