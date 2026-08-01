# Quick trade keys and item grouping

Two changes to the same two screens: sort item lists into categories, and
let `S`/`B` transact one unit without a trip through the quantity page.

Small — three crates, no schema change, no save-format change.

## Why

Selling one Power Cell currently costs four screens: inventory → item action
→ trader picker → quantity. A trade visit is normally a *run* of trades
(`return_to_trade_list` exists for exactly that reason), so the per-item cost
is paid over and over.

And both lists are unsorted. The trade screen shows your sellable pack, the
trader's stock, buybacks and programs as one flat run of rows in whatever
order the underlying vectors happen to hold, with no signal of what any row
*is*.

## Categories

A new `ItemCategory` in `crates/engine/src/items.rs`, beside `EquipmentSlot`.

Derived as a pure function of `ItemDef` — **not** a new `.ron` field:

| condition | category |
| --- | --- |
| `routine: Some(_)` | Routine |
| `equipment: Some((Weapon, _))` | Weapon |
| `equipment: Some((Armor, _))` | Armor |
| `equipment: Some((Module, _))` | Module |
| `consume: Some(_)` | Consumable |
| `role: Some(_)` | Currency |
| otherwise | Material |

Checked in that order, so an item that is both consumable and equippable
lands in its slot rather than in Consumable.

Deriving rather than authoring is the load-bearing choice: a modded item is
categorised for free, with no new field to forget, no `#[serde(default)]` to
add, and no `assets/items/README.md` schema change. It also cannot drift out
of sync with the behaviour it describes, because it *is* that behaviour.

Display order: **Consumable → Weapon → Armor → Module → Routine → Material →
Currency.**

## Sorting

Lists come back from the engine already sorted by `(category, name)`.

- `Game::player_status()` (`game/party.rs:8`) sorts the `inventory` field it
  builds.
- `Game::trade_options()` sorts the `buy` list of the `TradeDef` it already
  clones out of `StructureDb`.
- `Game::buyback_options()` and `Game::program_sale_options()` sort likewise.

**Sort in the view, never in `Inventory`.** The `Inventory` component is
`Vec<(ItemId, u32)>` and its order is persisted through
`PlayerSave::inventory`. Sorting the component would rewrite save contents
and change pickup-order semantics for no benefit; sorting the view changes
only what is displayed.

**This is also the whole answer to the indexing hazard.**
`handle_trade_action_key` computes a flat row index while `render/trade.rs`
draws the rows, and the code carries a warning that the two must stay
identically indexed or the game "would sell the line above or below the one
the player is looking at". Because the engine hands both sides the same
pre-sorted list, and no header rows are inserted, the flat index arithmetic
is completely unchanged. There is nothing for the two sides to disagree
about.

No section headers. Each row instead carries its category as a column, drawn
by gui on both screens.

## Quick keys

Capital `S` and `B`, acting on the highlighted row (`menu_selected`), one
unit per press. Five presses sell five.

**`Mode::TradeAction`** — `S` on a sell row sells one; `B` on a buy row or a
buyback row buys one. The wrong key for a row's direction transacts nothing
and writes a status line saying so, rather than guessing at intent.

**`Mode::Inventory`** — `S` sells one of the highlighted item.
`trader_in_range` already gates whether `[S]ell` is offered at all, so:

- no trader in range → status line, nothing happens
- exactly one → use it
- more than one → fall through to the existing trader picker with the choice
  pre-set. That is the `TradeOrigin::Inventory` path, which already exists
  and already skips the line-item list on the way back.

The quick key does not re-implement any rule about *what* may be sold. It
calls `Game::sell_item` and surfaces whatever that returns — so the trade
currency stays unsellable, with the engine's own message, exactly as it is
through the slow path. A second rule here would be a copy that drifts.

### Programs are deliberately excluded

`S` on a program row opens the existing confirmation screen, exactly as Enter
does. It does not sell.

`handle_trade_program_confirm_key`'s own doc explains why: "a mis-hit on a
screen that permanently destroys a levelled program must not be a sale." A
quick key *is* a mis-hit risk, so a program row is the one thing this feature
refuses to make faster. Selling a stack of Power Cells by accident costs a
few credits; selling a level-9 companion by accident cannot be undone.

## The behaviour change this forces

`selected_index` (`app/input.rs:19-28`) lowercases before matching, so today
`S` selects row 28 and `B` selects row 11. After this, **uppercase letters no
longer select rows anywhere in the game** — they are reserved for actions.
Lowercase selection is untouched.

Nothing documents uppercase-selects-rows as a feature and no test covers it,
but it is a real change and gets a test pinning both halves.

## Testing

Failing test first, per the usual discipline.

- **Category** — one case per `ItemDef` shape, including the precedence case
  (an item that is both consumable and equippable is not a Consumable) and an
  item declaring nothing, which must be Material rather than a panic.
- **Sort** — lists come back grouped in the documented order and stable
  within a category. Assert against the real `.ron` assets, not a fixture, so
  a shipped item that categorises surprisingly shows up here.
- **Quick keys** — `S` on a sell row moves exactly one unit and leaves the
  mode on `TradeAction`; `B` likewise; repeated presses accumulate.
- **Wrong direction** — `B` on a sell row transacts nothing and sets a status
  line.
- **Programs** — `S` on a program row lands on `Mode::TradeProgramConfirm`
  and the program still exists. `world.get::<Stats>(e).is_none()` is this
  repo's idiom for "entity is gone" (`tests/trade.rs`).
- **Selection** — lowercase `b` still selects row 11; uppercase `B` no longer
  selects any row.
- **Inventory quick-sell** — across zero, one and two traders in range.

Also: `S` on the trade currency must transact nothing and surface the
engine's own refusal, proving the quick path shares `sell_item`'s rules
rather than carrying a second copy of them.

Existing trade tests that assert a particular row order will need updating;
that is expected churn from the sort, not breakage.

## Gates

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

No `balance_sim` run needed: no tuning constant, species file or item *value*
changes — only display order and input handling.

No `SAVE_FORMAT_VERSION` bump: no field is added, removed or reordered, and
the `Inventory` component is deliberately not touched.

## Out of scope

- **Section headers.** Considered and cut in favour of a per-row category
  tag, because headers are unselectable rows and would put the flat index
  arithmetic back in play on the one screen where mis-indexing sells the
  wrong item.
- **Reserving `s`/`b` from the row-label alphabet.** Would have freed the
  lowercase keys, at the cost of shifting every label past row 10 on every
  menu in the game, since `row_label` and `selected_index` are shared.
- **Quantity presets** beyond one unit. `Mode::TradeQuantity` still exists
  and is still the way to move ten of something.
