# Type to filter a list

**Status:** Shipped in 0.19.37, 2026-10-23.

Builder check run; its questions are answered below.

Follows [arrow-only lists](2026-10-23-arrow-only-lists-design.md), which
freed lowercase letters inside lists so this could exist.

## Intent

On a long list, typing narrows it to the rows whose name contains what you
typed. Why: the dev `` ` `` console's Give item picker lists every item in
the game, and scrolling it to find one item is miserable. Inventory-style
screens have the same problem on a smaller scale. Give item comes first;
the inventory-family screens below come in the same change.

## Goals

- Typing lowercase letters (and digits and space, see below) on a filtering
  screen builds a filter. The list shows only rows whose name contains the
  filter as a substring, ignoring case: `ar` shows "foo bar" and "Armor".
- Backspace removes the last character of the filter.
- Esc clears the filter **only when a filter is applied**. With no filter,
  Esc does exactly what it does today: closes the screen, or on an `i` hub
  page moves focus to the page rail.
- Up/Down/PageUp/PageDown/Enter work as today on the shorter list, and
  capital-letter screen actions (`S` sell, `I` inspect, `U` fuse all…) act on
  the highlighted row of the filtered list.

## Non-goals

- Fuzzy or ranked matching, prefix-only matching, matching on anything but
  the row's displayed name (category, description, stats).
- A filter on screens whose lowercase keys already act:
  `InventoryItemAction`, `EraseQuantity`, `CraftQuantity`, `TradeQuantity`.
  These are short and stay as they are.
- The basket screens (`Transfer`, `Caravan`, `SettlementMarket`,
  `RouteCargo`) and `DepotFilter`. Their digit and `+ = -` keys need their
  own thought; a later batch.
- `BattleItem`: the classic battle screen deliberately keeps digit/letter
  row shortcuts and `[1]`/`[a]` labels (`battle_row`, arrow-only lists
  spec), so typing there would collide with them. `TacticalItem` has no
  labels and is in.
- `CompanionEquip`: it is always the three fixed slot rows; nothing to
  filter.
- Non-item lists (world map, help, contracts, research…). Later, on the same
  shared piece, if wanted.
- Remembering a filter after leaving the screen.

## Anti-goals

- Row labels coming back. The census test
  `no_list_renderer_outside_battle_names_menu_shortcut`
  (`crates/gui/src/render/mod.rs`) must stay green; filtered rows are drawn
  the same unlabelled way.
- The highlighted row on screen and the row an action applies to ever
  disagreeing. This is the main correctness risk (see Constraints).
- A typed letter firing a screen action, or a screen action's capital
  letter landing in the filter.
- A per-screen hand-written copy of the filter logic, the way the rename
  boxes each hand-roll their text input.

## Screens in this change

| Screen | Handler | Notes |
| --- | --- | --- |
| Dev Give item | `app/dev_item_picker.rs` | Enter gives the item and the filter stays, so several matches can be given in a row. |
| Inventory hub page | `app/inventory.rs` `Mode::Inventory` | Hub nav (`[`, `]`, Esc to rail) runs first today; Esc must reach the filter first when a filter is applied. |
| Equip swap | `Mode::EquipSwap` | |
| Trade | `app/trade.rs` `Mode::Trade` | |
| Stack market | `app/stack_market.rs` | |
| Craft | `app/crafting.rs` `Mode::Craft` | |
| Put down | `app/floor_hands.rs` | |
| Tactical item | `app/tactical.rs` `Mode::TacticalItem` | |

## Behaviour decided

By the user:

- Substring match anywhere in the name, case-insensitive.
- Backspace deletes one character.
- First Esc clears the filter only if one is applied; otherwise Esc behaves
  as today.
- Inventory page: the three equipment-slot rows (Weapon, Armor, Module)
  always show, pinned above the filtered item rows. The filter applies to
  item rows only.
- Coming back from a sub-screen opened on a filtered row (Inventory's item
  action menu, equip swap, inspect) restores the same filter and highlight,
  so several matches can be acted on in a row.

Claude's calls, played back and not objected to:

- Capitals stay screen actions (`input.rs` reserves uppercase for actions),
  so the filter takes only lowercase. Matching ignores case, so nothing is
  lost.
- Digits and space go into the filter too, on all screens in this change
  (verified: none of them binds digits or space today).
- Every change to the filter moves the highlight to the first match.
- The round-trip rule above applies on every screen in this change: a
  sub-screen that returns to the list (Trade's action and quantity pages,
  for instance) keeps the filter. Any other way out of the screen forgets
  it; reopening shows the full list. `keeps_highlight` (`app/input.rs`)
  already names the transitions that keep the highlight and is the natural
  place to decide this too.
- With no matches the list area shows one line saying nothing matches the
  filter text.
- Trade and Stack market rows span sections (sells, buys, buybacks,
  programs; offers and sells). The filter matches every row's displayed
  name in every section; a section heading whose rows are all filtered out
  is not drawn. The shared matcher therefore yields source rows, not flat
  positions, so `trade_row` / `market_row` resolve a filtered position to a
  section and index. Capital-letter actions clamp to the *filtered* count
  and do nothing on an empty filtered list.
- Dev Give item: Enter does not move the highlight; rows still show the
  held count, and matching uses the name only.
- On an `i` hub page, typing goes to the filter only while the list has
  focus (on the rail, keys stay rail navigation). Moving to the rail and
  back keeps the filter; turning the hub page (`[`, `]`, Left/Right) is
  leaving the screen and clears it. Esc reaching the filter first is a
  `screen_claims` arm (`app/hub.rs`), the existing way a screen takes a key
  back from hub nav.
- While the filter is non-empty, a line above the list shows what was
  typed. It is not drawn when the filter is empty.

## Constraints

- **Rows and actions must agree.** Handlers today call
  `selected_index(key, len)` and then index their own source list (the dev
  picker calls `dev_plain_items()` again and indexes it). Under a filter,
  the highlight is a position in the *filtered* list, and every handler
  must map it back to the source row through the same computation the
  renderer used. One app-core function computes the matching source
  indices; the handler and the `App` method gui draws from both call it.
  gui never filters on its own (CLAUDE.md: a "mirrors" must be a call).
- The matching is a pure function over names and the filter text, with
  unit tests.
- Gui reads everything through `App`; it never touches the engine's
  `World` (ADR 0017).
- Held keys: `keys::TextGate` (`crates/gui/src/keys.rs`) only silences a
  key whose press changed `app.mode`. Filter keystrokes don't change mode,
  so held-key repeat types repeatedly; that is acceptable.
- Help text: `assets/help/20-controls.md` documents list controls and gets
  the filter keys. Each screen's on-screen key legend mentions typing to
  filter if it lists Esc.
- CHANGELOG entry for the release.

## Left to the builder

- Where the filter text lives on `App` (one field shared by whichever list
  screen is active, cleared on mode change, is the obvious shape; per-screen
  state is fine if ADR 0014's direction is being followed there). Run the
  `design-patterns` dialog before choosing; if the shape adds a pattern not
  in `docs/adr/README.md`'s pattern index, write the ADR first.
- Exact wording of the filter line and the no-match line.
- Test layout: app-core unit tests per screen that a filtered Enter or
  capital action applies to the right source row, plus the pure matcher's
  tests.
