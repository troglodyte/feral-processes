# Per-copy item fusion

Date: 2026-08-07
Status: approved, ready for planning

## The problem

Fusing an item and keeping spares shows every spare as fused. Reported as a
display bug in the inventory screen; it is not one. `components::ItemFusions`
is keyed by `ItemId` and says so:

> Tracked per `ItemId` rather than per physical item, since inventory stacks
> aren't individually distinguishable.

So a fusion upgrades the *item type*: burn two Arc Lances and every Arc Lance
you still hold — plus every one you pick up afterwards, forever — equips at
tier 1. `Game::item_fusion_tier(&ItemId)` is what the six render sites read,
so `Arc Lance x4` coloured as fused is an accurate report of the model.

Programs are the opposite. `components::FusionCount` sits on the entity, so a
fused program is one individual. That mismatch is why this reads as a bug, and
the fix is to make gear behave the way programs already do.

## What changes

A fusion produces one stronger physical copy. Spares stay ordinary.

## Data model

`Inventory` is unchanged and becomes, by definition, the unfused stack.
Recipes, `Stock`, `assembler_system`, hauling, crafting costs and banking keep
seeing exactly what they see today — none of them can encounter a fused copy,
so none of them needs a tier rule. This is the seam that keeps the change from
touching the production chain at all.

A new player-only component replaces `ItemFusions`:

```rust
pub struct FusedGear { pub copies: Vec<(ItemId, u32, u32)> }  // (item, tier, qty)
```

Keyed by `(ItemId, tier)` rather than by position. Two copies of the same item
at the same tier are genuinely interchangeable, so an index would identify
nothing that the pair does not — and value keying avoids the positional-index
trap `BattleState::planned` documents. Tier is always >= 1; a tier-0 copy lives
in `Inventory` and nowhere else. The API mirrors `Inventory`: `add`, `take`,
`count`.

`EquippedItem::fusion_tier` is unchanged. It was already the per-copy record;
this change makes the ledger agree with it instead of shadowing it.

## Identity in the API

`Game::item_fusion_tier(&ItemId) -> u32` is deleted. It has no single answer
once copies differ, and leaving it would let a caller quietly pick one.

Every entry point that names an item for an action gains a tier beside it:

- `Game::equip_item(item, tier)`
- `Game::fuse_item(item, tier)`
- `Game::sell_item(structure, item, tier, qty)`
- the erase-quantity path
- `SwapChoice::Equip` carries the tier

`PlayerStatus::inventory` changes from `Vec<(ItemId, u32)>` to
`Vec<InventoryRow { item: ItemId, tier: u32, qty: u32 }>`, merging both stores
into one list sorted by category then tier. One row per `(item, tier)` — which
is the reported ask directly: `Arc Lance T1 x1` and `Arc Lance x3`, two rows.

Changing the type rather than adding a parallel `fused` list is deliberate:
every consumer breaks at compile time and has to state what it means about
tiers, instead of silently summing across them.

## Recipe

`fuse_item(item, tier)` consumes two copies at `tier` and yields one at
`tier + 1`. The source store is `Inventory` when `tier == 0` and `FusedGear`
otherwise. Refused when `tier + 1 > tuning::MAX_FUSIONS`.

Cost in base copies: T1 = 2, T2 = 4, T3 = 8. Today a T3 costs 6 and upgrades
the whole stack; the rise is the point.

The worn-copy rule survives unchanged. A copy worn at `tier` counts as one of
the two, is the survivor, and has its `EquippedItem::fusion_tier` raised in
place through the `apply_equipment_delta` swap already in `fuse_item`, so the
boost is felt without an unequip/re-equip. The existing ordering holds: every
refusal resolves before anything is taken from either store.

## Selling and buyback

Same unit price at every tier. `Game::item_value` and `sell_price` are
untouched, so `ItemDef::value`'s second meaning — the boss-loot bands
`surface_boss_loot` derives from it — is not disturbed.

`sell_item` gains a tier and takes from the matching store.
`resources::BuybackLedger`'s shelf becomes keyed by `(ItemId, tier)`. Without
that, buying back a mis-sold T3 hands back a T0 and silently deletes eight
copies of work. The ledger stays keyed by `(kind, tile)` and still has to be
wiped by name in `enter_next_zone`.

The trade screen splits rows by tier the same way the inventory does.

## Unequip

Returns the copy to `FusedGear` when its tier is above 0, `Inventory` when it
is 0. The existing ordering is unchanged: the outgoing bonus resolves before
the item leaves its slot, so a refusal can never destroy it.

## Save format

`SAVE_FORMAT_VERSION` 24 -> 25.

- `PlayerSave::item_fusions: Vec<(ItemId, u32)>` becomes
  `fused_gear: Vec<(ItemId, u32, u32)>`
- the buyback shelf's saved shape gains the tier

There is no migration and this spec does not add one — `SAVE_FORMAT_VERSION`'s
docs state the tradeoff, and a v24 save is rejected at load rather than
translated. Anyone mid-run loses that run, as with the last four bumps.

The three `dev-saves/*.ron` templates are RON and are re-stamped by
`savetool pack`, so they survive if their shape is updated. Hand-edit them by
the rule chosen for this change: one copy keeps the recorded tier, the rest
drop to tier 0, and a worn copy's `fusion_tier` is left alone.
`every_checked_in_template_still_loads` is the gate.

`Game::load`'s existing `MAX_FUSIONS` clamp moves to clamping each copy's
tier. It must still not touch `EquippedItem::fusion_tier` — that is the receipt
for a bonus already welded into `Stats`, and lowering it makes an unequip
subtract less than the equip added. See
`loading_a_legacy_over_ceiling_tier_clamps_the_ledger_not_the_worn_copy`.

## Blast radius

- **engine** — `components.rs`, `save.rs`, `game/lifecycle.rs`,
  `game/crafting.rs`, `game/trade.rs`, `resources.rs`
- **app-core** — status rows, the equip picker's `SwapChoice`, the inventory
  and trade handlers
- **gui** — `render/inventory.rs`, `render/trade.rs`, `render/base.rs`,
  `render/mod.rs`

Untouched by design: `Stock`, `assembler_system`, `task_progress_system`,
hauling, recipe costs, `collect_adjacent`.

## Tests

The reproducer first:

- fusing with six spares leaves one fused row and three unfused

Then:

- `2x T1 -> 1x T2`; a fourth fusion is refused at `MAX_FUSIONS`
- a worn copy plus one spare still fuses in place, bonus applying immediately
- unequipping a T2 returns it to `FusedGear`, not to the stack
- an assembler and a bench recipe never consume a fused copy
- selling a T2 and buying it back returns a T2
- a refused fusion spends nothing from either store
- `loading_a_legacy_over_ceiling_tier_clamps_the_ledger_not_the_worn_copy`
  reworked for the new store
- `every_checked_in_template_still_loads`
- `cargo test -p feral-processes-engine balance_sim` — fusion is an equipment
  bonus, so the curves are expected to move; a moved curve is the signal, and
  the new values get recorded rather than the test loosened

## Out of scope

- any premium on a fused copy's sale price
- fused copies as recipe or machine input
- schema migration for v24 saves
