# Settlements Phase 3 — the market

A settlement gets a shelf you can buy from and sell to, and the commit
door the caravan already owns becomes the door both of them use.

Spec: `docs/superpowers/archive/specs/2026-09-04-settlements-design.md`, Phase 3.
Read it, then `CLAUDE.md`, then invoke the `seams` skill and read
**`references/items.md`** — every trap in this plan comes from there.

## What the survey already settled — do not re-derive

Verified against the source on 2026-09-04.

**`Game::commit_caravan_basket`** (`crates/engine/src/game/caravan.rs:1363`)
is a clean two-pass. Its shape, and the split:

```
validate every sell   -> planned_sells, proceeds        no side effects
validate every buy    -> planned_buys,  cost            no side effects
held + proceeds < cost -> refuse whole                  no side effects
--- nothing below may refuse ---
apply sells                                             sells fund buys
apply buys
one tick
```

**Generic** (belongs in the shared core): the two-pass shape itself, the
`held + proceeds < cost` funding comparison, the currency debit/credit
through `Inventory::add`/`take`, the single `tick()`, the
`Sold s, bought b` message.

**Caravan-only** (must NOT move into the core): `caravan_reach`, the
`Caravan` entity and its visit, `caravan_view`/`caravan_shelf` row
resolution, `CaravanOfferKind`'s four-way `deliver_caravan_offer`, and
`CaravanMemory`/`spend_caravan_row`.

**There is no basket struct in the engine.** The door takes
`Vec<(GearCopy, u32)>` and `Vec<usize>` (shelf *indices*, never row
positions). App-core holds `caravan_amounts: Vec<u32>`
(`crates/app-core/src/lib.rs:2088`).

**`ShelfKey` costs nothing to reuse.** It is
`(StructureId, (i32, i32))` and `StructureId` is a plain
`type … = String` (`crates/engine/src/structures.rs:10`). A settlement
keys in as a minted string plus its `KnownSettlement.tile` with **no type
change and no save-format change** — and `BuybackLedger` already persists
through `SaveData::buyback_shelves`. What does *not* work is
`Game::shelf_key`, which resolves `Entity -> Structure -> Position`; a
settlement has no `Structure`. That needs a second constructor, not a
widened type.

**Pricing today** is `item_value * CARAVAN_MARKUP * zone`, floored above
the sum of craft-ingredient value (`caravan_unit_cost`, `caravan.rs:609`),
and `item_value * market_sell_rate()` on the sell side
(`caravan_sell_price`, `:1144`). **Nothing varies price by vendor
disposition today.**

## Decisions taken

Settled with the user before implementation.

| Question | Decision |
|---|---|
| Buyback | **In.** Nearly free — a minted `ShelfKey` string, no type or save change. |
| Temperament | **Live this phase, both directions off a neutral middle.** |
| Screen sharing | **Share plumbing, two screens.** New `Mode::SettlementMarket`. |

**The Temperament table** (constants go in `tuning.rs`, fitted later; these
are the shape, not final numbers):

| | you pay | they pay you |
|---|---|---|
| Open | −10% | +10% |
| Guarded | +10% | −10% |
| Mercantile | ~0% | −15% |

Mercantile is deliberately *not* the average of the other two: everything
is business, so it competes on the buy side and takes its margin on the
sell side. That asymmetry is the whole reason the third variant exists.

**Why not one screen for both.** The caravan screen has five tests pinning
invariants that are expensive to re-establish. Generalizing it puts them at
risk for a phase that is not about the caravan. Copying it outright is the
failure `CLAUDE.md` names — the copy that drifts is the one nobody runs.
Sharing the *plumbing* and the *commit core* takes the DRY win where it is
cheap and leaves the risky half alone.

## Task 1 — extract the commit core (engine, refactor only)

**No behaviour change. This task must not alter a single test's
expectations.**

**Files:** `crates/engine/src/game/caravan.rs`, plus wherever the extracted
core lands (a new `crates/engine/src/game/commerce.rs` is the natural home
— it is neither the caravan's nor the settlement's).

Produce a core that takes **already-validated** plans and applies them. The
validation stays with each vendor, because what may be refused differs
(a caravan checks `roster_room` and its own shelf; a town will check its
own). The core owns:

- the funding comparison, `held + proceeds < cost`, as **one expression**;
- the apply order, sells then buys;
- the currency debit/credit;
- the single `tick()`;
- the outcome sentence.

Give it a shape like `Game::settle_basket(plan) -> Result<String, String>`
where `plan` carries the proceeds, the cost, and two closures (or two
already-resolved vectors plus a per-kind delivery callback) — **decide the
exact shape when you see the borrows**, and say in the commit message why
you chose it. What matters is that `commit_caravan_basket` becomes a
*caller* and no second copy of the funding rule or the apply order exists.

**The gate for this task is the existing tests, unmodified.** These five
are the regression harness (`crates/engine/src/tests/caravans.rs`):

- `a_basket_is_funded_by_its_own_sales` (:1927)
- `an_unaffordable_basket_spends_nothing` (:1984)
- `a_basket_costs_one_tick_whatever_its_size` (:2022)
- `every_refusal_leaves_credits_and_cargo_exactly_as_they_were` (:1471)
- `buying_the_last_drawn_row_buys_the_slot_it_names` (:1845)

**The trap this task walks into.** `a_basket_is_funded_by_its_own_sales`
asserts the resulting **Credits**, not that the goods arrived — because
with the order reversed the goods *still arrive*: `Inventory::take` clamps
and the price vanishes out of an empty purse. If you write a new test for
the extracted core, assert the money, not the outcome.

## Task 2 — the shelf and the prices (engine)

