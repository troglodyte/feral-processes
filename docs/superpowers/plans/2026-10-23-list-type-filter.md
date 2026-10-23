# Type-to-filter Lists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development
> (one sonnet dispatch per phase) or superpowers:executing-plans. Steps use
> `- [ ]`.

**Goal:** On eight item lists, lowercase letters, digits and space build a
case-insensitive substring filter over row names. Backspace pops a character,
and Esc clears the filter before it does anything else.

**Spec:** `docs/superpowers/specs/2026-10-23-list-type-filter-design.md`. Read
it first. This plan does not repeat its behaviour list.

**Architecture:**
- **Matcher.** One pure function, `filter_matches`, takes names and the
  filter text and returns source indices.
- **Visible rows.** Each screen gets one app-core `*_visible` function that
  returns the indices its list shows. The handler and the gui renderer both
  call it, so gui never filters on its own.
- **State.** It lives on `App` as a small stack of `ListFilter { home: Mode,
  text, row }`.
- **Lifetime.** A filter lives while the current mode is its home or one of
  its home's sub-screens. One retention table, beside `keeps_highlight`,
  decides that. Coming back to the home mode restores `row`.
- **Patterns.** Only existing ones are used: "plain structs owned in one
  place" and "shared pure function". No ADR is needed.

## Decisions (confirmed by the user, 2026-10-23)

1. **"Trade" means `Mode::TradeAction`.** `Mode::Trade` is the picker for
   trader structures (`app/trade.rs:47`). The item rows and `trade_row`
   belong to `TradeAction` (`trade.rs:75`). The plan filters `TradeAction`
   and leaves `Trade` alone.
2. **Equip swap's Unequip row stays pinned.** It has no item name, so like
   the inventory's slot rows it is always shown, in its current position.
3. **Filter state is a stack, not one field.** Inventory → EquipSwap and
   Inventory → `S` → TradeAction each nest one filtered screen inside
   another. A single field would lose the Inventory filter on the way back.

## Global constraints

- Work on branch `list-type-filter`. It is checked out in the primary
  checkout, so ask the user before moving to a worktree. Never push, `git
  checkout` uncommitted work, `git stash` or `git add -A`. Stage explicit
  paths.
- `cargo fmt` and `cargo clippy --workspace --all-targets` must be clean at
  every commit. TDD, with a commit per green step.
- Capital letters are never captured by the filter. Lowercase letters,
  digits and space are captured only on the eight screens.
- Leave `no_list_renderer_outside_battle_names_menu_shortcut`
  (`gui/src/render/mod.rs:1855`) untouched and green.
- Every handler indexes source rows only through `visible[pos]`. Grep each
  touched handler for a leftover direct `list[idx]` or `list.get(self.menu_selected)`.
- Do not touch `docs/manual.md`, `README.md` or `TODO.md`.

## Key facts (verified 2026-10-23)

- **Keys.** `GameKey::Char(c)` covers letters, digits and space; Space parses
  to `Char(' ')` (`lib.rs:1017`). `Backspace` and `Esc` are their own
  variants.
- **Selection.** `selected_index(key, len)` is at `app/input.rs:135`.
- **Highlight reset.** `handle_key` resets `menu_selected` after dispatch
  unless `keeps_highlight(before, after)` holds (`input.rs:66` and `:428`).
  Handlers also set `menu_selected` themselves inside dispatch:
  `open_equip_swap` sets it to 0, and EquipSwap's `done` sets it to
  `slot_row`. To save the home row, snapshot it before dispatch.
- **Hub.** `handle_hub_nav` (`app/hub.rs:~178`) runs before the screen
  handler:
  - `[` and `]` turn the page first.
  - On the rail, every other key is already swallowed.
  - `screen_claims` (`hub.rs:206`) is how a screen takes Esc back from hub
    navigation.
  - Focus is `App.hub_on_rail`.
- **Renderers.** Most draw from `game` and are given `selected`. Shared rows
  are precomputed in `render/mod.rs` before `game` is borrowed (see the
  comment near `:1203`), so that is where `visible` and `filter` are
  computed and passed in.
- **Test helpers.** `tests/support.rs` has `test_app`,
  `app_on_inventory_with_many_items` (:481), `open_hub_screen` (:2888),
  `highlight_row` and `pick_row`. Add a shared `type_text(app, &str)` there.
  `tests/party.rs:144` has a private `type_name` to model it on.

## Review focus

1. **The highlight and the acted-on row agree.** Every phase must have a
   test that filters, moves the highlight off the first match, and then
   presses Enter and each capital action. The test asserts the *source* row
   that was used. Mutation-check one per phase by indexing the unfiltered
   list.
