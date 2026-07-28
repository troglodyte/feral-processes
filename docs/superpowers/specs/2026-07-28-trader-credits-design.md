# Trader Credits

Split the trader's money away from the build economy. Core Fragments stop
being what a trader pays you; a new item, Credits, takes that job.

## Why

Core Fragments are raw salvage — mined, scanned, looted, and consumed by
every build cost and craft recipe. Having a trader hand them over for your
junk made the trader a scrap dispenser rather than a merchant. Portal
Fragments are a breach key wearing a currency's clothes, and having two
unrelated "fragments" in the economy read as one material family that isn't.

## The resulting economy

| | Core Fragments | Credits | Portal Fragments |
|---|---|---|---|
| Role | `Currency` | `TradeCurrency` (new) | `CraftCurrency` |
| Source | mine, scan (`g`), drops, starting kit | selling at a trader, only | boss caches, kill drops |
| Sink | build costs, craft recipes | buying at a trader, only | opening a Zone Portal |
| Survives a breach | no | **yes** | no |

Credits surviving the breach is the point of the feature. Before you breach,
sell surplus scrap and unusable loot for Credits; they cross the portal when
Core Fragments don't; spend them on the far side once you have mined up
enough to stand a market. The trader becomes a value-laundering service.

Credits can still buy a Portal Fragment on the far side, so a stockpile can
in principle fund a breach out of a zone it never worked. That is priced
rather than forbidden — see *The chain-breach question*.

### Balance

No retune. The existing numbers already put the trader on the wrong side of
crafting, which is correct for a merchant:

- Sell rate is 1 Credit per unit of anything.
- ICE Breaker: crafts for 3 Core Fragments, market charges 4 Credits.
- Power Cell: crafts for 2 Core Fragments, market charges 3 Credits.

So sell-then-buy always loses ~25–33% against crafting. The trader's value is
converting junk you cannot use, and converting value into a form that
survives a breach — never efficiency. Nothing becomes strictly dominated in
either direction.

`balance_sim` does not model trading, so no curve should move. Run it anyway.

## Engine changes

### The fourth economy role

`EconomyRole::TradeCurrency`, wired exactly like the three that exist:

- field on `ItemDb`, matched in the `load_dir` role dispatch
- `ItemDb::trade_currency()` accessor
- an entry in `ItemDb::missing_roles`, so startup aborts when unfilled
- `Game::trade_currency()` in `game/catalog.rs`, `.expect("validated at startup")`

Required, not optional. A mod that *adds* item files keeps working; one that
replaced all of `assets/items/` fails loudly at startup with the role named.
That matches how the other three behave and is preferable to a silent
fallback to `Currency`.

### Trade goes through the new role

`game/trade.rs`: `sell_item`, `sell_companion` and `buy_item` swap
`self.currency()` for `self.trade_currency()`.

The self-sale guard in `sell_item` flips with it. Today it refuses to sell
the `Currency` item; now it refuses to sell **Credits** (selling money for
money is still meaningless), and Core Fragments become sellable. That is the
on-ramp — scrap into money — and it is what makes the pre-breach sell-off
possible.

### Credits survive the breach

`game/zone.rs:326` builds `spendable` from the `Currency` and `CraftCurrency`
roles and takes `u32::MAX` of each. Credits are simply not added to that
list, so they persist by omission. The comment there must say why, because
"currency is zone-local" is about to stop being true of all currency.

### The chain-breach question

**Considered and rejected: a guard forbidding traders from listing the
`Currency` or `CraftCurrency` item.** Recording it because the reasoning is
the load-bearing part, and the obvious next reader will propose it again.

With Credits persisting, a trader that sells Portal Fragments lets a player
stockpile Credits, breach, and immediately buy the next breach — skipping a
zone's content, which is exactly what the wipe exists to prevent. Pulling
`portal_fragment` off the Market closes that.

It also **severs mining from breaching entirely**, which is worse. That
listing is the only route from base production to progression, and
`balance_sim::ticks_to_afford_portal` measures the whole travelling-base
economy through its price — three balance tests assert *a breach pays for
itself* (payout ×2 per zone outrunning cost ×1.5) and become meaningless
without it. Removing it would make going deeper something you can only fight
for, never farm for, and would delete a regression gate.

The route is priced instead. At a 1-Credit sell rate and 8 Credits a
fragment:

| Zone | Fragments | Credits | Items sold |
|---|---|---|---|
| 1 | 10 | 80 | 80 |
| 3 | 20 | 160 | 160 |
| 5 | 30 | 240 | 240 |
| 7 | 40 | 320 | 320 |

Selling 160 items into a cargo cap *is* a zone's worth of engagement, so the
"skip" is not free and the hole is narrower than it first appears.

### Display names stop being literals

"Core Fragments" is hardcoded text in twelve places: `game/trade.rs` (six log
and error strings), `crates/gui/src/render/trade.rs` (five row formats),
`structures.rs` (`TradeDef` doc comments), `perks.rs`, and four structure
`.ron` descriptions.

This is a live bug independent of this feature — a mod that swaps the
`Currency` item today gets a UI that still says "Core Fragments". Every one
becomes `item_name(&trade_currency())`. `Game::item_name` is already public,
so the renderer needs no new surface beyond `Game::trade_currency()`.

Note the existing convention: item names are singular and log lines do not
pluralize (`"You sell 3 Power Cell"`). Naming the item "Credits" makes the
common case read correctly without introducing a pluralizer.

## Content changes

- **New** `assets/items/credits.ron` — `id: "credits"`, `name: "Credits"`,
  `role: Some(TradeCurrency)`. No `craftable`, no `droppable`, no
  `bank_limit`. Only a trader mints it.
- `assets/structures/black_market.ron` — `buy` list is unchanged
  (`ice_breaker` 4, `power_cell` 3, `portal_fragment` 8), now priced in
  Credits. Description rewritten.
- `assets/structures/terminal.ron`, `mining_node.ron`, `portal.ron` —
  descriptions reviewed for currency wording.
- `assets/items/core_fragment.ron` — description no longer claims to be
  "everything a trader pays you".
- `assets/items/README.md` — document `TradeCurrency`.

Starting inventory stays at 0 Credits. They are earned.

## Save format

Unchanged. `Inventory` is keyed by `ItemId`, so a new item is not a schema
change. Old saves load with zero Credits, which is also the correct starting
value.

## Testing

Existing `tests/trade.rs` cases assert Core Fragment payouts and become
Credits assertions — including the two whose names encode the old currency
(`sell_item_pays_out_core_fragments_at_the_structures_sell_rate`,
`buy_item_fails_without_enough_core_fragments_or_for_an_unlisted_item`).

New coverage:

- selling Core Fragments is now legal and pays Credits
- selling Credits is refused
- Credits survive a breach while Core Fragments and Portal Fragments do not
- `missing_roles` names `TradeCurrency` when no item claims it

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`, and
`cargo test -p feral-processes-engine balance_sim`.

## Docs

`docs/manual.md`, root `README.md` (the "Building and cronjobs" section names
Core Fragments as trader income), and `CHANGELOG.md`.
