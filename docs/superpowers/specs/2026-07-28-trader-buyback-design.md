# Trader buyback

A trader remembers what you sold it and will sell it back to you at double
what it paid.

## Why

Selling is currently one-way and irreversible. That makes every sale a small
act of dread — sell the wrong stack and it is gone, so the safe play is to
hoard and sell nothing until a breach forces the issue. A buyback shelf turns
a sale into a decision you can walk back for a fee, which is what makes
selling into a routine move rather than a last resort.

The fee is what keeps it honest. Every round trip is a net loss, so the shelf
is a safety net, never a strategy.

## Scope

Items only. Programs sold through `sell_companion` are still destroyed for
good — `dissolve_tamed_program` despawns the entity, and resurrecting one
would mean snapshotting its full stats, level, abilities and nickname into
save data. That is a separate feature, not part of this one.

## Design

### Where the stock lives

A new component on the trader **structure entity**:

```rust
pub struct BuybackStock(pub Vec<(ItemId, u32)>);
```

Inserted lazily on the first sale, so structure spawning is untouched.

A `Vec` rather than a `HashMap` because save bytes must be deterministic —
the same reason `SaveData::researched` is a sorted `Vec`. Insertion order is
driven by player actions, so it is already deterministic, and it gives the
trade screen a stable row order for free.

Attaching it to the structure gives three properties without any code to
enforce them:

- **Per-trader.** Selling to one Market does not stock another.
- **Dies with the zone.** `enter_next_zone` already despawns every
  `With<Structure>` entity, so the shelf is wiped on a breach. Credits remain
  the only cache that crosses, and "liquidate the stockpile you are about to
  lose" stays a real decision rather than a free warehouse.
- **Dies with the trader.** A raid that destroys the Market takes the shelf
  with it.

### Price

`tuning.rs` gains, in its economy section:

```rust
pub const BUYBACK_PRICE_MULTIPLIER: u32 = 2;
```

Unit cost is `(trade.sell_rate * BUYBACK_PRICE_MULTIPLIER).max(1)`. The floor
mirrors `program_payout`'s, so a modded `sell_rate: 0` cannot make a buyback
free.

The multiplier is code, not a `TradeDef` field, because it is an economy knob
and this repo keeps difficulty in `tuning.rs` while keeping content in
`.ron`. It also avoids a schema change, a serde default, and a
`assets/structures/README.md` field entry.

At the shipped Market (`sell_rate: 1`) this is: sells for 1 Credit, buys back
for 2.

**No exploit surface.** Stock is capped by what the player actually sold, and
every round trip loses Credits. It does mean a Portal Fragment you sold comes
back at 2 rather than the 8 Credit listing — that is intended, and
`balance_sim::ticks_to_afford_portal` measures the listing, which is
unchanged.

### Engine API

Renderers draw priced rows verbatim and never work a price out themselves —
the precedent `program_sale_options` sets. Three additions to
`game/trade.rs`:

- `sell_item` records `taken` of `item` into the structure's stock, after the
  sale has otherwise succeeded.
- `pub fn buyback_options(&self, structure: Entity) -> Vec<BuybackOption>`,
  where `BuybackOption { item, name, qty, unit_cost }`. Empty for a structure
  with no stock or no `trade`.
- `pub fn buy_back(&mut self, structure: Entity, item: ItemId, qty: u32) ->
  Result<(), String>`.

`buy_back` is separate from `buy_item` rather than folded into it: the stock
semantics differ (finite and decrementing, versus an infinite listing) and so
do the refusals. It keeps `buy_item`'s ordering discipline — Credits checked
and `check_room` run *before* anything moves — for the reason `sell_item`
documents about its own ordering.

A stock entry is removed when its quantity reaches zero, so a shelf that has
been fully bought back leaves no empty rows.

Routines and the trade currency already cannot be sold, so neither can ever
enter stock.

### Save format

`StructureSave` gains `buyback: Vec<(ItemId, u32)>`.
`SAVE_FORMAT_VERSION` goes 11 → 12. Existing saves stop loading, which is the
documented and intentional tradeoff in `save.rs`.

### UI

`TradeChoice` gains `BuyBack(ItemId)`, routed through the existing quantity
screen exactly as `Sell` and `Buy` are.

Row order on the trade action screen becomes **sell → buy → buyback →
programs**. Programs stay last because they branch to a separate confirm
mode. The buyback section is omitted entirely when the shelf is empty, so a
trader you have never sold to looks exactly as it does today.

### Text

`assets/structures/black_market.ron`'s description says "buy back ICE
Breakers, Power Cells and Portal Fragments" about the *ordinary* listing. That
wording now collides with the real thing and is reworded, and the description
gains a line on the shelf.

## Testing

Engine (`crates/engine/src/tests/trade.rs`):

- Selling records stock; the quantity matches what was taken, not what was
  asked for.
- Buying back returns the item, charges `2 × sell_rate` per unit, and
  decrements the shelf.
- Buying back the last of a stack removes the row entirely.
- Cannot buy back more than was sold.
- Stock is per-structure: a second trader has none of it.
- Stock survives a save/load round trip.
- Stock is gone after a zone breach.
- Selling a program creates no stock.
- Insufficient Credits refuses and leaves the shelf untouched.
- A full inventory (`check_room`) refuses and consumes neither shelf nor
  Credits.
- Buyback is refused during a battle and after game over, like every other
  trade action.

App-core (`crates/app-core/src/tests/`): buyback rows are numbered after buy
rows and before programs, and selecting one reaches the quantity screen.

Gates: `cargo test --workspace`, plus `cargo test -p feral-processes-engine
balance_sim` — no existing constant moves, but a new economy constant still
earns the check.

## Docs

- `assets/structures/README.md` — `sell_rate` gains a second consequence (it
  sets the buyback price at twice its value); no new field.
- Root `README.md` and `CHANGELOG.md`.