2. **Esc order.** With a filter, Esc only clears it, including on the `i`
   hub page through `screen_claims`. Without a filter, Esc behaves exactly
   as before.
3. **Retention.** The filter survives each sub-screen round trip the spec
   lists. It is cleared by a page turn and by any other way out. Reopening
   the screen shows the full list.
4. **Nesting.** An Inventory filter survives a filtered EquipSwap or
   TradeAction and comes back intact.

---

## Phase 1: core, hub, and the dev Give item picker

Create `crates/app-core/src/app/list_filter.rs`, registered in `app/mod.rs`.

- [ ] **Matcher.** Add `pub fn filter_matches<'a>(names: impl
  IntoIterator<Item = &'a str>, filter: &str) -> Vec<usize>`. Matching is a
  substring test that ignores case. An empty filter returns every index.
  Unit tests:
  - `ar` matches "foo bar" and "Armor".
  - An empty filter returns all indices.
  - No match returns an empty Vec.
  - A digit and a space match.
- [ ] **State.** Add `struct ListFilter { home: Mode, text: String, row:
  usize }` and an `App.list_filters: Vec<ListFilter>` field (`lib.rs`, near
  `menu_selected`). Accessors:
  - `fn filter_text(&self) -> &str` returns the text whose `home ==
    self.mode`, or `""`.
  - `fn filter_applied(&self) -> bool`.
- [ ] **Keys.** Add `fn take_filter_key(&mut self, key: GameKey) -> bool`.
  It acts only when `self.mode` is a filter home (`fn is_filter_home(Mode)`
  lists the eight). It returns true when it consumed the key:
  - `Char(c)` where `c` is a lowercase letter, a digit or `' '` pushes `c`.
  - `Backspace` pops a character. On an empty filter it is still consumed
    and does nothing.
  - `Esc` clears the filter if one is applied. With no filter it returns
    false.
  - A push, pop or clear sets `menu_selected` to the first match. For
    Inventory that is `3` if any item matches, or else `0`.
- [ ] **Retention.** Add `fn filter_lives(home: Mode, now: Mode) -> bool`,
  a `matches!` table placed next to `keeps_highlight`. In `handle_key`:
  - Snapshot `menu_selected` before dispatch.
  - After dispatch, if the mode changed:
    - If the old mode was a home, store the snapshot in its entry's `row`.
    - Drop every entry for which `!filter_lives(entry.home, self.mode)`.
    - If the new mode is a home with an entry, set `menu_selected` to that
      entry's `row`, clamped. This overrides the `opening_row` reset.
  - Fill the table phase by phase. Phase 1 needs only DevItemPicker →
    nothing, since its Enter keeps the mode.
- [ ] **Hub.** Add a `screen_claims` arm, `Esc if self.filter_applied() =>
  true`. A page turn is a mode change to another hub page, so retention
  drops the filter without extra code. Test both directions: rail → list
  keeps the filter, and `]` then `[` comes back unfiltered.
- [ ] **Dev picker.**
  - Add `pub fn dev_item_visible(game, filter) -> Vec<usize>` over
    `dev_plain_items()` names.
  - The handler calls `take_filter_key` first and then indexes
    `items[visible[idx]]`.
  - Make `App::dev_item_rows()` return only the visible rows.
  - Enter keeps both the filter and the highlight.
  - Tests: give the second match, then give again without retyping.
- [ ] **Gui shared lines.** Add `render/list_filter.rs` with a function that
  returns the "filter: …" line, shown only when the filter is non-empty,
  and the no-match line. It draws only through `Painter`/row helpers. Wire
  it into `dev_pickers.rs` and add "Type to filter." to `HINT`.
- [ ] **Screenshot check.** Run `cargo run -- --template stack --keys "<open
  console, open Give item, type ar>" --screenshot`, then Read the PNG.
- [ ] Gate: `cargo test -p feral-processes-app-core`, then commit.

## Phase 2: Inventory hub page and Equip swap

- [ ] **Inventory.**
  - Add `pub fn inventory_visible(game, filter) -> Vec<usize>` over
    `player_status().inventory` names. Position `p < 3` is a slot.
    Otherwise the row is `inventory[visible[p - 3]]`.
  - Rewrite `handle_inventory_key` (`app/inventory.rs:47`):
    - `total = 3 + visible.len()`.
    - `S`, `I` and Enter all resolve through `visible`.
    - `U` and `D` are unchanged.