**Files:** `crates/engine/src/game/settlement_market.rs` (new),
`crates/engine/src/tuning.rs`, `crates/engine/src/views.rs`.

- **`Game::settlement_shelf(key, epoch)`** — `caravan_shelf`'s shape
  (`caravan.rs:232`) with three differences: seeded from
  `(WorldMap::seed(), SettlementKey, epoch)` rather than
  `BaseGrid::seed()` + visit (a town is a property of the *world*, not of
  the base that travels); bucket weights biased by `Specialty`; row count
  and tier ceiling scaled by `SettlementKind`.
  **Three prohibitions carry over from `placement.rs`**: no
  `resources::GameRng`, a local `StdRng` only, and **never `%`** — reduce
  through `derive::index`.
- **`Game::settlement_unit_cost` / `settlement_sell_price`** — the
  Temperament multipliers, applied over the same base
  `caravan_unit_cost`/`caravan_sell_price` compute. **The craft-ingredient
  floor must survive**: an item bought below the sum of its ingredients'
  value is an infinite Credit loop, which is the first bound
  `references/items.md` names. Assert it holds at the *cheapest*
  temperament, not the neutral one.
- **`views::SettlementMarketView`** mirroring `CaravanView`
  (`views.rs:469`): offers with a stable `index`, sell rows, credits,
  currency. **The grouping/sort lives in the view, never in the shelf** —
  `caravan_view`'s rule, and sorting the shelf would make the round-robin
  unobservable.

**Tests:** the shelf is a function of its inputs (ask twice, same answer);
a different epoch rolls a different shelf; a mainframe carries more rows
than a server; each specialty actually biases its own bucket; **no price
at any temperament falls below the craft floor**; an empty catalogue
yields no market rather than panicking.

## Task 3 — buyback, keyed to a settlement (engine)

**File:** `crates/engine/src/game/trade.rs`.

- A second key constructor beside `Game::shelf_key` (`trade.rs:101`) —
  settlement-flavoured, reading `Settlements` by `SettlementKey` for the
  tile, minting a **prefixed** id (`"settlement/<def id>"`) so it can never
  collide with a structure id from `assets/structures/`.
- Route the settlement's sell path through the existing `stock_shelf`, and
  its buyback list/purchase through `buyback_options`/`buy_back`.

**Do not widen `ShelfKey`.** The type already fits and the save already
carries it; widening it to an enum is a save-format change bought for
nothing.

**Tests:** selling a rare copy to a town and buying it back returns **that
copy**, not an ordinary one — `references/items.md` states the key is not
decoration, and keyed on the item alone it hands back the wrong tier. And
a town's shelf and a structure's shelf at the same tile do not collide.

## Task 4 — the screen (app-core + gui)

**Files:** `crates/app-core/src/lib.rs`, `app/input.rs`,
`app/settlement_market.rs` (new), `crates/gui/src/render/`.

- `Mode::SettlementMarket`, opened from `Mode::Settlement` with an
  **UPPERCASE** key — lowercase letters are row selectors.
- Amounts state mirroring `caravan_amounts`, and the ceilings mirroring
  `caravan_budget` / `caravan_sell_available`: **one budget minus the other
  rows** on the buy side, **per row and static** on the sell side.
- **`Mode::SettlementMarket` MUST be added to the modifier fold at
  `crates/app-core/src/app/input.rs:184`**, beside `Mode::Caravan`.
  Omitted, Shift and Ctrl fold to bare arrows and silently become plain
  steps — the seam names this exactly.
- **Right increases, Left decreases**, as the caravan does. The transfer
  picker's inversion is for a signed row and does not apply here.
- Reuse `caravan_row`'s row-splitting shape and the popup-row helpers;
  do not copy `caravan_page_rows` wholesale.

**The Mode census, again:** `ALL_MODES` (`render/mod.rs`, currently
`[Mode; 93]` → 94), the popup dispatch match (ends in `_ => {}`, so a new
mode ships blank), `Mode::is_battle`, and
`every_screen_draws_a_refusal_exactly_once`.

**Tests:** a fit census over the **real catalogue**, both dimensions —
height because the page has no scroll, width because `draw_row` clips
vertically only and a long row is lost in silence. Plus: the basket
commits through the shared core; a short basket spends nothing; Shift and
Ctrl still modify (the fold).

## Task 5 — the three writes

`docs/seams.md` argument, `.claude/skills/seams/references/items.md` trap,
`CLAUDE.md` one sentence. At minimum: *the commit door is shared and the
vendors are its callers*, and *a settlement's buyback keys through a minted
string rather than a widened `ShelfKey`*.

`assets/settlements/README.md` gains what `specialty` and `temperament`
now actually do, since this is the phase where both stop being inert.

## Gates

Per task: `cargo fmt`, `cargo clippy --workspace`, the crate's tests.
Before the phase is done: `cargo test --workspace`.

`balance_sim` **is** a gate here, unlike Phase 2 — this phase touches
`tuning.rs` and prices. Run `cargo test -p feral-processes-engine
balance_sim` and treat a moved curve as the signal, not a broken test.

**Two hazards reading a result.** Never pipe `cargo test` through
`grep`/`tail` — the pipeline's exit code masks a failure; redirect to a
file. And this branch has two known intermittents
(`a_posted_worker_levels_up_in_the_base_log_beside_its_machine`,
`a_leech_fills_a_node_buffer_faster_than_a_striker`); one red that passes
on a single re-run is a known flake, not this work.

## Out of scope

Standing and refuse-service are **Phase 4**. This phase's market is open to
anyone who walks up. Do not add a standing gate now — it would be a second
price formula for Phase 4 to unpick.