- [ ] **Equip swap.** Add `pub fn equip_swap_visible(rows: &[SwapRow],
  filter) -> Vec<usize>`. It keeps the Unequip row pinned (decision 2).
  Route `handle_equip_swap_key` (`inventory.rs:164`), both Enter and `R`/`I`,
  through it.
- [ ] **Retention table.**
  - Inventory's filter lives through InventoryItemAction, EraseQuantity,
    TradeQuantity, ModCopy, RefitCopy, TrapDirection, ItemDescribe,
    EquipSwap, TradeAction and TradeProgramConfirm.
  - EquipSwap's own filter lives through RefitCopy and ItemDescribe.
  - Before writing the table, trace each sub-screen's return path in
    `inventory.rs`, `trade.rs:284,295` and `open_gear_inspect`.
- [ ] **Gui.**
  - `base_report/mod.rs:200` → `inventory::items_rows(game, selected,
    visible, filter)`: the slot rows always show, and the item rows are
    drawn from `visible` with `selected == p + 3`.
  - `draw_equip_swap` (`inventory.rs:441`) gets the same change.
  - Add `keycap("a-z", "filter")` to `screen_keys` (`frame.rs:~288`) for
    Inventory and EquipSwap.
- [ ] **Tests.**
  - Filter, highlight the second match, then check that Enter → item action
    → Esc comes back with the same filter and row.
  - `S` and `I` act on the filtered row.
  - A slot row still opens EquipSwap. Its own filter works there, and the
    Inventory filter is intact after `done`.
  - Esc clears first and then goes to the rail.
- [ ] Gate and commit.

## Phase 3: TradeAction and Stack market (sectioned lists)

- [ ] **Trade.**
  - Add `pub fn trade_visible(game, structure, filter) -> TradeVisible {
    sells, buys, buybacks, programs: Vec<usize> }`. Each section is filtered
    separately.
  - `trade_row(p, ..)` takes the four *visible* lengths.
  - The handler maps `TradeRow::Sell(k)` to `sells[vis.sells[k]]`, and the
    same for the other three sections.
  - Capital actions clamp to the visible total and do nothing when the
    visible list is empty.
- [ ] **Stack market.** Add `pub fn market_visible(view, filter) ->
  MarketVisible { offers, sells }`, used the same way with `market_row`.
  Enter still buys `view.offers[vis.offers[k]].index`, the shelf index.
- [ ] **Retention table.**
  - TradeAction's filter lives through TradeQuantity, TradeProgramConfirm
    and ItemDescribe.
  - StackMarket's filter lives through ItemDescribe.
- [ ] **Gui.**
  - `draw_trade_action_menu` (`render/trade.rs:35`) and `draw_stack_market`
    take the visible sets.
  - A section heading whose rows are all filtered out is skipped.
  - The running `idx` counter counts visible rows only.
  - Add "Type to filter" to the market's footer hint and to TradeAction's
    legend.
- [ ] **Tests.**
  - A filter that leaves rows in two sections: Enter, `S`, `B` and `I` each
    hit the right section and source row.
  - An empty filtered list: `S` does nothing.
  - TradeQuantity → back keeps the filter.
- [ ] Gate and commit.

## Phase 4: Craft, Put down, Tactical item

- [ ] **Craft.** Add `craft_visible`, matching on the recipe's result item
  name, which is the `craft_rows` head line. CraftQuantity → back keeps the
  filter.
- [ ] **Put down.** Add `put_down_visible` over `PutDownOption` labels.
  Make `App::put_down_rows()` return only the visible rows. The handler's
  `swap_remove(idx)` must take the visible index.
- [ ] **Tactical item.** Add `tactical_items_visible(game, filter)` over
  `game.item_name(id)` for `battle_usable_items()`. The renderer
  `render/tactical.rs:1697` takes `visible`.
- [ ] Add the gui legend or hint text to each, then add an Enter-on-a-
  filtered-row test per screen.
- [ ] Gate: `cargo test --workspace` and clippy, then commit.

## Phase 5: docs, review, land

- [ ] Add the filter keys to the list-controls paragraph in
  `assets/help/20-controls.md:3-5`: type to filter, Backspace, and Esc
  clears first.
- [ ] Add an entry to `docs/superpowers/INDEX.md` and set the spec's status
  to Shipped once it lands.
- [ ] Final whole-branch review with opus, given the diff as a file. Point
  it at the review focus above and at the spec's Anti-goals.
- [ ] Land it through the `deploy` skill, which handles the CHANGELOG
  section. No save-format change, so this is a patch bump.
